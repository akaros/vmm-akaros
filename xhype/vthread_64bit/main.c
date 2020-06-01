#include <assert.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

#include "change.h"
#include "utils.h"
#include "vthread.h"

#define TESTSTR "HAPPY"

int a = 1;
int b = 2;
size_t len;

void* calc_len(void) {
  len = strlen(TESTSTR);
  // __asm__("hlt\n"); // not necessary
  return NULL;
}

void* add_a(void) {
  b += a;
  // __asm__("hlt\n");
  return NULL;
}

uint8_t str_copy[32];

void* copy_str(void) {
  memcpy(str_copy, TESTSTR, strlen(TESTSTR) + 1);

  // __asm__("hlt\n");
  return NULL;
}

int main() {
  vth_init();

  struct vthread* vth1 = vthread_create(add_a, NULL);
  struct vthread* vth2 = vthread_create(copy_str, NULL);
  struct vthread* vth3 = vthread_create(calc_len, NULL);

  vthread_join(vth1, NULL);
  vthread_join(vth2, NULL);
  vthread_join(vth3, NULL);

  assert(b == 3);
  assert(len = strlen(TESTSTR));
  assert(memcmp(TESTSTR, str_copy, len) == 0);
  printf("b=%d, len=%d, str_copy=%s\n", b, len, str_copy);
  return 0;
}
