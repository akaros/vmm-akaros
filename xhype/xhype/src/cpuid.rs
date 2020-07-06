/* SPDX-License-Identifier: GPL-2.0-only */
extern "C" {
    fn cpuid(ieax: u32, iecx: u32, eaxp: *mut u32, ebxp: *mut u32, ecxp: *mut u32, edxp: *mut u32);
}

pub fn do_cpuid(eax: u32, ecx: u32) -> (u32, u32, u32, u32) {
    let mut o_eax = 0;
    let mut o_ebx = 0;
    let mut o_ecx = 0;
    let mut o_edx = 0;
    unsafe { cpuid(eax, ecx, &mut o_eax, &mut o_ebx, &mut o_ecx, &mut o_edx) }
    (o_eax, o_ebx, o_ecx, o_edx)
}

#[cfg(test)]
mod test {
    use super::do_cpuid;
    #[test]
    fn do_cpuid_test() {
        let (_, ebx, ecx, edx) = do_cpuid(0, 0);
        // GenuineIntel
        assert_eq!(ebx, 0x756e6547);
        assert_eq!(ecx, 0x6c65746e);
        assert_eq!(edx, 0x49656e69);
    }
}
