#ifndef __VMEXIT_QUAL_H__
#define __VMEXIT_QUAL_H__

#include <stdint.h>

enum {
  VMEXIT_QUAL_CR_TYPE_MOVETO = 0,
  VMEXIT_QUAL_CR_TYPE_MOVEFROM = 1,
  VMEXIT_QUAL_CR_TYPE_CLTS = 2,
  VMEXIT_QUAL_CR_TYPE_LMSW = 3
};

struct vmexit_qual_cr {
  uint64_t cr_num : 4;
  uint64_t type : 2;
  uint64_t lmsw_type : 1;
  uint64_t resv7 : 1;
  uint64_t g_reg : 4;
  uint64_t resv12 : 4;
  uint64_t lmsw_data : 16;
  uint64_t resv32 : 32;
};

#endif