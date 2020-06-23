use super::err::Error;
use super::hlt;
use super::mach::MachVMBlock;
use super::paging::*;
use super::x86::FL_RSVD_1;
use super::{GuestThread, VirtualMachine, X86Reg, VCPU};
use std::collections::HashMap;
use std::mem::size_of;
use std::sync::{Arc, RwLock};
use std::thread;
pub struct VThread {
    pub gth: GuestThread,
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
    pub fn spawn<F>(self, _f: F) -> Result<thread::JoinHandle<()>, Error>
    where
        F: FnOnce() -> (),
        F: Send + 'static,
    {
        let stack_size = (self.stack_size.unwrap_or(VTHREAD_STACK_SIZE) >> 3) << 3;
        let mut vthread_stack = MachVMBlock::new(stack_size)?;
        let stack_top = vthread_stack.start + vthread_stack.size - size_of::<usize>();
        vthread_stack.write(hlt as usize, stack_top - vthread_stack.start, 0);
        let mut paging = MachVMBlock::new(PAGING_SIZE)?;
        let pml4e: u64 = PG_P | PG_RW | (paging.start as u64 + PAGE_SIZE as u64);
        let paging_entries = paging.as_mut_slice::<u64>();
        paging_entries[0] = pml4e;
        for (i, pdpte) in paging_entries[512..].iter_mut().enumerate() {
            *pdpte = ((i as u64) << 30) | PG_P | PG_RW | PG_1GB_PS;
        }
        let mut init_regs = HashMap::new();
        init_regs.insert(X86Reg::RIP, F::call_once as u64);
        init_regs.insert(X86Reg::RFLAGS, FL_RSVD_1);
        init_regs.insert(X86Reg::RSP, stack_top as u64);
        init_regs.insert(X86Reg::CR3, paging.start as u64);
        let mut mem_maps = HashMap::new();
        mem_maps.insert(vthread_stack.start, vthread_stack);
        mem_maps.insert(paging.start, paging);
        let gth = GuestThread {
            vm: self.vm,
            init_regs: init_regs,
            init_vmcs: HashMap::new(),
            mem_maps: mem_maps,
        };
        Ok(thread::Builder::new().spawn(move || {
            let vcpu = VCPU::create().unwrap();
            gth.run_on(&vcpu).unwrap();
        })?)
    }

    #[cfg(not(feature = "vthread_closure"))]
    pub fn spawn(self, f: fn() -> ()) -> Result<thread::JoinHandle<()>, Error> {
        let stack_size = (self.stack_size.unwrap_or(VTHREAD_STACK_SIZE) >> 3) << 3;
        let mut vthread_stack = MachVMBlock::new(stack_size)?;
        let stack_top = vthread_stack.start + vthread_stack.size - size_of::<usize>();
        vthread_stack.write(hlt as usize, stack_top - vthread_stack.start, 0);
        let mut paging = MachVMBlock::new(PAGING_SIZE)?;
        let pml4e: u64 = PG_P | PG_RW | (paging.start as u64 + PAGE_SIZE as u64);
        let paging_entries = paging.as_mut_slice::<u64>();
        paging_entries[0] = pml4e;
        for (i, pdpte) in paging_entries[512..].iter_mut().enumerate() {
            *pdpte = ((i as u64) << 30) | PG_P | PG_RW | PG_1GB_PS;
        }
        let mut init_regs = HashMap::new();
        init_regs.insert(X86Reg::RIP, f as u64);
        init_regs.insert(X86Reg::RFLAGS, FL_RSVD_1);
        init_regs.insert(X86Reg::RSP, stack_top as u64);
        init_regs.insert(X86Reg::CR3, paging.start as u64);
        let mut mem_maps = HashMap::new();
        mem_maps.insert(vthread_stack.start, vthread_stack);
        mem_maps.insert(paging.start, paging);
        let gth = GuestThread {
            vm: self.vm,
            init_regs: init_regs,
            init_vmcs: HashMap::new(),
            mem_maps: mem_maps,
        };
        Ok(thread::Builder::new().spawn(move || {
            let vcpu = VCPU::create().unwrap();
            gth.run_on(&vcpu).unwrap();
        })?)
    }
}

impl VThread {
    pub fn create(
        vm: &Arc<RwLock<VirtualMachine>>,
        entry: unsafe extern "C" fn() -> (),
    ) -> Result<Self, Error> {
        let mut vthread_stack = MachVMBlock::new(VTHREAD_STACK_SIZE)?;
        let stack_top = vthread_stack.start + vthread_stack.size - size_of::<usize>();
        vthread_stack.write(hlt as usize, stack_top - vthread_stack.start, 0);
        let mut paging = MachVMBlock::new(PAGING_SIZE)?;
        let pml4e: u64 = PG_P | PG_RW | (paging.start as u64 + PAGE_SIZE as u64);
        let paging_entries = paging.as_mut_slice::<u64>();
        paging_entries[0] = pml4e;
        for (i, pdpte) in paging_entries[512..].iter_mut().enumerate() {
            *pdpte = ((i as u64) << 30) | PG_P | PG_RW | PG_1GB_PS;
        }
        let mut init_regs = HashMap::new();
        init_regs.insert(X86Reg::RIP, entry as u64);
        init_regs.insert(X86Reg::RFLAGS, FL_RSVD_1);
        init_regs.insert(X86Reg::RSP, stack_top as u64);
        init_regs.insert(X86Reg::CR3, paging.start as u64);
        let mut mem_maps = HashMap::new();
        mem_maps.insert(vthread_stack.start, vthread_stack);
        mem_maps.insert(paging.start, paging);
        let gth = GuestThread {
            vm: Arc::clone(vm),
            init_regs: init_regs,
            init_vmcs: HashMap::new(),
            mem_maps: mem_maps,
        };
        Ok(VThread { gth })
    }
}
