#include <Hypervisor/hv.h>
#include <pthread.h>
#include <stdio.h>
#include <unistd.h>

#include "utils.h"

void test_two_vms_in_two_processes() {
  pid_t p = fork();
  printf("---start test_two_vms_in_two_processes in process %d---\n", p);
  int err = hv_vm_create(HV_VM_DEFAULT);
  if (err) {
    print_red("cannot create vm for process %d, err=%x\n", p, err);
  } else {
    print_green("created vm for thread %d\n", p);
    err = hv_vm_destroy();
    if (err) {
      print_red("cannot destroy vm for process %d, err=%x\n", p, err);
    } else {
      print_green("destroyed vm for process %d\n", p);
    }
  }
  printf("---end test_two_vms_in_two_processes---\n");
}

int main() {
  test_two_vms_in_two_processes();
  return 0;
}