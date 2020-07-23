/* SPDX-License-Identifier: GPL-2.0-only */
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::null_mut;

#[inline]
pub fn round_up_4k(num: usize) -> usize {
    num.checked_add(0xfff).expect("overflow in round_up_4k()") & !0xfff
}

#[inline]
pub fn round_down_4k(num: usize) -> usize {
    num & !0xfff
}

pub fn get_tsc_frequency() -> u64 {
    let mut size = size_of::<u64>();
    let mut tsc_freq = 0;
    unsafe {
        sysctlbyname(
            "machdep.tsc.frequency\0".as_ptr(),
            &mut tsc_freq as *mut u64 as *mut c_void,
            &mut size,
            null_mut(),
            0,
        );
    }
    tsc_freq
}

pub fn mach_abs_time() -> u64 {
    unsafe { mach_absolute_time() }
}

pub fn mach_timebase_factor() -> Option<(u32, u32)> {
    let mut numer = 0;
    let mut denom = 0;
    let success = unsafe { mach_timebase(&mut numer, &mut denom) };
    if success {
        Some((numer, denom))
    } else {
        None
    }
}

extern "C" {
    fn sysctlbyname(
        name: *const u8,
        oldp: *mut c_void,
        oldlenp: *mut usize,
        newp: *mut c_void,
        newlen: usize,
    );
    fn mach_absolute_time() -> u64;
    fn mach_timebase(numer: *mut u32, denom: *mut u32) -> bool;
}

#[cfg(test)]
mod tests {
    use super::round_up_4k;

    #[test]
    fn round_up_4k_test_small() {
        assert_eq!(round_up_4k(0), 0);
        assert_eq!(round_up_4k(1), 4096);
        assert_eq!(round_up_4k(4095), 4096);
        assert_eq!(round_up_4k(4096), 4096);
        assert_eq!(round_up_4k(4097), 8192);
    }

    #[test]
    fn round_up_4k_test_big() {
        assert_eq!(round_up_4k(usize::MAX & !0xFFF), usize::MAX & !0xFFF);
        assert_eq!(round_up_4k(usize::MAX - 4095), usize::MAX & !0xFFF);
        assert_eq!(round_up_4k(usize::MAX - 4096), usize::MAX & !0xFFF);
        assert_eq!(round_up_4k(usize::MAX - 4097), usize::MAX & !0xFFF);
    }

    #[test]
    #[should_panic]
    fn round_up_4k_test_panic_max() {
        round_up_4k(usize::MAX);
    }

    #[test]
    #[should_panic]
    fn round_up_4k_test_panic() {
        round_up_4k((usize::MAX & !0xFFF) + 1);
    }
}
