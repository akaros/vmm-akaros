#include "vthread.h"

#include <Hypervisor/hv.h>
#include <mach/mach.h>
#include <mach/mach_vm.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/stat.h>

#include "utils.h"
#include "vmexit_qual.h"
#include "x86.h"

mach_vm_address_t h_text_addr;
mach_vm_address_t h_data1_addr;
mach_vm_address_t h_data2_addr;

mach_vm_size_t h_text_size;
mach_vm_size_t h_data1_size;
mach_vm_size_t h_data2_size;

uint8_t* bootstrap_mem;

struct gdte {
  uint64_t low_limit : 16;
  uint64_t low_base : 16;
  uint64_t mid_base : 8;
  uint64_t access : 8;
  uint64_t granularity : 8;
  uint64_t high_base : 8;
};

struct gdtr {
  uint64_t limit : 16;
  uint64_t base : 32;
};

struct gdte* gdt;
struct gdte* gdt64;

void vth_init() {
  mach_vm_size_t region_size;
  vm_region_basic_info_data_64_t info;
  mach_msg_type_number_t count = VM_REGION_BASIC_INFO_COUNT_64;
  mach_port_t object;

  mach_vm_address_t trial_addr = 1;
  GUARD(mach_vm_region(current_task(), &trial_addr, &region_size,
                       VM_REGION_BASIC_INFO, (vm_region_info_t)&info, &count,
                       &object),
        KERN_SUCCESS);
  h_text_addr = trial_addr;
  h_text_size = region_size;

  trial_addr += region_size;
  GUARD(mach_vm_region(current_task(), &trial_addr, &region_size,
                       VM_REGION_BASIC_INFO, (vm_region_info_t)&info, &count,
                       &object),
        KERN_SUCCESS);
  h_data1_addr = trial_addr;
  h_data1_size = region_size;

  trial_addr += region_size;
  GUARD(mach_vm_region(current_task(), &trial_addr, &region_size,
                       VM_REGION_BASIC_INFO, (vm_region_info_t)&info, &count,
                       &object),
        KERN_SUCCESS);
  h_data2_addr = trial_addr;
  h_data2_size = region_size;

  printf(
      "host text: %llx, %lld page, data1: %llx, %lld page, data2: %llx, %lld "
      "page\n",
      h_text_addr, h_text_size >> 12, h_data1_addr, h_data1_size >> 12,
      h_data2_addr, h_data2_size >> 12);

  GUARD(hv_vm_create(HV_VM_DEFAULT), HV_SUCCESS);

  bootstrap_mem = valloc(5 * PAGE_SIZE);
  bzero(bootstrap_mem, 5 * PAGE_SIZE);
  // read the bootstrap to buffer
  char* filename = "longmode";
  struct stat fstat;
  stat(filename, &fstat);
  GUARD(fstat.st_size < PAGE_SIZE, true);
  FILE* f = fopen(filename, "rb");
  GUARD(fread(bootstrap_mem, fstat.st_size, 1, f), 1);
  fclose(f);
  //   bootstrap_mem[0] = 0xf4;
  GUARD(hv_vm_map(bootstrap_mem, 0, 5 * PAGE_SIZE,
                  HV_MEMORY_READ | HV_MEMORY_WRITE | HV_MEMORY_EXEC),
        KERN_SUCCESS);
  gdt = (struct gdte*)(bootstrap_mem + (fstat.st_size + 8) / 8 * 8);
  gdt[2].low_limit = 0xffff;
  gdt[2].access = 0x9a;
  gdt[2].granularity = 0xcf;
  gdt[3].low_limit = 0xffffU;
  gdt[3].access = 0x92U;
  gdt[3].granularity = 0xcfU;
  // print_payload(gdt, 48);

  gdt64 = (struct gdte*)(bootstrap_mem + PAGE_SIZE - 4 * sizeof(struct gdte));
  bzero(gdt64, 4 * sizeof(struct gdte));
  gdt64[0].low_limit = 0xffff;
  gdt64[0].granularity = 1;
  gdt64[1].access = 0x9a;
  gdt64[1].granularity = 0xaf;
  gdt64[2].access = 0x92;
  gdt64[2].granularity = 0;
  struct gdtr* gdtreg = (struct gdtr*)&gdt64[3];
  gdtreg->base = PAGE_SIZE - 4 * sizeof(struct gdte);
  gdtreg->limit = 3 * 8 - 1;
  // print_payload(gdt64, 4 * sizeof(struct gdte));
  //   printf("%d\n", sizeof(*gdt));
}

void vcpu_unpaged_protected_mode(hv_vcpuid_t vcpu) {
  wvmcs(vcpu, VMCS_GUEST_CS, 0x10);
  wvmcs(vcpu, VMCS_GUEST_CS_AR, 0xc09b);
  wvmcs(vcpu, VMCS_GUEST_CS_LIMIT, 0xffffffff);
  wvmcs(vcpu, VMCS_GUEST_CS_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_DS, 0x18);
  wvmcs(vcpu, VMCS_GUEST_DS_AR, 0xc093);
  wvmcs(vcpu, VMCS_GUEST_DS_LIMIT, 0xffffffff);
  wvmcs(vcpu, VMCS_GUEST_DS_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_ES, 0x18);
  wvmcs(vcpu, VMCS_GUEST_ES_AR, 0xc093);
  wvmcs(vcpu, VMCS_GUEST_ES_LIMIT, 0xffffffff);
  wvmcs(vcpu, VMCS_GUEST_ES_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_FS, 0);
  wvmcs(vcpu, VMCS_GUEST_FS_AR, 0x93);
  wvmcs(vcpu, VMCS_GUEST_FS_LIMIT, 0xffff);
  wvmcs(vcpu, VMCS_GUEST_FS_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_GS, 0);
  wvmcs(vcpu, VMCS_GUEST_GS_AR, 0x93);
  wvmcs(vcpu, VMCS_GUEST_GS_LIMIT, 0xffff);
  wvmcs(vcpu, VMCS_GUEST_GS_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_SS, 0x18);
  wvmcs(vcpu, VMCS_GUEST_SS_AR, 0xc093);
  wvmcs(vcpu, VMCS_GUEST_SS_LIMIT, 0xffffffff);
  wvmcs(vcpu, VMCS_GUEST_SS_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_LDTR, 0);
  wvmcs(vcpu, VMCS_GUEST_LDTR_AR, 0x82);
  wvmcs(vcpu, VMCS_GUEST_LDTR_LIMIT, 0xffff);
  wvmcs(vcpu, VMCS_GUEST_LDTR_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_TR, 0);
  wvmcs(vcpu, VMCS_GUEST_TR_AR, 0x8b);  // AR_TYPE_BUSY_64_TSS
  wvmcs(vcpu, VMCS_GUEST_TR_LIMIT, 0);
  wvmcs(vcpu, VMCS_GUEST_TR_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_GDTR_LIMIT, 0x1f);
  wvmcs(vcpu, VMCS_GUEST_GDTR_BASE, (uint64_t)gdt - (uint64_t)bootstrap_mem);

  wvmcs(vcpu, VMCS_GUEST_IDTR_LIMIT, 0);
  wvmcs(vcpu, VMCS_GUEST_IDTR_BASE, 0);

  uint64_t cap_pin, cap_cpu, cap_cpu2, cap_entry;
  GUARD(hv_vmx_read_capability(HV_VMX_CAP_PINBASED, &cap_pin), HV_SUCCESS);
  GUARD(hv_vmx_read_capability(HV_VMX_CAP_PROCBASED, &cap_cpu), HV_SUCCESS);
  GUARD(hv_vmx_read_capability(HV_VMX_CAP_PROCBASED2, &cap_cpu2), HV_SUCCESS);
  GUARD(hv_vmx_read_capability(HV_VMX_CAP_ENTRY, &cap_entry), HV_SUCCESS);
  wvmcs(vcpu, VMCS_CTRL_PIN_BASED, cap2ctrl(cap_pin, 0));
  wvmcs(vcpu, VMCS_CTRL_CPU_BASED,
        cap2ctrl(cap_cpu,
                 CPU_BASED_HLT | CPU_BASED_CR8_LOAD | CPU_BASED_CR8_STORE));
  wvmcs(vcpu, VMCS_CTRL_CPU_BASED2, cap2ctrl(cap_cpu2, CPU_BASED2_RDTSCP));
  wvmcs(vcpu, VMCS_CTRL_VMENTRY_CONTROLS, cap2ctrl(cap_entry, 0));

  wvmcs(vcpu, VMCS_CTRL_EXC_BITMAP, 0x40000);

  uint64_t cr0 = X86_CR0_NE | X86_CR0_ET | X86_CR0_PE;
  wvmcs(vcpu, VMCS_GUEST_CR0, cr0);
  wvmcs(vcpu, VMCS_CTRL_CR0_MASK, 0xe0000031);
  wvmcs(vcpu, VMCS_CTRL_CR0_SHADOW, cr0);

  //   GUARD(hv_vcpu_enable_native_msr(vcpu, MSR_EFER, true), HV_SUCCESS);

  uint64_t cr4 = X86_CR4_VMXE;
  wvmcs(vcpu, VMCS_GUEST_CR4, cr4);
  wvmcs(vcpu, VMCS_CTRL_CR4_MASK, 0x2000);
  wvmcs(vcpu, VMCS_CTRL_CR4_SHADOW, cr4);
}

uint64_t vmx_get_guest_reg(int vcpu, int ident) {
  switch (ident) {
    case 0:
      return (rreg(vcpu, HV_X86_RAX));
    case 1:
      return (rreg(vcpu, HV_X86_RCX));
    case 2:
      return (rreg(vcpu, HV_X86_RDX));
    case 3:
      return (rreg(vcpu, HV_X86_RBX));
    case 4:
      return (rvmcs(vcpu, VMCS_GUEST_RSP));
    case 5:
      return (rreg(vcpu, HV_X86_RBP));
    case 6:
      return (rreg(vcpu, HV_X86_RSI));
    case 7:
      return (rreg(vcpu, HV_X86_RDI));
    case 8:
      return (rreg(vcpu, HV_X86_R8));
    case 9:
      return (rreg(vcpu, HV_X86_R9));
    case 10:
      return (rreg(vcpu, HV_X86_R10));
    case 11:
      return (rreg(vcpu, HV_X86_R11));
    case 12:
      return (rreg(vcpu, HV_X86_R12));
    case 13:
      return (rreg(vcpu, HV_X86_R13));
    case 14:
      return (rreg(vcpu, HV_X86_R14));
    case 15:
      return (rreg(vcpu, HV_X86_R15));
    default:
      abort();
  }
}

void* vcpu_create_run(void* arg_vth) {
  struct vthread* vth = (struct vthread*)arg_vth;
  hv_vcpuid_t vcpu;
  GUARD(hv_vcpu_create(&vcpu, HV_VCPU_DEFAULT), HV_SUCCESS);

  GUARD(hv_vcpu_enable_native_msr(vcpu, MSR_LSTAR, 1), HV_SUCCESS);
  GUARD(hv_vcpu_enable_native_msr(vcpu, MSR_CSTAR, 1), HV_SUCCESS);
  GUARD(hv_vcpu_enable_native_msr(vcpu, MSR_STAR, 1), HV_SUCCESS);
  GUARD(hv_vcpu_enable_native_msr(vcpu, MSR_SF_MASK, 1), HV_SUCCESS);
  GUARD(hv_vcpu_enable_native_msr(vcpu, MSR_KGSBASE, 1), HV_SUCCESS);
  GUARD(hv_vcpu_enable_native_msr(vcpu, MSR_GSBASE, 1), HV_SUCCESS);
  GUARD(hv_vcpu_enable_native_msr(vcpu, MSR_FSBASE, 1), HV_SUCCESS);
  GUARD(hv_vcpu_enable_native_msr(vcpu, MSR_SYSENTER_CS_MSR, 1), HV_SUCCESS);
  GUARD(hv_vcpu_enable_native_msr(vcpu, MSR_SYSENTER_ESP_MSR, 1), HV_SUCCESS);
  GUARD(hv_vcpu_enable_native_msr(vcpu, MSR_SYSENTER_EIP_MSR, 1), HV_SUCCESS);
  GUARD(hv_vcpu_enable_native_msr(vcpu, MSR_TSC, 1), HV_SUCCESS);
  GUARD(hv_vcpu_enable_native_msr(vcpu, MSR_IA32_TSC_AUX, 1), HV_SUCCESS);

  vcpu_unpaged_protected_mode(vcpu);

  wreg(vcpu, HV_X86_RIP, 0);
  wreg(vcpu, HV_X86_RFLAGS, 0x2);

  //   printf("bitmap msr=%llx, %llx\n", rvmcs(vcpu, vmcs_ctr))
  hvdump(vcpu);

  for (int i = 0; i < 10; i += 1) {
    printf("\n");
    hv_return_t err = hv_vcpu_run(vcpu);
    if (err) {
      print_red("hv_vcpu_run: err = %llx\n", err);
      hvdump(vcpu);
      abort();
    }
    uint64_t exit_reason = rvmcs(vcpu, VMCS_RO_EXIT_REASON);
    uint64_t exit_instr_len = rvmcs(vcpu, VMCS_RO_VMEXIT_INSTR_LEN);
    uint64_t qual = rvmcs(vcpu, VMCS_RO_EXIT_QUALIFIC);
    printf("exit_reason = %llu, len=%llu\n", exit_reason, exit_instr_len);
    uint64_t bp = rreg(vcpu, HV_X86_RBP);
    uint64_t sp = rreg(vcpu, HV_X86_RSP);
    uint64_t ip = rreg(vcpu, HV_X86_RIP);
    uint64_t rax = rreg(vcpu, HV_X86_RAX);
    uint64_t rbx = rreg(vcpu, HV_X86_RBX);
    uint64_t rcx = rreg(vcpu, HV_X86_RCX);
    uint64_t rdx = rreg(vcpu, HV_X86_RDX);
    uint64_t gla = rvmcs(vcpu, VMCS_RO_GUEST_LIN_ADDR);
    uint64_t gpa = rvmcs(vcpu, VMCS_GUEST_PHYSICAL_ADDRESS);
    uint64_t cr3 = rvmcs(vcpu, VMCS_GUEST_CR3);
    uint64_t efer_g = rvmcs(vcpu, VMCS_GUEST_IA32_EFER);
    printf(
        "cr3 = %llx, bp = 0x%llx, sp=0x%llx, ip=0x%llx, rax=0x%llx, "
        "rbx=0x%llx, rcx=0x%llx, efer = %llx\n",
        cr3, bp, sp, ip, rax, rbx, rcx, efer_g);
    printf("gla=0x%llx, gpa=0x%llx\n", gla, gpa);
    printf("instruction:\n");
    print_payload(bootstrap_mem + ip, 16);
    // printf("stack:\n");
    // uint8_t* guest_sp_h =
    //     vm_stack_h + (sp - (host_text_size + host_bss_size +
    //     host_data_size));
    // uint8_t* guest_stack_top_h = vm_stack_h + vm_stack_size;
    // print_payload(guest_sp_h, guest_stack_top_h - guest_sp_h);
    // print_payload((uint8_t*)vm_text_addr + (gla - VM_PHYS_TEXT), 2);
    printf("exit_reason = ");
    if (exit_reason == VMX_REASON_HLT) {
      print_red("VMX_REASON_HLT\n");
      break;
    } else if (exit_reason == VMX_REASON_IRQ) {
      printf("VMX_REASON_IRQ\n");
    } else if (exit_reason == VMX_REASON_EPT_VIOLATION) {
      printf("VMX_REASON_EPT_VIOLATION\n");
      print_ept_vio_qualifi(qual);
      continue;
    } else if (exit_reason == VMX_REASON_MOV_CR) {
      printf("VMX_REASON_MOV_CR\n");
      struct vmexit_qual_cr* qual_cr = (struct vmexit_qual_cr*)&qual;
      if (qual_cr->cr_num == 0) {
        if (qual_cr->type == VMEXIT_QUAL_CR_TYPE_MOVETO) {
          uint64_t regval = vmx_get_guest_reg(vcpu, qual_cr->g_reg);
          wvmcs(vcpu, VMCS_CTRL_CR0_SHADOW, regval);
          wvmcs(vcpu, VMCS_GUEST_CR0, regval);
          printf("update cr0 to %llx\n", regval);
          uint64_t efer = rvmcs(vcpu, VMCS_GUEST_IA32_EFER);
          if ((regval & X86_CR0_PG) && (efer & EFER_LME)) {
            printf("turn on lma\n");
            efer |= EFER_LMA;
            wvmcs(vcpu, VMCS_GUEST_IA32_EFER, efer);
            uint64_t ctrl_entry = rvmcs(vcpu, VMCS_CTRL_VMENTRY_CONTROLS);
            wvmcs(vcpu, VMCS_CTRL_VMENTRY_CONTROLS,
                  ctrl_entry | VMENTRY_GUEST_IA32E);
          } else {
          }
        } else {
          print_red("qual_cr->type = %llx\n", qual_cr->type);
          abort();
        }
      } else if (qual_cr->cr_num == 4) {
        if (qual_cr->type == VMEXIT_QUAL_CR_TYPE_MOVETO) {
          uint64_t regval = vmx_get_guest_reg(vcpu, qual_cr->g_reg);
          wvmcs(vcpu, VMCS_CTRL_CR4_SHADOW, regval);
          wvmcs(vcpu, VMCS_GUEST_CR4, regval);
          printf("update cr4 to %llx\n", regval);
          //   uint64_t efer = rvmcs(vcpu, VMCS_GUEST_IA32_EFER);
        } else {
          print_red("qual_cr->type = %llx\n", qual_cr->type);
          abort();
        }
      } else if (qual_cr->cr_num == 8) {
        print_red("access cr8\n");
        abort();
      }
      //   uint64_t reg_num = qual & 0xf;
      //   if (qual_cr->cr_num == 0) {
      //     if (qual)
      //     } else if (reg_num == 4) {
      //   } else if (reg_num == 8) {
      //   }
    } else if (exit_reason == VMX_REASON_RDMSR) {
      printf("VMX_REASON_RDMSR\n");
      if (rcx == MSR_EFER) {
        uint64_t efer_value = rvmcs(vcpu, VMCS_GUEST_IA32_EFER);
        uint32_t new_eax = (uint32_t)(efer_value & ~0Ul);
        uint32_t new_edx = (uint32_t)(efer_value >> 32);
        wreg(vcpu, HV_X86_RAX, new_eax);
        wreg(vcpu, HV_X86_RDX, new_edx);
        printf("return efer %llx to vm\n", efer_value);
      } else {
        printf("read unknow msr: %llx\n", rcx);
        break;
      }
    } else if (exit_reason == VMX_REASON_WRMSR) {
      printf("VMX_REASON_RDMSR\n");
      if (rcx == MSR_EFER) {
        uint64_t new_msr = ((uint64_t)rdx << 32) | rax;
        wvmcs(vcpu, VMCS_GUEST_IA32_EFER, new_msr);
        printf("write %llx to efer\n", new_msr);
      } else {
        printf("write unkown msr: %llx\n", rcx);
        break;
      }
    } else {
      printf("other unhandled VMEXIT (%lld)\n", exit_reason);
      break;
    }
    wvmcs(vcpu, VMCS_GUEST_RIP, ip + exit_instr_len);
  };
  return NULL;
}

// will call malloc
struct vthread* vthread_create(void* entry, void* arg) {
  struct vthread* vth = (struct vthread*)malloc(sizeof(struct vthread));
  vth->entry = entry;
  pthread_t pth;
  pthread_create(&pth, NULL, vcpu_create_run, entry);
  vth->pth = pth;
  return vth;
}

void vthread_join(struct vthread* vth, void** retval_loc) {
  pthread_join(vth->pth, retval_loc);
}