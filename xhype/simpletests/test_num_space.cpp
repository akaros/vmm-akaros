#include <Hypervisor/hv.h>
#include <vector>

#define GUARD(x, r)                   \
  {                                   \
    uint64_t ret = (uint64_t)(x);     \
    if (ret != r) {                   \
      printf("%s = %llx\n", #x, ret); \
      exit(1);                        \
    }                                 \
  }

int main() {
    GUARD(hv_vm_create(HV_VM_DEFAULT), HV_SUCCESS);
    std::vector<hv_vm_space_t> spaces;
    for(int i = 1; ; i += 1) {
        hv_vm_space_t sid;
        if(hv_vm_space_create(&sid) == HV_SUCCESS) {
            spaces.push_back(sid);
        } else {
            printf("max number of spaces = %d\n", i);
            break;
        }
    }
    for(auto sid: spaces) {
        GUARD(hv_vm_space_destroy(sid), HV_SUCCESS);
    }
    GUARD(hv_vm_destroy(), HV_SUCCESS);
}