#include <stdint.h>

void raw_vmcall(uint64_t vmcall_nr, void* args)
{
	long ret;

	asm volatile("vmcall"
	             : "=a"(ret)
	             : "D"(vmcall_nr), "S"(args));
}