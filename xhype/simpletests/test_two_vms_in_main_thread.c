#include <Hypervisor/hv.h>
#include <stdio.h>
#include <unistd.h>

#include "utils.h"

void test_two_vms_in_main_thread_create_destroy() {
  printf("---start test_two_vms_in_main_thread_create_destroy---\n");
  hv_return_t err = hv_vm_create(HV_VM_DEFAULT);
  if (err) {
    print_red("cannot create the 1st vm, err=%x\n", err);
    return;
  } else {
    print_green("created the 1st vm\n");
    err = hv_vm_destroy();
    if (err) {
      print_red("cannot destroy the 1st vm, err = %d\n", err);
      return;
    } else {
      print_green("destroyed the 1st vm\n");
    }
    err = hv_vm_create(HV_VM_DEFAULT);
    if (err) {
      print_red("cannot create the 2nd vm, err=%x\n", err);
    } else {
      print_green("created the 2nd vm\n");
      err = hv_vm_destroy();
      if (err) {
        print_red("cannot destroy the 2nd vm, err = %d\n", err);
        return;
      } else {
        print_green("destroyed the 2nd vm\n");
      }
    }
  }
  printf("---end test_two_vms_in_main_thread_create_destroy---\n");
}

void test_two_vms_at_the_same_time() {
  printf("---start test_two_vms_at_the_same_time---\n");
  hv_return_t err = hv_vm_create(HV_VM_DEFAULT);
  if (err) {
    print_red("cannot create the 1st vm, err=%x\n", err);
    return;
  } else {
    print_green("created the 1st vm\n");
    err = hv_vm_create(HV_VM_DEFAULT);
    if (err) {
      print_red("cannot create the 2nd vm, err=%x\n", err);
    } else {
      print_green("created the 2nd vm\n");
    }
    hv_vm_destroy();  // program fails here because hv_vm_create is called twice
                      // in the same thread
  }
  printf("---end test_two_vms_at_the_same_time---\n");
}

int main() {
  test_two_vms_in_main_thread_create_destroy();
  test_two_vms_at_the_same_time();
  return 0;
}