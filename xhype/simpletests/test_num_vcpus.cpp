#include <Hypervisor/hv.h>
#include <pthread.h>

#include <vector>

#define GUARD(x, r)                   \
  {                                   \
    uint64_t ret = (uint64_t)(x);     \
    if (ret != r) {                   \
      printf("%s = %llx\n", #x, ret); \
      exit(1);                        \
    }                                 \
  }

pthread_mutex_t mutex = PTHREAD_MUTEX_INITIALIZER;
pthread_cond_t cond = PTHREAD_COND_INITIALIZER;
bool should_quit = false;
int total_count = 0;

void* create_vcpu(void* args) {
  uint64_t thread_id = (uint64_t)(*(pthread_t*)args);
  hv_vcpuid_t cpuid;
  pthread_mutex_lock(&mutex);
  if (!should_quit) {
    if (hv_vcpu_create(&cpuid, HV_VCPU_DEFAULT) == HV_SUCCESS) {
      // printf("thread %lld: created vcpu %d\n", thread_id, cpuid);
      total_count += 1;
      while (!should_quit) {
        pthread_cond_wait(&cond, &mutex);
      }
      GUARD(hv_vcpu_destroy(cpuid), HV_SUCCESS);
      pthread_cond_signal(&cond);
    } else {
      printf(
          "thread %lld: maximal number of vcpus a single process can create =  "
          "%d\n",
          thread_id, total_count);
      should_quit = true;
      pthread_cond_signal(&cond);
    }
  }
  pthread_mutex_unlock(&mutex);
  return NULL;
}

void max_total_num_of_vcpus() {
  std::vector<pthread_t> threads;
  bool should_continue = true;
  while (should_continue) {
    pthread_t th;
    threads.push_back(th);
    pthread_create(&threads.back(), NULL, create_vcpu, &threads.back());
    pthread_mutex_lock(&mutex);
    should_continue = !should_quit;
    pthread_mutex_unlock(&mutex);
  }
  for (auto th : threads) {
    pthread_join(th, NULL);
  }
}

void max_num_vcpus_of_a_single_thread() {
  std::vector<hv_vcpuid_t> vcpus;
  int i = 0;
  while (true) {
    hv_vcpuid_t newcpu;
    if (hv_vcpu_create(&newcpu, HV_VCPU_DEFAULT) == HV_SUCCESS) {
      i += 1;
      vcpus.push_back(newcpu);
    } else {
      printf("maximal number of vcpus a single thread can create = %d\n", i);
      for (auto vcpu : vcpus) {
        GUARD(hv_vcpu_destroy(vcpu), HV_SUCCESS);
      }
      vcpus.clear();
      break;
    }
  }
}

int main() {
  GUARD(hv_vm_create(HV_VM_DEFAULT), HV_SUCCESS);
  max_num_vcpus_of_a_single_thread();  // 1
  max_total_num_of_vcpus();            // 32
  hv_vm_destroy();
}