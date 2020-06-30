/* SPDX-License-Identifier: GPL-2.0-only */
mod ffi;

/// x86 architectural register
#[allow(non_camel_case_types)]
#[derive(Clone)]
#[repr(C)]
pub enum X86Reg {
    RIP,
    RFLAGS,
    RAX,
    RCX,
    RDX,
    RBX,
    RSI,
    RDI,
    RSP,
    RBP,
    R8,
    R9,
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
    CS,
    SS,
    DS,
    ES,
    FS,
    GS,
    IDT_BASE,
    IDT_LIMIT,
    GDT_BASE,
    GDT_LIMIT,
    LDTR,
    LDT_BASE,
    LDT_LIMIT,
    LDT_AR,
    TR,
    TSS_BASE,
    TSS_LIMIT,
    TSS_AR,
    CR0,
    CR1,
    CR2,
    CR3,
    CR4,
    DR0,
    DR1,
    DR2,
    DR3,
    DR4,
    DR5,
    DR6,
    DR7,
    TPR,
    XCR0,
    REGISTERS_MAX,
}

/// VMX cabability
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
#[repr(C)]
pub enum VMXCap {
    /// Pin-based VMX capabilities
    PINBASED = 0,
    /// Primary proc-based VMX capabilities
    PROCBASED = 1,
    /// Secondary proc-based VMX capabilities
    PROCBASED2 = 2,
    /// VM-entry VMX capabilities
    ENTRY = 3,
    /// VM-exit VMX capabilities
    EXIT = 4,
    /// VMX preemption timer frequency
    PREEMPTION_TIMER = 32,
}
