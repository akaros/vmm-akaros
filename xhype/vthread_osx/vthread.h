#include <Hypervisor/hv.h>
#include <pthread.h>

struct virtual_machine {
  uint8_t* vm_mem;
  size_t mem_size;
  hv_memory_flags_t mem_flags;
};

struct vthread {
  pthread_t pth;
  struct virtual_machine* vm;
  uint8_t* entry;
};

hv_return_t vm_init(struct virtual_machine const* vm);

struct vthread* vthread_create(struct virtual_machine const* vm, void* entry,
                               void* arg);

// void vthread_run(struct vthread* vth);

void vthread_join(struct vthread* vth, void** retval_loc);
