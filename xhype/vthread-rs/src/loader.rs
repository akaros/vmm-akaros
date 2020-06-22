use super::consts::*;
use super::mach::MachVMBlock;
use super::paging::*;
use super::x86::*;
use super::Error;
use super::{GuestThread, VirtualMachine, X86Reg};
use std::collections::HashMap;
use std::fs::{metadata, File};
use std::io::{Read, Seek, SeekFrom};
use std::mem;
use std::mem::size_of;

const LOW_MEM_SIZE: usize = 100 * MiB;

const E820_MAX: usize = 128;
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

// https://uefi.org/sites/default/files/resources/ACPI_6_3_May16.pdf
// Table 5-27 RSDP Structure

#[repr(packed)]
#[derive(Default)]
struct AcpiTableRsdp {
    signature: [u8; 8],
    checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_physical_addrress: u32,
    length: u32,
    xsdt_physical_address: u64,
    extended_checksum: u8,
    reserved: [u8; 3],
}

#[repr(packed)]
#[derive(Default)]
struct AcpiTableHeader {}

extern "C" {
    // fn gencsum_c(target: *mut u8, data: *const u8, len: i32);
    fn acpi_tb_checksum_c(buffer: *const u8, length: u32) -> u8;
}

// FixMe: implement this function in rust
fn acpi_tb_checksum<T>(buffer: &T, len: u32) -> u8 {
    unsafe { acpi_tb_checksum_c(buffer as *const T as *const u8, len) }
}

const ACPI_RSDP_CHECKSUM_LENGTH: u32 = 20;
const ACPI_RSDP_XCHECKSUM_LENGTH: u32 = 36;
impl VirtualMachine {
    fn alloc_intr_pages(&mut self, low_mem: &mut MachVMBlock) {
        // fix me: allocate apic page? fee00000
        // allocate vapic and pir pages
        let pir_offset = self.cores as usize * PAGE_SIZE;
        for i in 0..self.cores {
            let vapic_offset = i as usize * PAGE_SIZE;
            low_mem.as_mut_slice()[vapic_offset + 0x20 / 4] = i;
            low_mem.as_mut_slice()[vapic_offset + 0x30 / 4] = 0x01060015u32;
            low_mem.as_mut_slice()[vapic_offset + 0xd0 / 4] = 1 << i;
        }
        // Ok(())
    }

    fn setup_biostables(&mut self, low_mem: &mut MachVMBlock) {
        let rsdp_offset = 0xe0000;
        let mut rdsp = AcpiTableRsdp {
            signature: *b"RSD PTR ",
            revision: 2,
            length: 36,
            xsdt_physical_address: rsdp_offset,
            ..Default::default()
        };
        rdsp.checksum = 0;
        rdsp.checksum = !acpi_tb_checksum(&rdsp, ACPI_RSDP_CHECKSUM_LENGTH) + 1;
        debug_assert_eq!(acpi_tb_checksum(&rdsp, ACPI_RSDP_CHECKSUM_LENGTH), 0);
        rdsp.extended_checksum = 0;
        rdsp.extended_checksum = !acpi_tb_checksum(&rdsp, ACPI_RSDP_XCHECKSUM_LENGTH) + 1;
        if (rdsp.revision >= 2) {
            debug_assert_eq!(acpi_tb_checksum(&rdsp, ACPI_RSDP_XCHECKSUM_LENGTH), 0);
        }
        unimplemented!()
    }
}

pub fn load_linux<'a>(
    vm: &'a VirtualMachine,
    kernel_path: &str,
    rd_path: &str,
    cmd_line: &str,
    mem_size: usize,
) -> Result<GuestThread<'a>, Error> {
    unimplemented!()
}

const RD_OFFSET: usize = 0x100000;
pub fn load_linux64<'a>(
    vm: &'a VirtualMachine,
    kernel_path: &str,
    rd_path: &str,
    cmd_line: &str,
    mem_size: usize,
) -> Result<GuestThread<'a>, &'static str> {
    // dbg!(high_mem.start);

    let bp_offset = mem_size - PAGE_SIZE;
    let cmd_line_offset = bp_offset - PAGE_SIZE;
    let gdt_offset = cmd_line_offset - PAGE_SIZE;
    let pml4_offset = gdt_offset - PAGE_SIZE;
    let first_pdpt_offset = pml4_offset - PAGE_SIZE;

    let kn_meta = metadata(kernel_path).unwrap();
    let mut kernel_file = File::open(kernel_path).unwrap();
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
        // dbg!(header);
        return Err("kernel too old\n");
    }
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

    bp.hdr.hardware_subarch = 0;
    bp.hdr.type_of_loader = 0xd;

    let mut low_mem = MachVMBlock::new(LOW_MEM_SIZE).unwrap();

    // load ramdisk
    let rd_meta = metadata(rd_path).unwrap();
    let mut rd_file = File::open(rd_path).unwrap();
    rd_file
        .read_exact(&mut low_mem[RD_OFFSET..RD_OFFSET + rd_meta.len() as usize])
        .unwrap();

    bp.hdr.ramdisk_image = (RD_OFFSET & 0xffffffff) as u32;
    bp.ext_ramdisk_image = (RD_OFFSET >> 32) as u32;
    let rd_size = rd_meta.len();
    bp.hdr.ramdisk_size = (rd_size & 0xffffffff) as u32;
    bp.ext_ramdisk_size = (rd_size >> 32) as u32;

    bp.e820_table[0] = E820Entry {
        addr: 0,
        size: 0x97fc0,
        r#type: 1,
    };
    bp.e820_table[1] = E820Entry {
        addr: RD_OFFSET as u64,
        size: (LOW_MEM_SIZE - RD_OFFSET) as u64,
        r#type: 1,
    };
    bp.e820_table[2] = E820Entry {
        addr: high_mem.start as u64,
        size: high_mem.size as u64,
        r#type: 1,
    };
    bp.e820_entries = 3;

    high_mem.write(bp, bp_offset, 0);

    // setup gdt
    let gdt_entries: [u64; 4] = [0, 0, 0x00af9a000000ffff, 0x00cf92000000ffff];
    high_mem.write(gdt_entries, gdt_offset, 0);

    // identity paging
    let pml4e_base = high_mem.start + pml4_offset;
    let first_pdpt_base = high_mem.start + first_pdpt_offset;
    let pml4e: u64 = PG_P | PG_RW | first_pdpt_base as u64;
    high_mem.write(pml4e, pml4_offset, 0);
    for i in 0..512 {
        let pdpte: u64 = (i << 30) | PG_P | PG_RW | PG_1GB_PS;
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
    let init_vmcs = HashMap::new();
    let mem_maps = vec![(0, low_mem), (high_mem.start, high_mem)]
        .into_iter()
        .collect();
    Ok(GuestThread {
        vm,
        init_regs,
        init_vmcs,
        mem_maps,
    })
}
