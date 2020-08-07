/* SPDX-License-Identifier: GPL-2.0-only */

/*!
https://www.gnu.org/software/grub/manual/multiboot/multiboot.html
*/

use crate::bios::setup_bios_tables;
use crate::consts::x86::*;
use crate::consts::*;
use crate::err::Error;
use crate::hv::{gen_exec_ctrl, vmx::*, vmx_read_capability, VMXCap, X86Reg};
use crate::utils::round_up_4k;
use crate::{GuestThread, VirtualMachine};
use crossbeam_channel::unbounded as channel;
use std::fs::{metadata, File};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::mem::{size_of, transmute};
use std::sync::Arc;

pub const MAGIC1: u32 = 0x1BADB002;
pub const HEADER_POS_LIMIT: usize = 8192;

/* is there basic lower/upper memory information? */
pub const MULTIBOOT_INFO_MEMORY: u32 = 0x00000001;
/* is there a boot device set? */
pub const MULTIBOOT_INFO_BOOTDEV: u32 = 0x00000002;
/* is the command-line defined? */
pub const MULTIBOOT_INFO_CMDLINE: u32 = 0x00000004;
/* are there modules to do something with? */
pub const MULTIBOOT_INFO_MODS: u32 = 0x00000008;

/* These next two are mutually exclusive */

/* is there a symbol table loaded? */
pub const MULTIBOOT_INFO_AOUT_SYMS: u32 = 0x00000010;
/* is there an ELF section header table? */
pub const MULTIBOOT_INFO_ELF_SHDR: u32 = 0x00000020;

/* is there a full memory map? */
pub const MULTIBOOT_INFO_MEM_MAP: u32 = 0x00000040;

/* Is there drive info? */
pub const MULTIBOOT_INFO_DRIVE_INFO: u32 = 0x00000080;

/* Is there a config table? */
pub const MULTIBOOT_INFO_CONFIG_TABLE: u32 = 0x00000100;

/* Is there a boot loader name? */
pub const MULTIBOOT_INFO_BOOT_LOADER_NAME: u32 = 0x00000200;

/* Is there a APM table? */
pub const MULTIBOOT_INFO_APM_TABLE: u32 = 0x00000400;

/* Is there video information? */
pub const MULTIBOOT_INFO_VBE_INFO: u32 = 0x00000800;
pub const MULTIBOOT_INFO_FRAMEBUFFER_INFO: u32 = 0x00001000;

pub const MULTIBOOT_MEMORY_AVAILABLE: u32 = 1;
pub const MULTIBOOT_MEMORY_RESERVED: u32 = 2;
pub const MULTIBOOT_MEMORY_ACPI_RECLAIMABLE: u32 = 3;
pub const MULTIBOOT_MEMORY_NVS: u32 = 4;
pub const MULTIBOOT_MEMORY_BADRAM: u32 = 5;

#[derive(Debug)]
#[repr(C)]
pub struct MultibootHeader {
    pub magic: u32,
    pub flags: u32,
    pub checksum: u32,
    pub header_addr: u32,
    pub load_addr: u32,
    pub load_end_addr: u32,
    pub bss_end_addr: u32,
    pub entry_addr: u32,
    pub mode_type: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

#[repr(C, packed)]
pub struct MultibootMmapEntry {
    pub size: u32,
    pub addr: u64,
    pub len: u64,
    pub type_: u32,
}

#[repr(C)]
#[derive(Default, Debug)]
pub struct MultibootInfo {
    flags: u32,
    mem_lower: u32,
    mem_upper: u32,
    boot_device: u32,
    cmdline: u32,
    mods_count: u32,
    mods_addr: u32,
    syms: [u32; 4],
    mmap_length: u32,
    mmap_addr: u32,
    drives_length: u32,
    drives_addr: u32,
    config_table: u32,
    boot_loader_name: u32,
    apm_table: u32,
    vbe_control_info: u32,
    vbe_mode_info: u32,
    vbe_mode: u16,
    vbe_interface_seg: u16,
    vbe_interface_off: u16,
    vbe_interface_len: u16,
    framebuffer_addr: u64,
    framebuffer_pitch: u32,
    framebuffer_width: u32,
    framebuffer_height: u32,
    framebuffer_bpp: u8,
    framebuffer_type: u8,
    // color info
}

const LOW64K: usize = 64 * KiB;

/// current multiboot() is only tested by Harvey
pub fn multiboot(
    vm: &Arc<VirtualMachine>,
    kernel_path: String,
    _mem_size: usize, // to do: add high memories to multiboot kernels
    load_addr: u64,   // a guest physical address indicating where the guest image should be loaded
    rip: u64,         // initial rip
) -> Result<Vec<GuestThread>, Error> {
    let mut kernel_file = BufReader::new(File::open(&kernel_path)?);
    let mut header_pos = HEADER_POS_LIMIT;
    for i in (0..HEADER_POS_LIMIT).step_by(size_of::<u32>()) {
        let mut buff = [0; 4];
        kernel_file.read_exact(&mut buff)?;
        let v = u32::from_ne_bytes(buff);
        if v == MAGIC1 {
            header_pos = i;
            break;
        }
    }
    if header_pos == HEADER_POS_LIMIT {
        return Err(format!(
            "cannot find magic value 0x{} with the first {} bytes of image {}",
            MAGIC1, HEADER_POS_LIMIT, &kernel_path
        ))?;
    }
    kernel_file
        .seek(SeekFrom::Start(header_pos as u64))
        .unwrap();
    let mut buff = [0u8; size_of::<MultibootHeader>()];
    kernel_file.read_exact(&mut buff).unwrap();
    let header: MultibootHeader = unsafe { transmute(buff) };
    if header
        .magic
        .wrapping_add(header.flags)
        .wrapping_add(header.checksum)
        != 0
    {
        return Err(format!(
            "{}: Multiboot header checksum failed",
            &kernel_path
        ))?;
    }

    // we do not support bit 2 and 16 in the flags yet.
    // see https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#Header-layout
    assert_eq!(header.flags & ((1 << 2) | (1 << 16)), 0);

    let num_gth = vm.cores;
    // setup low memory
    let mut low_mem;
    if let Some(mem) = vm.low_mem.as_ref() {
        low_mem = mem.write().unwrap();
        // let mut mem = low_mem.write().unwrap();
        setup_bios_tables(0xe0000, &mut low_mem, num_gth);
    } else {
        return Err("multiboot requires low memory".to_string())?;
    }

    // load kernel to address 1MiB
    let kn_meta = metadata(&kernel_path)?;
    kernel_file.seek(SeekFrom::Start(0))?;
    kernel_file
        .read_exact(&mut low_mem[load_addr as usize..(load_addr + kn_meta.len()) as usize])?;

    let mbi_addr = MiB + round_up_4k(kn_meta.len() as usize);
    let mmap_table_addr = mbi_addr + round_up_4k(size_of::<MultibootInfo>());

    let entry1 = MultibootMmapEntry {
        size: 20,
        addr: 0,
        len: LOW64K as u64,
        type_: MULTIBOOT_MEMORY_AVAILABLE,
    };
    let entry2 = MultibootMmapEntry {
        size: 20,
        addr: MiB as u64,
        len: (low_mem.size - MiB) as u64,
        type_: MULTIBOOT_MEMORY_AVAILABLE,
    };

    let mbi = MultibootInfo {
        flags: MULTIBOOT_INFO_MEMORY
            // | MULTIBOOT_INFO_CMDLINE
            | MULTIBOOT_INFO_MODS
            | MULTIBOOT_INFO_MEM_MAP
            | MULTIBOOT_INFO_DRIVE_INFO,
        mem_lower: 64,
        mem_upper: ((low_mem.size - MiB) / KiB) as u32,
        // boot_device, not given since the kernel is not loaded from a disk
        // cmdline: 0, // to do
        mods_count: 0,
        mmap_length: size_of::<MultibootMmapEntry>() as u32 * 2,
        mmap_addr: mmap_table_addr as u32,
        drives_length: 0, // no drive

        ..Default::default()
    };

    low_mem.write(mbi, mbi_addr, 0);
    low_mem.write(entry1, mmap_table_addr, 0);
    low_mem.write(entry2, mmap_table_addr + size_of::<MultibootMmapEntry>(), 0);
    let init_regs = vec![
        (X86Reg::RAX, MAGIC1 as u64),
        (X86Reg::RBX, mbi_addr as u64),
        (X86Reg::RIP, rip),
        (X86Reg::RFLAGS, FL_RSVD_1),
        (X86Reg::RSP, low_mem.size as u64),
    ]
    .into_iter()
    .collect();
    let ctrl_pin = gen_exec_ctrl(vmx_read_capability(VMXCap::Pin)?, 0, 0);
    let ctrl_cpu = gen_exec_ctrl(
        vmx_read_capability(VMXCap::CPU)?,
        CPU_BASED_HLT | CPU_BASED_CR8_LOAD | CPU_BASED_CR8_STORE,
        0,
    );

    let ctrl_cpu2 = gen_exec_ctrl(
        vmx_read_capability(VMXCap::CPU2)?,
        CPU_BASED2_RDTSCP | CPU_BASED2_INVPCID,
        0,
    );
    let ctrl_entry = gen_exec_ctrl(vmx_read_capability(VMXCap::Entry)?, 0, 0);
    let cr0 = X86_CR0_NE | X86_CR0_PE;
    let cr4 = X86_CR4_VMXE;
    let init_vmcs = vec![
        (VMCS_GUEST_CS_AR, 0xc09b),
        (VMCS_GUEST_CS_LIMIT, 0xffffffff),
        (VMCS_GUEST_DS_AR, 0xc093),
        (VMCS_GUEST_DS_LIMIT, 0xffffffff),
        (VMCS_GUEST_ES_AR, 0xc093),
        (VMCS_GUEST_ES_LIMIT, 0xffffffff),
        (VMCS_GUEST_FS_AR, 0xc093),
        (VMCS_GUEST_FS_LIMIT, 0xffffffff),
        (VMCS_GUEST_GS_AR, 0xc093),
        (VMCS_GUEST_GS_LIMIT, 0xffffffff),
        (VMCS_GUEST_SS_AR, 0xc093),
        (VMCS_GUEST_SS_LIMIT, 0xffffffff),
        (VMCS_GUEST_LDTR_AR, 0x82),
        (VMCS_GUEST_TR_AR, 0x8b),
        (VMCS_CTRL_PIN_BASED, ctrl_pin),
        (VMCS_CTRL_CPU_BASED, ctrl_cpu),
        (VMCS_CTRL_CPU_BASED2, ctrl_cpu2),
        (VMCS_CTRL_VMENTRY_CONTROLS, ctrl_entry),
        (VMCS_CTRL_EXC_BITMAP, 0xffffffff & !(1 << 14) & !(1 << 3)), // currently we track all exceptions except #BP and #PF.
        (VMCS_GUEST_CR0, cr0),
        (VMCS_CTRL_CR0_MASK, X86_CR0_PG | X86_CR0_PE),
        (VMCS_CTRL_CR0_SHADOW, cr0),
        (VMCS_GUEST_CR4, cr4),
        (VMCS_CTRL_CR4_MASK, X86_CR4_VMXE | X86_CR4_PAE),
        (VMCS_CTRL_CR4_SHADOW, cr4 & !X86_CR4_VMXE),
    ]
    .into_iter()
    .collect();

    let mut guest_threads = Vec::with_capacity(num_gth as usize);
    let mut vector_senders = Vec::with_capacity(num_gth as usize);

    let vm = Arc::new(vm);
    for i in 0..num_gth {
        let (sender, receiver) = channel();
        vector_senders.push(sender);
        let mut gth = GuestThread::new(&vm, i);
        gth.vector_receiver = Some(receiver);
        guest_threads.push(gth);
    }
    *vm.vector_senders.lock().unwrap() = Some(vector_senders);
    guest_threads[0].init_regs = init_regs;
    guest_threads[0].init_vmcs = init_vmcs;
    Ok(guest_threads)
}
