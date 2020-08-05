use crossbeam_channel::unbounded as channel;
#[allow(unused_imports)]
use log::*;
use std::fs::{metadata, File};
use std::io::Read;
use std::sync::Arc;
use xhype::consts::x86::*;
use xhype::consts::*;
use xhype::hv::X86Reg;
use xhype::hv::{gen_exec_ctrl, vmx::*, vmx_read_capability, VMXCap};
use xhype::{
    err::Error,
    utils::{parse_msr_policy, parse_port_policy},
    GuestThread, VMManager, VirtualMachine,
};

const SEG_AR_ACCESSED: u64 = 1 << 0;
const SEG_AR_W: u64 = 1 << 1;
const SEG_AR_CODE: u64 = 1 << 3;
const SEG_PRES: u64 = 1 << (15 - 8);
const SEG_S: u64 = 1 << 4;

fn load_firemware(
    vm: &Arc<VirtualMachine>,
    file1_path: &str,
    addr1: u64,
    file2_path: &str,
    addr2: u64,
    rip: u64,
) -> Result<GuestThread, Error> {
    let mut low_mem;
    if let Some(mem) = vm.low_mem.as_ref() {
        low_mem = mem.write().unwrap();
    } else {
        return Err("low memory is required to test firmware".to_string())?;
    }
    {
        let file1_meta = metadata(&file1_path)?;
        let mut file1 = File::open(&file1_path)?;
        trace!("file1 size = {:x}", file1_meta.len());
        file1
            .read_exact(&mut low_mem[addr1 as usize..(addr1 as usize + file1_meta.len() as usize)])
            .unwrap();
        let codes = format!("{:02x?}", &low_mem[0xfffffff0..]);
        trace!("{:02x?}", codes.replace(',', ""));
        println!("{} loaded", file1_path);
    }
    {
        let file2_meta = metadata(&file2_path)?;
        trace!("file2 size = {:x}", file2_meta.len());
        let mut file2 = File::open(&file2_path)?;
        file2
            .read_exact(&mut low_mem[addr2 as usize..(addr2 as usize + file2_meta.len() as usize)])
            .unwrap();
        println!("{} loaded", file2_path);
    }

    let ctrl_pin = gen_exec_ctrl(vmx_read_capability(VMXCap::Pin)?, 0, 0);
    let ctrl_cpu = gen_exec_ctrl(
        vmx_read_capability(VMXCap::CPU)?,
        CPU_BASED_HLT | CPU_BASED_CR8_LOAD | CPU_BASED_CR8_STORE,
        0,
    );

    let ctrl_cpu2 = gen_exec_ctrl(vmx_read_capability(VMXCap::CPU2)?, 0, 0);
    let ctrl_entry = gen_exec_ctrl(vmx_read_capability(VMXCap::Entry)?, 0, 0);
    let cr0 = X86_CR0_NE;
    let cr4 = X86_CR4_VMXE;
    let init_vmcs = vec![
        (
            VMCS_GUEST_CS_AR,
            SEG_AR_CODE | SEG_S | SEG_PRES | SEG_AR_W | SEG_AR_ACCESSED,
        ),
        (VMCS_GUEST_CS_LIMIT, 0xffff),
        (VMCS_GUEST_CS, 0xf000),
        (VMCS_GUEST_CS_BASE, 0xffff0000),
        (
            VMCS_GUEST_DS_AR,
            SEG_S | SEG_PRES | SEG_AR_W | SEG_AR_ACCESSED,
        ),
        (VMCS_GUEST_DS_LIMIT, 0xffff),
        (
            VMCS_GUEST_ES_AR,
            SEG_S | SEG_PRES | SEG_AR_W | SEG_AR_ACCESSED,
        ),
        (VMCS_GUEST_ES_LIMIT, 0xffff),
        (
            VMCS_GUEST_FS_AR,
            SEG_S | SEG_PRES | SEG_AR_W | SEG_AR_ACCESSED,
        ),
        (VMCS_GUEST_FS_LIMIT, 0xffff),
        (
            VMCS_GUEST_GS_AR,
            SEG_S | SEG_PRES | SEG_AR_W | SEG_AR_ACCESSED,
        ),
        (VMCS_GUEST_GS_LIMIT, 0xffff),
        (
            VMCS_GUEST_SS_AR,
            SEG_S | SEG_PRES | SEG_AR_W | SEG_AR_ACCESSED,
        ),
        (VMCS_GUEST_SS_LIMIT, 0xffff),
        /* As documented in Intel Vol.3 22.39, access right of LDTR following
        power-up is Present, R/W, but setting it to Present, R/W causes VM entry
        fail. */
        (VMCS_GUEST_LDTR_AR, 0x10000),
        (VMCS_GUEST_LDTR_LIMIT, 0xffff),
        (VMCS_GUEST_TR_AR, SEG_PRES | SEG_AR_W | SEG_AR_ACCESSED),
        (VMCS_GUEST_TR_LIMIT, 0xffff),
        (VMCS_GUEST_IDTR_LIMIT, 0xffff),
        (VMCS_GUEST_GDTR_LIMIT, 0xffff),
        (VMCS_CTRL_PIN_BASED, ctrl_pin),
        (VMCS_CTRL_CPU_BASED, ctrl_cpu),
        (VMCS_CTRL_CPU_BASED2, ctrl_cpu2),
        (VMCS_CTRL_VMENTRY_CONTROLS, ctrl_entry),
        (VMCS_CTRL_EXC_BITMAP, 0xffffffff), // currently we track all exceptions
        (VMCS_GUEST_CR0, cr0),
        (VMCS_CTRL_CR0_MASK, X86_CR0_PG | X86_CR0_PE),
        (VMCS_CTRL_CR0_SHADOW, cr0),
        (VMCS_GUEST_CR4, cr4),
        (VMCS_CTRL_CR4_MASK, X86_CR4_VMXE | X86_CR4_PAE),
        (VMCS_CTRL_CR4_SHADOW, cr4 & !X86_CR4_VMXE),
    ]
    .into_iter()
    .collect();
    let init_regs = vec![(X86Reg::RIP, rip), (X86Reg::RFLAGS, FL_RSVD_1)]
        .into_iter()
        .collect();
    let mut gth = GuestThread::new(vm, 0);
    let (vector_sender, vector_receiver) = channel();
    gth.vector_receiver = Some(vector_receiver);
    *vm.vector_senders.lock().unwrap() = Some(vec![vector_sender]);
    gth.init_vmcs = init_vmcs;
    gth.init_regs = init_regs;
    Ok(gth)
}

fn boot_firmware() {
    let file1 = std::env::var("F1").unwrap();
    let file2 = std::env::var("F2").unwrap();
    let addr1 = u64::from_str_radix(&std::env::var("ADDR1").unwrap(), 16).unwrap();
    let addr2 = u64::from_str_radix(&std::env::var("ADDR2").unwrap(), 16).unwrap();
    let rip = u64::from_str_radix(&std::env::var("RIP").unwrap(), 16).unwrap();
    let vmm = VMManager::new().unwrap();
    let mut vm = vmm.create_vm(1, Some(4 * GiB)).unwrap();
    let (port_policy, port_list) = parse_port_policy();
    let (msr_policy, msr_list) = parse_msr_policy();
    vm.port_list = port_list;
    vm.port_policy = port_policy;
    vm.msr_list = msr_list;
    vm.msr_policy = msr_policy;
    let vm = Arc::new(vm);
    let gth = load_firemware(&vm, &file1, addr1, &file2, addr2, rip).unwrap();
    let handler = gth.start();
    match handler.join().unwrap() {
        Ok(_) => {
            println!("guest thread terminates correctly");
        }
        Err(e) => {
            println!("guest thread terminates with error: {:?}", e);
        }
    }
}

/**
This example loads two blobs (can be the same file) to two address and set RIP to `0xfffffff0`. To use the example, set the following environment variables:
* `F1`: path to the first blob
* `ADDR1`: guest physical address where the blob should be loaded
* `F2` and `ADDR2`: similar to the two above
* `RIP`: Usually it should be set to `fff0`(no `0x` prefix), because as default, the initial CS base is `0xffff0000`, which add together to be `0xfffffff0`.

Optional Variables:
* `LOG_DIR`: directory to save log files
* `RUST_LOG`: log lovel
* `STDIN_RAW`:set it to `False` for debug pursue
*  `XHYPE_UNKNOWN_MSR` and `XHYPE_UNKNOWN_PORT`: see docs in `utils.rs`.
*/
fn main() {
    if let Ok(directory) = std::env::var("LOG_DIR") {
        flexi_logger::Logger::with_env()
            .log_to_file()
            .directory(directory)
            .format(flexi_logger::opt_format)
            .start()
            .unwrap();
    }
    boot_firmware();
}
