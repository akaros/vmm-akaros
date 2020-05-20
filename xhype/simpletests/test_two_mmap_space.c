/*
  To compile:
    make test_two_mmap_space
  To run:
    ./test_two_mmap_space
 */

#include <Hypervisor/hv.h>
#include <stdint.h>
#include <stdlib.h>

#include "utils.h"

#define MEM_REGION_SIZE 1024

void guard(int n) {
  if (n != 0) {
    abort();
  }
}

int main() {
  hv_vm_space_t vm1, vm2;
  vm1 = HV_VM_SPACE_DEFAULT;
  if (hv_vm_create(HV_VM_DEFAULT)) {
    print_red("cannot create a vm\n");
    return -1;
  }
  if (hv_vm_space_create(&vm2)) {
    print_red("cannot create vm2\n");
    return -1;
  } else {
    print_green("vm2 = %d\n", vm2);
  }
  uint8_t *mem_region1 = (uint8_t *)valloc(MEM_REGION_SIZE);
  uint8_t *mem_region2 = (uint8_t *)valloc(MEM_REGION_SIZE);
  if ((!mem_region1) || (!mem_region2)) {
    print_red("cannot allocate two memory segments\n");
    return -1;
  }
  hv_return_t err1 =
      hv_vm_map_space(vm1, mem_region1, 0, MEM_REGION_SIZE,
                      HV_MEMORY_READ | HV_MEMORY_WRITE | HV_MEMORY_EXEC);
  if (err1) {
    print_red(
        "cannot map guest physical address %x - %x to host virtual address %x "
        "- %x, err = %x\n",
        0, MEM_REGION_SIZE - 1, mem_region1, mem_region1 + MEM_REGION_SIZE - 1,
        err1);
  } else {
    print_green(
        "map guest physical address %x - %x to host virtual address %p - %p\n",
        0, MEM_REGION_SIZE - 1, mem_region1, mem_region1 + MEM_REGION_SIZE - 1);
  }
  hv_return_t err2 =
      hv_vm_map_space(vm2, mem_region2, 0, MEM_REGION_SIZE,
                      HV_MEMORY_READ | HV_MEMORY_WRITE | HV_MEMORY_EXEC);
  if (err2) {
    print_red(
        "cannot map guest physical address %x - %x to host virtual address %p "
        "- %p, err = %x\n",
        0, MEM_REGION_SIZE - 1, mem_region2, mem_region2 + MEM_REGION_SIZE - 1,
        err2);
  } else {
    print_green(
        "map guest physical address %x - %x to host virtual address %p - %p\n",
        0, MEM_REGION_SIZE - 1, mem_region2, mem_region2 + MEM_REGION_SIZE - 1);
  }
  if (!err1) {
    guard(hv_vm_unmap_space(vm1, 0, MEM_REGION_SIZE));
  }
  if (!err2) {
    guard(hv_vm_unmap_space(vm2, 0, MEM_REGION_SIZE));
  }
  guard(hv_vm_destroy());
  return 0;
}