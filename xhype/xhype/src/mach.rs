/* SPDX-License-Identifier: GPL-2.0-only */

#![allow(non_camel_case_types)]

use super::err::Error;
use std::mem::size_of;
use std::ops::{Index, IndexMut};
use std::slice;
use std::slice::SliceIndex;

type kern_return_t = u32;
type vm_map_t = u32;
type mach_vm_address_t = usize;
type mach_vm_size_t = usize;
type vm_region_flavor_t = i32;
type vm_region_info_t = *mut i32;
type mach_msg_type_number_t = u32;
type mach_port_t = u32;

// defined in <mach/vm_region.h>
const VM_REGION_BASIC_INFO_64: vm_region_flavor_t = 9;
const VM_REGION_BASIC_INFO_COUNT_64: mach_msg_type_number_t = 9;

// defined in <mach/vm_region.h>
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

// defined in <mach/vm_statistics.h>
const VM_FLAGS_FIXED: i32 = 0x0000;
const VM_FLAGS_ANYWHERE: i32 = 0x0001;

extern "C" {
    static mach_task_self_: mach_port_t; // defined in <mach/mach_init.h>

    // defined in <mach/mach_vm.h>
    fn mach_vm_region(
        target_task: vm_map_t,
        address: *mut mach_vm_address_t,
        size: *mut mach_vm_size_t,
        flavor: vm_region_flavor_t,
        info: vm_region_info_t,
        infoCnt: *mut mach_msg_type_number_t,
        object_name: *mut mach_port_t,
    ) -> kern_return_t;

    // defined in <mach/mach_vm.h>
    fn mach_vm_allocate(
        target: vm_map_t,
        address: *mut mach_vm_address_t,
        size: mach_vm_size_t,
        flags: i32,
    ) -> kern_return_t;

    // defined in <mach/mach_vm.h>
    fn mach_vm_deallocate(
        target: vm_map_t,
        address: mach_vm_address_t,
        size: mach_vm_size_t,
    ) -> kern_return_t;
}

pub fn vm_self_region(start_addr: usize) -> Result<(usize, usize, RegionBasicInfo64), Error> {
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
            &mut info as *mut RegionBasicInfo64 as *mut i32,
            &mut count,
            &mut object,
        )
    } {
        0 => Ok((address, size, info)),
        n => Err((n, "mach_vm_region"))?,
    }
}

pub fn vm_allocate(size: usize) -> Result<usize, Error> {
    let mut address = 0;
    match unsafe { mach_vm_allocate(mach_task_self_, &mut address, size, VM_FLAGS_ANYWHERE) } {
        0 => Ok(address),
        e => Err((e, "mach_vm_allocate"))?,
    }
}

pub fn vm_allocate_fixed(addr: usize, size: usize) -> Result<(), Error> {
    let mut address = addr;
    match unsafe { mach_vm_allocate(mach_task_self_, &mut address, size, VM_FLAGS_FIXED) } {
        0 => Ok(()),
        e => Err((e, "mach_vm_allocate"))?,
    }
}

pub fn vm_deallocate(addr: usize, size: usize) -> Result<(), Error> {
    match unsafe { mach_vm_deallocate(mach_task_self_, addr, size) } {
        0 => Ok(()),
        e => Err((e, "mach_vm_deallocate"))?,
    }
}

#[derive(Debug)]
pub struct MachVMBlock {
    pub start: usize,
    pub size: usize,
}

impl MachVMBlock {
    pub fn new(size: usize) -> Result<Self, Error> {
        let start = vm_allocate(size)?;
        Ok(MachVMBlock { start, size })
    }

    pub fn new_fixed(start: usize, size: usize) -> Result<Self, Error> {
        vm_allocate_fixed(start, size)?;
        Ok(MachVMBlock { start, size })
    }

    pub fn new_aligned(size: usize, align: usize) -> Result<Self, Error> {
        let start = vm_allocate(size + align)?;
        let start_aligned = (start / align + 1) * align;
        vm_deallocate(start, start_aligned - start)?;
        vm_deallocate(start_aligned + size, start % align)?;
        Ok(MachVMBlock {
            start: start_aligned,
            size: size,
        })
    }

    pub fn write<T>(&mut self, val: T, offset: usize, index: usize) {
        debug_assert!((index + 1) * size_of::<T>() + offset <= self.size);
        let ptr = (self.start + offset + index * size_of::<T>()) as *mut T;
        unsafe {
            ptr.write(val);
        }
    }

    pub fn read<T>(&self, offset: usize, index: usize) -> T {
        debug_assert!((index + 1) * size_of::<T>() + offset <= self.size);
        let ptr = (self.start + offset + index * size_of::<T>()) as *const T;
        unsafe { ptr.read() }
    }

    pub fn as_slice<T>(&self) -> &[T] {
        unsafe { slice::from_raw_parts(self.start as *const T, self.size / size_of::<T>()) }
    }

    pub fn as_mut_slice<T>(&mut self) -> &mut [T] {
        unsafe { slice::from_raw_parts_mut(self.start as *mut T, self.size / size_of::<T>()) }
    }
}

impl Drop for MachVMBlock {
    fn drop(&mut self) {
        vm_deallocate(self.start, self.size).unwrap()
    }
}

impl<I: SliceIndex<[u8]>> Index<I> for MachVMBlock {
    type Output = I::Output;
    #[inline]
    fn index(&self, index: I) -> &Self::Output {
        &self.as_slice()[index]
    }
}

impl<I: SliceIndex<[u8]>> IndexMut<I> for MachVMBlock {
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        &mut self.as_mut_slice()[index]
    }
}

#[cfg(test)]
mod tests {
    use super::vm_allocate;
    #[test]
    fn detect_regions_test() {
        let mut addr = 1;
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
                    println!("error = {:?}", n);
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
        vm_allocate_fixed(addr_fixed, one_page).unwrap();
        vm_deallocate(addr_fixed, one_page).unwrap();
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
            let mut mem_block = MachVMBlock::new_fixed(0x500000000, one_page).unwrap();
            start = mem_block.start;
            mem_block.write(two_int, 0, 0);
            let two_int2: TwoInt = mem_block.read(0, 0);
            assert_eq!(two_int2.a, a);
            assert_eq!(two_int2.b, b);
            // mem_block is dropped here and the corresponding memory is freed.
        }
        // allocate memory at `start' again to verify that `mem_block' is
        // correctly dropped.
        let mem_block2 = MachVMBlock::new_fixed(start, one_page).unwrap();
        // verify that the memory region is cleared to 0.
        let two_int3: TwoInt = mem_block2.read(0, 0);
        assert_eq!(two_int3.a, 0);
        assert_eq!(two_int3.b, 0);
    }

    #[test]
    fn vm_block_slice_test() {
        let mut block = MachVMBlock::new(4096).unwrap();
        let p_mut = block.as_mut_slice();
        p_mut[3] = 1u64;
        let p: &[u64] = block.as_slice();
        assert_eq!(p[3], 1u64);
    }

    #[test]
    fn vm_block_index_test() {
        let mut block = MachVMBlock::new(4096).unwrap();
        block[0] = 4;
        assert_eq!(block[0], 4);
        assert_eq!(&block[0..2], [4, 0])
    }
}
