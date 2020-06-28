use super::Error;

#[allow(non_camel_case_types)]
#[allow(dead_code)]
mod ffi;
pub mod vmx;
#[allow(unused_imports)]
use crate::consts::msr::*;
#[allow(unused_imports)]
use crate::x86::*;
use ffi::*;
use std::hash::Hash;
use vmx::*;

// constants

// Hypervisor Framework return codes
pub const HV_SUCCESS: hv_return_t = 0;
pub const HV_ERROR: hv_return_t = 0xfae94001;
pub const HV_BUSY: hv_return_t = 0xfae94002;
pub const HV_BAD_ARGUMENT: hv_return_t = 0xfae94003;
pub const HV_NO_RESOURCES: hv_return_t = 0xfae94005;
pub const HV_NO_DEVICE: hv_return_t = 0xfae94006;
pub const HV_UNSUPPORTED: hv_return_t = 0xfae9400f;

pub const HV_VM_DEFAULT: hv_vm_options_t = 0 << 0;

pub const HV_VM_SPACE_DEFAULT: u64 = 0;

// Guest physical memory region permissions for hv_vm_map() and hv_vm_protect()
pub const HV_MEMORY_READ: hv_memory_flags_t = 1 << 0;
pub const HV_MEMORY_WRITE: hv_memory_flags_t = 1 << 1;
pub const HV_MEMORY_EXEC: hv_memory_flags_t = 1 << 2;

// Option for hv_vcpu_create()
pub const HV_VCPU_DEFAULT: u64 = 0;

/// VMX cabability
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
#[repr(C)]
pub enum VMXCap {
    /// Pin-based VMX capabilities
    PIN = 0,
    /// Primary proc-based VMX capabilities
    CPU = 1,
    /// Secondary proc-based VMX capabilities
    CPU2 = 2,
    /// VM-entry VMX capabilities
    ENTRY = 3,
    /// VM-exit VMX capabilities
    EXIT = 4,
    /// VMX preemption timer frequency
    PREEMPTION_TIMER = 32,
}

#[inline]
fn check_ret(hv_ret: u32, msg: &'static str) -> Result<(), Error> {
    match hv_ret {
        0 => Ok(()),
        e => Err((e, msg))?,
    }
}

/// Supported hypervisor capabilities
#[allow(non_camel_case_types)]
#[derive(Clone)]
#[repr(C)]
pub enum HVCap {
    HV_CAP_VCPUMAX,
    HV_CAP_ADDRSPACEMAX,
}

pub fn capability(cap: HVCap) -> Result<u64, Error> {
    let mut value: u64 = 0;
    match unsafe { hv_capability(cap, &mut value) } {
        0 => Ok(value),
        n => Err((n, "hv_capability"))?,
    }
}

pub fn vm_create(flags: u64) -> Result<(), Error> {
    check_ret(unsafe { hv_vm_create(flags) }, "hv_vm_create")
}

pub fn vm_destroy() -> Result<(), Error> {
    check_ret(unsafe { hv_vm_destroy() }, "hv_vm_destroy")
}

#[derive(Debug)]
pub struct MemSpace {
    id: u32,
}

pub static DEFAULT_MEM_SPACE: MemSpace = MemSpace { id: 0 };

impl MemSpace {
    pub fn create() -> Result<Self, Error> {
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
            unsafe { hv_vm_map_space(self.id, uva, gpa, size, flags) },
            "hv_vm_map_space",
        )
    }

    pub fn unmap(&mut self, gpa: usize, size: usize) -> Result<(), Error> {
        check_ret(
            unsafe { hv_vm_unmap_space(self.id, gpa, size) },
            "hv_vm_unmap_space",
        )
    }
}

impl Drop for MemSpace {
    fn drop(&mut self) {
        self.destroy().unwrap();
    }
}

pub fn vmx_read_capability(field: VMXCap) -> Result<u64, Error> {
    let mut value = 0;
    match unsafe { hv_vmx_read_capability(field, &mut value) } {
        0 => Ok(value),
        n => Err((n, "hv_vmx_read_capability"))?,
    }
}

pub fn cap2ctrl(cap: u64, ctrl: u64) -> u64 {
    let low = cap & 0xffffffff;
    let high = cap >> 32;
    let r = ctrl | low;
    debug_assert_eq!(r & high, r);
    r
    // (ctrl | (cap & 0xffffffff)) & (cap >> 32)
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

pub struct VCPU {
    id: u32,
}

impl VCPU {
    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn create() -> Result<Self, Error> {
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
            unsafe { hv_vmx_vcpu_set_apic_address(self.id, address) },
            "hv_vmx_vcpu_set_apic_address",
        )
    }

    pub fn enable_msrs(&self) -> Result<(), Error> {
        self.enable_native_msr(MSR_LSTAR, true)?;
        self.enable_native_msr(MSR_CSTAR, true)?;
        self.enable_native_msr(MSR_STAR, true)?;
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

    pub fn long_mode(&self) -> Result<(), Error> {
        self.write_vmcs(VMCS_GUEST_CS, 0x10)?;
        self.write_vmcs(VMCS_GUEST_CS_AR, 0xa09b)?;
        self.write_vmcs(VMCS_GUEST_CS_LIMIT, 0xffffffff)?;
        self.write_vmcs(VMCS_GUEST_CS_BASE, 0)?;

        self.write_vmcs(VMCS_GUEST_DS, 0x18)?;
        self.write_vmcs(VMCS_GUEST_DS_AR, 0xc093)?;
        self.write_vmcs(VMCS_GUEST_DS_LIMIT, 0xffffffff)?;
        self.write_vmcs(VMCS_GUEST_DS_BASE, 0)?;

        self.write_vmcs(VMCS_GUEST_ES, 0x18)?;
        self.write_vmcs(VMCS_GUEST_ES_AR, 0xc093)?;
        self.write_vmcs(VMCS_GUEST_ES_LIMIT, 0xffffffff)?;
        self.write_vmcs(VMCS_GUEST_ES_BASE, 0)?;

        self.write_vmcs(VMCS_GUEST_FS, 0)?;
        self.write_vmcs(VMCS_GUEST_FS_AR, 0x93)?;
        self.write_vmcs(VMCS_GUEST_FS_LIMIT, 0xffff)?;
        self.write_vmcs(VMCS_GUEST_FS_BASE, 0)?;

        self.write_vmcs(VMCS_GUEST_GS, 0)?;
        self.write_vmcs(VMCS_GUEST_GS_AR, 0x93)?;
        self.write_vmcs(VMCS_GUEST_GS_LIMIT, 0xffff)?;
        self.write_vmcs(VMCS_GUEST_GS_BASE, 0)?;

        self.write_vmcs(VMCS_GUEST_SS, 0x18)?;
        self.write_vmcs(VMCS_GUEST_SS_AR, 0xc093)?;
        self.write_vmcs(VMCS_GUEST_SS_LIMIT, 0xffffffff)?;
        self.write_vmcs(VMCS_GUEST_SS_BASE, 0)?;

        self.write_vmcs(VMCS_GUEST_LDTR, 0)?;
        self.write_vmcs(VMCS_GUEST_LDTR_AR, 0x82)?;
        self.write_vmcs(VMCS_GUEST_LDTR_LIMIT, 0xffff)?;
        self.write_vmcs(VMCS_GUEST_LDTR_BASE, 0)?;

        self.write_vmcs(VMCS_GUEST_GDTR_BASE, 0x17)?;
        self.write_vmcs(VMCS_GUEST_GDTR_LIMIT, 0xfe0)?;

        self.write_vmcs(VMCS_GUEST_TR, 0)?;
        self.write_vmcs(VMCS_GUEST_TR_AR, 0x8b)?;
        self.write_vmcs(VMCS_GUEST_TR_LIMIT, 0)?;
        self.write_vmcs(VMCS_GUEST_TR_BASE, 0)?;

        self.write_vmcs(VMCS_GUEST_IDTR_LIMIT, 0)?;
        self.write_vmcs(VMCS_GUEST_IDTR_BASE, 0)?;

        let cap_pin = vmx_read_capability(VMXCap::PIN)?;
        let cap_cpu = vmx_read_capability(VMXCap::CPU)?;
        let cap_cpu2 = vmx_read_capability(VMXCap::CPU2)?;
        let cap_entry = vmx_read_capability(VMXCap::ENTRY)?;

        self.write_vmcs(VMCS_CTRL_PIN_BASED, cap2ctrl(cap_pin, 0))?;
        self.write_vmcs(
            VMCS_CTRL_CPU_BASED,
            cap2ctrl(
                cap_cpu,
                CPU_BASED_HLT | CPU_BASED_CR8_LOAD | CPU_BASED_CR8_STORE,
            ),
        )?;
        // Hypervisor.framework does not support X2APIC virtualization
        self.write_vmcs(
            VMCS_CTRL_CPU_BASED2,
            cap2ctrl(cap_cpu2, CPU_BASED2_RDTSCP | CPU_BASED2_VIRTUAL_APIC),
        )?;
        self.write_vmcs(
            VMCS_CTRL_VMENTRY_CONTROLS,
            cap2ctrl(cap_entry, VMENTRY_GUEST_IA32E),
        )?;

        self.write_vmcs(VMCS_CTRL_EXC_BITMAP, 0xffffffff & !(1 << 14))?;

        let cr0 = X86_CR0_NE | X86_CR0_ET | X86_CR0_PE | X86_CR0_PG;
        self.write_vmcs(VMCS_GUEST_CR0, cr0)?;
        self.write_vmcs(VMCS_CTRL_CR0_MASK, X86_CR0_PE | X86_CR0_PG)?;
        self.write_vmcs(VMCS_CTRL_CR0_SHADOW, X86_CR0_PE | X86_CR0_PG)?;

        let cr4 = X86_CR4_VMXE | X86_CR4_OSFXSR | X86_CR4_OSXSAVE | X86_CR4_PAE;
        self.write_vmcs(VMCS_GUEST_CR4, cr4)?;
        self.write_vmcs(VMCS_CTRL_CR4_MASK, X86_CR4_VMXE)?;
        self.write_vmcs(VMCS_CTRL_CR4_SHADOW, 0)?;

        let efer = X86_EFER_LMA | X86_EFER_LME;
        self.write_vmcs(VMCS_GUEST_IA32_EFER, efer)
    }
}

impl Drop for VCPU {
    fn drop(&mut self) {
        check_ret(unsafe { hv_vcpu_destroy(self.id) }, "hv_vcpu_destroy").unwrap();
    }
}

mod tests {
    #[allow(unused_imports)]
    use super::vmx::*;
    #[allow(unused_imports)]
    use super::*;
    #[test]
    fn hv_vcpu_test() {
        vm_create(0).unwrap();
        println!("cpu2 cap={:x}", vmx_read_capability(VMXCap::CPU2).unwrap());
        {
            let vcpu = VCPU::create().unwrap();
            vcpu.write_reg(X86Reg::RFLAGS, 0x2u64).unwrap();
            assert_eq!(vcpu.read_reg(X86Reg::RFLAGS).unwrap(), 0x2u64);
            println!(
                "vapic addr = {:x}",
                vcpu.read_vmcs(VMCS_CTRL_APIC_ACCESS).unwrap()
            );
        }
        {
            let vcpu = VCPU::create().unwrap();
            vcpu.write_vmcs(VMCS_CTRL_CR4_MASK, 0x2000).unwrap();
            assert_eq!(vcpu.read_vmcs(VMCS_CTRL_CR4_MASK).unwrap(), 0x2000);
        }

        vm_destroy().unwrap();
    }

    #[test]
    fn hv_capability_test() {
        let vcpu_cap = capability(HVCap::HV_CAP_VCPUMAX).unwrap();
        let space_cap = capability(HVCap::HV_CAP_ADDRSPACEMAX).unwrap();
        println!("vcpu max = {}, space cap = {}.", vcpu_cap, space_cap)
    }

    #[test]
    fn hv_mem_space_drop_test() {
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
}
