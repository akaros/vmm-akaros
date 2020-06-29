/* SPDX-License-Identifier: GPL-2.0-only */
#include <stdint.h>

void cpuid(uint32_t ieax, uint32_t iecx, uint32_t *eaxp, uint32_t *ebxp,
           uint32_t *ecxp, uint32_t *edxp) {
  asm volatile("cpuid"
               : "=a"(*eaxp), "=b"(*ebxp), "=c"(*ecxp), "=d"(*edxp)
               : "a"(ieax), "c"(iecx));
}