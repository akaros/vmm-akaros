# xhype

xhype is a virtualization library on macOS written in Rust which implements the 
virtual vthread model originated from [Akaros OS](https://github.com/akaros/akaros).
xhype is based on the [Hypervisor](https://developer.apple.com/documentation/hypervisor) framework and requires at least macOS 10.15.

## Usage

The directory `examples` contains an example showing how to boot Linux with xhype. 

Build this example

``` 
cargo build --example xhype_linux
```

and then run

``` 
KN_PATH=<path/to/bzImage> CMD_LINE="earlyprintk=serial console=tty tsc=unstable" sudo -E target/debug/examples/xhype_linux
```

`sudo` is required to create a macOS vmnet interface. See more details in the comments of `examples/xhype_linux.rs`

### Environment variables

* `STDIN_RAW` : set it to `False` to prevent xhype from changing stdin to a raw IO path
* `DEBUG_FIFO` : an fifo used for send instructions to xhype when `VMX_REASON_MTF` happens

#### Force a guest to exit after every instruction

Currently xhype implement such single-step debugging by Monitor trap flag. To use it:

1. Make an fifo: `mkfifo /path/to/fifo-debug`
2. Set the environment variable: export DEBUG_FIFO=/path/to/fifo-debug
3. Turn on `CPU_BASED_MTF` in `Primary Processor-Based VM-Execution Controls` . See an example in `fn load_firemware()` of `firmware.rs`
4. Start xhype, the guest will be paused after every instruction. Open another terminal and write one byte to `/path/to/fifo-debug` to resume the VM, for example `echo 'a' > /path/to/fifo-debug` . 

## Docs

Run `cargo doc && python3 -m http.server --directory target/doc` and then open [http://127.0.0.1:8000/xhype](http://127.0.0.1:8000/xhype) in the browser.

## Notes on Hypervisor.framework

### EPT management

The memory mapping is done by [hv_vm_map](https://developer.apple.com/documentation/hypervisor/1441187-hv_vm_map?language=objc) or [hv_vm_map_space](https://developer.apple.com/documentation/hypervisor/3181550-hv_vm_map_space?language=objc). When this function is called, 
this framework does not setup EPT tables immediately, instead, it just only takes
a note. So when a guest is started, the actual EPT table is empty and EPT violation
happens. Then the framework will setup the EPT table. After that, depending on 
which function is used to run the guest:

* If [hv_vcpu_run](https://developer.apple.com/documentation/hypervisor/1441231-hv_vcpu_run?language=objc) is used, then this EPT violation is further returned back to user space.
* If [hv_vcpu_run_until](https://developer.apple.com/documentation/hypervisor/3181548-hv_vcpu_run_until?language=objc) is used, then the framework resumes the VM without redelivering this VM exit to user space.

### VCPU related functions

Since a VCPU is bound to the pthread which creates it, functions starting with `hv_vcpu` can only be called by the thread owning the VCPU. The only exception is [hv_vcpu_interrupt](https://developer.apple.com/documentation/hypervisor/1441468-hv_vcpu_interrupt?language=objc).
It is used by thread A to stop a VCPU created by thread B. Depending on the state of thread B:

* if thread B is executing the VCPU, a vm exit of `VMX_REASON_IRQ` is generated.
* if thread B is not executing the VCPU, nothing happens.

### Spurious vm exits 

Two types of spurious vm exits would be observed if a VCPU is started by [hv_vcpu_run](https://developer.apple.com/documentation/hypervisor/1441231-hv_vcpu_run?language=objc): 1. `VMX_REASON_IRQ` as discussed in the paragraphs above, and 2. `VMX_REASON_IRQ` , which is 
the interrupt from the real APIC timer of the machine. Those interrupts are generated
such that the pthread running the VCPU could be stopped and managed by macOS kernel 
scheduler. Again, using [hv_vcpu_run_until](https://developer.apple.com/documentation/hypervisor/3181548-hv_vcpu_run_until?language=objc), the framework itself will 
handle this type of vm exit and will not deliver this vm exit to user space.

## Known issues

1. The emulation of IO APIC, local APIC, RTC, and PIC device is just minimal and needs further development.
2. `docode.c` currently only supports `mov` instruction.
3. We need a more sophisticated way to gracefully join the threads spawned for virtual hardware, including IO APIC and virtio queues.
4. The implementation of single-step debugging based on Monitor trap flag does not take interrupts into consideration.
