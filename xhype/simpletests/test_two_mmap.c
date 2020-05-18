#include <Hypervisor/hv.h>
#include <stdint.h>
#include <stdlib.h>

#include "utils.h"

#define MEM_REGION_SIZE 1024

int main() {
  if (hv_vm_create(HV_VM_DEFAULT)) {
    print_red("cannot create a vm\n");
    return -1;
  }
  uint8_t *mem_region1 = (uint8_t *)valloc(MEM_REGION_SIZE);
  uint8_t *mem_region2 = (uint8_t *)valloc(MEM_REGION_SIZE);
  if ((!mem_region1) || (!mem_region2)) {
    print_red("cannot allocate two memory segments\n");
    return -1;
  }
  hv_return_t err1 =
      hv_vm_map(mem_region1, 0, MEM_REGION_SIZE,
                HV_MEMORY_READ | HV_MEMORY_WRITE | HV_MEMORY_EXEC);
  if (err1) {
    print_red(
        "cannot map guest physical address %x - %x to host virtual address %x "
        "- %x, err = %x\n",
        0, MEM_REGION_SIZE - 1, mem_region1, mem_region1 + MEM_REGION_SIZE - 1,
        err1);
  } else {
    print_green(
        "map guest physical address %x - %x to host virtual address %x - %x\n",
        0, MEM_REGION_SIZE - 1, mem_region1, mem_region1 + MEM_REGION_SIZE - 1);
  }
  hv_return_t err2 =
      hv_vm_map(mem_region2, MEM_REGION_SIZE, MEM_REGION_SIZE,
                HV_MEMORY_READ | HV_MEMORY_WRITE | HV_MEMORY_EXEC);
  if (err2) {
    print_red(
        "cannot map guest physical address %x - %x to host virtual address %x "
        "- %x, err = %x\n",
        MEM_REGION_SIZE, 2 * MEM_REGION_SIZE - 1, mem_region2,
        mem_region2 + MEM_REGION_SIZE - 1, err2);
  } else {
    print_green(
        "map guest physical address %x - %x to host virtual address %x - %x\n",
        MEM_REGION_SIZE, 2 * MEM_REGION_SIZE - 1, mem_region2,
        mem_region2 + MEM_REGION_SIZE - 1);
  }
  if (!err1) {
    hv_vm_unmap(0, MEM_REGION_SIZE);
  }
  if (!err2) {
    hv_vm_unmap(MEM_REGION_SIZE, MEM_REGION_SIZE);
  }
  hv_vm_destroy();
  return 0;
}