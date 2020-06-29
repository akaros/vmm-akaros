/* SPDX-License-Identifier: GPL-2.0-only */
use super::consts::*;
use super::mach::MachVMBlock;
use super::x86::*;
use super::Error;
use super::{GuestThread, VirtualMachine, X86Reg};
use crate::bios::setup_bios_tables;
use crate::utils::round_up;
#[allow(unused_imports)]
use log::*;
use std::collections::HashMap;
use std::fs::{metadata, File};
use std::io::{Read, Seek, SeekFrom};
use std::mem;
use std::mem::size_of;
use std::sync::{Arc, RwLock};

const LOW64K: usize = 64 * KiB;
const LOW_MEM_SIZE: usize = 1 * MiB;
const RESEARVED_START: usize = 0xC0000000;
const RESEARVED_SIZE: usize = 4 * GiB - RESEARVED_START;
const RD_ADDR: usize = 16 * MiB;

const E820_MAX: usize = 128;
const E820_RAM: u32 = 1;
const E820_RESERVED: u32 = 2;
#[repr(C, packed)]
pub struct E820Entry {
    addr: u64,
    size: u64,
    r#type: u32,
}

#[repr(C, packed)]
pub struct BootParams {
    screen_info: [u8; 0x040 - 0x000],             // 0x000
    apm_bios_info: [u8; 0x054 - 0x040],           // 0x040
    _pad2: [u8; 4],                               // 0x054
    tboot_addr: u64,                              // 0x058
    ist_info: [u8; 0x070 - 0x060],                // 0x060
    acpi_rsdp_addr: [u8; 0x078 - 0x070],          // 0x070
    _pad3: [u8; 0x080 - 0x078],                   // 0x078
    hd0_info: [u8; 0x090 - 0x080],                // 0x080
    hd1_info: [u8; 0x0a0 - 0x090],                // 0x090
    sys_desc_table: [u8; 0x0b0 - 0x0a0],          // 0x0a0
    olpc_ofw_header: [u8; 0x0c0 - 0x0b0],         // 0x0b0
    ext_ramdisk_image: u32,                       // 0x0c0
    ext_ramdisk_size: u32,                        // 0x0c4
    ext_cmd_line_ptr: u32,                        // 0x0c8
    _pad4: [u8; 116],                             // 0x0cc
    edid_info: [u8; 0x1c0 - 0x140],               // 0x140
    efi_info: [u8; 0x1e0 - 0x1c0],                // 0x1c0
    alt_mem_k: u32,                               // 0x1e0
    scratch: u32,                                 // 0x1e4
    e820_entries: u8,                             // 0x1e8
    eddbuf_entries: [u8; 0x1ea - 0x1e9],          // 0x1e9
    edd_mbr_sig_buf_entries: [u8; 0x1eb - 0x1ea], // 0x1ea
    kbd_status: [u8; 0x1ec - 0x1eb],              // 0x1eb
    secure_boot: [u8; 0x1ed - 0x1ec],             // 0x1ec
    _pad5: [u8; 0x1ef - 0x1ed],                   // 0x1ed
    sentinel: [u8; 0x1f0 - 0x1ef],                // 0x1ef
    _pad6: [u8; 0x1f1 - 0x1f0],                   // 0x1f0
    hdr: SetupHeader,                             // 0x1f1
    _pad7: [u8; 0x290 - 0x1f1 - size_of::<SetupHeader>()],
    edd_mbr_sig_buffer: [u8; 0x2d0 - 0x290], // 0x290
    e820_table: [E820Entry; E820_MAX],       // 0x2d0
    _pad8: [u8; 0xd00 - 0xcd0],              // 0xcd0
    eddbuf: [u8; 0xeec - 0xd00],             // 0xd00
    _pad9: [u8; 276],                        // 0xeec
}

#[repr(C, packed)]
pub struct SetupHeader {
    setup_sects: u8,
    root_flags: u16,
    syssize: u32,
    ram_size: u16,
    vid_mode: u16,
    root_dev: u16,
    boot_flag: u16,
    jump: u16,
    header: u32,
    version: u16,
    realmode_swtch: u32,
    start_sys: u16,
    kernel_version: u16,
    type_of_loader: u8,
    loadflags: u8,
    setup_move_size: u16,
    code32_start: u32,
    ramdisk_image: u32,
    ramdisk_size: u32,
    bootsect_kludge: u32,
    heap_end_ptr: u16,
    ext_loader_ver: u8,
    ext_loader_type: u8,
    cmd_line_ptr: u32,
    initrd_addr_max: u32,
    kernel_alignment: u32,
    relocatable_kernel: u8,
    min_alignment: u8,
    xloadflags: u16,
    cmdline_size: u32,
    hardware_subarch: u32,
    hardware_subarch_data: u64,
    payload_offset: u32,
    payload_length: u32,
    setup_data: u64,
    pref_address: u64,
    init_size: u32,
    handover_offset: u32,
}

impl BootParams {
    pub fn new() -> Self {
        // Definitely unsafe is not preferred,
        // but rust cannot derive Default for T[n] where n > 32
        unsafe { mem::zeroed() }
    }
}

const HEADER_OFFSET: u64 = 0x01f1;
const HDRS: u32 = 0x53726448;
const ENTRY_64: usize = 0x200;

pub fn load_linux64(
    vm: &Arc<RwLock<VirtualMachine>>,
    kernel_path: String,
    rd_path: Option<String>,
    cmd_line: String,
    mem_size: usize,
) -> Result<Vec<GuestThread>, Error> {
    // first we make sure the kernel is not too old
    let kn_meta = metadata(&kernel_path).unwrap();
    let mut kernel_file = File::open(&kernel_path).unwrap();
    let header: SetupHeader = {
        let mut buff = [0u8; size_of::<SetupHeader>()];
        kernel_file.seek(SeekFrom::Start(HEADER_OFFSET)).unwrap();
        kernel_file.read_exact(&mut buff).unwrap();
        unsafe { mem::transmute(buff) }
    };
    if header.setup_sects == 0
        || header.boot_flag != 0xaa55
        || header.header != HDRS
        || header.version < 0x020a
        || header.loadflags & 1 == 0
        || header.relocatable_kernel == 0
    {
        return Err("kernel too old\n")?;
    }

    // setup low memory
    let mut low_mem = MachVMBlock::new(LOW_MEM_SIZE).unwrap();

    let num_gth = { vm.read().unwrap().cores };
    let bios_table_size = setup_bios_tables(num_gth, 0xe0000, &mut low_mem);
    trace!("bios_table_size = {:x}", bios_table_size);
    let mut vapic_block = MachVMBlock::new(num_gth as usize * PAGE_SIZE * 2)?;

    for i in 0..num_gth {
        let vapic_offset = i as usize * PAGE_SIZE;
        vapic_block.write(i as u32, vapic_offset + 0x20, 0);
        vapic_block.write(0x01060015u32, vapic_offset + 0x30, 0);
        vapic_block.write((1 << i) as u32, vapic_offset + 0xd0, 0);
    }

    let vapic_block_start = vapic_block.start;

    let bp_offset = mem_size - PAGE_SIZE;
    let cmd_line_offset = bp_offset - PAGE_SIZE;
    let gdt_offset = cmd_line_offset - PAGE_SIZE;
    let pml4_offset = gdt_offset - PAGE_SIZE;
    let first_pdpt_offset = pml4_offset - PAGE_SIZE;

    let mut high_mem =
        MachVMBlock::new_aligned(mem_size, header.kernel_alignment as usize).unwrap();
    let mut bp = BootParams::new();
    bp.hdr = header;

    // load kernel
    let kernel_offset = (bp.hdr.setup_sects as u64 + 1) * 512;
    kernel_file.seek(SeekFrom::Start(kernel_offset)).unwrap();
    kernel_file
        .read_exact(&mut high_mem[0..(kn_meta.len() - kernel_offset) as usize])
        .unwrap();

    // command line
    let cmd_line_base = high_mem.start + cmd_line_offset;
    &high_mem[cmd_line_offset..(cmd_line_offset + cmd_line.len())]
        .clone_from_slice(cmd_line.as_bytes());
    bp.hdr.cmd_line_ptr = (cmd_line_base & 0xffffffff) as u32;
    bp.ext_cmd_line_ptr = (cmd_line_base >> 32) as u32;

    // bp.hdr.hardware_subarch = 0;
    // bp.hdr.type_of_loader = 0xd;
    // load ramdisk
    let rd_size;
    let rd_mem;
    if let Some(rd_path) = rd_path {
        let rd_meta = metadata(&rd_path)?;
        let mut rd_file = File::open(&rd_path)?;
        rd_size = rd_meta.len() as usize;
        let mut rd_mem_block = MachVMBlock::new(round_up(rd_size))?;
        rd_file.read_exact(&mut rd_mem_block[0..rd_size])?;
        bp.hdr.ramdisk_image = (RD_ADDR & 0xffffffff) as u32;
        bp.ext_ramdisk_image = (RD_ADDR >> 32) as u32;
        bp.hdr.ramdisk_size = (rd_size & 0xffffffff) as u32;
        bp.ext_ramdisk_size = (rd_size >> 32) as u32;
        bp.hdr.root_dev = 0x100;
        rd_mem = Some(rd_mem_block);
    } else {
        rd_size = 0;
        rd_mem = None;
    }

    let mut index = 0;
    bp.e820_table[index] = E820Entry {
        addr: 0,
        size: PAGE_SIZE as u64,
        r#type: E820_RESERVED,
    };
    index += 1;
    bp.e820_table[index] = E820Entry {
        addr: PAGE_SIZE as u64,
        size: (LOW64K - PAGE_SIZE) as u64,
        r#type: E820_RAM,
    };
    index += 1;
    if rd_size > 0 {
        bp.e820_table[index] = E820Entry {
            addr: LOW64K as u64,
            size: (RD_ADDR - LOW64K) as u64,
            r#type: E820_RESERVED,
        };
        index += 1;
        bp.e820_table[index] = E820Entry {
            addr: RD_ADDR as u64,
            size: round_up(rd_size) as u64,
            r#type: E820_RAM,
        };
        index += 1;
        let rd_end = RD_ADDR + round_up(rd_size);
        bp.e820_table[index] = E820Entry {
            addr: rd_end as u64,
            size: (high_mem.start - rd_end) as u64,
            r#type: E820_RESERVED,
        };
        index += 1;
    } else {
        bp.e820_table[index] = E820Entry {
            addr: LOW64K as u64,
            size: (high_mem.start - LOW64K) as u64,
            r#type: E820_RESERVED,
        };
        index += 1;
    }
    bp.e820_table[index] = E820Entry {
        addr: high_mem.start as u64,
        size: high_mem.size as u64,
        r#type: 1,
    };
    index += 1;
    bp.e820_entries = index as u8;

    high_mem.write(bp, bp_offset, 0);

    // setup virtio, fix me, not finished
    // let virtio_mmio_base_addr_hint = high_mem.start + high_mem.size;

    // setup gdt
    let gdt_entries: [u64; 4] = [0, 0, 0x00af9a000000ffff, 0x00cf92000000ffff];
    high_mem.write(gdt_entries, gdt_offset, 0);

    // identity paging
    let pml4e_base = high_mem.start + pml4_offset;
    let first_pdpt_base = high_mem.start + first_pdpt_offset;
    let pml4e: u64 = PG_P | PG_RW | first_pdpt_base as u64;
    high_mem.write(pml4e, pml4_offset, 0);
    for i in 0..512 {
        let pdpte: u64 = (i << 30) | PG_P | PG_RW | PG_PS;
        high_mem.write(pdpte, first_pdpt_offset, i as usize);
    }

    let init_regs = vec![
        (X86Reg::CR3, pml4e_base as u64),
        (X86Reg::GDT_BASE, (high_mem.start + gdt_offset) as u64),
        (X86Reg::GDT_LIMIT, 0x1f),
        (X86Reg::RFLAGS, FL_RSVD_1),
        (X86Reg::RSI, (high_mem.start + bp_offset) as u64),
        (X86Reg::RIP, (high_mem.start + ENTRY_64) as u64),
    ]
    .into_iter()
    .collect();
    let mut mem_maps = HashMap::new();
    mem_maps.insert(0, low_mem);
    mem_maps.insert(high_mem.start, high_mem);
    if let Some(rd_mem_block) = rd_mem {
        mem_maps.insert(RD_ADDR, rd_mem_block);
    }
    let apic_page = MachVMBlock::new(PAGE_SIZE)?;
    mem_maps.insert(APIC_GPA, apic_page);
    mem_maps.insert(vapic_block_start, vapic_block);
    {
        let mut vm_ = vm.write().unwrap();
        vm_.map_guest_mem(mem_maps)?;
    }
    let mut guest_threads = vec![];
    for i in 0..num_gth {
        let mut gth = GuestThread::new(vm, i);
        gth.vapic_addr = vapic_block_start + i as usize * PAGE_SIZE;
        gth.posted_irq_desc = vapic_block_start + (i + num_gth) as usize * PAGE_SIZE;
        info!("guest thread {} with vapid_addr = {:x}", i, gth.vapic_addr);
        guest_threads.push(gth);
    }
    guest_threads[0].init_regs = init_regs;
    Ok(guest_threads)
}
