use std::env;
use vthread_rs::{loader, VMManager};
fn main() {
    env_logger::init();
    let memsize = 1 << 30;
    let vmm = VMManager::new().unwrap();
    let kn_path = env::var("KN_PATH").unwrap();
    let rd_path = env::var("RD_PATH").unwrap();
    let cmd_line = env::var("CMD_Line").unwrap_or("auto".to_string());
    let vm = vmm.create_vm(1).unwrap();
    let gth = loader::load_linux64(&vm, &kn_path, &rd_path, &cmd_line, memsize).unwrap();
    let vcpu = vmm.create_vcpu().unwrap();
    match gth.run_on(&vcpu) {
        Ok(_) => {
            println!("guest terminates normally");
        }
        Err(e) => {
            println!("guest terminates with error: {:?}", e);
        }
    }
}
