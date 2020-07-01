/*
Copyright (c) 2016 Saurav Sachidanand
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:
The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.
THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
*/

/*!
This is a Rust library that taps into functionality that enables
hardware-accelerated execution of virtual machines on OS X.
It binds to the `Hypervisor` framework on OS X, and exposes a safe Rust
interface through the `xhypervisor` module, and an unsafe foreign function
interface through the `xhypervisor::ffi` module.
To use this library, you need
* OS X Yosemite (10.10), or newer
* an Intel processor with the VT-x feature set that includes Extended Page
Tables (EPT) and Unrestricted Mode. To verify this, run and expect the following
in your Terminal:
  ```shell
  $ sysctl kern.hv_support
  kern.hv_support: 1
  ```
!*/

#[allow(non_camel_case_types)]
pub mod ffi;
pub mod vmx;

/// Virtual CPU
pub struct VCPU {
    /// Virtual CPU ID
    pub id: u32,
}

/// x86 architectural register
#[allow(non_camel_case_types)]
#[derive(Clone)]
#[repr(C)]
pub enum x86Reg {
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
