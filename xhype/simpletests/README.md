## Simple tests of Hypervisor.framework

This directory contains some simple tests to understand macOS's Hypervisor.framework. 

### Test two hv_vm_create() calls in a single process

``` shell
make all
./test_two_vms_in_main_thread
./test_two_vms_in_two_threads
./test_two_vms_in_two_processes
```

In Apple's document, `hv_vm_create()` will create a VM instance for the current
task. It looks like one process can only call `hv_vm_create()` once before 
`hv_vm_destroy()` is called. 

`test_two_vms_in_main_thread` tries calling `hv_vm_create()` in the main thread 
twice before `hv_vm_destroy()` is called. The result is the second 
`hv_vm_create()` returns error and also the following `hv_vm_destroy()` makes 
the program crash. This means the second `hv_vm_create()` has side effects.

`test_two_vms_in_two_threads` tried calling `hv_vm_create()` in two different 
threads. Similarly, before one thread calls `hv_vm_destroy()` , the other thread's
calling `hv_vm_create()` will fail and also has side effects: the program crashes
when the first thread calls `hv_vm_destroy()` .

Since a single process can only create one VM instance, 
`test_two_vms_in_two_processes` simply uses `fork()` to create two processes 
and calls `hv_vm_create()` . There are no crashes this time.

The fact that `hv_vm_create()` can only be called once is also mentioned in [xhype](https://github.com/machyve/xhyve#xhyve-architecture), another macOS virtualization solution based on Hypervisor.framework:

> Hypervisor.framework seems to enforce a strict 1:1 relationship between a host process/VM and host thread/vCPU, that means VMs and vCPUs can only be interacted with by the processes and threads that created them. 

### Test two hv_vm_map() calls in a single process

According to Apple's document, `hv_vm_map()` will

> maps a region in the virtual address space of the current task into the guest physical address space of the VM.

However, similarly, it seems that `hv_vm_map()` can only be called once before
`hv_vm_unmap()` . 

``` shell
./test_two_mmap
```

`test_two_mmap` tries mapping two regions of the host's virtual address to 
the guest's physical address. The second `hv_vm_map()` fails.
