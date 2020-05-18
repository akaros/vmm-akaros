#include "vthread.h"

#include <Hypervisor/hv_arch_vmx.h>
#include <Hypervisor/hv_vmx.h>
#include <stdio.h>
#include <stdlib.h>

#include "utils.h"

hv_return_t vm_init(struct virtual_machine const* vm) {
  hv_return_t err;
  err = hv_vm_create(HV_VM_DEFAULT);
  if (err) {
    print_red("cannot create vm, err = %x\n", err);
    abort();
  }
  err = hv_vm_map(vm->vm_mem, 0, vm->mem_size, vm->mem_flags);
  if (err) {
    print_red(
        "cannot map guest physical address %x - %x to host virtual address %x"
        "- %x, err = %x\n",
        0, vm->mem_size - 1, vm->vm_mem, vm->vm_mem + vm->mem_size - 1, err);
    abort();
  }
  return 0;
}

/* read GPR */
uint64_t rreg(hv_vcpuid_t vcpu, hv_x86_reg_t reg) {
  uint64_t v;

  if (hv_vcpu_read_register(vcpu, reg, &v)) {
    abort();
  }

  return v;
}

/* write GPR */
void wreg(hv_vcpuid_t vcpu, hv_x86_reg_t reg, uint64_t v) {
  if (hv_vcpu_write_register(vcpu, reg, v)) {
    abort();
  }
}

/* read VMCS field */
static uint64_t rvmcs(hv_vcpuid_t vcpu, uint32_t field) {
  uint64_t v;

  if (hv_vmx_vcpu_read_vmcs(vcpu, field, &v)) {
    abort();
  }

  return v;
}

/* write VMCS field */
static void wvmcs(hv_vcpuid_t vcpu, uint32_t field, uint64_t v) {
  if (hv_vmx_vcpu_write_vmcs(vcpu, field, v)) {
    abort();
  }
}

/* desired control word constrained by hardware/hypervisor capabilities */
static uint64_t cap2ctrl(uint64_t cap, uint64_t ctrl) {
  return (ctrl | (cap & 0xffffffff)) & (cap >> 32);
}

void* vcpu_create_run(void* vthread) {
  struct vthread* vth = vthread;
  struct virtual_machine* vm = vth->vm;
  hv_vcpuid_t vcpu;
  if (hv_vcpu_create(&vcpu, HV_VCPU_DEFAULT)) {
    abort();
  }
  /* get hypervisor enforced capabilities of the machine, (see Intel docs) */
  uint64_t vmx_cap_pinbased, vmx_cap_procbased, vmx_cap_procbased2,
      vmx_cap_entry;
  if (hv_vmx_read_capability(HV_VMX_CAP_PINBASED, &vmx_cap_pinbased)) {
    abort();
  }
  if (hv_vmx_read_capability(HV_VMX_CAP_PROCBASED, &vmx_cap_procbased)) {
    abort();
  }
  if (hv_vmx_read_capability(HV_VMX_CAP_PROCBASED2, &vmx_cap_procbased2)) {
    abort();
  }
  if (hv_vmx_read_capability(HV_VMX_CAP_ENTRY, &vmx_cap_entry)) {
    abort();
  }
  /* vCPU setup */
#define VMCS_PRI_PROC_BASED_CTLS_HLT (1 << 7)
#define VMCS_PRI_PROC_BASED_CTLS_CR8_LOAD (1 << 19)
#define VMCS_PRI_PROC_BASED_CTLS_CR8_STORE (1 << 20)

  /* set VMCS control fields */
  wvmcs(vcpu, VMCS_CTRL_PIN_BASED, cap2ctrl(vmx_cap_pinbased, 0));
  wvmcs(vcpu, VMCS_CTRL_CPU_BASED,
        cap2ctrl(vmx_cap_procbased, VMCS_PRI_PROC_BASED_CTLS_HLT |
                                        VMCS_PRI_PROC_BASED_CTLS_CR8_LOAD |
                                        VMCS_PRI_PROC_BASED_CTLS_CR8_STORE));
  wvmcs(vcpu, VMCS_CTRL_CPU_BASED2, cap2ctrl(vmx_cap_procbased2, 0));
  wvmcs(vcpu, VMCS_CTRL_VMENTRY_CONTROLS, cap2ctrl(vmx_cap_entry, 0));
  wvmcs(vcpu, VMCS_CTRL_EXC_BITMAP, 0xffffffff);
  wvmcs(vcpu, VMCS_CTRL_CR0_MASK, 0x60000000);
  wvmcs(vcpu, VMCS_CTRL_CR0_SHADOW, 0);
  wvmcs(vcpu, VMCS_CTRL_CR4_MASK, 0);
  wvmcs(vcpu, VMCS_CTRL_CR4_SHADOW, 0);
  /* set VMCS guest state fields */
  wvmcs(vcpu, VMCS_GUEST_CS, 0);
  wvmcs(vcpu, VMCS_GUEST_CS_LIMIT, 0xffff);
  wvmcs(vcpu, VMCS_GUEST_CS_AR, 0x9b);
  wvmcs(vcpu, VMCS_GUEST_CS_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_DS, 0);
  wvmcs(vcpu, VMCS_GUEST_DS_LIMIT, 0xffff);
  wvmcs(vcpu, VMCS_GUEST_DS_AR, 0x93);
  wvmcs(vcpu, VMCS_GUEST_DS_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_ES, 0);
  wvmcs(vcpu, VMCS_GUEST_ES_LIMIT, 0xffff);
  wvmcs(vcpu, VMCS_GUEST_ES_AR, 0x93);
  wvmcs(vcpu, VMCS_GUEST_ES_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_FS, 0);
  wvmcs(vcpu, VMCS_GUEST_FS_LIMIT, 0xffff);
  wvmcs(vcpu, VMCS_GUEST_FS_AR, 0x93);
  wvmcs(vcpu, VMCS_GUEST_FS_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_GS, 0);
  wvmcs(vcpu, VMCS_GUEST_GS_LIMIT, 0xffff);
  wvmcs(vcpu, VMCS_GUEST_GS_AR, 0x93);
  wvmcs(vcpu, VMCS_GUEST_GS_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_SS, 0);
  wvmcs(vcpu, VMCS_GUEST_SS_LIMIT, 0xffff);
  wvmcs(vcpu, VMCS_GUEST_SS_AR, 0x93);
  wvmcs(vcpu, VMCS_GUEST_SS_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_LDTR, 0);
  wvmcs(vcpu, VMCS_GUEST_LDTR_LIMIT, 0);
  wvmcs(vcpu, VMCS_GUEST_LDTR_AR, 0x10000);
  wvmcs(vcpu, VMCS_GUEST_LDTR_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_TR, 0);
  wvmcs(vcpu, VMCS_GUEST_TR_LIMIT, 0);
  wvmcs(vcpu, VMCS_GUEST_TR_AR, 0x83);
  wvmcs(vcpu, VMCS_GUEST_TR_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_GDTR_LIMIT, 0);
  wvmcs(vcpu, VMCS_GUEST_GDTR_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_IDTR_LIMIT, 0);
  wvmcs(vcpu, VMCS_GUEST_IDTR_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_CR0, 0x20);
  wvmcs(vcpu, VMCS_GUEST_CR3, 0x0);
  wvmcs(vcpu, VMCS_GUEST_CR4, 0x2000);

  wreg(vcpu, HV_X86_RIP, vth->entry - vm->vm_mem);
  wreg(vcpu, HV_X86_RFLAGS, 0x2);
  wreg(vcpu, HV_X86_RSP, vm->mem_size - 1);

  for (int i = 0; i < 3; i += 1) {
    hv_return_t err = hv_vcpu_run(vcpu);
    if (err) {
      print_red("hv_vcpu_run: err = %llx\n", err);
      abort();
    }
    uint64_t exit_reason = rvmcs(vcpu, VMCS_RO_EXIT_REASON);
    // uint64_t exit_instr_len = rvmcs(vcpu, VMCS_RO_VMEXIT_INSTR_LEN);
    // printf("exit_reason = %llu, len=%llu\n", exit_reason, exit_instr_len);
    // uint64_t bp = rreg(vcpu, HV_X86_RBP);
    // uint64_t sp = rreg(vcpu, HV_X86_RSP);
    // uint64_t ip = rreg(vcpu, HV_X86_RIP);
    // uint64_t rax = rreg(vcpu, HV_X86_RAX);
    // printf("bp = %llx, sp=%llx, ip=%lld, rax=%llx\n", bp, sp, ip, rax);
    // print_payload(vm->vm_mem + ip, 20);
    printf("exit_reason = \n");
    if (exit_reason == VMX_REASON_HLT) {
      print_red("VMX_REASON_HLT\n");
      break;
    } else if (exit_reason == VMX_REASON_IRQ) {
      printf("VMX_REASON_IRQ\n");
    } else if (exit_reason == VMX_REASON_EPT_VIOLATION) {
      printf("VMX_REASON_EPT_VIOLATION\n");
    } else {
      printf("other unhandled VMEXIT (%llx)\n", exit_reason);
      break;
    }
  };
  if (hv_vcpu_destroy(vcpu)) {
    abort();
  }
  hv_vm_unmap(0, vm->mem_size);
  return NULL;
}

struct vthread* vthread_create(struct virtual_machine const* vm, void* entry,
                               void* arg) {
  struct vthread* vth =
      (struct vthread*)malloc(sizeof(struct vthread));  // memory leak!
  vth->entry = entry;
  vth->vm = (struct virtual_machine*)vm;
  pthread_t pth;
  pthread_create(&pth, NULL, vcpu_create_run, vth);
  vth->pth = pth;
  return vth;
}

void vthread_join(struct vthread* vth, void** retval_loc) {
  pthread_join(vth->pth, retval_loc);
}