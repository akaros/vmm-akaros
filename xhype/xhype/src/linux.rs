/* SPDX-License-Identifier: GPL-2.0-only */

use std::mem::size_of;

pub const E820_RAM: u32 = 1;
pub const E820_RESERVED: u32 = 2;
pub const E820_ACPI: u32 = 3;
pub const E820_NVS: u32 = 4;
pub const E820_UNUSABLE: u32 = 5;

#[repr(C, packed)]
pub struct E820Entry {
    addr: u64,
    size: u64,
    r#type: u32,
}

pub const E820_MAX: usize = 128;
#[repr(C, packed)]
pub struct BootParams {
    screen_info: [u8; 0x040 - 0x000],     // 0x000
    apm_bios_info: [u8; 0x054 - 0x040],   // 0x040
    _pad2: [u8; 4],                       // 0x054
    tboot_addr: u64,                      // 0x058
    ist_info: [u8; 0x070 - 0x060],        // 0x060
    acpi_rsdp_addr: [u8; 0x078 - 0x070],  // 0x070
    _pad3: [u8; 8],                       // 0x078
    hd0_info: [u8; 0x090 - 0x080],        // 0x080 /* obsolete! */
    hd1_info: [u8; 0x0a0 - 0x090],        // 0x090 /* obsolete! */
    sys_desc_table: [u8; 0x0b0 - 0x0a0],  // 0x0a0 /* obsolete! */
    olpc_ofw_header: [u8; 0x0c0 - 0x0b0], // 0x0b0
    ext_ramdisk_image: u32,               // 0x0c0
    ext_ramdisk_size: u32,                // 0x0c4
    ext_cmd_line_ptr: u32,                // 0x0c8
    _pad4: [u8; 116],                     // 0x0cc
    edid_info: [u8; 0x1c0 - 0x140],       // 0x140
    efi_info: [u8; 0x1e0 - 0x1c0],        // 0x1c0
    alt_mem_k: u32,                       // 0x1e0
    scratch: u32,                         // 0x1e4 /* obsolete! */
    e820_entries: u8,                     // 0x1e8
    eddbuf_entries: u8,                   // 0x1e9
    edd_mbr_sig_buf_entries: u8,          // 0x1ea
    kbd_status: u8,                       // 0x1eb
    secure_boot: u8,                      // 0x1ec
    _pad5: [u8; 2],                       // 0x1ed
    sentinel: u8,                         // 0x1ef
    _pad6: [u8; 1],                       // 0x1f0
    hdr: SetupHeader,                     // 0x1f1
    _pad7: [u8; 0x290 - 0x1f1 - size_of::<SetupHeader>()],
    edd_mbr_sig_buffer: [u32; 16],     // 0x290
    e820_table: [E820Entry; E820_MAX], // 0x2d0
    _pad8: [u8; 48],                   // 0xcd0
    eddbuf: [u8; 0xeec - 0xd00],       // 0xd00
    _pad9: [u8; 276],                  // 0xeec
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
    kernel_info_offset: u32,
}
