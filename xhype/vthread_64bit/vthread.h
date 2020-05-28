#ifndef __VTHREAD_H__
#define __VTHREAD_H__

#include <pthread.h>
#include <stdbool.h>
#include <stdint.h>
#include <unistd.h>
void vth_init();

struct vthread {
  pthread_t pth;
  uint8_t* entry;
};

struct vthread* vthread_create(void* entry, void* arg);

void vthread_join(struct vthread* vth, void** retval_loc);

#endif