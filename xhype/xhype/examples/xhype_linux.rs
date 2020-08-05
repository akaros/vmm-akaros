/* SPDX-License-Identifier: GPL-2.0-only */

/*!
This example shows how to use xhype to boot Linux.

Necessary environment variables:
    KN_PATH:    complete path to Linux kernel image,
    CMD_Line:   Linux kernel command line,

Optional environment variables:
    RD_PATH:    complete path to a ramdisk file
    RUST_LOG:   log level: trace, debug, info, warn, error, none; see
                https://docs.rs/flexi_logger/0.15.10/flexi_logger/struct.LogSpecification.html for details.
    LOG_DIR:    directory to save log files
    XHYPE_UNKNOWN_PORT: policy regarding guests' access to unknown IO ports
    XHYPE_UNKNOWN_MSR: policy regarding guests' access to unknown MSRs

Set policy regarding guests' access to unknown IO ports:
    format: [random|allone];[apply|except];[list of port numbers in hex]
    examples:
        * allone;except;
            This is the default policy: xhype returns 0xff, 0xffff, or 0xffffffff
            (depending on the size) to the guest if an unknown port is accessed
        * random;apply;21,a1,60
            xhype returns a random number to the guest if the guest reads from
            port 0x21, 0xa1, or 0x60. For any other ports that xhype does not
            know how to handle, the whole program stops.
        * allone;except;21,a1,60
            xhype returns 0xff, 0xffff, or 0xffffffff (depending on the size) to
            the guest if an unknown port is accessed, except port 0x20, 0xa1, and
            0x60. For these three ports, xhype stops.

Set policy regarding guests' access to unknown MSRs:
    format: [random|allone|gp];[apply|except];[list of MSRs]
    examples:
        * gp;except;
            This is the default policy: xhype injects a general protection fault
            if an unknown MSR is accessed.
        * random;apply;64e,64d
            xhype returns a random value to the guest if MSR 64e or 64d is accessed
            and xhype does not known how to handle it. Otherwise xhype handles
            the request or stops.

To run this example, in the crate root directory, run
    `cargo run --example xhype_linux`

Suggestions:
1. Currently xhype will make stdin into a raw IO path, that means `ctrl-C` cannot
kill the program. So to kill it, run `killall xhype_linux` in another terminal.
!*/

use std::env;
use std::sync::Arc;
use xhype::consts::*;
use xhype::err::Error;
use xhype::utils::{parse_msr_policy, parse_port_policy};
use xhype::virtio::VirtioDevice;
use xhype::{linux, VMManager};

fn boot_linux() {
    let (port_policy, port_list) = parse_port_policy();
    let (msr_policy, msr_list) = parse_msr_policy();
    let low_mem_size = Some(100 * MiB);
    let memory_size = 1 * GiB;
    let vmm = VMManager::new().unwrap();
    let kn_path = env::var("KN_PATH").unwrap();
    let rd_path = env::var("RD_PATH").ok();
    let cmd_line = env::var("CMD_Line").unwrap_or("auto".to_string());
    let mut vm = vmm.create_vm(1, low_mem_size).unwrap();
    vm.add_virtio_mmio_device(VirtioDevice::new_vmnet(
        "virtio-vmnet".to_string(),
        0,
        vm.irq_sender.clone(),
        vm.gpa2hva.clone(),
    ));
    vm.add_virtio_mmio_device(VirtioDevice::new_rng(
        "virtio-rng".into(),
        1,
        vm.irq_sender.clone(),
        vm.gpa2hva.clone(),
    ));
    vm.port_list = port_list;
    vm.port_policy = port_policy;
    vm.msr_list = msr_list;
    vm.msr_policy = msr_policy;
    let vm = Arc::new(vm);
    let guest_threads = linux::load_linux64(&vm, kn_path, rd_path, cmd_line, memory_size).unwrap();
    let join_handlers: Vec<std::thread::JoinHandle<Result<(), Error>>> =
        guest_threads.into_iter().map(|gth| gth.start()).collect();
    for (i, handler) in join_handlers.into_iter().enumerate() {
        let r = handler.join().unwrap();
        match r {
            Ok(_) => {
                println!("guest thread {} terminates correctly", i);
            }
            Err(e) => {
                println!("guest thread {} terminates with error: {:?}", i, e);
            }
        }
    }
}

fn main() {
    if let Ok(directory) = std::env::var("LOG_DIR") {
        flexi_logger::Logger::with_env()
            .log_to_file()
            .directory(directory)
            .start()
            .unwrap();
    }
    boot_linux();
}
