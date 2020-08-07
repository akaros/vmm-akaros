/* SPDX-License-Identifier: GPL-2.0-only */
use elf_rs::Elf;
use std::env;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use xhype::err::Error;
use xhype::utils::{parse_msr_policy, parse_port_policy};
use xhype::{multiboot, VMManager};

fn boot_harvey() {
    let (port_policy, port_list) = parse_port_policy();
    let (msr_policy, msr_list) = parse_msr_policy();
    let low_mem_size = 1 << 30; // 1GB
    let vmm = VMManager::new().unwrap();
    let kn_path = env::var("HARVEY_PATH").unwrap();
    let initial_rip;
    let mut load_addr = None;
    {
        /*
         look for initial rip and load_addr from Harvey Elf file

         initial rip is actually the entry point field in an elf file,
         here it is expected that the entry point is within the .bootstrap section

         load_addr is calculate by the following steps:
         we look for a bootstrap section S, and then make load_addr = S.addr - S.offset,
         where S.addr is the guest physical address where S should loaded, and
         S.offset is S's offset in the kernel image file. In this way we can make
         sure that after the image is loaded, the bootstrap section is located at
         guest physical address S.addr

        */
        let mut kernel_file = File::open(&kn_path).unwrap();
        let mut buf = Vec::new();
        kernel_file.read_to_end(&mut buf).unwrap();
        let elf = Elf::from_bytes(&buf).unwrap();
        if let Elf::Elf64(e) = elf {
            initial_rip = e.header().entry_point();
            for header in e.section_header_iter() {
                if header.section_name() == ".bootstrap" {
                    load_addr = Some(header.sh.addr() - header.sh.offset());
                }
            }
        } else {
            panic!("{} is not a 64-bit elf file", &kn_path);
        }
    }
    if load_addr.is_none() {
        panic!("cannot find a bootstrap section in {}", &kn_path);
    }

    let mut vm = vmm.create_vm(1, Some(low_mem_size)).unwrap();
    vm.port_list = port_list;
    vm.port_policy = port_policy;
    vm.msr_list = msr_list;
    vm.msr_policy = msr_policy;
    let vm = Arc::new(vm);
    let guest_threads =
        multiboot::multiboot(&vm, kn_path, low_mem_size, load_addr.unwrap(), initial_rip).unwrap();
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

/**
 This example shows booting Harvey with xhype
 Necessary environment variables:
    * HARVEY_PATH:  path to Harvey 64-bit ELF file

Optional environment variables:
    RUST_LOG:   log level: trace, debug, info, warn, error, none; see
                https://docs.rs/flexi_logger/0.15.10/flexi_logger/struct.LogSpecification.html for details.
    LOG_DIR:    directory to save log files
    XHYPE_UNKNOWN_PORT: policy regarding guests' access to unknown IO ports
    XHYPE_UNKNOWN_MSR: policy regarding guests' access to unknown MSRs

See the readme.md in crate root directory for more environment variables of xhype.

To run this example, in the crate root directory, run
    `cargo run --example xhype_harvey`
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
    boot_harvey();
}
