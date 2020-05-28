src = $(wildcard *.[c|s])
obj = $(src:.c=.o)

CC=clang
CFLAGS=-g 
LDFLAGS=-framework Hypervisor

main: main.o vthread.o utils.o
	$(CC) -o $@ $^ $(CFLAGS) $(LDFLAGS) 

.PHONY: clean
clean:
	rm -f *.o main 