This directory contains a simple example of the vthread model on macOS based on 
Hypervisor.framework. 

To compile and run the example, 

``` shell
make && ./main
```

`hv_return_t vm_init(struct virtual_machine const* vm)` will initialize the VM 
instance and map a region of the guest's physical address to a region of 
the host's virtual address, described in the parameter `vm` .

`struct vthread* vthread_create(struct virtual_machine const* vm, void* entry, void* arg)` 
will create a new virtual thread, corresponding to a vcpu of Hypervisor.framework, 
in a new pthread. `entry` is the starting position of the code segment of this 
virtual thread. It is represented by a host virtual address.

`void vthread_join(struct vthread* vth, void** retval_loc)` will make the 
process wait until `vth` finishes execution.

`main.c` allocates a region of memory `hltcode` and writes `0xf4` (hlt) to 
position 0 and 10. `hltcode` is used as the guest's physical memory. Two 
virtual threads are then created, with entry being `hltcode` and `hltcode+10` 
respectively. Both of the two threads exit with `VMX_REASON_HLT` as the vmexit
reason.
