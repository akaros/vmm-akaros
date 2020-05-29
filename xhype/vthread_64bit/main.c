#include <assert.h>
#include <stdio.h>
#include <unistd.h>

#include "utils.h"
#include "vthread.h"

int a = 1;
int b = 2;

int negate(int num) { return -num; }

void* add_a(void) {
  a = negate(a);
  __asm__("hlt\n");
  return NULL;
}

void* add_b(void) {
  b += 133;
  __asm__("hlt\n");
  return NULL;
}

int main() {
  vth_init();

  printf("a = %d\n", a);
  printf("b = %d\n", b);
  struct vthread* vth = vthread_create(add_b, NULL);
  struct vthread* vth2 = vthread_create(add_a, NULL);

  vthread_join(vth, NULL);
  vthread_join(vth2, NULL);
  printf("after vthread_join, a = %d, b = %d\n", a, b);
  assert(a == -1);
  assert(b == 135);
  return 0;
}