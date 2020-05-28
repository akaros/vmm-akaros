#include <stdio.h>
#include <unistd.h>

#include "vthread.h"

int main() {
  printf("pid=%d\n", getpid());
  vth_init();

  struct vthread* vth = vthread_create(NULL, NULL);
  vthread_join(vth, NULL);

  return 0;
}