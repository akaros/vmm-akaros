use std::mem::size_of;

type kern_return_t = u32;
type vm_map_t = u32;
type mach_vm_address_t = usize;
type mach_vm_size_t = usize;
type vm_region_flavor_t = i32;
type vm_region_info_t = *mut RegionBasicInfo64;
type mach_msg_type_number_t = u32;
type mach_port_t = u32;

const VM_REGION_BASIC_INFO_64: vm_region_flavor_t = 9;
const VM_REGION_BASIC_INFO_COUNT_64: mach_msg_type_number_t = 9;

const VM_FLAGS_FIXED: i32 = 0x0000;
const VM_FLAGS_ANYWHERE: i32 = 0x0001;

#[repr(C)]
#[derive(Debug, Default)]
pub struct RegionBasicInfo64 {
    pub protection: i32,
    pub max_protection: i32,
    pub inheritance: u32,
    pub shared: u32,
    pub reserved: u32,
    pub offset: u64,
    pub behavior: i32,
    pub user_wired_count: u16,
}

extern "C" {
    static mach_task_self_: mach_port_t;
    fn mach_vm_region(
        target_task: vm_map_t,
        address: *mut mach_vm_address_t,
        size: *mut mach_vm_size_t,
        flavor: vm_region_flavor_t,
        info: vm_region_info_t,
        infoCnt: *mut mach_msg_type_number_t,
        object_name: *mut mach_port_t,
    ) -> kern_return_t;

    fn mach_vm_allocate(
        target: vm_map_t,
        address: *mut mach_vm_address_t,
        size: mach_vm_size_t,
        flags: i32,
    ) -> kern_return_t;

    fn mach_vm_deallocate(
        target: vm_map_t,
        address: mach_vm_address_t,
        size: mach_vm_size_t,
    ) -> kern_return_t;
}

pub fn vm_self_region(start_addr: usize) -> Result<(usize, usize, RegionBasicInfo64), u32> {
    let mut address = start_addr;
    let mut size = 0;
    let mut count = VM_REGION_BASIC_INFO_COUNT_64;
    let mut object = 0;
    let mut info: RegionBasicInfo64 = RegionBasicInfo64::default();
    let flavor = VM_REGION_BASIC_INFO_64;
    match unsafe {
        mach_vm_region(
            mach_task_self_,
            &mut address,
            &mut size,
            flavor,
            &mut info,
            &mut count,
            &mut object,
        )
    } {
        0 => Ok((address, size, info)),
        n => Err(n),
    }
}

pub fn vm_allocate(size: usize) -> Result<usize, u32> {
    let mut address = 0;
    match unsafe { mach_vm_allocate(mach_task_self_, &mut address, size, VM_FLAGS_ANYWHERE) } {
        0 => Ok(address),
        e => Err(e),
    }
}

pub fn vm_allocate_fixed(addr: usize, size: usize) -> Result<(), u32> {
    let mut address = addr;
    match unsafe { mach_vm_allocate(mach_task_self_, &mut address, size, VM_FLAGS_FIXED) } {
        0 => Ok(()),
        e => Err(e),
    }
}

pub fn vm_deallocate(addr: usize, size: usize) -> Result<(), u32> {
    match unsafe { mach_vm_deallocate(mach_task_self_, addr, size) } {
        0 => Ok(()),
        e => Err(e),
    }
}

pub struct MachVMBlock {
    pub start: usize,
    pub size: usize,
}

impl MachVMBlock {
    pub fn new(size: usize) -> Result<Self, u32> {
        let start = vm_allocate(size)?;
        Ok(MachVMBlock { start, size })
    }

    pub fn new_fixed(start: usize, size: usize) -> Result<Self, u32> {
        vm_allocate_fixed(start, size)?;
        Ok(MachVMBlock { start, size })
    }

    pub fn write<T>(&self, val: T, offset: usize) -> Result<(), &str> {
        if size_of::<T>() + offset > self.size {
            return Err("overflow");
        }
        let start_ptr = self.start as *mut T;
        unsafe {
            let ptr = start_ptr.offset(offset as isize);
            ptr.write(val);
        }
        Ok(())
    }
}

impl Drop for MachVMBlock {
    fn drop(&mut self) {
        vm_deallocate(self.start, self.size).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::vm_allocate;
    #[test]
    fn detect_regions() {
        let mut addr = 1;
        println!("&addr = {:x}", &addr as *const usize as usize);
        loop {
            match vm_self_region(addr) {
                Ok((start, size, info)) => {
                    println!(
                        "start = {:x}, size = {}, prot = {}, max_prot = {}",
                        start, size, info.protection, info.max_protection
                    );
                    addr = start + size;
                }
                Err(n) => {
                    println!("error = {}", n);
                    break;
                }
            }
        }
    }

    use super::{vm_allocate_fixed, vm_deallocate, vm_self_region};
    #[test]
    fn mach_mem_alloc_test() {
        let one_page = 4096;
        let addr_anywhere = vm_allocate(4096).unwrap();
        println!("addr_anywhere = {:x}", addr_anywhere);
        vm_deallocate(addr_anywhere, one_page).unwrap();
        let addr_fixed: usize = 0x400000000;
        match vm_allocate_fixed(addr_fixed, one_page) {
            Ok(()) => {
                println!("addr_fixed = {:x}", addr_fixed);
                vm_deallocate(addr_fixed, one_page).unwrap();
            }
            Err(e) => println!("vm_allocate_fixed error = {:x}", e),
        }
    }

    use super::MachVMBlock;
    struct TwoInt {
        a: i32,
        b: i32,
    }
    #[test]
    fn mach_vm_block_test() {
        let one_page = 4096;
        let start;
        let a = 1;
        let b = 2;
        {
            let two_int = TwoInt { a, b };
            let mem_block = MachVMBlock::new(one_page).unwrap();
            start = mem_block.start;
            println!("mem_block start = {:x}", start);
            mem_block.write(two_int, 0).unwrap();
            let ptr = mem_block.start as *const TwoInt;
            unsafe {
                assert_eq!((*ptr).a, a);
                assert_eq!((*ptr).b, b);
            }
            println!("will drop mem_block");
        }
        println!("mem_block dropped");
        println!("try allocating memory at {:x} again", start);
        match MachVMBlock::new_fixed(start, one_page) {
            Ok(mem_block) => {
                println!("success");
                let ptr = mem_block.start as *const TwoInt;
                unsafe {
                    assert_eq!((*ptr).a, 0);
                    assert_eq!((*ptr).b, 0);
                }
            }
            Err(e) => {
                println!("fail, error = {:x}", e);
                panic!();
            }
        }
    }
}
