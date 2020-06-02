src = $(wildcard *.[c|s])
obj = $(src:.c=.o)

CC=clang
CXX=clang++
CFLAGS=-g 
CXXFLAGS=-std=c++11
LDFLAGS=-framework Hypervisor -L. -lchange

libchange.dylib: change.c
	$(CC) -o $@ -dynamiclib $(CFLAGS) $^

main: main.o vthread.o utils.o identity_map.o vthread_hlt.o libchange.dylib
	$(CXX) -o $@ $^ $(CFLAGS) $(LDFLAGS) 

.PHONY: clean
clean:
	rm -f *.o main *.dylib