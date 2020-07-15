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
This mod is based on [xhypervisor](https://crates.io/crates/xhypervisor) and
wraps the [Hypervisor](https://developer.apple.com/documentation/hypervisor)
framework of macOS into safe Rust interfaces.

This mod requires macOS (10.15) or newer.
!*/

#[allow(non_camel_case_types)]
pub mod ffi;
pub mod vmx;

use crate::consts::msr::*;
use crate::err::Error;
use ffi::*;
use vmx::*;

#[inline]
fn check_ret(hv_ret: u32, msg: &'static str) -> Result<(), Error> {
    match hv_ret {
        0 => Ok(()),
        e => Err((e, msg))?,
    }
}

////////////////////////////////////////////////////////////////////////////////
// vm create/destroy
////////////////////////////////////////////////////////////////////////////////

pub(crate) fn vm_create(flags: u64) -> Result<(), Error> {
    check_ret(unsafe { hv_vm_create(flags) }, "hv_vm_create")
}

pub(crate) fn vm_destroy() -> Result<(), Error> {
    check_ret(unsafe { hv_vm_destroy() }, "hv_vm_destroy")
}

////////////////////////////////////////////////////////////////////////////////
// guest physical memory address space
////////////////////////////////////////////////////////////////////////////////

/// Guest physical memory address space
#[derive(Debug)]
pub struct MemSpace {
    pub(crate) id: u32,
}

/// default memory space that is available after `hv_vm_create()` is called.
pub static DEFAULT_MEM_SPACE: MemSpace = MemSpace { id: 0 };

impl MemSpace {
    pub(crate) fn create() -> Result<Self, Error> {
        let mut id: u32 = 0;
        match unsafe { hv_vm_space_create(&mut id) } {
            0 => Ok(MemSpace { id }),
            n => Err((n, "hv_vm_space_create"))?,
        }
    }

    fn destroy(&self) -> Result<(), Error> {
        check_ret(
            unsafe { hv_vm_space_destroy(self.id) },
            "hv_vm_space_destroy",
        )
    }

    pub fn map(&mut self, uva: usize, gpa: usize, size: usize, flags: u64) -> Result<(), Error> {
        check_ret(
            unsafe {
                hv_vm_map_space(
                    self.id,
                    uva as *const std::ffi::c_void,
                    gpa as u64,
                    size,
                    flags,
                )
            },
            "hv_vm_map_space",
        )
    }

    pub fn unmap(&mut self, gpa: usize, size: usize) -> Result<(), Error> {
        check_ret(
            unsafe { hv_vm_unmap_space(self.id, gpa as u64, size) },
            "hv_vm_unmap_space",
        )
    }
}

impl Drop for MemSpace {
    fn drop(&mut self) {
        self.destroy().unwrap();
    }
}

////////////////////////////////////////////////////////////////////////////////
// virtual cpu
////////////////////////////////////////////////////////////////////////////////

/// Virtual CPU
///
/// A VCPU is bound to the thread which creates it. It should never be shared
/// between threads or accessed by other threads. One thread can only create one
/// VCPU.
pub struct VCPU {
    /// Virtual CPU ID
    pub(crate) id: u32,
}

/// x86 architectural register
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
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

impl VCPU {
    pub fn id(&self) -> u32 {
        self.id
    }

    pub(crate) fn create() -> Result<Self, Error> {
        let mut vcpu_id: u32 = 0;
        let flags = HV_VCPU_DEFAULT;
        match unsafe { hv_vcpu_create(&mut vcpu_id, flags) } {
            0 => Ok(VCPU { id: vcpu_id }),
            e => Err((e, "hv_vcpu_create"))?,
        }
    }

    pub fn dump(&self) -> Result<(), Error> {
        println!(
            "VMCS_CTRL_PIN_BASED: {:x}",
            self.read_vmcs(VMCS_CTRL_PIN_BASED)?
        );
        println!(
            "VMCS_CTRL_CPU_BASED: {:x}",
            self.read_vmcs(VMCS_CTRL_CPU_BASED)?
        );
        println!(
            "VMCS_CTRL_CPU_BASED2: {:x}",
            self.read_vmcs(VMCS_CTRL_CPU_BASED2)?
        );
        println!(
            "VMCS_CTRL_VMENTRY_CONTROLS: {:x}",
            self.read_vmcs(VMCS_CTRL_VMENTRY_CONTROLS)?
        );
        println!(
            "VMCS_CTRL_EXC_BITMAP: {:x}",
            self.read_vmcs(VMCS_CTRL_EXC_BITMAP)?
        );
        println!(
            "VMCS_CTRL_CR0_MASK: {:x}",
            self.read_vmcs(VMCS_CTRL_CR0_MASK)?
        );
        println!(
            "VMCS_CTRL_CR0_SHADOW: {:x}",
            self.read_vmcs(VMCS_CTRL_CR0_SHADOW)?
        );
        println!(
            "VMCS_CTRL_CR4_MASK: {:x}",
            self.read_vmcs(VMCS_CTRL_CR4_MASK)?
        );
        println!(
            "VMCS_CTRL_CR4_SHADOW: {:x}",
            self.read_vmcs(VMCS_CTRL_CR4_SHADOW)?
        );
        println!("VMCS_GUEST_CS: {:x}", self.read_vmcs(VMCS_GUEST_CS)?);
        println!(
            "VMCS_GUEST_CS_LIMIT: {:x}",
            self.read_vmcs(VMCS_GUEST_CS_LIMIT)?
        );
        println!("VMCS_GUEST_CS_AR: {:x}", self.read_vmcs(VMCS_GUEST_CS_AR)?);
        println!(
            "VMCS_GUEST_CS_BASE: {:x}",
            self.read_vmcs(VMCS_GUEST_CS_BASE)?
        );
        println!("VMCS_GUEST_DS: {:x}", self.read_vmcs(VMCS_GUEST_DS)?);
        println!(
            "VMCS_GUEST_DS_LIMIT: {:x}",
            self.read_vmcs(VMCS_GUEST_DS_LIMIT)?
        );
        println!("VMCS_GUEST_DS_AR: {:x}", self.read_vmcs(VMCS_GUEST_DS_AR)?);
        println!(
            "VMCS_GUEST_DS_BASE: {:x}",
            self.read_vmcs(VMCS_GUEST_DS_BASE)?
        );
        println!("VMCS_GUEST_ES: {:x}", self.read_vmcs(VMCS_GUEST_ES)?);
        println!(
            "VMCS_GUEST_ES_LIMIT: {:x}",
            self.read_vmcs(VMCS_GUEST_ES_LIMIT)?
        );
        println!("VMCS_GUEST_ES_AR: {:x}", self.read_vmcs(VMCS_GUEST_ES_AR)?);
        println!(
            "VMCS_GUEST_ES_BASE: {:x}",
            self.read_vmcs(VMCS_GUEST_ES_BASE)?
        );
        println!("VMCS_GUEST_FS: {:x}", self.read_vmcs(VMCS_GUEST_FS)?);
        println!(
            "VMCS_GUEST_FS_LIMIT: {:x}",
            self.read_vmcs(VMCS_GUEST_FS_LIMIT)?
        );
        println!("VMCS_GUEST_FS_AR: {:x}", self.read_vmcs(VMCS_GUEST_FS_AR)?);
        println!(
            "VMCS_GUEST_FS_BASE: {:x}",
            self.read_vmcs(VMCS_GUEST_FS_BASE)?
        );
        println!("VMCS_GUEST_GS: {:x}", self.read_vmcs(VMCS_GUEST_GS)?);
        println!(
            "VMCS_GUEST_GS_LIMIT: {:x}",
            self.read_vmcs(VMCS_GUEST_GS_LIMIT)?
        );
        println!("VMCS_GUEST_GS_AR: {:x}", self.read_vmcs(VMCS_GUEST_GS_AR)?);
        println!(
            "VMCS_GUEST_GS_BASE: {:x}",
            self.read_vmcs(VMCS_GUEST_GS_BASE)?
        );
        println!("VMCS_GUEST_SS: {:x}", self.read_vmcs(VMCS_GUEST_SS)?);
        println!(
            "VMCS_GUEST_SS_LIMIT: {:x}",
            self.read_vmcs(VMCS_GUEST_SS_LIMIT)?
        );
        println!("VMCS_GUEST_SS_AR: {:x}", self.read_vmcs(VMCS_GUEST_SS_AR)?);
        println!(
            "VMCS_GUEST_SS_BASE: {:x}",
            self.read_vmcs(VMCS_GUEST_SS_BASE)?
        );
        println!("VMCS_GUEST_LDTR: {:x}", self.read_vmcs(VMCS_GUEST_LDTR)?);
        println!(
            "VMCS_GUEST_LDTR_LIMIT: {:x}",
            self.read_vmcs(VMCS_GUEST_LDTR_LIMIT)?
        );
        println!(
            "VMCS_GUEST_LDTR_AR: {:x}",
            self.read_vmcs(VMCS_GUEST_LDTR_AR)?
        );
        println!(
            "VMCS_GUEST_LDTR_BASE: {:x}",
            self.read_vmcs(VMCS_GUEST_LDTR_BASE)?
        );
        println!(
            "VMCS_GUEST_GDTR_BASE: {:x}",
            self.read_vmcs(VMCS_GUEST_GDTR_BASE)?
        );
        println!(
            "VMCS_GUEST_GDTR_LIMIT: {:x}",
            self.read_vmcs(VMCS_GUEST_GDTR_LIMIT)?
        );
        println!(
            "VMCS_GUEST_IDTR_LIMIT: {:x}",
            self.read_vmcs(VMCS_GUEST_IDTR_LIMIT)?
        );
        println!(
            "VMCS_GUEST_IDTR_BASE: {:x}",
            self.read_vmcs(VMCS_GUEST_IDTR_BASE)?
        );
        println!(
            "VMCS_GUEST_IA32_EFER: {:x}",
            self.read_vmcs(VMCS_GUEST_IA32_EFER)?
        );
        println!("RIP: {:x}", self.read_reg(X86Reg::RIP)?);
        println!("RFLAGS: {:x}", self.read_reg(X86Reg::RFLAGS)?);
        println!("RAX: {:x}", self.read_reg(X86Reg::RAX)?);
        println!("RCX: {:x}", self.read_reg(X86Reg::RCX)?);
        println!("RDX: {:x}", self.read_reg(X86Reg::RDX)?);
        println!("RBX: {:x}", self.read_reg(X86Reg::RBX)?);
        println!("RSI: {:x}", self.read_reg(X86Reg::RSI)?);
        println!("RDI: {:x}", self.read_reg(X86Reg::RDI)?);
        println!("RSP: {:x}", self.read_reg(X86Reg::RSP)?);
        println!("RBP: {:x}", self.read_reg(X86Reg::RBP)?);
        println!("R8: {:x}", self.read_reg(X86Reg::R8)?);
        println!("R9: {:x}", self.read_reg(X86Reg::R9)?);
        println!("R10: {:x}", self.read_reg(X86Reg::R10)?);
        println!("R11: {:x}", self.read_reg(X86Reg::R11)?);
        println!("R12: {:x}", self.read_reg(X86Reg::R12)?);
        println!("R13: {:x}", self.read_reg(X86Reg::R13)?);
        println!("R14: {:x}", self.read_reg(X86Reg::R14)?);
        println!("R15: {:x}", self.read_reg(X86Reg::R15)?);
        println!("CR0: {:x}", self.read_reg(X86Reg::CR0)?);
        println!("CR2: {:x}", self.read_reg(X86Reg::CR2)?);
        println!("CR3: {:x}", self.read_reg(X86Reg::CR3)?);
        println!("CR4: {:x}", self.read_reg(X86Reg::CR4)?);
        Ok(())
    }

    pub fn set_space(&self, space: &MemSpace) -> Result<(), Error> {
        check_ret(
            unsafe { hv_vcpu_set_space(self.id, space.id) },
            "hv_vcpu_set_space",
        )
    }

    pub fn read_reg(&self, reg: X86Reg) -> Result<u64, Error> {
        let mut value = 0;
        match unsafe { hv_vcpu_read_register(self.id, reg, &mut value) } {
            0 => Ok(value),
            n => Err((n, "hv_vcpu_read_register"))?,
        }
    }

    pub fn write_reg(&self, reg: X86Reg, value: u64) -> Result<(), Error> {
        check_ret(
            unsafe { hv_vcpu_write_register(self.id, reg, value) },
            "hv_vcpu_write_register",
        )
    }

    pub fn read_msr(&self, msr: u32) -> Result<u64, Error> {
        let mut value = 0;
        match unsafe { hv_vcpu_read_msr(self.id, msr, &mut value) } {
            0 => Ok(value),
            n => Err((n, "hv_vcpu_read_msr"))?,
        }
    }

    pub fn write_msr(&self, msr: u32, value: u64) -> Result<(), Error> {
        check_ret(
            unsafe { hv_vcpu_write_msr(self.id, msr, value) },
            "hv_vcpu_write_msr",
        )
    }

    pub fn enable_native_msr(&self, msr: u32, enable: bool) -> Result<(), Error> {
        check_ret(
            unsafe { hv_vcpu_enable_native_msr(self.id, msr, enable) },
            "hv_vcpu_enable_native_msr",
        )
    }

    pub fn run(&self) -> Result<(), Error> {
        check_ret(unsafe { hv_vcpu_run(self.id) }, "hv_vcpu_run")
    }

    pub fn read_vmcs(&self, field: u32) -> Result<u64, Error> {
        let mut value = 0;
        match unsafe { hv_vmx_vcpu_read_vmcs(self.id, field, &mut value) } {
            0 => Ok(value),
            n => Err((n, "hv_vmx_vcpu_read_vmcs"))?,
        }
    }

    pub fn write_vmcs(&self, field: u32, value: u64) -> Result<(), Error> {
        check_ret(
            unsafe { hv_vmx_vcpu_write_vmcs(self.id, field, value) },
            "hv_vmx_vcpu_write_vmcs",
        )
    }

    pub fn set_vapic_address(&self, address: usize) -> Result<(), Error> {
        check_ret(
            unsafe { hv_vmx_vcpu_set_apic_address(self.id, address as u64) },
            "hv_vmx_vcpu_set_apic_address",
        )
    }

    pub fn enable_msrs(&self) -> Result<(), Error> {
        self.enable_native_msr(MSR_STAR, true)?;
        self.enable_native_msr(MSR_LSTAR, true)?;
        self.enable_native_msr(MSR_CSTAR, true)?;
        self.enable_native_msr(MSR_SYSCALL_MASK, true)?;
        self.enable_native_msr(MSR_KERNEL_GS_BASE, true)?;
        self.enable_native_msr(MSR_GS_BASE, true)?;
        self.enable_native_msr(MSR_FS_BASE, true)?;
        self.enable_native_msr(MSR_IA32_SYSENTER_CS, true)?;
        self.enable_native_msr(MSR_IA32_SYSENTER_ESP, true)?;
        self.enable_native_msr(MSR_IA32_SYSENTER_EIP, true)?;
        self.enable_native_msr(MSR_IA32_TSC, true)?;
        self.enable_native_msr(MSR_TSC_AUX, true)
    }
}

impl Drop for VCPU {
    fn drop(&mut self) {
        // Before destroying a VCPU, always set its memory space to the default
        // one, since it is observed that if a VCPU is set to a memory space A
        // and then destroyed, destroying memory space A will result in an error.
        // see details in fn hv_mem_space_destroy_test().
        self.set_space(&DEFAULT_MEM_SPACE).unwrap();
        check_ret(unsafe { hv_vcpu_destroy(self.id) }, "hv_vcpu_destroy").unwrap();
    }
}

////////////////////////////////////////////////////////////////////////////////
// VMX cabability
////////////////////////////////////////////////////////////////////////////////

/// VMX cabability
#[derive(Clone, Debug)]
#[repr(C)]
pub enum VMXCap {
    /// Pin-based VMX capabilities
    Pin = 0,
    /// Primary proc-based VMX capabilities
    CPU = 1,
    /// Secondary proc-based VMX capabilities
    CPU2 = 2,
    /// VM-entry VMX capabilities
    Entry = 3,
    /// VM-exit VMX capabilities
    Exit = 4,
    /// VMX preemption timer frequency
    PreemptionTimer = 32,
}

pub fn vmx_read_capability(field: VMXCap) -> Result<u64, Error> {
    let mut value = 0;
    match unsafe { hv_vmx_read_capability(field, &mut value) } {
        0 => Ok(value),
        n => Err((n, "hv_vmx_read_capability"))?,
    }
}

/// generate execution control of VMCS based on the capability and provided
/// settings.
pub fn gen_exec_ctrl(cap: u64, one_setting: u64, zero_setting: u64) -> u64 {
    debug_assert_eq!(one_setting & zero_setting, 0);
    let low = cap & 0xffffffff;
    let high = cap >> 32;
    debug_assert_eq!(high | one_setting, high);
    debug_assert_eq!(low & !zero_setting, low);
    one_setting | low
}

mod test {
    #[allow(unused_imports)]
    use super::vmx::*;
    #[allow(unused_imports)]
    use super::*;
    #[test]
    fn hv_vcpu_test() {
        vm_create(0).unwrap();
        let vcpu1_id;
        {
            let vcpu = VCPU::create().unwrap();
            vcpu1_id = vcpu.id();
            vcpu.write_reg(X86Reg::RFLAGS, 0x2u64).unwrap();
            assert_eq!(vcpu.read_reg(X86Reg::RFLAGS).unwrap(), 0x2u64);
            println!(
                "vapic addr = {:x}",
                vcpu.read_vmcs(VMCS_CTRL_APIC_ACCESS).unwrap()
            );
        }
        let vcpu2_id;
        {
            let vcpu = VCPU::create().unwrap();
            vcpu2_id = vcpu.id();
            vcpu.write_vmcs(VMCS_CTRL_CR4_MASK, 0x2000).unwrap();
            assert_eq!(vcpu.read_vmcs(VMCS_CTRL_CR4_MASK).unwrap(), 0x2000);
        }
        assert_eq!(vcpu1_id, vcpu2_id);
        vm_destroy().unwrap();
    }

    #[test]
    fn hv_mem_space_test() {
        vm_create(0).unwrap();
        let space1_id;
        {
            let space1 = MemSpace::create().unwrap();
            space1_id = space1.id;
        }
        let space2_id;
        {
            let space2 = MemSpace::create().unwrap();
            space2_id = space2.id;
        }
        assert_eq!(space1_id, space2_id);
        vm_destroy().unwrap();
    }

    /*
        This test is meant to show that, if we set one vcpu to a non-default
        memory space and then destroy that vcpu, then we cannot destroy this
        memory space any more, because the memory space will still think itself
        occupied by some vcpu and hv_vm_space_destroy() will return HV_BUSY.
        This is why in VCPU::drop() we have to set the memory space of any vcpu
        to the default memory space and then destroy the vcpu.
        tested on macOS 10.15.5
    */
    #[test]
    fn hv_mem_space_destroy_test() {
        let mut ret;
        let mut space_id = 0;
        let mut vcpu = 0;
        unsafe {
            ret = hv_vm_create(HV_VM_DEFAULT);
            assert_eq!(ret, HV_SUCCESS);
            ret = hv_vm_space_create(&mut space_id);
            assert_eq!(ret, HV_SUCCESS);
            ret = hv_vcpu_create(&mut vcpu, HV_VCPU_DEFAULT);
            assert_eq!(ret, HV_SUCCESS);
            ret = hv_vcpu_set_space(vcpu, space_id);
            assert_eq!(ret, HV_SUCCESS);
            ret = hv_vcpu_destroy(vcpu);
            assert_eq!(ret, HV_SUCCESS);
            ret = hv_vm_space_destroy(space_id);
            // vcpu is destroyed, but the memory space still think itself occupied by the vcpu
            assert_eq!(ret, HV_BUSY);
        }
    }
}
