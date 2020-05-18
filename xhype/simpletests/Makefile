src = $(wildcard *.c)
obj = $(src:.c=.o)

CC=clang
CFLAGS=-g
LDFLAGS=-framework Hypervisor

test_two_vms_in_main_thread: test_two_vms_in_main_thread.o utils.o
	$(CC) -o $@ $^ $(CFLAGS) $(LDFLAGS) 

test_two_vms_in_two_threads: test_two_vms_in_two_threads.o utils.o
	$(CC) -o $@ $^ $(CFLAGS) $(LDFLAGS) 

test_two_vms_in_two_processes: test_two_vms_in_two_processes.o utils.o
	$(CC) -o $@ $^ $(CFLAGS) $(LDFLAGS) 

test_two_mmap: test_two_mmap.o utils.o
	$(CC) -o $@ $^ $(CFLAGS) $(LDFLAGS) 

.PHONY: clean
clean:
	rm -f $(obj) test_two_vms_in_main_thread test_two_vms_in_two_threads test_two_vms_in_two_processes test_two_mmap