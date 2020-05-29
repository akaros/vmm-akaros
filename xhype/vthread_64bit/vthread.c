#include "vthread.h"

#include <Hypervisor/hv.h>
#include <mach/mach.h>
#include <mach/mach_vm.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/stat.h>

#include "paging.h"
#include "utils.h"
#include "vmexit_qual.h"
#include "x86.h"

mach_vm_address_t h_text_addr;
mach_vm_address_t h_data1_addr;
mach_vm_address_t h_data2_addr;

mach_vm_size_t h_text_size;
mach_vm_size_t h_data1_size;
mach_vm_size_t h_data2_size;

uint8_t* guest_identity_paging;

// the following codes are not necessary anymore
// struct gdte {
//   uint64_t low_limit : 16;
//   uint64_t low_base : 16;
//   uint64_t mid_base : 8;
//   uint64_t access : 8;
//   uint64_t granularity : 8;
//   uint64_t high_base : 8;
// };

// struct gdtr {
//   uint64_t limit : 16;
//   uint64_t base : 32;
// };

// struct gdte* gdt;
// struct gdte* gdt64;

void vth_init() {
  // use mach_vm_region() to get the starting address of current process's text
  // and data
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

  // do an identity map from the guest's physical address to the host's virtual
  // address
  GUARD(hv_vm_map((uint8_t*)h_text_addr, h_text_addr, h_text_size,
                  HV_MEMORY_READ | HV_MEMORY_EXEC),
        HV_SUCCESS);
  GUARD(hv_vm_map((uint8_t*)h_data1_addr, h_data1_addr, h_data1_size,
                  HV_MEMORY_READ | HV_MEMORY_WRITE),
        HV_SUCCESS);
  GUARD(hv_vm_map((uint8_t*)h_data2_addr, h_data2_addr, h_data2_size,
                  HV_MEMORY_READ | HV_MEMORY_WRITE),
        HV_SUCCESS);

  // the following codes are not necessary any more.

  // bootstrap_mem = valloc(5 * PAGE_SIZE);
  // bzero(bootstrap_mem, 5 * PAGE_SIZE);

  // read the bootstrap to buffer
  // char* filename = "longmode";
  // struct stat fstat;
  // stat(filename, &fstat);
  // GUARD(fstat.st_size < PAGE_SIZE, true);
  // FILE* f = fopen(filename, "rb");
  // GUARD(fread(bootstrap_mem, fstat.st_size, 1, f), 1);
  // fclose(f);

  // GUARD(hv_vm_map(bootstrap_mem, 0, 5 * PAGE_SIZE,
  //                 HV_MEMORY_READ | HV_MEMORY_WRITE | HV_MEMORY_EXEC),
  //       KERN_SUCCESS);
  // gdt = (struct gdte*)(bootstrap_mem + (fstat.st_size + 8) / 8 * 8);
  // gdt[2].low_limit = 0xffff;
  // gdt[2].access = 0x9a;
  // gdt[2].granularity = 0xcf;
  // gdt[3].low_limit = 0xffffU;
  // gdt[3].access = 0x92U;
  // gdt[3].granularity = 0xcfU;

  // gdt64 = (struct gdte*)(bootstrap_mem + PAGE_SIZE - 4 * sizeof(struct
  // gdte)); bzero(gdt64, 4 * sizeof(struct gdte)); gdt64[0].low_limit = 0xffff;
  // gdt64[0].granularity = 1;
  // gdt64[1].access = 0x9a;
  // gdt64[1].granularity = 0xaf;
  // gdt64[2].access = 0x92;
  // gdt64[2].granularity = 0;
  // struct gdtr* gdtreg = (struct gdtr*)&gdt64[3];
  // gdtreg->base = PAGE_SIZE - 4 * sizeof(struct gdte);
  // gdtreg->limit = 3 * 8 - 1;

  // page tables for the guest, we place page tables at the begining of the
  // guest's physical address space, i.e., at physical address 0;
  guest_identity_paging = valloc((1 + 512) * PAGE_SIZE);
  GUARD(hv_vm_map(guest_identity_paging, 0, (1 + 512) * PAGE_SIZE,
                  HV_MEMORY_READ | HV_MEMORY_WRITE | HV_MEMORY_EXEC),
        HV_SUCCESS);

  // settting up the page tables for the guest. Here we use 1GB pages for the
  // guest.
  // PML4 is placed at guest's physicall address 0x200_000
  // We also have 512 PDPTs placed at guest's physical address 0x0 to 0x1ff_000
  // respectively.
  struct PML4E* pml4e =
      (struct PML4E*)(guest_identity_paging + 512 * PAGE_SIZE);
  for (int i = 0; i < 512; i += 1) {
    pml4e[i].pres = 1;
    pml4e[i].rw = 1;
    pml4e[i].pdpt_base = i;
    struct PDPTE_1GB* pdpte =
        (struct PDPTE_1GB*)(guest_identity_paging + i * PAGE_SIZE);
    for (int j = 0; j < 512; j += 1) {
      pdpte[j].pres = 1;
      pdpte[j].rw = 1;
      pdpte[j].ps = 1;  // use 1GB page
      pdpte[j].pg_base = (i << 9) + j;
    }
  }
}

void vcpu_long_mode(hv_vcpuid_t vcpu) {
  // setttup segment registers
  wvmcs(vcpu, VMCS_GUEST_CS, 0x10);
  wvmcs(vcpu, VMCS_GUEST_CS_AR, 0xa09b);  // Granularity, 64 bits flag
  wvmcs(vcpu, VMCS_GUEST_CS_LIMIT, 0xffffffff);
  wvmcs(vcpu, VMCS_GUEST_CS_BASE, 0x0);

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
  wvmcs(vcpu, VMCS_GUEST_TR_AR, 0x8b);
  wvmcs(vcpu, VMCS_GUEST_TR_LIMIT, 0);
  wvmcs(vcpu, VMCS_GUEST_TR_BASE, 0);

  wvmcs(vcpu, VMCS_GUEST_GDTR_LIMIT, 0x17);
  wvmcs(vcpu, VMCS_GUEST_GDTR_BASE, 0xfe0);

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
  wvmcs(vcpu, VMCS_CTRL_VMENTRY_CONTROLS,
        cap2ctrl(cap_entry, VMENTRY_GUEST_IA32E));  // indicate that the guest
                                                    // will be in 64bit mode

  wvmcs(vcpu, VMCS_CTRL_EXC_BITMAP, 0x40000);

  uint64_t cr0 = X86_CR0_NE | X86_CR0_ET | X86_CR0_PE;  // turn on protection
  cr0 |= X86_CR0_PG;                                    // turn on pageing
  wvmcs(vcpu, VMCS_GUEST_CR0, cr0);
  wvmcs(vcpu, VMCS_CTRL_CR0_MASK, 0xe0000031);
  wvmcs(vcpu, VMCS_CTRL_CR0_SHADOW, cr0);

  // guest's physical address to its PML4
  wvmcs(vcpu, VMCS_GUEST_CR3, 512 * PAGE_SIZE);

  uint64_t cr4 = X86_CR4_VMXE;
  cr4 |= X86_CR4_PAE;  // turn on 64bit paging
  wvmcs(vcpu, VMCS_GUEST_CR4, cr4);
  // make the guest unable to find it running in a virtual machine
  wvmcs(vcpu, VMCS_CTRL_CR4_MASK, X86_CR4_VMXE);
  wvmcs(vcpu, VMCS_CTRL_CR4_SHADOW, 0);

  uint64_t efer = 0;
  efer |= EFER_LME | EFER_LMA;  // turn on 64bit paging
  wvmcs(vcpu, VMCS_GUEST_IA32_EFER, efer);
}

// Intel manuel, Volume 3, table 27-3
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

  // the following codes are crucial for turning 64bit for the guest.
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

  vcpu_long_mode(vcpu);

  // allocate memory for the virutl thread's stack
  size_t vth_stack_size = 8 * PAGE_SIZE;
  mach_vm_address_t vth_stack;

  mach_vm_allocate(mach_task_self(), &vth_stack, vth_stack_size,
                   VM_FLAGS_ANYWHERE);
  // uint8_t* vth_stack = valloc(vth_stack_size);
  GUARD(hv_vm_map((void*)vth_stack, vth_stack, vth_stack_size,
                  HV_MEMORY_READ | HV_MEMORY_WRITE),
        HV_SUCCESS);
  printf("vth_stack top = 0x%llx\n", vth_stack + vth_stack_size - 1);
  // printf("vth-entry = %p\n", vth->entry);
  // set the guest's rip to the starting position of the code entry
  wreg(vcpu, HV_X86_RIP, (uint64_t)(vth->entry));
  wreg(vcpu, HV_X86_RFLAGS, 0x2);
  // set the guest's rsp to the top address of its stack
  wreg(vcpu, HV_X86_RSP, (uint64_t)(vth_stack + vth_stack_size));

  // temporarily limit number of iterations to 10
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
    print_payload((void*)ip, 16);
    printf("stack:\n");
    print_payload((void*)sp, vth_stack + vth_stack_size - sp);

    printf("exit_reason = ");
    if (exit_reason == VMX_REASON_HLT) {
      print_red("VMX_REASON_HLT\n");
      break;
    } else if (exit_reason == VMX_REASON_IRQ) {
      printf("VMX_REASON_IRQ\n");
      continue;
    } else if (exit_reason == VMX_REASON_EPT_VIOLATION) {
      printf("VMX_REASON_EPT_VIOLATION\n");
      print_ept_vio_qualifi(qual);
      continue;
    } else if (exit_reason == VMX_REASON_MOV_CR) {
      printf("VMX_REASON_MOV_CR\n");
      // the host will simulate the cr access for the host
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
        } else {
          print_red("qual_cr->type = %llx\n", qual_cr->type);
          abort();
        }
      } else if (qual_cr->cr_num == 8) {
        print_red("access cr8\n");
        abort();
      }
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
    // advance the instrunction pointer by exit_instr_len, since the host have
    // done the instruction for the guest
    wvmcs(vcpu, VMCS_GUEST_RIP, ip + exit_instr_len);
  };
  hvdump(vcpu);
  GUARD(hv_vcpu_destroy(vcpu), HV_SUCCESS);
  mach_vm_deallocate(mach_task_self(), vth_stack, vth_stack_size);
  return NULL;
}

// will call malloc
struct vthread* vthread_create(void* entry, void* arg) {
  struct vthread* vth = (struct vthread*)malloc(sizeof(struct vthread));
  vth->entry = entry;
  pthread_t pth;
  pthread_create(&pth, NULL, vcpu_create_run, vth);
  vth->pth = pth;
  return vth;
}

void vthread_join(struct vthread* vth, void** retval_loc) {
  pthread_join(vth->pth, retval_loc);
}