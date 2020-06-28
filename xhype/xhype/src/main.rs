#![cfg_attr(feature = "vthread_closure", feature(fn_traits))]
use std::env;
use std::sync::{Arc, RwLock};
use xhype::err::Error;
use xhype::vthread::Builder;
use xhype::{linux, vmcall, VMManager};

static mut NUM_A: i32 = 4;
static mut NUM_B: i32 = 2;

fn change_a() {
    unsafe {
        NUM_A = 2;
        vmcall(1, &GOOD_STR as *const &str as *const u8);
    }
}

fn change_b() {
    unsafe {
        NUM_B = 100;
    }
}

const GOOD_STR: &str = "good";
const CLOSURE_STR: &str = "vmcall inside a clousre";

fn vthread_test() {
    println!("initially, a = {}, b = {}", unsafe { NUM_A }, unsafe {
        NUM_B
    });
    let vmm = VMManager::new().unwrap();
    let vm = Arc::new(RwLock::new(vmm.create_vm(1).unwrap()));
    let vth_a = if cfg!(feature = "vthread_closure") {
        Builder::new(&vm)
            .spawn(|| unsafe {
                NUM_A = 3;
                vmcall(1, &CLOSURE_STR as *const &str as *const u8);
            })
            .unwrap()
    } else {
        Builder::new(&vm).spawn(change_a).unwrap()
    };
    let vth_b = if cfg!(feature = "vthread_closure") {
        Builder::new(&vm)
            .spawn(|| unsafe {
                NUM_B = 101;
            })
            .unwrap()
    } else {
        Builder::new(&vm)
            .name("vth_b".to_string())
            .spawn(change_b)
            .unwrap()
    };
    vth_a.join().unwrap().unwrap();
    vth_b.join().unwrap().unwrap();
    println!("a = {}, b = {}", unsafe { NUM_A }, unsafe { NUM_B });
}

fn kernel_test() {
    let memsize = 1 << 30; // 1GB
    let vmm = VMManager::new().unwrap();
    let kn_path = env::var("KN_PATH").unwrap();
    let rd_path = env::var("RD_PATH").ok();
    let cmd_line = env::var("CMD_Line").unwrap_or("auto".to_string());
    let vm = Arc::new(RwLock::new(vmm.create_vm(1).unwrap()));
    let guest_threads = linux::load_linux64(&vm, kn_path, rd_path, cmd_line, memsize).unwrap();
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
    env_logger::init();
    vthread_test();
    kernel_test();
}
