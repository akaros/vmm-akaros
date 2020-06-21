use super::{GuestThread, HandleResult, X86Reg, VCPU};
use log::{info, warn};

extern "C" {
    fn cpuid(ieax: u32, iecx: u32, eaxp: *mut u32, ebxp: *mut u32, ecxp: *mut u32, edxp: *mut u32);
}

fn do_cpuid(eax: u32, ebx: u32) -> (u32, u32, u32, u32) {
    let mut o_eax = 0;
    let mut o_ebx = 0;
    let mut o_ecx = 0;
    let mut o_edx = 0;
    unsafe {
        cpuid(
            eax,
            ebx,
            &mut o_eax as *mut u32,
            &mut o_ebx as *mut u32,
            &mut o_ecx as *mut u32,
            &mut o_edx as *mut u32,
        )
    }
    (o_eax, o_ebx, o_ecx, o_edx)
}

pub fn handle_cpuid(vcpu: &VCPU, _gth: &GuestThread) -> HandleResult {
    let eax_in = vcpu.read_reg(X86Reg::RAX).unwrap() as u32;
    let ecx_in = vcpu.read_reg(X86Reg::RCX).unwrap() as u32;
    let (mut eax, mut ebx, mut ecx, mut edx) = do_cpuid(eax_in, ecx_in);
    match eax_in {
        0x01 => {
            /* Set the hypervisor bit to let the guest know it is
             * virtualized */
            ecx |= 1 << 31;
            /* Unset the monitor capability bit so that the guest does not
             * try to use monitor/mwait. */
            ecx &= !(1 << 3);
            /* Unset the vmx capability bit so that the guest does not try
             * to turn it on. */
            ecx &= !(1 << 5);
            /* Unset the perf capability bit so that the guest does not try
             * to turn it on. */
            ecx &= !(1 << 15);
            /* Set the guest pcore id into the apic ID field in CPUID. */
            ebx &= 0x0000ffff;
            // FIX me, not finished
            // ebx |= (current->vmm.nr_guest_pcores & 0xff) << 16;
            // ebx |= (tf->tf_guest_pcoreid & 0xff) << 24;
            ebx |= (1 & 0xff) << 16;
            ebx |= (vcpu.id() & 0xff) << 24;
            warn!(
                "set number of logical processors = 1, apic id = {}",
                vcpu.id()
            );
            // unimplemented!();
        }
        0x07 => {
            /* Do not advertise TSC_ADJUST */
            ebx &= !(1 << 1);
        }
        0x0a => {
            eax = 0;
            ebx = 0;
            ecx = 0;
            edx = 0;
        }
        0x40000000 => {
            /* Signal the use of KVM. */
            eax = 0;
            ebx = 0;
            ecx = 0;
            edx = 0;
            unimplemented!();
        }
        0x40000003 => {
            /* Hypervisor Features. */
            /* Unset the monitor capability bit so that the guest does not
             * try to use monitor/mwait. */
            edx &= !(1 << 0);
        }
        0x40000100 => {
            /* Signal the use of AKAROS. */
            eax = 0;
            unimplemented!();
        }
        /* Hypervisor Features. */
        0x40000103 => {
            /* Unset the monitor capability bit so that the guest does not
             * try to use monitor/mwait. */
            edx &= !(1 << 0);
        }
        _ => {}
    }
    vcpu.write_reg(X86Reg::RAX, eax as u64).unwrap();
    vcpu.write_reg(X86Reg::RBX, ebx as u64).unwrap();
    vcpu.write_reg(X86Reg::RCX, ecx as u64).unwrap();
    vcpu.write_reg(X86Reg::RDX, edx as u64).unwrap();
    info!(
        "cpuid, eax_in={:x}, ecx_in={:x}, eax={:x}, ebx={:x}, ecx={:x}, edx={:x}",
        eax_in, ecx_in, eax, ebx, ecx, edx
    );
    HandleResult::Next
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
