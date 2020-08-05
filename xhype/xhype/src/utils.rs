/* SPDX-License-Identifier: GPL-2.0-only */
use crate::{MsrPolicy, PolicyList, PortPolicy};
use std::collections::HashSet;
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

pub fn make_stdin_raw() {
    unsafe { make_stdin_raw_c() }
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
    fn make_stdin_raw_c();
}

/**
Parse the port policy from environment variable `XHYPE_UNKNOWN_PORT`

format: `[random|allone];[apply|except];[list of port numbers in hex]`

examples:

* `allone;except;`

This is the default policy: xhype returns 0xff, 0xffff, or 0xffffffff
(depending on the size) to the guest if an unknown port is accessed

* `random;apply;21,a1,60`

xhype returns a random number to the guest if the guest reads from
port 0x21, 0xa1, or 0x60. For any other ports that xhype does not
know how to handle, the whole program stops.

* `allone;except;21,a1,60`

xhype returns `0xff`, `0xffff`, or `0xffffffff` (depending on the size) to
the guest if an unknown port is accessed, except port 0x20, 0xa1, and
0x60. For these three ports, xhype stops.
*/
pub fn parse_port_policy() -> (PortPolicy, PolicyList<u16>) {
    let mut port_policy = PortPolicy::AllOne;
    let mut port_list = PolicyList::Except(HashSet::new());
    if let Ok(policy) = std::env::var("XHYPE_UNKNOWN_PORT") {
        let parts: Vec<&str> = policy.split(';').collect();
        if parts.len() == 3 {
            port_policy = match parts[0] {
                "random" => PortPolicy::Random,
                "allone" => PortPolicy::AllOne,
                _ => panic!("unknown policy: {}", policy),
            };
            let set = parts[2]
                .split(',')
                .map(|n| u16::from_str_radix(n, 16).unwrap_or(0))
                .collect();
            port_list = match parts[1] {
                "apply" => PolicyList::Apply(set),
                "except" => PolicyList::Except(set),
                _ => panic!("unknown policy: {}", policy),
            }
        }
    }
    (port_policy, port_list)
}

/**
Parse the port policy from environment variable `XHYPE_UNKNOWN_MSR`

format: `[random|allone|gp];[apply|except];[list of MSRs]`

examples:

* `gp;except;`

This is the default policy: xhype injects a general protection fault
if an unknown MSR is accessed.

* `random;apply;64e,64d`

xhype returns a random value to the guest if MSR `64e` or `64d` is accessed
and xhype does not known how to handle it. Otherwise xhype handles
the request or stops.
*/

pub fn parse_msr_policy() -> (MsrPolicy, PolicyList<u32>) {
    let mut msr_policy = MsrPolicy::GP;
    let mut msr_list = PolicyList::Except(HashSet::new());
    if let Ok(policy) = std::env::var("XHYPE_UNKNOWN_MSR") {
        let parts: Vec<&str> = policy.split(';').collect();
        if parts.len() == 3 {
            msr_policy = match parts[0] {
                "random" => MsrPolicy::Random,
                "allone" => MsrPolicy::AllOne,
                "gp" => MsrPolicy::GP,
                _ => panic!("unknown policy: {}", policy),
            };
            let set = parts[2]
                .split(',')
                .map(|n| u32::from_str_radix(n, 16).unwrap_or(0))
                .collect();
            msr_list = match parts[1] {
                "apply" => PolicyList::Apply(set),
                "except" => PolicyList::Except(set),
                _ => panic!("unknown policy: {}", policy),
            }
        }
    }
    (msr_policy, msr_list)
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
