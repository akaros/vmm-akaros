/* SPDX-License-Identifier: GPL-2.0-only */

use crate::consts::msr::*;
use crate::consts::x86::*;
use crate::err::Error;
use crate::hlt;
use crate::hv::vmx::*;
use crate::hv::{gen_exec_ctrl, vmx_read_capability, VMXCap};
use crate::mach::MachVMBlock;
use crate::utils::round_up_4k;
use crate::{GuestThread, VirtualMachine, X86Reg, VCPU};
use std::mem::size_of;
use std::sync::Arc;
use std::thread;

pub struct VThread {
    pub gth: GuestThread,
}

impl VThread {
    fn new(vm: &Arc<VirtualMachine>, stack_size: usize, entry: usize) -> Result<Self, Error> {
        let mut vthread_stack = MachVMBlock::new(stack_size)?;
        let stack_top = vthread_stack.start + vthread_stack.size - size_of::<usize>();
        // use the address of function hlt as the return address
        vthread_stack.write(hlt as usize, stack_top - vthread_stack.start, 0);
        let mut paging = MachVMBlock::new(PAGING_SIZE)?;
        let pml4e: u64 = PG_P | PG_RW | (paging.start as u64 + PAGE_SIZE as u64);
        let paging_entries = paging.as_mut_slice::<u64>();
        paging_entries[0] = pml4e;
        for (i, pdpte) in paging_entries[512..].iter_mut().enumerate() {
            *pdpte = ((i as u64) << 30) | PG_P | PG_RW | PG_PS;
        }
        let init_regs = vec![
            (X86Reg::RIP, entry as u64),
            (X86Reg::RFLAGS, FL_RSVD_1),
            (X86Reg::RSP, stack_top as u64),
            (X86Reg::CR3, paging.start as u64),
        ]
        .into_iter()
        .collect();

        let ctrl_pin = gen_exec_ctrl(vmx_read_capability(VMXCap::Pin)?, 0, 0);
        let ctrl_cpu = gen_exec_ctrl(
            vmx_read_capability(VMXCap::CPU)?,
            CPU_BASED_HLT | CPU_BASED_CR8_LOAD | CPU_BASED_CR8_STORE,
            0,
        );
        let ctrl_cpu2 = gen_exec_ctrl(vmx_read_capability(VMXCap::CPU2)?, CPU_BASED2_RDTSCP, 0);
        let ctrl_entry = gen_exec_ctrl(vmx_read_capability(VMXCap::Entry)?, VMENTRY_GUEST_IA32E, 0);
        let cr0 = X86_CR0_NE | X86_CR0_PE | X86_CR0_PG;
        let cr4 = X86_CR4_VMXE | X86_CR4_PAE;
        let efer = EFER_LMA | EFER_LME;
        let init_vmcs = vec![
            (VMCS_GUEST_CS_AR, 0xa09b),
            (VMCS_GUEST_CS_LIMIT, 0xffffffff),
            (VMCS_GUEST_DS_AR, 0xc093),
            (VMCS_GUEST_DS_LIMIT, 0xffffffff),
            (VMCS_GUEST_ES_AR, 0xc093),
            (VMCS_GUEST_ES_LIMIT, 0xffffffff),
            (VMCS_GUEST_FS_AR, 0x93),
            (VMCS_GUEST_GS_AR, 0x93),
            (VMCS_GUEST_SS_AR, 0xc093),
            (VMCS_GUEST_SS_LIMIT, 0xffffffff),
            (VMCS_GUEST_LDTR_AR, 0x82),
            (VMCS_GUEST_TR_AR, 0x8b),
            (VMCS_CTRL_PIN_BASED, ctrl_pin),
            (VMCS_CTRL_CPU_BASED, ctrl_cpu),
            (VMCS_CTRL_CPU_BASED2, ctrl_cpu2),
            (VMCS_CTRL_VMENTRY_CONTROLS, ctrl_entry),
            (VMCS_CTRL_EXC_BITMAP, 0xffffffff),
            (VMCS_GUEST_CR0, cr0),
            (VMCS_CTRL_CR0_MASK, X86_CR0_PE | X86_CR0_PG),
            (VMCS_CTRL_CR0_SHADOW, cr0),
            (VMCS_GUEST_CR4, cr4),
            (VMCS_CTRL_CR4_MASK, X86_CR4_VMXE),
            (VMCS_CTRL_CR4_SHADOW, 0),
            (VMCS_GUEST_IA32_EFER, efer),
        ]
        .into_iter()
        .collect();
        let mem_maps = vec![(vthread_stack.start, vthread_stack), (paging.start, paging)]
            .into_iter()
            .collect();
        vm.map_guest_mem(mem_maps)?;
        let mut gth = GuestThread::new(vm, 0);
        gth.init_regs = init_regs;
        gth.init_vmcs = init_vmcs;
        Ok(VThread { gth })
    }
}

const VTHREAD_STACK_SIZE: usize = 10 * PAGE_SIZE;
const PAGING_SIZE: usize = 2 * PAGE_SIZE;

pub struct Builder {
    name: Option<String>,
    stack_size: Option<usize>,
    vm: Arc<VirtualMachine>,
}

impl Builder {
    pub fn new(vm: &Arc<VirtualMachine>) -> Self {
        Builder {
            name: None,
            stack_size: None,
            vm: Arc::clone(vm),
        }
    }

    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn stack_size(mut self, size: usize) -> Self {
        self.stack_size = Some(size);
        self
    }

    pub fn spawn(self, f: fn() -> ()) -> Result<JoinHandle<()>, Error> {
        let stack_size = round_up_4k(self.stack_size.unwrap_or(VTHREAD_STACK_SIZE));
        let mut vth = VThread::new(&self.vm, stack_size, f as usize)?;
        let handle = thread::Builder::new()
            .name(self.name.unwrap_or("<unnamed-vthread>".to_string()))
            .spawn(move || {
                let vcpu = VCPU::create()?;
                vth.gth.run_on(&vcpu)
            })?;
        Ok(JoinHandle { handle })
    }
}

pub struct JoinHandle<T> {
    handle: thread::JoinHandle<Result<T, Error>>,
}

impl<T> JoinHandle<T> {
    pub fn join(self) -> Result<T, Error> {
        match self.handle.join() {
            Err(e) => Err(Error::Thread(e)),
            Ok(r) => r,
        }
    }
}

pub fn spawn(vm: &Arc<VirtualMachine>, f: fn() -> ()) -> JoinHandle<()> {
    Builder::new(vm).spawn(f).expect("failed to spawn vthread")
}

#[cfg(test)]
mod tests {
    use crate::{vthread, VMManager};
    use std::sync::{Arc, RwLock};

    static mut NUM_A: i32 = 1;
    static mut NUM_B: i32 = 2;
    fn double_a() {
        unsafe {
            NUM_A <<= 1;
        }
    }
    fn decrement_b() {
        unsafe {
            NUM_B -= 1;
        }
    }

    #[test]
    fn vthread_test() {
        let original_a = unsafe { NUM_A };
        let original_b = unsafe { NUM_B };
        let vmm = VMManager::new().unwrap();
        let vm = Arc::new(vmm.create_vm(1).unwrap());
        let handle1 = vthread::spawn(&vm, double_a);
        let handle2 = vthread::spawn(&vm, decrement_b);
        handle1.join().unwrap();
        handle2.join().unwrap();
        unsafe {
            assert_eq!(NUM_A, 2 * original_a);
            assert_eq!(NUM_B, original_b - 1);
        }
    }
}
