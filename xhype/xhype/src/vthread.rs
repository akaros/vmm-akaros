/* SPDX-License-Identifier: GPL-2.0-only */

use super::err::Error;
use super::hlt;
use super::mach::MachVMBlock;
use super::x86::FL_RSVD_1;
use super::{GuestThread, VirtualMachine, X86Reg, VCPU};
use crate::utils::round_up;
use crate::x86::*;
use std::mem::size_of;
use std::sync::{Arc, RwLock};
use std::thread;

pub struct VThread {
    pub gth: GuestThread,
}

impl VThread {
    fn new(
        vm: &Arc<RwLock<VirtualMachine>>,
        stack_size: usize,
        entry: usize,
    ) -> Result<Self, Error> {
        let mut vthread_stack = MachVMBlock::new(stack_size)?;
        let stack_top = vthread_stack.start + vthread_stack.size - size_of::<usize>();
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
        let mem_maps = vec![(vthread_stack.start, vthread_stack), (paging.start, paging)]
            .into_iter()
            .collect();
        {
            let mut vm = vm.write().unwrap();
            vm.map_guest_mem(mem_maps)?;
        }
        let mut gth = GuestThread::new(vm, 0);
        gth.init_regs = init_regs;
        Ok(VThread { gth })
    }
}

const VTHREAD_STACK_SIZE: usize = 10 * PAGE_SIZE;
const PAGING_SIZE: usize = 2 * PAGE_SIZE;

pub struct Builder {
    name: Option<String>,
    stack_size: Option<usize>,
    vm: Arc<RwLock<VirtualMachine>>,
}

impl Builder {
    pub fn new(vm: &Arc<RwLock<VirtualMachine>>) -> Self {
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

    #[cfg(feature = "vthread_closure")]
    pub fn spawn<F>(self, _f: F) -> Result<JoinHandle<()>, Error>
    where
        F: FnOnce() -> (),
        F: Send + 'static,
    {
        let stack_size = round_up(self.stack_size.unwrap_or(VTHREAD_STACK_SIZE));
        let vth = VThread::new(&self.vm, stack_size, F::call_once as usize)?;
        let handle = thread::Builder::new()
            .name(self.name.unwrap_or("<unnamed-vthread>".to_string()))
            .spawn(move || {
                let vcpu = VCPU::create()?;
                vth.gth.run_on(&vcpu)
            })?;
        Ok(JoinHandle { handle })
    }

    #[cfg(not(feature = "vthread_closure"))]
    pub fn spawn(self, f: fn() -> ()) -> Result<JoinHandle<()>, Error> {
        let stack_size = round_up(self.stack_size.unwrap_or(VTHREAD_STACK_SIZE));
        let vth = VThread::new(&self.vm, stack_size, f as usize)?;
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

#[cfg(test)]
mod tests {
    use super::VThread;
    use crate::{VMManager, VCPU};

    static mut NUM_A: i32 = 1;
    extern "C" fn add_a() {
        unsafe {
            NUM_A += 3;
        }
    }

    use std::sync::{Arc, RwLock};
    #[test]
    fn vthread_test() {
        let vmm = VMManager::new().unwrap();
        let vm = Arc::new(RwLock::new(vmm.create_vm(1).unwrap()));
        let vth = VThread::new(&vm, 4096, add_a as usize).unwrap();
        let vcpu = VCPU::create().unwrap();
        vth.gth.run_on(&vcpu).unwrap();
        unsafe {
            assert_eq!(NUM_A, 4);
        }
    }
}
