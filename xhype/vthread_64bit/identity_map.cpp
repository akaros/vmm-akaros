#include "identity_map.h"

#include <mach/mach.h>
#include <mach/mach_vm.h>

#include <map>
#include <vector>

#include "paging.h"
#include "utils.h"

#define FourKB (1 << 12)
#define TwoMB (1 << 21)
#define OneGB (1 << 30)

#define ONE_GB_SHIFT 30
#define TWO_MB_SHIFT 21
#define FOUR_KB_SHIFT 12

#define TwoMBAligned(x) (!(x & 0x1FFFFFULL))
#define OneGBAligned(x) (!(x & 0x3FFFFFFFULL))

struct region {
  mach_vm_address_t start;
  mach_vm_size_t size;
  vm_prot_t protection;
};

#define TEST 0

mach_vm_address_t guest_stack_addr;

bool setup_identity_map() {
  std::vector<struct region> regions_1gb;
  std::vector<struct region> regions_2mb;
  std::vector<struct region> regions_4kb;
  mach_vm_address_t addr = 1;
  mach_vm_size_t size = 0;
  vm_region_basic_info_data_64_t info;
  mach_msg_type_number_t count = VM_REGION_BASIC_INFO_COUNT_64;
  mach_port_t object;
  int estimated_pgs = 1;  // PML4
  while (true) {
    kern_return_t ret =
        mach_vm_region(current_task(), &addr, &size, VM_REGION_BASIC_INFO_64,
                       (vm_region_info_t)&info, &count, &object);
    if (ret == KERN_SUCCESS) {
      if (OneGBAligned(size) && OneGBAligned(addr)) {
        regions_1gb.push_back({addr, size, info.protection});
#if TEST
        printf("region start: %llx(1GB), size %llx(1GB), count %u, prot=%x\n",
               addr, size, count, info.protection);
#endif
        estimated_pgs += size >> ONE_GB_SHIFT;
      } else if (TwoMBAligned(size) && TwoMBAligned(addr)) {
        regions_2mb.push_back({addr, size, info.protection});
#if TEST
        printf("region start: %llx(2MB), size %llx(2MB), count %u, prot=%x\n",
               addr, size, count, info.protection);
#endif
        estimated_pgs += size >> (TWO_MB_SHIFT - 1);
      } else {
        regions_4kb.push_back({addr, size, info.protection});
#if TEST
        printf("region start: %llx(4KB), size %llx(4KB), count %u, prot=%x\n",
               addr, size, count, info.protection);
#endif
        estimated_pgs += size >> (FOUR_KB_SHIFT - 2);
      }
    } else {
      break;
    }
    addr += size;
  }
#if TEST
  std::map<uint64_t, uint64_t> h2g;
#endif
  int available_pg = 1;
  uint64_t guest_available_addr = OneGB;
  uint8_t* guest_paging_h = (uint8_t*)valloc(estimated_pgs * FourKB);
  bzero(guest_paging_h, estimated_pgs * FourKB);
  struct PML4E* pml4e = (struct PML4E*)guest_paging_h;
  for (const auto& region : regions_1gb) {
    GUARD(hv_vm_map((void*)region.start, guest_available_addr, region.size,
                    region.protection),
          HV_SUCCESS);
#if TEST
    h2g[region.start] = guest_available_addr;
#endif
    for (int i = 0; i < (region.size >> ONE_GB_SHIFT); i += 1) {
      uint64_t l_addr = region.start + i * OneGB;
      struct linear_addr_1gb_t* linear_addr =
          (struct linear_addr_1gb_t*)&l_addr;
      if (pml4e[linear_addr->pml4].pres == 0) {
        pml4e[linear_addr->pml4].pres = 1;
        pml4e[linear_addr->pml4].rw = 1;
        pml4e[linear_addr->pml4].pdpt_base = available_pg;
        available_pg += 1;
      }
      struct PDPTE_1GB* pdpte =
          (struct PDPTE_1GB*)(guest_paging_h +
                              pml4e[linear_addr->pml4].pdpt_base * FourKB);
      pdpte[linear_addr->pdpt].pres = 1;
      pdpte[linear_addr->pdpt].rw = 1;
      pdpte[linear_addr->pdpt].pg_base = guest_available_addr >> ONE_GB_SHIFT;
      pdpte[linear_addr->pdpt].ps = 1;

      guest_available_addr += OneGB;
    }
  }
  for (auto region : regions_2mb) {
    GUARD(hv_vm_map((void*)region.start, guest_available_addr, region.size,
                    region.protection),
          HV_SUCCESS);
#if TEST
    h2g[region.start] = guest_available_addr;
#endif
    for (int i = 0; i < (region.size >> TWO_MB_SHIFT); i += 1) {
      uint64_t l_addr = region.start + i * TwoMB;
      struct linear_addr_2mb_t* linear_addr =
          (struct linear_addr_2mb_t*)&l_addr;
      if (pml4e[linear_addr->pml4].pres == 0) {
        pml4e[linear_addr->pml4].pres = 1;
        pml4e[linear_addr->pml4].rw = 1;
        pml4e[linear_addr->pml4].pdpt_base = available_pg;
        available_pg += 1;
      }
      struct PDPTE* pdpte =
          (struct PDPTE*)(guest_paging_h +
                          pml4e[linear_addr->pml4].pdpt_base * FourKB);
      if (pdpte[linear_addr->pdpt].pres == 0) {
        pdpte[linear_addr->pdpt].pres = 1;
        pdpte[linear_addr->pdpt].rw = 1;
        pdpte[linear_addr->pdpt].pd_base = available_pg;
        available_pg += 1;
      }
      struct PDE_2MB* pde =
          (struct PDE_2MB*)(guest_paging_h +
                            pdpte[linear_addr->pdpt].pd_base * FourKB);
      pde[linear_addr->pd].pres = 1;
      pde[linear_addr->pd].rw = 1;
      pde[linear_addr->pd].ps = 1;
      pde[linear_addr->pd].pg_base = guest_available_addr >> TWO_MB_SHIFT;

      guest_available_addr += TwoMB;
    }
  }
  for (auto region : regions_4kb) {
    GUARD(hv_vm_map((void*)region.start, guest_available_addr, region.size,
                    region.protection),
          HV_SUCCESS);
#if TEST
    h2g[region.start] = guest_available_addr;
#endif
    for (int i = 0; i < (region.size >> FOUR_KB_SHIFT); i += 1) {
      uint64_t l_addr = region.start + i * FourKB;
      struct linear_addr_4kb_t* linear_addr =
          (struct linear_addr_4kb_t*)&l_addr;
      if (pml4e[linear_addr->pml4].pres == 0) {
        pml4e[linear_addr->pml4].pres = 1;
        pml4e[linear_addr->pml4].rw = 1;
        pml4e[linear_addr->pml4].pdpt_base = available_pg;
        available_pg += 1;
      }
      struct PDPTE* pdpte =
          (struct PDPTE*)(guest_paging_h +
                          pml4e[linear_addr->pml4].pdpt_base * FourKB);
      if (pdpte[linear_addr->pdpt].pres == 0) {
        pdpte[linear_addr->pdpt].pres = 1;
        pdpte[linear_addr->pdpt].rw = 1;
        pdpte[linear_addr->pdpt].pd_base = available_pg;
        available_pg += 1;
      }
      struct PDE* pde =
          (struct PDE*)(guest_paging_h +
                        pdpte[linear_addr->pdpt].pd_base * FourKB);
      if (pde[linear_addr->pd].pres == 0) {
        pde[linear_addr->pd].pres = 1;
        pde[linear_addr->pd].rw = 1;
        pde[linear_addr->pd].pt_base = available_pg;
        available_pg += 1;
      }
      struct PTE* pte =
          (struct PTE*)(guest_paging_h + pde[linear_addr->pd].pt_base * FourKB);
      pte[linear_addr->pt].pres = 1;
      pte[linear_addr->pt].rw = 1;
      pte[linear_addr->pt].pg_base = guest_available_addr >> FOUR_KB_SHIFT;

      guest_available_addr += FourKB;
    }
  }
#if TEST
  printf("paging use memory %d pages\n", available_pg);
#endif
  GUARD(hv_vm_map(guest_paging_h, 0, estimated_pgs * FourKB,
                  HV_MEMORY_READ | HV_MEMORY_WRITE),
        HV_SUCCESS);
#if TEST
  printf("test 1gb\n");
  for (const auto& region : regions_1gb) {
    printf("\n");
    GUARD(simulate_paging(0, guest_paging_h, region.start), h2g[region.start]);
  }
  printf("test 2mb\n");
  for (auto region : regions_2mb) {
    printf("\n");
    GUARD(simulate_paging(0, guest_paging_h, region.start), h2g[region.start]);
  }
  printf("test 4kb\n");
  for (auto region : regions_4kb) {
    printf("\n");
    GUARD(simulate_paging(0, guest_paging_h, region.start), h2g[region.start]);
  }
#endif
  return true;
}