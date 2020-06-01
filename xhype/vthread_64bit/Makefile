src = $(wildcard *.[c|s])
obj = $(src:.c=.o)

CC=clang
CXX=clang++
CFLAGS=-g 
CXXFLAGS=-std=c++11
LDFLAGS=-framework Hypervisor

main: main.o vthread.o utils.o identity_map.o vthread_hlt.o
	$(CXX) -o $@ $^ $(CFLAGS) $(LDFLAGS) 

.PHONY: clean
clean:
	rm -f *.o main 