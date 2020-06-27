use crate::hv::vmx::get_guest_reg;
#[allow(unused_imports)]
use crate::{Error, GuestThread, HandleResult, X86Reg, VCPU};
#[allow(unused_imports)]
use log::{error, info, trace, warn};

type MemAccessFn = fn(&GuestThread, usize, &mut u64, u8, bool) -> Result<(), Error>;

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
            if *byte & 0x08 > 0 {
                decode.operand_bytes = 8;
            }
            if *byte & 0x04 > 0 {
                decode.rex_r = true;
            }
            if *byte & 0x02 > 0 {
                decode.rex_x = true;
            }
            if *byte & 0x01 > 0 {
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
        Err(Error::Program("No modrm"))
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
            error!("decode: had {} address bytes", decode.address_bytes);
            Err(Error::Program("decode: wrong address bytes"))
        }
    }
}

fn modrm_sib_bytes_16(mod_: u8, rm: u8) -> u8 {
    let mut ret = 1;
    match mod_ {
        0 => {
            if rm == 6 {
                ret += 2;
            }
        }
        1 => {
            ret += 1;
            if rm == 4 {
                ret += 1;
            }
        }
        2 => ret += 2,
        _ => {}
    };
    ret
}

fn modrm_sib_bytes_32(mod_: u8, rm: u8) -> u8 {
    let mut ret = 1;
    match mod_ {
        0 => {
            if rm == 4 {
                ret += 1;
            } else if rm == 5 {
                ret += 4;
            }
        }
        1 => {
            ret += 1;
            if rm == 4 {
                ret += 1;
            }
        }
        2 => {
            ret += 4;
            if rm == 4 {
                ret += 1;
            }
        }
        _ => {}
    };
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
            error!("decode: had {} address bytes", decode.address_bytes);
            Err(Error::Program("wrong address bytes"))
        }
    }
}

fn decode_opcode(insn: &[u8], decode: &mut X86Decode) -> Result<(), Error> {
    let opcodes = &insn[(decode.prefix_sz as usize)..];
    let unknown = Err(Error::Program("unknown opcode"));
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
            decode.is_store = !(opcodes[0] & 2 > 0);
        }
        0x89 | 0x8b => decode.is_store = !(opcodes[0] & 2 > 0), // mov
        0x0f => {
            decode.opcode_sz = 2;
            match opcodes[1] {
                0xb7 => decode.operand_bytes = 2, // movzw
                0xb6 => decode.operand_bytes = 1, // movzb
                _ => return unknown,
            }
        }
        _ => {
            error!("unknown decode: {:02x}", opcodes[0]);
            return unknown;
        }
    };
    decode.modrm_sib_sz = modrm_sib_bytes(insn, decode)?;
    Ok(())
}

fn add_8081(
    _vcpu: &VCPU,
    _gth: &GuestThread,
    _insn: &[u8],
    _decode: &X86Decode,
    _access: MemAccessFn,
    _gpa: usize,
) -> Result<(), Error> {
    Err(Error::Program("add_8081 unimplemented"))
}

fn execute_op(
    vcpu: &VCPU,
    gth: &GuestThread,
    insn: &[u8],
    decode: &X86Decode,
    access: MemAccessFn,
    gpa: usize,
) -> Result<(), Error> {
    let opcodes = &insn[decode.prefix_sz as usize..];
    let unknown = Err(Error::Program("unknown opcode"));
    let mod_reg = modrm_get_reg(insn, decode);
    match opcodes[0] {
        0x80 | 0x81 => match mod_reg? {
            0 => add_8081(vcpu, gth, insn, &decode, access, gpa),
            7 => unimplemented!(),
            _ => unknown,
        },
        0x88 | 0x89 | 0x8a | 0x8b => {
            let reg = get_guest_reg(mod_reg? as u64);
            let mut reg_value = vcpu.read_reg(reg)?;
            access(
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
                vcpu.write_reg(reg, reg_value)
            } else {
                Ok(())
            }
        }
        _ => unknown,
    }
}

pub fn emulate_mem_insn(
    vcpu: &VCPU,
    gth: &GuestThread,
    insn: &[u8],
    access: MemAccessFn,
    gpa: usize,
) -> Result<(), Error> {
    let mut decode = X86Decode::default();
    decode_prefix(insn, &mut decode);
    decode_opcode(insn, &mut decode)?;
    match execute_op(vcpu, gth, insn, &decode, access, gpa) {
        Ok(()) => Ok(()),
        Err(e) => {
            error!(
                "emulate mem instruction {:02x?} fail, gpa = {:x}, decode = {:?}",
                insn, gpa, decode
            );
            Err(e)
        }
    }
}
