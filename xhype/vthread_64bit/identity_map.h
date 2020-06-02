#ifndef __IDENTITY_MAP_H__
#define __IDENTITY_MAP_H__

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
bool setup_identity_map();

void setup_pml4();
bool map_address(uint64_t addr);

#ifdef __cplusplus
}
#endif

#endif