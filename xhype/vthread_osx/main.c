#include <Hypervisor/hv.h>
#include <Hypervisor/hv_vmx.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "vthread.h"

#define MEM_SIZE 4096

void hltcall(void) { __asm__("hlt\n\t"); }

int main() {
  uint8_t* hltcode = valloc(MEM_SIZE);  // using valloc is essential!
  hltcode[0] = 0xf4;
  hltcode[10] = 0xf4;
  struct virtual_machine vm = {
      hltcode, MEM_SIZE, HV_MEMORY_READ | HV_MEMORY_EXEC | HV_MEMORY_WRITE};

  if (vm_init(&vm)) {
    printf("vm_init fail\n");
    return -1;
  }

  struct vthread* vth = vthread_create(&vm, hltcode, NULL);
  struct vthread* vth2 = vthread_create(&vm, hltcode + 10, NULL);
  vthread_join(vth, NULL);
  vthread_join(vth2, NULL);
  return 0;
}