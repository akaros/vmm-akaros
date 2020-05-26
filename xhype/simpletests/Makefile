src = $(wildcard *.c)
obj = $(src:.c=.o)

CC=clang
CXX=clang++
CFLAGS=-g
CXXFLAGS=-g -std=c++11
LDFLAGS=-framework Hypervisor

all=test_two_vms_in_main_thread test_two_vms_in_two_threads test_two_vms_in_two_processes test_two_mmap test_two_mmap_space test_num_vcpus test_num_space

test_two_vms_in_main_thread: test_two_vms_in_main_thread.o utils.o
	$(CC) -o $@ $^ $(CFLAGS) $(LDFLAGS) 

test_two_vms_in_two_threads: test_two_vms_in_two_threads.o utils.o
	$(CC) -o $@ $^ $(CFLAGS) $(LDFLAGS) 

test_two_vms_in_two_processes: test_two_vms_in_two_processes.o utils.o
	$(CC) -o $@ $^ $(CFLAGS) $(LDFLAGS) 

test_two_mmap: test_two_mmap.o utils.o
	$(CC) -o $@ $^ $(CFLAGS) $(LDFLAGS) 

test_two_mmap_space: test_two_mmap_space.o utils.o
	$(CC) -o $@ $^ $(CFLAGS) $(LDFLAGS) 

test_num_vcpus: test_num_vcpus.o utils.o 
	$(CXX) -o $@ $^ $(CXXFLAGS) $(LDFLAGS) 

test_num_space: test_num_space.o utils.o 
	$(CXX) -o $@ $^ $(CXXFLAGS) $(LDFLAGS) 

all: $(all)

.PHONY: clean
clean:
	rm -f $(obj) $(all)