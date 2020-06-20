use std::result::Result;

#[allow(non_camel_case_types)]
#[allow(dead_code)]
mod ffi;
pub mod vmx;
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

fn check_ret(hv_ret: u32) -> Result<(), u32> {
    match hv_ret {
        0 => Ok(()),
        n => Err(n),
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

pub fn capability(cap: HVCap) -> Result<u64, u32> {
    let mut value: u64 = 0;
    match unsafe { hv_capability(cap, &mut value) } {
        0 => Ok(value),
        n => Err(n),
    }
}

pub fn vm_create(flags: u64) -> Result<(), u32> {
    check_ret(unsafe { hv_vm_create(flags) })
}

pub fn vm_destroy() -> Result<(), u32> {
    check_ret(unsafe { hv_vm_destroy() })
}

#[derive(Debug)]
pub struct MemSpace {
    id: u32,
}

pub const DEFAULT_MEM_SPACE: MemSpace = MemSpace { id: 0 };

impl MemSpace {
    pub fn create() -> Result<Self, u32> {
        let mut id: u32 = 0;
        match unsafe { hv_vm_space_create(&mut id) } {
            0 => Ok(MemSpace { id }),
            n => Err(n),
        }
    }

    pub fn destroy(&self) -> Result<(), u32> {
        check_ret(unsafe { hv_vm_space_destroy(self.id) })
    }

    pub fn map(&self, uva: usize, gpa: usize, size: usize, flags: u64) -> Result<(), u32> {
        check_ret(unsafe { hv_vm_map_space(self.id, uva, gpa, size, flags) })
    }

    pub fn unmap(&self, gpa: usize, size: usize) -> Result<(), u32> {
        check_ret(unsafe { hv_vm_unmap_space(self.id, gpa, size) })
    }
}

pub fn vmx_read_capability(field: VMXCap) -> Result<u64, u32> {
    let mut value = 0;
    match unsafe { hv_vmx_read_capability(field, &mut value) } {
        0 => Ok(value),
        n => Err(n),
    }
}

pub fn cap2ctrl(cap: u64, ctrl: u64) -> u64 {
    (ctrl | (cap & 0xffffffff)) & (cap >> 32)
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
    pub fn create() -> Result<Self, u32> {
        let mut vcpu_id: u32 = 0;
        let flags = HV_VCPU_DEFAULT;
        match unsafe { hv_vcpu_create(&mut vcpu_id, flags) } {
            0 => Ok(VCPU { id: vcpu_id }),
            e => Err(e),
        }
    }

    pub fn dump(&self) -> Result<(), u32> {
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

    pub fn set_space(&self, space: &MemSpace) -> Result<(), u32> {
        check_ret(unsafe { hv_vcpu_set_space(self.id, space.id) })
    }

    pub fn read_reg(&self, reg: X86Reg) -> Result<u64, u32> {
        let mut value = 0;
        match unsafe { hv_vcpu_read_register(self.id, reg, &mut value) } {
            0 => Ok(value),
            n => Err(n),
        }
    }

    pub fn write_reg(&self, reg: X86Reg, value: u64) -> Result<(), u32> {
        check_ret(unsafe { hv_vcpu_write_register(self.id, reg, value) })
    }

    pub fn read_msr(&self, msr: u32) -> Result<u64, u32> {
        let mut value = 0;
        match unsafe { hv_vcpu_read_msr(self.id, msr, &mut value) } {
            0 => Ok(value),
            n => Err(n),
        }
    }

    pub fn write_msr(&self, msr: u32, value: u64) -> Result<(), u32> {
        check_ret(unsafe { hv_vcpu_write_msr(self.id, msr, value) })
    }

    pub fn enable_native_msr(&self, msr: u32, enable: bool) -> Result<(), u32> {
        check_ret(unsafe { hv_vcpu_enable_native_msr(self.id, msr, enable) })
    }

    pub fn run(&self) -> Result<(), u32> {
        check_ret(unsafe { hv_vcpu_run(self.id) })
    }

    pub fn read_vmcs(&self, field: u32) -> Result<u64, u32> {
        let mut value = 0;
        match unsafe { hv_vmx_vcpu_read_vmcs(self.id, field, &mut value) } {
            0 => Ok(value),
            n => Err(n),
        }
    }
    pub fn write_vmcs(&self, field: u32, value: u64) -> Result<(), u32> {
        check_ret(unsafe { hv_vmx_vcpu_write_vmcs(self.id, field, value) })
    }
}

impl Drop for VCPU {
    fn drop(&mut self) {
        self.set_space(&DEFAULT_MEM_SPACE).unwrap();
        check_ret(unsafe { hv_vcpu_destroy(self.id) }).unwrap();
    }
}

mod tests {
    #[allow(unused_imports)]
    use super::*;
    #[test]
    fn hv_vcpu_test() {
        vm_create(0).unwrap();
        println!("pin cap={:x}", vmx_read_capability(VMXCap::ENTRY).unwrap());
        {
            let vcpu = VCPU::create().unwrap();
            vcpu.write_reg(X86Reg::RFLAGS, 0x2).unwrap();
            assert_eq!(vcpu.read_reg(X86Reg::RFLAGS), Ok(0x2));
        }
        {
            let vcpu = VCPU::create().unwrap();
            vcpu.write_vmcs(VMCS_CTRL_CR4_MASK, 0x2000).unwrap();
            assert_eq!(vcpu.read_vmcs(VMCS_CTRL_CR4_MASK), Ok(0x2000));
        }

        vm_destroy().unwrap();
    }

    #[test]
    fn hv_capability_test() {
        let vcpu_cap = capability(HVCap::HV_CAP_VCPUMAX).unwrap();
        let space_cap = capability(HVCap::HV_CAP_ADDRSPACEMAX).unwrap();
        println!("vcpu max = {}, space cap = {}.", vcpu_cap, space_cap)
    }
}
