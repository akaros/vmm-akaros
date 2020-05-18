#include <Hypervisor/hv.h>
#include <pthread.h>
#include <stdio.h>
#include <unistd.h>

#include "utils.h"

pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
pthread_cond_t cond = PTHREAD_COND_INITIALIZER;
int num = 0;

void* create_destroy_vm_with_lock(void* thread_num) {
  printf("thread %d waiting on lock...\n", *(int*)thread_num);
  pthread_mutex_lock(&lock);
  printf("thread %d get lock\n", *(int*)thread_num);
  hv_return_t err = hv_vm_create(HV_VM_DEFAULT);
  num += 1;
  pthread_cond_signal(&cond);
  if (err) {
    print_red("cannot create vm for thread %d, err=%x\n", *(int*)thread_num,
              err);
  } else {
    print_green("created vm for thread %d\n", *(int*)thread_num);
    while (num < 2) {
      printf("thread %d wait on cond...\n", *(int*)thread_num);
      pthread_cond_wait(&cond, &lock);
      printf("thread %d wake...\n", *(int*)thread_num);
    }
    printf("calling hv_vm_destroy in thread %d...\n", *(int*)thread_num);
    err = hv_vm_destroy();  // program fails here
    if (err) {
      print_red("cannot destroy vm for thead %d, err=%x\n", *(int*)thread_num,
                err);
    } else {
      print_green("destroyed vm for thread %d\n", *(int*)thread_num);
    }
  }
  pthread_mutex_unlock(&lock);
  return NULL;
}

void test_two_vms_in_two_threads() {
  printf("---start test_two_vms_in_two_threads---\n");
  int thread_nums[] = {1, 2};
  pthread_t threads[2];
  for (int i = 0; i < 2; i += 1) {
    pthread_create(&threads[i], NULL, create_destroy_vm_with_lock,
                   &thread_nums[i]);
  }
  for (int i = 0; i < 2; i += 1) {
    pthread_join(threads[i], NULL);
  }
  printf("---end test_two_vms_in_two_threads---\n");
}

int main() {
  test_two_vms_in_two_threads();
  return 0;
}