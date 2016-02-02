/*
 * MSR emulation
 *
 * Copyright 2015 Google Inc.
 *
 * See LICENSE for details.
 */

#include <stdio.h>
#include <sys/types.h>
#include <pthread.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <parlib/arch/arch.h>
#include <parlib/ros_debug.h>
#include <unistd.h>
#include <errno.h>
#include <stdlib.h>
#include <string.h>
#include <sys/uio.h>
#include <stdint.h>
#include <err.h>
#include <sys/mman.h>
#include <ros/vmm.h>
#include <ros/arch/msr-index.h>
#include <vmm/virtio.h>
#include <vmm/virtio_mmio.h>
#include <vmm/virtio_ids.h>
#include <vmm/virtio_config.h>

struct emmsr {
	uint32_t reg;
	char *name;
	int (*f) (struct vmctl * vcpu, struct emmsr *, uint32_t);
	bool written;
	uint32_t edx, eax;
};
// Might need to mfence rdmsr.  supposedly wrmsr serializes, but not for x2APIC
static int
read_msr(struct vmctl *vcpu, uint32_t reg, uint32_t *edx, uint32_t *eax)
{
	uint64_t msr_val[128];
	int fd = open("#arch/msr", O_RDWR);

	if (pread(fd, msr_val, sizeof(msr_val), reg)<0) {
		fprintf(stderr, "MSR read failed %r\n");
		close(fd);
		return -1;
	}
	else {
		*edx = msr_val[vcpu->core] >> 32;
		*eax = msr_val[vcpu->core] & 0xffffffff;
		close(fd);
		return 0;
	}
}

static int
write_msr(struct vmctl *vcpu, uint32_t reg, uint32_t edx, uint32_t eax)
{
	uint64_t msr_val;
	int fd = open("#arch/msr", O_RDWR);

	msr_val = ((uint64_t)edx <<32) | eax;
	if (pwrite(fd, &msr_val, sizeof(msr_val), reg)<0) {
		fprintf(stderr, "MSR write failed %r\n");
		close(fd);
		return -1;
	}
	else {
		close(fd);
		return 0;
	}
}

int emsr_miscenable(struct vmctl *vcpu, struct emmsr *, uint32_t);
int emsr_mustmatch(struct vmctl *vcpu, struct emmsr *, uint32_t);
int emsr_readonly(struct vmctl *vcpu, struct emmsr *, uint32_t);
int emsr_readzero(struct vmctl *vcpu, struct emmsr *, uint32_t);
int emsr_fakewrite(struct vmctl *vcpu, struct emmsr *, uint32_t);
int emsr_ok(struct vmctl *vcpu, struct emmsr *, uint32_t);

int emsr_lapicvec(struct vmctl *vcpu, struct emmsr *msr, uint32_t opcode);
int emsr_lapicinitialcount(struct vmctl *vcpu, struct emmsr *msr, uint32_t opcode);

struct emmsr emmsrs[] = {
	{MSR_IA32_MISC_ENABLE, "MSR_IA32_MISC_ENABLE", emsr_miscenable},
	{MSR_IA32_SYSENTER_CS, "MSR_IA32_SYSENTER_CS", emsr_ok},
	{MSR_IA32_SYSENTER_EIP, "MSR_IA32_SYSENTER_EIP", emsr_ok},
	{MSR_IA32_SYSENTER_ESP, "MSR_IA32_SYSENTER_ESP", emsr_ok},
	{MSR_IA32_UCODE_REV, "MSR_IA32_UCODE_REV", emsr_fakewrite},
	{MSR_CSTAR, "MSR_CSTAR", emsr_fakewrite},
	{MSR_IA32_VMX_BASIC_MSR, "MSR_IA32_VMX_BASIC_MSR", emsr_fakewrite},
	{MSR_IA32_VMX_PINBASED_CTLS_MSR, "MSR_IA32_VMX_PINBASED_CTLS_MSR",
	 emsr_fakewrite},
	{MSR_IA32_VMX_PROCBASED_CTLS_MSR, "MSR_IA32_VMX_PROCBASED_CTLS_MSR",
	 emsr_fakewrite},
	{MSR_IA32_VMX_PROCBASED_CTLS2, "MSR_IA32_VMX_PROCBASED_CTLS2",
	 emsr_fakewrite},
	{MSR_IA32_VMX_EXIT_CTLS_MSR, "MSR_IA32_VMX_EXIT_CTLS_MSR",
	 emsr_fakewrite},
	{MSR_IA32_VMX_ENTRY_CTLS_MSR, "MSR_IA32_VMX_ENTRY_CTLS_MSR",
	 emsr_fakewrite},
	{MSR_IA32_ENERGY_PERF_BIAS, "MSR_IA32_ENERGY_PERF_BIAS",
	 emsr_fakewrite},
	{MSR_LBR_SELECT, "MSR_LBR_SELECT", emsr_ok},
	{MSR_LBR_TOS, "MSR_LBR_TOS", emsr_ok},
	{MSR_LBR_NHM_FROM, "MSR_LBR_NHM_FROM", emsr_ok},
	{MSR_LBR_NHM_TO, "MSR_LBR_NHM_TO", emsr_ok},
	{MSR_LBR_CORE_FROM, "MSR_LBR_CORE_FROM", emsr_ok},
	{MSR_LBR_CORE_TO, "MSR_LBR_CORE_TO", emsr_ok},

	// grumble. 
	{MSR_OFFCORE_RSP_0, "MSR_OFFCORE_RSP_0", emsr_ok},
	{MSR_OFFCORE_RSP_1, "MSR_OFFCORE_RSP_1", emsr_ok},
	// louder.
	{MSR_PEBS_LD_LAT_THRESHOLD, "MSR_PEBS_LD_LAT_THRESHOLD", emsr_ok},
	// aaaaaahhhhhhhhhhhhhhhhhhhhh
	{MSR_ARCH_PERFMON_EVENTSEL0, "MSR_ARCH_PERFMON_EVENTSEL0", emsr_ok},
	{MSR_ARCH_PERFMON_EVENTSEL1, "MSR_ARCH_PERFMON_EVENTSEL0", emsr_ok},
	{MSR_IA32_PERF_CAPABILITIES, "MSR_IA32_PERF_CAPABILITIES", emsr_ok},
	// unsafe.
	{MSR_IA32_APICBASE, "MSR_IA32_APICBASE", emsr_fakewrite},

	// mostly harmless.
	{MSR_TSC_AUX, "MSR_TSC_AUX", emsr_fakewrite},
	{MSR_RAPL_POWER_UNIT, "MSR_RAPL_POWER_UNIT", emsr_readzero},
	{MSR_LAPIC_TIMER, "MSR_LAPIC_TIMER", emsr_lapicvec},
	{MSR_LAPIC_THERMAL, "MSR_LAPIC_THERMAL", emsr_fakewrite},
	{MSR_LAPIC_INITCOUNT, "MSR_LAPIC_INITCOUNT", emsr_lapicinitialcount},
	//{MSR_LAPIC_INITCOUNT, "MSR_LAPIC_INITCOUNT", emsr_fakewrite},
};

static uint64_t set_low32(uint64_t hi, uint32_t lo)
{
	return (hi & 0xffffffff00000000ULL) | lo;
}

static uint64_t set_low16(uint64_t hi, uint16_t lo)
{
	return (hi & 0xffffffffffff0000ULL) | lo;
}

static uint64_t set_low8(uint64_t hi, uint8_t lo)
{
	return (hi & 0xffffffffffffff00ULL) | lo;
}

/* this may be the only register that needs special handling.
 * If there others then we might want to extend teh emmsr struct.
 */
int emsr_miscenable(struct vmctl *vcpu, struct emmsr *msr,
		    uint32_t opcode) {
	uint32_t eax, edx;

	if (read_msr(vcpu, msr->reg, &edx, &eax) < 0) {
		return SHUTDOWN_UNHANDLED_EXIT_REASON;
	}

	/* we just let them read the misc msr for now. */
	if (opcode == EXIT_REASON_MSR_READ) {
		vcpu->regs.tf_rax = set_low32(vcpu->regs.tf_rax, eax);
		vcpu->regs.tf_rax |= MSR_IA32_MISC_ENABLE_PEBS_UNAVAIL;
		vcpu->regs.tf_rdx = set_low32(vcpu->regs.tf_rdx, edx);
		return 0;
	} else {
		/* if they are writing what is already written, that's ok. */
		if (((uint32_t) vcpu->regs.tf_rax == eax)
		    && ((uint32_t) vcpu->regs.tf_rdx == edx))
			return 0;
	}
	fprintf(stderr, 
		"%s: Wanted to write 0x%x:0x%x, but could not; value was 0x%x:0x%x\n",
		 msr->name, (uint32_t) vcpu->regs.tf_rdx,
		 (uint32_t) vcpu->regs.tf_rax, edx, eax);
	return SHUTDOWN_UNHANDLED_EXIT_REASON;
}

int emsr_mustmatch(struct vmctl *vcpu, struct emmsr *msr,
		   uint32_t opcode) {
	uint32_t eax, edx;

	if (read_msr(vcpu, msr->reg, &edx, &eax) < 0) {
		return SHUTDOWN_UNHANDLED_EXIT_REASON;
	}
	/* we just let them read the misc msr for now. */
	if (opcode == EXIT_REASON_MSR_READ) {
		vcpu->regs.tf_rax = set_low32(vcpu->regs.tf_rax, eax);
		vcpu->regs.tf_rdx = set_low32(vcpu->regs.tf_rdx, edx);
		return 0;
	} else {
		/* if they are writing what is already written, that's ok. */
		if (((uint32_t) vcpu->regs.tf_rax == eax)
		    && ((uint32_t) vcpu->regs.tf_rdx == edx))
			return 0;
	}
	fprintf(stderr,
		"%s: Wanted to write 0x%x:0x%x, but could not; value was 0x%x:0x%x\n",
		 msr->name, (uint32_t) vcpu->regs.tf_rdx,
		 (uint32_t) vcpu->regs.tf_rax, edx, eax);
	return SHUTDOWN_UNHANDLED_EXIT_REASON;
}

int emsr_ok(struct vmctl *vcpu, struct emmsr *msr, uint32_t opcode)
{
	if (opcode == EXIT_REASON_MSR_READ) {
		if (read_msr(vcpu, msr->reg, (uint32_t *)&(vcpu->regs.tf_rdx),
		         (uint32_t *)&(vcpu->regs.tf_rax)) < 0) {
			return SHUTDOWN_UNHANDLED_EXIT_REASON;
		}
	} else {
		write_msr(vcpu, msr->reg, (uint32_t)vcpu->regs.tf_rdx,
		          (uint32_t)vcpu->regs.tf_rax);
	}
	return 0;
}

int emsr_readonly(struct vmctl *vcpu, struct emmsr *msr, uint32_t opcode)
{
	uint32_t eax, edx;
	if (read_msr(vcpu, (uint32_t) vcpu->regs.tf_rcx, &edx, &eax) < 0) {
		return SHUTDOWN_UNHANDLED_EXIT_REASON;
	}
	/* we just let them read the misc msr for now. */
	if (opcode == EXIT_REASON_MSR_READ) {
		vcpu->regs.tf_rax = set_low32(vcpu->regs.tf_rax, eax);
		vcpu->regs.tf_rdx = set_low32(vcpu->regs.tf_rdx, edx);
		return 0;
	}

	fprintf(stderr,"%s: Tried to write a readonly register\n", msr->name);
	return SHUTDOWN_UNHANDLED_EXIT_REASON;
}

int emsr_readzero(struct vmctl *vcpu, struct emmsr *msr, uint32_t opcode)
{
	if (opcode == EXIT_REASON_MSR_READ) {
		vcpu->regs.tf_rax = 0;
		vcpu->regs.tf_rdx = 0;
		return 0;
	}

	fprintf(stderr,"%s: Tried to write a readonly register\n", msr->name);
	return SHUTDOWN_UNHANDLED_EXIT_REASON;
}

/* pretend to write it, but don't write it. */
int emsr_fakewrite(struct vmctl *vcpu, struct emmsr *msr, uint32_t opcode)
{
	uint32_t eax, edx;

	if (!msr->written) {
		if (read_msr(vcpu, msr->reg, &edx, &eax) < 0) {
			return SHUTDOWN_UNHANDLED_EXIT_REASON;
		}
	} else {
		edx = msr->edx;
		eax = msr->eax;
	}
	/* we just let them read the misc msr for now. */
	if (opcode == EXIT_REASON_MSR_READ) {
		vcpu->regs.tf_rax = set_low32(vcpu->regs.tf_rax, eax);
		vcpu->regs.tf_rdx = set_low32(vcpu->regs.tf_rdx, edx);
		return 0;
	} else {
		/* if they are writing what is already written, that's ok. */
		if (((uint32_t) vcpu->regs.tf_rax == eax)
		    && ((uint32_t) vcpu->regs.tf_rdx == edx))
			return 0;
		msr->edx = vcpu->regs.tf_rdx;
		msr->eax = vcpu->regs.tf_rax;
		msr->written = true;
	}
	return 0;
}

int emsr_lapicvec(struct vmctl *vcpu, struct emmsr *msr, uint32_t opcode)
{
	uint32_t eax, edx;

	if (opcode == EXIT_REASON_MSR_WRITE) {
		edx = vcpu->regs.tf_rdx;
		eax = vcpu->regs.tf_rax;
		// Read the written value into vcpu
		vcpu->timer_msr = ((uint64_t)edx << 32) | eax;
		msr->written = true;
	} else {
		if (!msr->written)
			if (read_msr(vcpu, msr->reg, &edx, &eax) < 0) {
				return SHUTDOWN_UNHANDLED_EXIT_REASON;
			}
		else {
			edx = (uint32_t)(vcpu->timer_msr >> 32);
			eax = (uint32_t)vcpu->timer_msr;
		}
		vcpu->regs.tf_rax = set_low32(vcpu->regs.tf_rax, eax);
		vcpu->regs.tf_rdx = set_low32(vcpu->regs.tf_rdx, edx);
	}
	return 0;
}

int emsr_lapicinitialcount(struct vmctl *vcpu, struct emmsr *msr, uint32_t opcode)
{
	uint32_t eax, edx;

	//fprintf(stderr, "WE ARE HEREEEEE\n");

	if (opcode == EXIT_REASON_MSR_WRITE) {
		edx = vcpu->regs.tf_rdx;
		eax = vcpu->regs.tf_rax;
		// Read the written value into vcpu
		vcpu->initial_count = ((uint64_t)edx << 32) | eax;
		msr->written = true;
	} else {
		if (!msr->written)
			if (read_msr(vcpu, msr->reg, &edx, &eax) < 0) {
				return SHUTDOWN_UNHANDLED_EXIT_REASON;
			}
		else {
			edx = (uint32_t)(vcpu->initial_count >> 32);
			eax = (uint32_t)vcpu->initial_count;
		}
		vcpu->regs.tf_rax = set_low32(vcpu->regs.tf_rax, eax);
		vcpu->regs.tf_rdx = set_low32(vcpu->regs.tf_rdx, edx);
	}
	return 0;
}

int
msrio(struct vmctl *vcpu, uint32_t opcode) {
	int i;
	for (i = 0; i < sizeof(emmsrs)/sizeof(emmsrs[0]); i++) {
		if (emmsrs[i].reg != vcpu->regs.tf_rcx)
			continue;
		return emmsrs[i].f(vcpu, &emmsrs[i], opcode);
	}
	fprintf(stderr,"msrio for 0x%lx failed\n", vcpu->regs.tf_rcx);
	return SHUTDOWN_UNHANDLED_EXIT_REASON;
}

