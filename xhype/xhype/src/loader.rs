use super::consts::*;
use super::mach::MachVMBlock;
use super::paging::*;
use super::x86::*;
use super::Error;
use super::{GuestThread, VirtualMachine, X86Reg};
use log::warn;
use std::collections::HashMap;
use std::fs::{metadata, File};
use std::io::{Read, Seek, SeekFrom};
use std::mem;
use std::mem::size_of;
use std::num::Wrapping;
use std::sync::{Arc, RwLock};

const LOW64K: usize = 64 * KiB;
const LOW_MEM_SIZE: usize = 1 * MiB;
const RESEARVED_START: usize = 0xC0000000;
const RESEARVED_SIZE: usize = 4 * GiB - RESEARVED_START;
const RD_ADDR: usize = 16 * MiB;

const APIC_GPA: usize = 0xfee00000;

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

// https://uefi.org/sites/default/files/resources/ACPI_6_3_May16.pdf
// Table 5-27 RSDP Structure

#[repr(packed)]
#[derive(Default)]
struct AcpiTableRsdp {
    signature: [u8; 8],         /* ACPI signature, contains "RSD PTR " */
    checksum: u8,               /* ACPI 1.0 checksum */
    oem_id: [u8; 6],            /* OEM identification */
    revision: u8,               /* Must be (0) for ACPI 1.0 or (2) for ACPI 2.0+ */
    rsdt_physical_address: u32, /* 32-bit physical address of the RSDT */
    length: u32,                /* Table length in bytes, including header (ACPI 2.0+) */
    xsdt_physical_address: u64, /* 64-bit physical address of the XSDT (ACPI 2.0+) */
    extended_checksum: u8,      /* Checksum of entire table (ACPI 2.0+) */
    reserved: [u8; 3],          /* Reserved, must be zero */
}
const ACPI_RSDP_CHECKSUM_LENGTH: usize = 20;
const ACPI_RSDP_XCHECKSUM_LENGTH: usize = 36;
const ACPI_RSDP_CHECKSUM_OFFSET: usize = 8;
const ACPI_RSDP_XCHECKSUM_OFFSET: usize = 32;

#[repr(packed)]
#[derive(Default)]
struct AcpiTableHeader {
    signature: [u8; 4],         /* ASCII table signature */
    length: u32,                /* Length of table in bytes, including this header */
    revision: u8,               /* ACPI Specification minor version number */
    checksum: u8,               /* To make sum of entire table == 0 */
    oem_id: [u8; 6],            /* ASCII OEM identification */
    oem_table_id: [u8; 8],      /* ASCII OEM table identification */
    oem_revision: u32,          /* OEM revision number */
    asl_compiler_id: [u8; 4],   /* ASCII ASL compiler vendor ID */
    asl_compiler_revision: u32, /* ASL compiler version */
}
const ACPI_TABLE_HEADER_CHECKSUM_OFFSET: usize = 9;

impl AcpiTableHeader {
    fn new() -> Self {
        AcpiTableHeader {
            revision: 2,
            checksum: 0,
            oem_id: *b"AKAROS",
            oem_table_id: *b"ALPHABET",
            oem_revision: 0,
            asl_compiler_id: *b"RON ",
            asl_compiler_revision: 0,
            ..Default::default()
        }
    }
}

#[repr(packed)]
#[derive(Default)]
struct AcpiGenericAddress {
    space_id: u8,     /* Address space where struct or register exists */
    bit_width: u8,    /* Size in bits of given register */
    bit_offset: u8,   /* Bit offset within the register */
    access_width: u8, /* Minimum Access size (ACPI 3.0) */
    address: u64,     /* 64-bit address of struct or register */
}

#[repr(packed)]
#[derive(Default)]
struct AcpiTableFadt {
    header: AcpiTableHeader,                 /* Common ACPI table header */
    facs: u32,                               /* 32-bit physical address of FACS */
    dsdt: u32,                               /* 32-bit physical address of DSDT */
    model: u8,               /* System Interrupt Model (ACPI 1.0) - not used in ACPI 2.0+ */
    preferred_profile: u8,   /* Conveys preferred power management profile to OSPM. */
    sci_interrupt: u16,      /* System vector of SCI interrupt */
    smi_command: u32,        /* 32-bit Port address of SMI command port */
    acpi_enable: u8,         /* Value to write to SMI_CMD to enable ACPI */
    acpi_disable: u8,        /* Value to write to SMI_CMD to disable ACPI */
    s4_bios_request: u8,     /* Value to write to SMI_CMD to enter S4BIOS state */
    pstate_control: u8,      /* Processor performance state control */
    pm1a_event_block: u32,   /* 32-bit port address of Power Mgt 1a Event Reg Blk */
    pm1b_event_block: u32,   /* 32-bit port address of Power Mgt 1b Event Reg Blk */
    pm1a_control_block: u32, /* 32-bit port address of Power Mgt 1a Control Reg Blk */
    pm1b_control_block: u32, /* 32-bit port address of Power Mgt 1b Control Reg Blk */
    pm2_control_block: u32,  /* 32-bit port address of Power Mgt 2 Control Reg Blk */
    pm_timer_block: u32,     /* 32-bit port address of Power Mgt Timer Ctrl Reg Blk */
    gpe0_block: u32,         /* 32-bit port address of General Purpose Event 0 Reg Blk */
    gpe1_block: u32,         /* 32-bit port address of General Purpose Event 1 Reg Blk */
    pm1_event_length: u8,    /* Byte Length of ports at pm1x_event_block */
    pm1_control_length: u8,  /* Byte Length of ports at pm1x_control_block */
    pm2_control_length: u8,  /* Byte Length of ports at pm2_control_block */
    pm_timer_length: u8,     /* Byte Length of ports at pm_timer_block */
    gpe0_block_length: u8,   /* Byte Length of ports at gpe0_block */
    gpe1_block_length: u8,   /* Byte Length of ports at gpe1_block */
    gpe1_base: u8,           /* Offset in GPE number space where GPE1 events start */
    cst_control: u8,         /* Support for the _CST object and C-States change notification */
    c2_latency: u16,         /* Worst case HW latency to enter/exit C2 state */
    c3_latency: u16,         /* Worst case HW latency to enter/exit C3 state */
    flush_size: u16,         /* Processor memory cache line width, in bytes */
    flush_stride: u16,       /* Number of flush strides that need to be read */
    duty_offset: u8,         /* Processor duty cycle index in processor P_CNT reg */
    duty_width: u8,          /* Processor duty cycle value bit width in P_CNT register */
    day_alarm: u8,           /* Index to day-of-month alarm in RTC CMOS RAM */
    month_alarm: u8,         /* Index to month-of-year alarm in RTC CMOS RAM */
    century: u8,             /* Index to century in RTC CMOS RAM */
    boot_flags: u16,         /* IA-PC Boot Architecture Flags (see below for individual flags) */
    reserved: u8,            /* Reserved, must be zero */
    flags: u32,              /* Miscellaneous flag bits (see below for individual flags) */
    reset_register: AcpiGenericAddress, /* 64-bit address of the Reset register */
    reset_value: u8,         /* Value to write to the reset_register port to reset the system */
    arm_boot_flags: u16, /* ARM-Specific Boot Flags (see below for individual flags) (ACPI 5.1) */
    minor_revision: u8,  /* FADT Minor Revision (ACPI 5.1) */
    xfacs: u64,          /* 64-bit physical address of FACS */
    xdsdt: u64,          /* 64-bit physical address of DSDT */
    xpm1a_event_block: AcpiGenericAddress, /* 64-bit Extended Power Mgt 1a Event Reg Blk address */
    xpm1b_event_block: AcpiGenericAddress, /* 64-bit Extended Power Mgt 1b Event Reg Blk address */
    xpm1a_control_block: AcpiGenericAddress, /* 64-bit Extended Power Mgt 1a Control Reg Blk address */
    xpm1b_control_block: AcpiGenericAddress, /* 64-bit Extended Power Mgt 1b Control Reg Blk address */
    xpm2_control_block: AcpiGenericAddress, /* 64-bit Extended Power Mgt 2 Control Reg Blk address */
    xpm_timer_block: AcpiGenericAddress, /* 64-bit Extended Power Mgt Timer Ctrl Reg Blk address */
    xgpe0_block: AcpiGenericAddress, /* 64-bit Extended General Purpose Event 0 Reg Blk address */
    xgpe1_block: AcpiGenericAddress, /* 64-bit Extended General Purpose Event 1 Reg Blk address */
    sleep_control: AcpiGenericAddress, /* 64-bit Sleep Control register (ACPI 5.0) */
    sleep_status: AcpiGenericAddress, /* 64-bit Sleep Status register (ACPI 5.0) */
    hypervisor_id: u64,              /* Hypervisor Vendor ID (ACPI 6.0) */
}

#[repr(packed)]
#[derive(Default)]
struct AcpiTableMadt {
    header: AcpiTableHeader, /* Common ACPI table header */
    address: u32,            /* Physical address of local APIC */
    flags: u32,
}

#[repr(packed)]
#[derive(Default)]
struct AcpiSubtableHader {
    r#type: u8,
    length: u8,
}

#[repr(packed)]
#[derive(Default)]
struct AcpiMadtLocalApic {
    header: AcpiSubtableHader,
    processor_id: u8, /* ACPI processor id */
    id: u8,           /* Processor's local APIC id */
    lapic_flags: u32,
}

#[repr(packed)]
#[derive(Default)]
struct AcpiMadtIoApic {
    header: AcpiSubtableHader,
    id: u8,               /* I/O APIC ID */
    reserved: u8,         /* reserved - must be zero */
    address: u32,         /* APIC physical address */
    global_irq_base: u32, /* Global system interrupt where INTI lines start */
}

#[repr(packed)]
#[derive(Default)]
struct AcpiMadtLocalX2apic {
    header: AcpiSubtableHader,
    reserved: u16,      /* reserved - must be zero */
    local_apic_id: u32, /* Processor x2APIC ID  */
    lapic_flags: u32,
    uid: u32, /* ACPI processor UID */
}

#[inline]
fn gencsum(data: &[u8]) -> u8 {
    (!data.iter().map(|x| Wrapping(*x)).sum::<Wrapping<u8>>() + Wrapping(1)).0
}
#[inline]
fn acpi_tb_checksum(data: &[u8]) -> u8 {
    data.iter().map(|x| Wrapping(*x)).sum::<Wrapping<u8>>().0
}

#[inline]
fn round_up(num: usize) -> usize {
    (num + 0xfff) & !0xfff
}

#[inline]
fn round_down(num: usize) -> usize {
    num & !0xfff
}

impl VirtualMachine {
    pub fn alloc_intr_pages(&self) -> Result<MachVMBlock, Error> {
        // fix me: allocate apic page? fee00000
        // allocate vapic and pir pages
        // first self.cores pages are for vapic_page, the second self.cores are
        // for posted irq
        let total_size = self.cores as usize * PAGE_SIZE * 2;
        let mut vapic_block = MachVMBlock::new(total_size)?;

        for i in 0..self.cores {
            let vapic_offset = i as usize * PAGE_SIZE;
            vapic_block.write(i as u32, vapic_offset + 0x20, 0);
            vapic_block.write(0x01060015u32, vapic_offset + 0x30, 0);
            vapic_block.write((1 << i) as u32, vapic_offset + 0xd0, 0);
        }
        Ok(vapic_block)
    }

    pub fn setup_bios_tables(&self, start: usize, low_mem: &mut MachVMBlock) -> usize {
        let rsdp_offset = start;
        let xsdt_offset = rsdp_offset + size_of::<AcpiTableRsdp>();
        let xsdt_entry_offset = xsdt_offset + size_of::<AcpiTableHeader>();
        const NUM_XSDT_ENTRIES: usize = 9;
        let fadt_offset = xsdt_entry_offset + NUM_XSDT_ENTRIES * size_of::<usize>();
        let dsdt_offset = fadt_offset + size_of::<AcpiTableFadt>();
        let madt_offset = dsdt_offset + 36;
        let madt_local_apic_offset = madt_offset + size_of::<AcpiTableMadt>();
        let io_apic_offset =
            madt_local_apic_offset + self.cores as usize * size_of::<AcpiMadtLocalApic>();
        let local_x2apic_offset = io_apic_offset + size_of::<AcpiMadtIoApic>();
        let total_size =
            local_x2apic_offset + self.cores as usize * size_of::<AcpiMadtLocalX2apic>() - start;

        // rsdp
        let rdsp = AcpiTableRsdp {
            signature: *b"RSD PTR ",
            revision: 2,
            length: 36,
            xsdt_physical_address: xsdt_offset as u64,
            ..Default::default()
        };
        //dbg!(&rdsp);
        low_mem.write(rdsp, rsdp_offset, 0);
        low_mem[rsdp_offset + ACPI_RSDP_CHECKSUM_OFFSET] =
            gencsum(&low_mem[rsdp_offset..(rsdp_offset + ACPI_RSDP_CHECKSUM_LENGTH)]);
        debug_assert_eq!(
            acpi_tb_checksum(&low_mem[rsdp_offset..(rsdp_offset + ACPI_RSDP_CHECKSUM_LENGTH)]),
            0
        );
        low_mem[rsdp_offset + ACPI_RSDP_XCHECKSUM_OFFSET] =
            gencsum(&low_mem[rsdp_offset..(rsdp_offset + ACPI_RSDP_XCHECKSUM_LENGTH)]);
        debug_assert_eq!(
            acpi_tb_checksum(&low_mem[rsdp_offset..(rsdp_offset + ACPI_RSDP_XCHECKSUM_LENGTH)]),
            0
        );

        // xsdt
        let xsdt_total_length = size_of::<AcpiTableHeader>() + size_of::<u64>() * NUM_XSDT_ENTRIES;
        let xsdt = AcpiTableHeader {
            signature: *b"XSDT",
            length: xsdt_total_length as u32,
            ..AcpiTableHeader::new()
        };
        //dbg!(&xsdt);
        low_mem.write(xsdt, xsdt_offset, 0);
        // xsdt entries
        let mut xsdt_entries: [u64; NUM_XSDT_ENTRIES] = [0; NUM_XSDT_ENTRIES];
        xsdt_entries[0] = fadt_offset as u64;
        xsdt_entries[3] = madt_offset as u64;
        //dbg!(&xsdt_entries);
        low_mem.write(xsdt_entries, xsdt_entry_offset, 0);
        low_mem[xsdt_offset + ACPI_TABLE_HEADER_CHECKSUM_OFFSET] =
            gencsum(&low_mem[xsdt_offset..(xsdt_offset + xsdt_total_length)]);
        debug_assert_eq!(
            acpi_tb_checksum(&low_mem[xsdt_offset..(xsdt_offset + xsdt_total_length)]),
            0
        );
        // fadt
        let fadt = AcpiTableFadt {
            header: AcpiTableHeader {
                signature: *b"FACP",
                length: size_of::<AcpiTableFadt>() as u32,
                ..AcpiTableHeader::new()
            },
            xdsdt: dsdt_offset as u64,
            ..Default::default()
        };
        //dbg!(&fadt);
        low_mem.write(fadt, fadt_offset, 0);
        low_mem[fadt_offset + ACPI_TABLE_HEADER_CHECKSUM_OFFSET] =
            gencsum(&low_mem[fadt_offset..(fadt_offset + size_of::<AcpiTableFadt>())]);
        debug_assert_eq!(
            acpi_tb_checksum(&low_mem[fadt_offset..(fadt_offset + size_of::<AcpiTableFadt>())]),
            0
        );

        // dsdt
        let dsdt_dsdttbl_header: [u8; 36] = [
            0x44, 0x53, 0x44, 0x54, 0x24, 0x00, 0x00, 0x00, /* 00000000    "DSDT$..." */
            0x02, 0xF3, 0x4D, 0x49, 0x4B, 0x45, 0x00, 0x00, /* 00000008    "..MIKE.." */
            0x44, 0x53, 0x44, 0x54, 0x54, 0x42, 0x4C, 0x00, /* 00000010    "DSDTTBL." */
            0x00, 0x00, 0x00, 0x00, 0x49, 0x4E, 0x54, 0x4C, /* 00000018    "....INTL" */
            0x14, 0x02, 0x14, 0x20, /* 00000020    "... " */
        ];
        low_mem.write(dsdt_dsdttbl_header, dsdt_offset, 0);

        // mddt
        let madt_total_length = size_of::<AcpiTableMadt>()
            + size_of::<AcpiMadtIoApic>()
            + self.cores as usize
                * (size_of::<AcpiMadtLocalApic>() + size_of::<AcpiMadtLocalX2apic>());
        let madt = AcpiTableMadt {
            header: AcpiTableHeader {
                signature: *b"APIC",
                length: madt_total_length as u32,
                ..AcpiTableHeader::new()
            },
            address: APIC_GPA as u32,
            flags: 0,
            ..Default::default()
        };
        //dbg!(&madt);
        low_mem.write(madt, madt_offset, 0);

        // local apic
        for i in 0..self.cores {
            let lapic = AcpiMadtLocalApic {
                header: AcpiSubtableHader {
                    r#type: 0,
                    length: size_of::<AcpiMadtLocalApic>() as u8,
                },
                processor_id: i as u8,
                id: i as u8,
                lapic_flags: 1,
            };
            //dbg!(i, &lapic);
            low_mem.write(lapic, madt_local_apic_offset, i as usize)
        }

        // io apiic
        let io_apic = AcpiMadtIoApic {
            header: AcpiSubtableHader {
                r#type: 1,
                length: size_of::<AcpiMadtIoApic>() as u8,
            },
            id: 0,
            address: 0xfec00000,
            global_irq_base: 0,
            ..Default::default()
        };
        //dbg!(&io_apic);
        low_mem.write(io_apic, io_apic_offset, 0);

        // local x2apic
        for i in 0..self.cores {
            let x2apic = AcpiMadtLocalX2apic {
                header: AcpiSubtableHader {
                    r#type: 9,
                    length: size_of::<AcpiMadtLocalX2apic>() as u8,
                },
                local_apic_id: i,
                uid: i,
                lapic_flags: 1,
                ..Default::default()
            };
            //dbg!(i, &x2apic);
            low_mem.write(x2apic, local_x2apic_offset, i as usize)
        }
        low_mem[madt_offset + ACPI_TABLE_HEADER_CHECKSUM_OFFSET] =
            gencsum(&low_mem[madt_offset..(madt_offset + madt_total_length)]);
        debug_assert_eq!(
            acpi_tb_checksum(&low_mem[madt_offset..(madt_offset + madt_total_length)]),
            0
        );

        (total_size + 0xfff) & !0xfff
    }
}

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
    let vapic_block;
    {
        let vm_ = &*vm.read().unwrap();
        vapic_block = vm_.alloc_intr_pages()?;
        let bios_table_size = vm_.setup_bios_tables(0xe0000, &mut low_mem);
        println!("bios_table_size = {:x}", bios_table_size);
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
    let num_gth;
    {
        let vm_ = &mut *vm.write().unwrap();
        vm_.map_guest_mem(mem_maps)?;
        num_gth = vm_.cores;
    }
    let mut guest_threads = vec![];
    for i in 0..num_gth {
        guest_threads.push(GuestThread {
            id: i,
            vm: Arc::clone(vm),
            init_regs: HashMap::new(),
            init_vmcs: HashMap::new(),
            vapic_addr: vapic_block_start + i as usize * PAGE_SIZE,
            posted_irq_desc: vapic_block_start + (i + num_gth) as usize * PAGE_SIZE,
        });
    }
    guest_threads[0].init_regs = init_regs;
    Ok(guest_threads)
}

mod test {
    use super::{
        AcpiMadtIoApic, AcpiMadtLocalApic, AcpiMadtLocalX2apic, AcpiTableFadt, AcpiTableHeader,
        AcpiTableMadt, AcpiTableRsdp,
    };
    use std::mem::size_of;
    #[test]
    fn bois_table_struct_test() {
        assert_eq!(size_of::<AcpiTableRsdp>(), 36);
        assert_eq!(size_of::<AcpiTableHeader>(), 36);
        assert_eq!(size_of::<AcpiTableFadt>(), 276);
        assert_eq!(size_of::<AcpiTableMadt>(), 44);
        assert_eq!(size_of::<AcpiMadtLocalApic>(), 8);
        assert_eq!(size_of::<AcpiMadtIoApic>(), 12);
        assert_eq!(size_of::<AcpiMadtLocalX2apic>(), 16);
    }
}
