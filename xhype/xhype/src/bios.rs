use crate::mach::MachVMBlock;
use crate::x86::*;
use std::mem::size_of;
use std::num::Wrapping;
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

pub fn setup_bios_tables(cores: u32, start: usize, low_mem: &mut MachVMBlock) -> usize {
    let rsdp_offset = start;
    let xsdt_offset = rsdp_offset + size_of::<AcpiTableRsdp>();
    let xsdt_entry_offset = xsdt_offset + size_of::<AcpiTableHeader>();
    const NUM_XSDT_ENTRIES: usize = 9;
    let fadt_offset = xsdt_entry_offset + NUM_XSDT_ENTRIES * size_of::<usize>();
    let dsdt_offset = fadt_offset + size_of::<AcpiTableFadt>();
    let madt_offset = dsdt_offset + 36;
    let madt_local_apic_offset = madt_offset + size_of::<AcpiTableMadt>();
    let io_apic_offset = madt_local_apic_offset + cores as usize * size_of::<AcpiMadtLocalApic>();
    let local_x2apic_offset = io_apic_offset + size_of::<AcpiMadtIoApic>();
    let total_size =
        local_x2apic_offset + cores as usize * size_of::<AcpiMadtLocalX2apic>() - start;

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
        + cores as usize * (size_of::<AcpiMadtLocalApic>() + size_of::<AcpiMadtLocalX2apic>());
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
    for i in 0..cores {
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
        address: IO_APIC_BASE as u32,
        global_irq_base: 0,
        ..Default::default()
    };
    //dbg!(&io_apic);
    low_mem.write(io_apic, io_apic_offset, 0);

    // local x2apic
    for i in 0..cores {
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

mod test {
    #[allow(unused_imports)]
    use super::{
        AcpiMadtIoApic, AcpiMadtLocalApic, AcpiMadtLocalX2apic, AcpiTableFadt, AcpiTableHeader,
        AcpiTableMadt, AcpiTableRsdp,
    };
    #[allow(unused_imports)]
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
