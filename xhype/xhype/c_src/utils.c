/* SPDX-License-Identifier: GPL-2.0-only */

#include <mach/mach_time.h>
#include <mach/kern_return.h>
#include <stdbool.h>

bool mach_timebase(uint32_t* numer, uint32_t* denom) {
    mach_timebase_info_data_t base;
    kern_return_t ret = mach_timebase_info(&base);
    *numer = base.numer;
    *denom = base.denom;
    return ret == KERN_SUCCESS;
}