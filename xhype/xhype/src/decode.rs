/* SPDX-License-Identifier: GPL-2.0-only */

/* functions in this module are inspired by Akaros/user/vmm/decode.c */

use crate::{err::Error, vmexit::vmx_guest_reg, GuestThread, VCPU};
#[allow(unused_imports)]
use log::*;

type MemAccessFn = fn(&VCPU, &mut GuestThread, usize, &mut u64, u8, bool) -> Result<(), Error>;

#[derive(Debug)]
pub struct X86Decode {
    prefix_sz: u8,
    opcode_sz: u8,
    modrm_sib_sz: u8,
    imm_sz: u8,
    operand_bytes: u8,
    address_bytes: u8,
    has_modrm: bool,
    is_store: bool,
    rex_r: bool,
    rex_x: bool,
    rex_b: bool,
}

impl Default for X86Decode {
    fn default() -> Self {
        X86Decode {
            prefix_sz: 0,
            opcode_sz: 1,
            modrm_sib_sz: 0,
            imm_sz: 0,
            operand_bytes: 4,
            address_bytes: 8,
            has_modrm: true,
            is_store: false,
            rex_r: false,
            rex_x: false,
            rex_b: false,
        }
    }
}

fn decode_prefix(insn: &[u8], decode: &mut X86Decode) {
    let mut prefix_sz = 0;
    for byte in insn.iter() {
        if *byte == 0x66 {
            /* Operand-size override prefix */
            /* Ignore 0x66 if REX.W changed us to 8 bytes (64).
             * Though we should only see 0x66 before REX.W.
             *
             * If this was handling 32 bit code but with cs.d clear
             * (default 16), 66 should set us to 4 bytes. */
            if decode.operand_bytes == 4 {
                decode.operand_bytes = 2;
            }
        } else if *byte == 0x67 {
            /* Address-size override prefix */
            decode.address_bytes = 4;
        } else if *byte & 0xf0 == 0x40 {
            /* REX.* */
            if *byte & 0x08 != 0 {
                decode.operand_bytes = 8;
            }
            if *byte & 0x04 != 0 {
                decode.rex_r = true;
            }
            if *byte & 0x02 != 0 {
                decode.rex_x = true;
            }
            if *byte & 0x01 != 0 {
                decode.rex_b = true;
            }
        } else {
            break;
        }
        prefix_sz += 1;
    }
    decode.prefix_sz = prefix_sz;
}

fn get_modrm(insn: &[u8], decode: &X86Decode) -> Result<u8, Error> {
    if decode.has_modrm {
        Ok(insn[(decode.prefix_sz + decode.opcode_sz) as usize])
    } else {
        Err("No modrm".to_string())?
    }
}

fn modrm_get_reg(insn: &[u8], decode: &X86Decode) -> Result<u8, Error> {
    let modrm = get_modrm(insn, decode)?;
    let reg = (modrm >> 3) & 7;
    match decode.address_bytes {
        2 => {
            error!("decode: had 2 address bytes");
            Ok(reg)
        }
        4 | 8 => {
            if decode.rex_r {
                Ok(reg + 8)
            } else {
                Ok(reg)
            }
        }
        _ => {
            let msg = format!("decode: had {} address bytes", decode.address_bytes);
            error!("{}", msg);
            Err(msg)?
        }
    }
}

fn modrm_sib_bytes_16(mod_: u8, rm: u8) -> u8 {
    let mut ret = 1;
    match (mod_, rm) {
        (0, 6) => ret += 2,
        (0, _) => {}
        (1, 4) => ret += 1 + 1,
        (1, _) => ret += 1,
        (2, _) => ret += 2,
        _ => {}
    }
    ret
}

fn modrm_sib_bytes_32(mod_: u8, rm: u8) -> u8 {
    let mut ret = 1;
    match (mod_, rm) {
        (0, 4) => ret += 1,
        (0, 5) => ret += 4,
        (0, _) => {}
        (1, 4) => ret += 1 + 1,
        (1, _) => ret += 1,
        (2, 4) => ret += 4 + 1,
        (2, _) => ret += 4,
        _ => {}
    }
    ret
}

fn modrm_sib_bytes(insn: &[u8], decode: &X86Decode) -> Result<u8, Error> {
    let modrm = get_modrm(insn, decode)?;
    let mod_ = modrm >> 6;
    let rm = modrm & 0x7;
    match decode.address_bytes {
        2 => Ok(modrm_sib_bytes_16(mod_, rm)),
        4 | 8 => Ok(modrm_sib_bytes_32(mod_, rm)),
        _ => {
            let msg = format!("decode: had {} address bytes", decode.address_bytes);
            Err(msg)?
        }
    }
}

fn decode_opcode(insn: &[u8], decode: &mut X86Decode) -> Result<(), Error> {
    let opcodes = &insn[(decode.prefix_sz as usize)..];
    let unknown = Err(format!("unknown opcodes: {:x?}", opcodes))?;
    let reg = modrm_get_reg(insn, &decode);
    match opcodes[0] {
        0x80 => match reg? {
            0 | 7 => {
                // add | cmp
                decode.imm_sz = 1;
                decode.operand_bytes = 1;
            }
            _ => return unknown,
        },
        0x81 => match reg? {
            0 | 7 => decode.imm_sz = if decode.address_bytes == 2 { 2 } else { 4 }, // add | cmp
            _ => return unknown,
        },
        0x3a => decode.operand_bytes = 1, // cmp /r
        0x88 | 0x8a => {
            // mov
            decode.operand_bytes = 1;
            decode.is_store = !(opcodes[0] & 2 != 0);
        }
        0x89 | 0x8b => decode.is_store = !(opcodes[0] & 2 != 0), // mov
        0x0f => {
            decode.opcode_sz = 2;
            match opcodes[1] {
                0xb7 => decode.operand_bytes = 2, // movzw
                0xb6 => decode.operand_bytes = 1, // movzb
                _ => return unknown,
            }
        }
        _ => {
            return unknown;
        }
    };
    decode.modrm_sib_sz = modrm_sib_bytes(insn, decode)?;
    Ok(())
}

fn execute_op(
    vcpu: &VCPU,
    gth: &mut GuestThread,
    insn: &[u8],
    decode: &X86Decode,
    access: MemAccessFn,
    gpa: usize,
) -> Result<(), Error> {
    let opcodes = &insn[decode.prefix_sz as usize..];
    let mod_reg = modrm_get_reg(insn, decode);
    match opcodes[0] {
        0x88 | 0x89 | 0x8a | 0x8b => {
            let reg = vmx_guest_reg(mod_reg? as u64);
            let mut reg_value = vcpu.read_reg(reg)?;
            access(
                vcpu,
                gth,
                gpa,
                &mut reg_value,
                decode.operand_bytes,
                decode.is_store,
            )?;
            if !decode.is_store {
                if decode.operand_bytes == 4 {
                    reg_value &= 0xffffffff;
                }
                vcpu.write_reg(reg, reg_value)?;
            }
            Ok(())
        }
        _ => Err(format!("unknown opcodes: {:?}", opcodes))?,
    }
}

pub fn emulate_mem_insn(
    vcpu: &VCPU,
    gth: &mut GuestThread,
    insn: &[u8],
    access: MemAccessFn,
    gpa: usize,
) -> Result<(), Error> {
    let mut decode = X86Decode::default();
    decode_prefix(insn, &mut decode);
    decode_opcode(insn, &mut decode)?;
    match execute_op(vcpu, gth, insn, &decode, access, gpa) {
        Ok(()) => Ok(()),
        Err(e) => Err(format!(
            "emulate memory instruction fail at gpa={:x}, decode = {:?}, error = {:?}",
            gpa, decode, e
        ))?,
    }
}
