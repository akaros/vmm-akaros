/* SPDX-License-Identifier: GPL-2.0-only */
/* CPU model specific register (MSR) numbers */

/* x86-64 specific MSRs */
pub const MSR_EFER:u32 = 		0xc0000080	/* extended feature register */;
pub const MSR_STAR:u32 = 		0xc0000081	/* legacy mode SYSCALL target */;
pub const MSR_LSTAR:u32 = 		0xc0000082	/* long mode SYSCALL target */;
pub const MSR_CSTAR:u32 = 		0xc0000083	/* compat mode SYSCALL target */;
pub const MSR_SYSCALL_MASK:u32 = 	0xc0000084	/* EFLAGS mask for syscall */;
pub const MSR_FS_BASE:u32 = 		0xc0000100	/* 64bit FS base */;
pub const MSR_GS_BASE:u32 = 		0xc0000101	/* 64bit GS base */;
pub const MSR_KERNEL_GS_BASE:u32 = 	0xc0000102	/* SwapGS GS shadow */;
pub const MSR_TSC_AUX:u32 = 		0xc0000103	/* Auxiliary TSC */;

/* EFER bits: */
pub const _EFER_SCE:u32 = 		0	/* SYSCALL/SYSRET */;
pub const _EFER_LME:u32 = 		8	/* Long mode enable */;
pub const _EFER_LMA:u32 = 		10	/* Long mode active (read-only) */;
pub const _EFER_NX:u32 = 		11	/* No execute enable */;
pub const _EFER_SVME:u32 = 		12	/* Enable virtualization */;
pub const _EFER_LMSLE:u32 = 		13	/* Long Mode Segment Limit Enable */;
pub const _EFER_FFXSR:u32 = 		14	/* Enable Fast FXSAVE/FXRSTOR */;

pub const EFER_SCE: u32 = (1 << _EFER_SCE);
pub const EFER_LME: u32 = (1 << _EFER_LME);
pub const EFER_LMA: u32 = (1 << _EFER_LMA);
pub const EFER_NX: u32 = (1 << _EFER_NX);
pub const EFER_SVME: u32 = (1 << _EFER_SVME);
pub const EFER_LMSLE: u32 = (1 << _EFER_LMSLE);
pub const EFER_FFXSR: u32 = (1 << _EFER_FFXSR);

/* Intel MSRs. Some also available on other CPUs */
pub const MSR_IA32_PERFCTR0: u32 = 0x000000c1;
pub const MSR_IA32_PERFCTR1: u32 = 0x000000c2;
pub const MSR_ARCH_PERFMON_EVENTSEL0: u32 = 0x186;
pub const MSR_ARCH_PERFMON_EVENTSEL1: u32 = 0x187;

pub const ARCH_PERFMON_EVENTSEL_EVENT: u64 = 0x000000FF;
pub const ARCH_PERFMON_EVENTSEL_UMASK: u64 = 0x0000FF00;
pub const ARCH_PERFMON_EVENTSEL_USR: u64 = (1 << 16);
pub const ARCH_PERFMON_EVENTSEL_OS: u64 = (1 << 17);
pub const ARCH_PERFMON_EVENTSEL_EDGE: u64 = (1 << 18);
pub const ARCH_PERFMON_EVENTSEL_PIN_CONTROL: u64 = (1 << 19);
pub const ARCH_PERFMON_EVENTSEL_INT: u64 = (1 << 20);
pub const ARCH_PERFMON_EVENTSEL_ANY: u64 = (1 << 21);
pub const ARCH_PERFMON_EVENTSEL_ENABLE: u64 = (1 << 22);
pub const ARCH_PERFMON_EVENTSEL_INV: u64 = (1 << 23);
pub const ARCH_PERFMON_EVENTSEL_CMASK: u64 = 0xFF000000;

pub const MSR_FSB_FREQ: u32 = 0x000000cd;
pub const MSR_PLATFORM_INFO: u32 = 0x000000ce;

pub const MSR_NHM_SNB_PKG_CST_CFG_CTL: u32 = 0x000000e2;
pub const NHM_C3_AUTO_DEMOTE: u32 = (1 << 25);
pub const NHM_C1_AUTO_DEMOTE: u32 = (1 << 26);
pub const ATM_LNC_C6_AUTO_DEMOTE: u32 = (1 << 25);
pub const SNB_C1_AUTO_UNDEMOTE: u32 = (1 << 27);
pub const SNB_C3_AUTO_UNDEMOTE: u32 = (1 << 28);

pub const MSR_MTRRCAP: u32 = 0x000000fe;
pub const MSR_IA32_BBL_CR_CTL: u32 = 0x00000119;
pub const MSR_IA32_BBL_CR_CTL3: u32 = 0x0000011e;

pub const MSR_IA32_SYSENTER_CS: u32 = 0x00000174;
pub const MSR_IA32_SYSENTER_ESP: u32 = 0x00000175;
pub const MSR_IA32_SYSENTER_EIP: u32 = 0x00000176;

pub const MSR_IA32_MCG_CAP: u32 = 0x00000179;
pub const MSR_IA32_MCG_STATUS: u32 = 0x0000017a;
pub const MSR_IA32_MCG_CTL: u32 = 0x0000017b;

pub const MSR_OFFCORE_RSP_0: u32 = 0x000001a6;
pub const MSR_OFFCORE_RSP_1: u32 = 0x000001a7;
pub const MSR_NHM_TURBO_RATIO_LIMIT: u32 = 0x000001ad;
pub const MSR_TURBO_RATIO_LIMIT: u32 = 0x000001ad;
pub const MSR_IVT_TURBO_RATIO_LIMIT: u32 = 0x000001ae;

pub const MSR_LBR_SELECT: u32 = 0x000001c8;
pub const MSR_LBR_TOS: u32 = 0x000001c9;
pub const MSR_LBR_NHM_FROM: u32 = 0x00000680;
pub const MSR_LBR_NHM_TO: u32 = 0x000006c0;
pub const MSR_LBR_CORE_FROM: u32 = 0x00000040;
pub const MSR_LBR_CORE_TO: u32 = 0x00000060;

pub const MSR_IA32_PEBS_ENABLE: u32 = 0x000003f1;
// pub const MSR_P4_PEBS_MATRIX_VERT: u32 = 0x000003f2;
pub const MSR_PEBS_LD_LAT_THRESHOLD: u32 = 0x000003f6;
pub const MSR_IA32_DS_AREA: u32 = 0x00000600;
pub const MSR_IA32_PERF_CAPABILITIES: u32 = 0x00000345;

pub const MSR_MTRRfix64K_00000: u32 = 0x00000250;
pub const MSR_MTRRfix16K_80000: u32 = 0x00000258;
pub const MSR_MTRRfix16K_A0000: u32 = 0x00000259;
pub const MSR_MTRRfix4K_C0000: u32 = 0x00000268;
pub const MSR_MTRRfix4K_C8000: u32 = 0x00000269;
pub const MSR_MTRRfix4K_D0000: u32 = 0x0000026a;
pub const MSR_MTRRfix4K_D8000: u32 = 0x0000026b;
pub const MSR_MTRRfix4K_E0000: u32 = 0x0000026c;
pub const MSR_MTRRfix4K_E8000: u32 = 0x0000026d;
pub const MSR_MTRRfix4K_F0000: u32 = 0x0000026e;
pub const MSR_MTRRfix4K_F8000: u32 = 0x0000026f;
pub const MSR_MTRRDEF_TYPE: u32 = 0x000002ff;

pub const MSR_IA32_CR_PAT: u32 = 0x00000277;

pub const MSR_IA32_DEBUGCTLMSR: u32 = 0x000001d9;
pub const MSR_IA32_LASTBRANCHFROMIP: u32 = 0x000001db;
pub const MSR_IA32_LASTBRANCHTOIP: u32 = 0x000001dc;
pub const MSR_IA32_LASTINTFROMIP: u32 = 0x000001dd;
pub const MSR_IA32_LASTINTTOIP: u32 = 0x000001de;

/* X86 X2APIC registers */
pub const MSR_LAPIC_ID: u32 = 0x00000802;
pub const MSR_LAPIC_VERSION: u32 = 0x00000803;
pub const MSR_LAPIC_TPR: u32 = 0x00000808;
pub const MSR_LAPIC_PPR: u32 = 0x0000080a;
pub const MSR_LAPIC_EOI: u32 = 0x0000080b;
pub const MSR_LAPIC_LDR: u32 = 0x0000080d;
pub const MSR_LAPIC_SPURIOUS: u32 = 0x0000080f;

pub const MSR_LAPIC_ISR_31_0: u32 = 0x00000810;
pub const MSR_LAPIC_ISR_63_32: u32 = 0x00000811;
pub const MSR_LAPIC_ISR_95_64: u32 = 0x00000812;
pub const MSR_LAPIC_ISR_127_96: u32 = 0x00000813;
pub const MSR_LAPIC_ISR_159_128: u32 = 0x00000814;
pub const MSR_LAPIC_ISR_191_160: u32 = 0x00000815;
pub const MSR_LAPIC_ISR_223_192: u32 = 0x00000816;
pub const MSR_LAPIC_ISR_255_224: u32 = 0x00000817;
// For easier looping
pub const MSR_LAPIC_ISR_START: u32 = MSR_LAPIC_ISR_31_0;
pub const MSR_LAPIC_ISR_END: u32 = (MSR_LAPIC_ISR_255_224 + 1);

pub const MSR_LAPIC_TMR_31_0: u32 = 0x00000818;
pub const MSR_LAPIC_TMR_63_32: u32 = 0x00000819;
pub const MSR_LAPIC_TMR_95_64: u32 = 0x0000081a;
pub const MSR_LAPIC_TMR_127_96: u32 = 0x0000081b;
pub const MSR_LAPIC_TMR_159_128: u32 = 0x0000081c;
pub const MSR_LAPIC_TMR_191_160: u32 = 0x0000081d;
pub const MSR_LAPIC_TMR_223_192: u32 = 0x0000081e;
pub const MSR_LAPIC_TMR_255_224: u32 = 0x0000081f;
// For easier looping
pub const MSR_LAPIC_TMR_START: u32 = MSR_LAPIC_TMR_31_0;
pub const MSR_LAPIC_TMR_END: u32 = (MSR_LAPIC_TMR_255_224 + 1);

pub const MSR_LAPIC_IRR_31_0: u32 = 0x00000820;
pub const MSR_LAPIC_IRR_63_32: u32 = 0x00000821;
pub const MSR_LAPIC_IRR_95_64: u32 = 0x00000822;
pub const MSR_LAPIC_IRR_127_96: u32 = 0x00000823;
pub const MSR_LAPIC_IRR_159_128: u32 = 0x00000824;
pub const MSR_LAPIC_IRR_191_160: u32 = 0x00000825;
pub const MSR_LAPIC_IRR_223_192: u32 = 0x00000826;
pub const MSR_LAPIC_IRR_255_224: u32 = 0x00000827;
// For easier looping
pub const MSR_LAPIC_IRR_START: u32 = MSR_LAPIC_IRR_31_0;
pub const MSR_LAPIC_IRR_END: u32 = (MSR_LAPIC_IRR_255_224 + 1);

pub const MSR_LAPIC_ESR: u32 = 0x00000828;
pub const MSR_LAPIC_LVT_CMCI: u32 = 0x0000082f;
pub const MSR_LAPIC_ICR: u32 = 0x00000830;
pub const MSR_LAPIC_LVT_TIMER: u32 = 0x00000832;
pub const MSR_LAPIC_LVT_THERMAL: u32 = 0x00000833;
pub const MSR_LAPIC_LVT_PERFMON: u32 = 0x00000834;
pub const MSR_LAPIC_LVT_LINT0: u32 = 0x00000835;
pub const MSR_LAPIC_LVT_LINT1: u32 = 0x00000836;
pub const MSR_LAPIC_LVT_ERROR_REG: u32 = 0x00000837;
pub const MSR_LAPIC_INITIAL_COUNT: u32 = 0x00000838;
pub const MSR_LAPIC_CURRENT_COUNT: u32 = 0x00000839;
pub const MSR_LAPIC_DIVIDE_CONFIG_REG: u32 = 0x0000083e;
pub const MSR_LAPIC_SELF_IPI: u32 = 0x0000083f;

pub const MSR_LAPIC_END: u32 = (MSR_LAPIC_SELF_IPI + 1);

/* DEBUGCTLMSR bits (others vary by model): */
pub const DEBUGCTLMSR_LBR:u32 = 			(1 <<  0)/* last branch recording */;
pub const DEBUGCTLMSR_BTF:u32 = 			(1 <<  1)/* single-step on branches */;
pub const DEBUGCTLMSR_TR: u32 = (1 << 6);
pub const DEBUGCTLMSR_BTS: u32 = (1 << 7);
pub const DEBUGCTLMSR_BTINT: u32 = (1 << 8);
pub const DEBUGCTLMSR_BTS_OFF_OS: u32 = (1 << 9);
pub const DEBUGCTLMSR_BTS_OFF_USR: u32 = (1 << 10);
pub const DEBUGCTLMSR_FREEZE_LBRS_ON_PMI: u32 = (1 << 11);

pub const MSR_IA32_MC0_CTL: u32 = 0x00000400;
pub const MSR_IA32_MC0_STATUS: u32 = 0x00000401;
pub const MSR_IA32_MC0_ADDR: u32 = 0x00000402;
pub const MSR_IA32_MC0_MISC: u32 = 0x00000403;

/* C-state Residency Counters */
pub const MSR_PKG_C3_RESIDENCY: u32 = 0x000003f8;
pub const MSR_PKG_C6_RESIDENCY: u32 = 0x000003f9;
pub const MSR_PKG_C7_RESIDENCY: u32 = 0x000003fa;
pub const MSR_CORE_C3_RESIDENCY: u32 = 0x000003fc;
pub const MSR_CORE_C6_RESIDENCY: u32 = 0x000003fd;
pub const MSR_CORE_C7_RESIDENCY: u32 = 0x000003fe;
pub const MSR_PKG_C2_RESIDENCY: u32 = 0x0000060d;
pub const MSR_PKG_C8_RESIDENCY:u32 = 		0x00000630	/* HSW-ULT only */;
pub const MSR_PKG_C9_RESIDENCY:u32 = 		0x00000631	/* HSW-ULT only */;
pub const MSR_PKG_C10_RESIDENCY:u32 = 		0x00000632	/* HSW-ULT only */;

/* Run Time Average Power Limiting (RAPL) Interface */

pub const MSR_RAPL_POWER_UNIT: u32 = 0x00000606;

pub const MSR_PKG_POWER_LIMIT: u32 = 0x00000610;
pub const MSR_PKG_ENERGY_STATUS: u32 = 0x00000611;
pub const MSR_PKG_PERF_STATUS: u32 = 0x00000613;
pub const MSR_PKG_POWER_INFO: u32 = 0x00000614;

pub const MSR_DRAM_POWER_LIMIT: u32 = 0x00000618;
pub const MSR_DRAM_ENERGY_STATUS: u32 = 0x00000619;
pub const MSR_DRAM_PERF_STATUS: u32 = 0x0000061b;
pub const MSR_DRAM_POWER_INFO: u32 = 0x0000061c;

pub const MSR_PP0_POWER_LIMIT: u32 = 0x00000638;
pub const MSR_PP0_ENERGY_STATUS: u32 = 0x00000639;
pub const MSR_PP0_POLICY: u32 = 0x0000063a;
pub const MSR_PP0_PERF_STATUS: u32 = 0x0000063b;

pub const MSR_PP1_POWER_LIMIT: u32 = 0x00000640;
pub const MSR_PP1_ENERGY_STATUS: u32 = 0x00000641;
pub const MSR_PP1_POLICY: u32 = 0x00000642;

pub const MSR_AMD64_MC0_MASK: u32 = 0xc0010044;

// pub const MSR_IA32_MCx_CTL(x):u32 = 		(MSR_IA32_MC0_CTL + 4*(x));
// pub const MSR_IA32_MCx_STATUS(x):u32 = 		(MSR_IA32_MC0_STATUS + 4*(x));
// pub const MSR_IA32_MCx_ADDR(x):u32 = 		(MSR_IA32_MC0_ADDR + 4*(x));
// pub const MSR_IA32_MCx_MISC(x):u32 = 		(MSR_IA32_MC0_MISC + 4*(x));

// pub const MSR_AMD64_MCx_MASK(x):u32 = 		(MSR_AMD64_MC0_MASK + (x));

/* These are consecutive and not in the normal 4er MCE bank block */
pub const MSR_IA32_MC0_CTL2: u32 = 0x00000280;
// pub const MSR_IA32_MCx_CTL2(x):u32 = 		(MSR_IA32_MC0_CTL2 + (x));

pub const MSR_P6_PERFCTR0: u32 = 0x000000c1;
pub const MSR_P6_PERFCTR1: u32 = 0x000000c2;
pub const MSR_P6_EVNTSEL0: u32 = 0x00000186;
pub const MSR_P6_EVNTSEL1: u32 = 0x00000187;

pub const MSR_KNC_PERFCTR0: u32 = 0x00000020;
pub const MSR_KNC_PERFCTR1: u32 = 0x00000021;
pub const MSR_KNC_EVNTSEL0: u32 = 0x00000028;
pub const MSR_KNC_EVNTSEL1: u32 = 0x00000029;

/* AMD64 MSRs. Not complete. See the architecture manual for a more
complete list. */

pub const MSR_AMD64_PATCH_LEVEL: u32 = 0x0000008b;
pub const MSR_AMD64_TSC_RATIO: u32 = 0xc0000104;
pub const MSR_AMD64_NB_CFG: u32 = 0xc001001f;
pub const MSR_AMD64_PATCH_LOADER: u32 = 0xc0010020;
pub const MSR_AMD64_OSVW_ID_LENGTH: u32 = 0xc0010140;
pub const MSR_AMD64_OSVW_STATUS: u32 = 0xc0010141;
pub const MSR_AMD64_DC_CFG: u32 = 0xc0011022;
pub const MSR_AMD64_IBSFETCHCTL: u32 = 0xc0011030;
pub const MSR_AMD64_IBSFETCHLINAD: u32 = 0xc0011031;
pub const MSR_AMD64_IBSFETCHPHYSAD: u32 = 0xc0011032;
pub const MSR_AMD64_IBSFETCH_REG_COUNT: u32 = 3;
pub const MSR_AMD64_IBSFETCH_REG_MASK: u32 = ((1 << MSR_AMD64_IBSFETCH_REG_COUNT) - 1);
pub const MSR_AMD64_IBSOPCTL: u32 = 0xc0011033;
pub const MSR_AMD64_IBSOPRIP: u32 = 0xc0011034;
pub const MSR_AMD64_IBSOPDATA: u32 = 0xc0011035;
pub const MSR_AMD64_IBSOPDATA2: u32 = 0xc0011036;
pub const MSR_AMD64_IBSOPDATA3: u32 = 0xc0011037;
pub const MSR_AMD64_IBSDCLINAD: u32 = 0xc0011038;
pub const MSR_AMD64_IBSDCPHYSAD: u32 = 0xc0011039;
pub const MSR_AMD64_IBSOP_REG_COUNT: u32 = 7;
pub const MSR_AMD64_IBSOP_REG_MASK: u32 = ((1 << MSR_AMD64_IBSOP_REG_COUNT) - 1);
pub const MSR_AMD64_IBSCTL: u32 = 0xc001103a;
pub const MSR_AMD64_IBSBRTARGET: u32 = 0xc001103b;
pub const MSR_AMD64_IBS_REG_COUNT_MAX:u32 = 	8	/* includes MSR_AMD64_IBSBRTARGET */;

/* Fam 15h MSRs */
pub const MSR_F15H_PERF_CTL: u32 = 0xc0010200;
pub const MSR_F15H_PERF_CTR: u32 = 0xc0010201;

/* Fam 10h MSRs */
pub const MSR_FAM10H_MMIO_CONF_BASE: u32 = 0xc0010058;
pub const FAM10H_MMIO_CONF_ENABLE: u32 = (1 << 0);
pub const FAM10H_MMIO_CONF_BUSRANGE_MASK: u32 = 0xf;
pub const FAM10H_MMIO_CONF_BUSRANGE_SHIFT: u32 = 2;
pub const FAM10H_MMIO_CONF_BASE_MASK: u64 = 0xfffffff;
pub const FAM10H_MMIO_CONF_BASE_SHIFT: u32 = 20;
pub const MSR_FAM10H_NODE_ID: u32 = 0xc001100c;

/* K8 MSRs */
pub const MSR_K8_TOP_MEM1: u32 = 0xc001001a;
pub const MSR_K8_TOP_MEM2: u32 = 0xc001001d;
pub const MSR_K8_SYSCFG: u32 = 0xc0010010;
pub const MSR_K8_INT_PENDING_MSG: u32 = 0xc0010055;
/* C1E active bits in int pending message */
pub const K8_INTP_C1E_ACTIVE_MASK: u32 = 0x18000000;
pub const MSR_K8_TSEG_ADDR: u32 = 0xc0010112;
pub const K8_MTRRFIXRANGE_DRAM_ENABLE:u32 = 	0x00040000	/* MtrrFixDramEn bit    */;
pub const K8_MTRRFIXRANGE_DRAM_MODIFY:u32 = 	0x00080000	/* MtrrFixDramModEn bit */;
pub const K8_MTRR_RDMEM_WRMEM_MASK:u32 = 	0x18181818	/* Mask: RdMem|WrMem    */;

/* K7 MSRs */
pub const MSR_K7_EVNTSEL0: u32 = 0xc0010000;
pub const MSR_K7_PERFCTR0: u32 = 0xc0010004;
pub const MSR_K7_EVNTSEL1: u32 = 0xc0010001;
pub const MSR_K7_PERFCTR1: u32 = 0xc0010005;
pub const MSR_K7_EVNTSEL2: u32 = 0xc0010002;
pub const MSR_K7_PERFCTR2: u32 = 0xc0010006;
pub const MSR_K7_EVNTSEL3: u32 = 0xc0010003;
pub const MSR_K7_PERFCTR3: u32 = 0xc0010007;
pub const MSR_K7_CLK_CTL: u32 = 0xc001001b;
pub const MSR_K7_HWCR: u32 = 0xc0010015;
pub const MSR_K7_FID_VID_CTL: u32 = 0xc0010041;
pub const MSR_K7_FID_VID_STATUS: u32 = 0xc0010042;

/* K6 MSRs */
pub const MSR_K6_WHCR: u32 = 0xc0000082;
pub const MSR_K6_UWCCR: u32 = 0xc0000085;
pub const MSR_K6_EPMR: u32 = 0xc0000086;
pub const MSR_K6_PSOR: u32 = 0xc0000087;
pub const MSR_K6_PFIR: u32 = 0xc0000088;

/* Centaur-Hauls/IDT defined MSRs. */
pub const MSR_IDT_FCR1: u32 = 0x00000107;
pub const MSR_IDT_FCR2: u32 = 0x00000108;
pub const MSR_IDT_FCR3: u32 = 0x00000109;
pub const MSR_IDT_FCR4: u32 = 0x0000010a;

pub const MSR_IDT_MCR0: u32 = 0x00000110;
pub const MSR_IDT_MCR1: u32 = 0x00000111;
pub const MSR_IDT_MCR2: u32 = 0x00000112;
pub const MSR_IDT_MCR3: u32 = 0x00000113;
pub const MSR_IDT_MCR4: u32 = 0x00000114;
pub const MSR_IDT_MCR5: u32 = 0x00000115;
pub const MSR_IDT_MCR6: u32 = 0x00000116;
pub const MSR_IDT_MCR7: u32 = 0x00000117;
pub const MSR_IDT_MCR_CTRL: u32 = 0x00000120;

/* VIA Cyrix defined MSRs*/
pub const MSR_VIA_FCR: u32 = 0x00001107;
pub const MSR_VIA_LONGHAUL: u32 = 0x0000110a;
pub const MSR_VIA_RNG: u32 = 0x0000110b;
pub const MSR_VIA_BCR2: u32 = 0x00001147;

/* Transmeta defined MSRs */
pub const MSR_TMTA_LONGRUN_CTRL: u32 = 0x80868010;
pub const MSR_TMTA_LONGRUN_FLAGS: u32 = 0x80868011;
pub const MSR_TMTA_LRTI_READOUT: u32 = 0x80868018;
pub const MSR_TMTA_LRTI_VOLT_MHZ: u32 = 0x8086801a;

/* Intel defined MSRs. */
pub const MSR_IA32_P5_MC_ADDR: u32 = 0x00000000;
pub const MSR_IA32_P5_MC_TYPE: u32 = 0x00000001;
pub const MSR_IA32_TSC: u32 = 0x00000010;
pub const MSR_IA32_PLATFORM_ID: u32 = 0x00000017;
pub const MSR_IA32_EBL_CR_POWERON: u32 = 0x0000002a;
pub const MSR_EBC_FREQUENCY_ID: u32 = 0x0000002c;
pub const MSR_IA32_FEATURE_CONTROL: u32 = 0x0000003a;
pub const MSR_IA32_TSC_ADJUST: u32 = 0x0000003b;

pub const FEATURE_CONTROL_LOCKED: u32 = (1 << 0);
pub const FEATURE_CONTROL_VMXON_ENABLED_INSIDE_SMX: u32 = (1 << 1);
pub const FEATURE_CONTROL_VMXON_ENABLED_OUTSIDE_SMX: u32 = (1 << 2);

pub const MSR_IA32_APICBASE: u32 = 0x0000001b;
pub const MSR_IA32_APICBASE_BSP: u32 = (1 << 8);
pub const MSR_IA32_APICBASE_ENABLE: u32 = (1 << 11);
pub const MSR_IA32_APICBASE_BASE: u32 = (0xfffff << 12);

pub const MSR_IA32_TSCDEADLINE: u32 = 0x000006e0;

pub const MSR_IA32_UCODE_WRITE: u32 = 0x00000079;
pub const MSR_IA32_UCODE_REV: u32 = 0x0000008b;

pub const MSR_IA32_PERF_STATUS: u32 = 0x00000198;
pub const MSR_IA32_PERF_CTL: u32 = 0x00000199;
pub const MSR_AMD_PSTATE_DEF_BASE: u32 = 0xc0010064;
pub const MSR_AMD_PERF_STATUS: u32 = 0xc0010063;
pub const MSR_AMD_PERF_CTL: u32 = 0xc0010062;

pub const MSR_IA32_MPERF: u32 = 0x000000e7;
pub const MSR_IA32_APERF: u32 = 0x000000e8;

pub const MSR_IA32_THERM_CONTROL: u32 = 0x0000019a;
pub const MSR_IA32_THERM_INTERRUPT: u32 = 0x0000019b;

pub const THERM_INT_HIGH_ENABLE: u32 = (1 << 0);
pub const THERM_INT_LOW_ENABLE: u32 = (1 << 1);
pub const THERM_INT_PLN_ENABLE: u32 = (1 << 24);

pub const MSR_IA32_THERM_STATUS: u32 = 0x0000019c;

pub const THERM_STATUS_PROCHOT: u32 = (1 << 0);
pub const THERM_STATUS_POWER_LIMIT: u32 = (1 << 10);

pub const MSR_THERM2_CTL: u32 = 0x0000019d;

pub const MSR_THERM2_CTL_TM_SELECT: u64 = (1 << 16);

pub const MSR_IA32_MISC_ENABLE: u32 = 0x000001a0;

pub const MSR_IA32_TEMPERATURE_TARGET: u32 = 0x000001a2;

pub const MSR_IA32_ENERGY_PERF_BIAS: u32 = 0x000001b0;
pub const ENERGY_PERF_BIAS_PERFORMANCE: u32 = 0;
pub const ENERGY_PERF_BIAS_NORMAL: u32 = 6;
pub const ENERGY_PERF_BIAS_POWERSAVE: u32 = 15;

pub const MSR_IA32_PACKAGE_THERM_STATUS: u32 = 0x000001b1;

pub const PACKAGE_THERM_STATUS_PROCHOT: u32 = (1 << 0);
pub const PACKAGE_THERM_STATUS_POWER_LIMIT: u32 = (1 << 10);

pub const MSR_IA32_PACKAGE_THERM_INTERRUPT: u32 = 0x000001b2;

pub const PACKAGE_THERM_INT_HIGH_ENABLE: u32 = (1 << 0);
pub const PACKAGE_THERM_INT_LOW_ENABLE: u32 = (1 << 1);
pub const PACKAGE_THERM_INT_PLN_ENABLE: u32 = (1 << 24);

/* Thermal Thresholds Support */
pub const THERM_INT_THRESHOLD0_ENABLE: u32 = (1 << 15);
pub const THERM_SHIFT_THRESHOLD0: u32 = 8;
pub const THERM_MASK_THRESHOLD0: u32 = (0x7f << THERM_SHIFT_THRESHOLD0);
pub const THERM_INT_THRESHOLD1_ENABLE: u32 = (1 << 23);
pub const THERM_SHIFT_THRESHOLD1: u32 = 16;
pub const THERM_MASK_THRESHOLD1: u32 = (0x7f << THERM_SHIFT_THRESHOLD1);
pub const THERM_STATUS_THRESHOLD0: u32 = (1 << 6);
pub const THERM_LOG_THRESHOLD0: u32 = (1 << 7);
pub const THERM_STATUS_THRESHOLD1: u32 = (1 << 8);
pub const THERM_LOG_THRESHOLD1: u32 = (1 << 9);

/* MISC_ENABLE bits: architectural */
pub const MSR_IA32_MISC_ENABLE_FAST_STRING: u64 = (1 << 0);
pub const MSR_IA32_MISC_ENABLE_TCC: u64 = (1 << 1);
pub const MSR_IA32_MISC_ENABLE_EMON: u64 = (1 << 7);
pub const MSR_IA32_MISC_ENABLE_BTS_UNAVAIL: u64 = (1 << 11);
pub const MSR_IA32_MISC_ENABLE_PEBS_UNAVAIL: u64 = (1 << 12);
pub const MSR_IA32_MISC_ENABLE_ENHANCED_SPEEDSTEP: u64 = (1 << 16);
pub const MSR_IA32_MISC_ENABLE_MWAIT: u64 = (1 << 18);
pub const MSR_IA32_MISC_ENABLE_LIMIT_CPUID: u64 = (1 << 22);
pub const MSR_IA32_MISC_ENABLE_XTPR_DISABLE: u64 = (1 << 23);
pub const MSR_IA32_MISC_ENABLE_XD_DISABLE: u64 = (1 << 34);

/* MISC_ENABLE bits: model-specific, meaning may vary from core to core */
pub const MSR_IA32_MISC_ENABLE_X87_COMPAT: u64 = (1 << 2);
pub const MSR_IA32_MISC_ENABLE_TM1: u64 = (1 << 3);
pub const MSR_IA32_MISC_ENABLE_SPLIT_LOCK_DISABLE: u64 = (1 << 4);
pub const MSR_IA32_MISC_ENABLE_L3CACHE_DISABLE: u64 = (1 << 6);
pub const MSR_IA32_MISC_ENABLE_SUPPRESS_LOCK: u64 = (1 << 8);
pub const MSR_IA32_MISC_ENABLE_PREFETCH_DISABLE: u64 = (1 << 9);
pub const MSR_IA32_MISC_ENABLE_FERR: u64 = (1 << 10);
pub const MSR_IA32_MISC_ENABLE_FERR_MULTIPLEX: u64 = (1 << 10);
pub const MSR_IA32_MISC_ENABLE_TM2: u64 = (1 << 13);
pub const MSR_IA32_MISC_ENABLE_ADJ_PREF_DISABLE: u64 = (1 << 19);
pub const MSR_IA32_MISC_ENABLE_SPEEDSTEP_LOCK: u64 = (1 << 20);
pub const MSR_IA32_MISC_ENABLE_L1D_CONTEXT: u64 = (1 << 24);
pub const MSR_IA32_MISC_ENABLE_DCU_PREF_DISABLE: u64 = (1 << 37);
pub const MSR_IA32_MISC_ENABLE_TURBO_DISABLE: u64 = (1 << 38);
pub const MSR_IA32_MISC_ENABLE_IP_PREF_DISABLE: u64 = (1 << 39);

pub const MSR_IA32_TSC_DEADLINE: u32 = 0x000006E0;
pub const MSR_IA32_SPEC_CTRL: u32 = 0x48;
pub const MSR_IA32_PRED_CMD: u32 = 0x49;

/* P4/Xeon+ specific */
pub const MSR_IA32_MCG_EAX: u32 = 0x00000180;
pub const MSR_IA32_MCG_EBX: u32 = 0x00000181;
pub const MSR_IA32_MCG_ECX: u32 = 0x00000182;
pub const MSR_IA32_MCG_EDX: u32 = 0x00000183;
pub const MSR_IA32_MCG_ESI: u32 = 0x00000184;
pub const MSR_IA32_MCG_EDI: u32 = 0x00000185;
pub const MSR_IA32_MCG_EBP: u32 = 0x00000186;
pub const MSR_IA32_MCG_ESP: u32 = 0x00000187;
pub const MSR_IA32_MCG_EFLAGS: u32 = 0x00000188;
pub const MSR_IA32_MCG_EIP: u32 = 0x00000189;
pub const MSR_IA32_MCG_RESERVED: u32 = 0x0000018a;

/* Pentium IV performance counter MSRs */
pub const MSR_P4_BPU_PERFCTR0: u32 = 0x00000300;
pub const MSR_P4_BPU_PERFCTR1: u32 = 0x00000301;
pub const MSR_P4_BPU_PERFCTR2: u32 = 0x00000302;
pub const MSR_P4_BPU_PERFCTR3: u32 = 0x00000303;
pub const MSR_P4_MS_PERFCTR0: u32 = 0x00000304;
pub const MSR_P4_MS_PERFCTR1: u32 = 0x00000305;
pub const MSR_P4_MS_PERFCTR2: u32 = 0x00000306;
pub const MSR_P4_MS_PERFCTR3: u32 = 0x00000307;
pub const MSR_P4_FLAME_PERFCTR0: u32 = 0x00000308;
pub const MSR_P4_FLAME_PERFCTR1: u32 = 0x00000309;
pub const MSR_P4_FLAME_PERFCTR2: u32 = 0x0000030a;
pub const MSR_P4_FLAME_PERFCTR3: u32 = 0x0000030b;
pub const MSR_P4_IQ_PERFCTR0: u32 = 0x0000030c;
pub const MSR_P4_IQ_PERFCTR1: u32 = 0x0000030d;
pub const MSR_P4_IQ_PERFCTR2: u32 = 0x0000030e;
pub const MSR_P4_IQ_PERFCTR3: u32 = 0x0000030f;
pub const MSR_P4_IQ_PERFCTR4: u32 = 0x00000310;
pub const MSR_P4_IQ_PERFCTR5: u32 = 0x00000311;
pub const MSR_P4_BPU_CCCR0: u32 = 0x00000360;
pub const MSR_P4_BPU_CCCR1: u32 = 0x00000361;
pub const MSR_P4_BPU_CCCR2: u32 = 0x00000362;
pub const MSR_P4_BPU_CCCR3: u32 = 0x00000363;
pub const MSR_P4_MS_CCCR0: u32 = 0x00000364;
pub const MSR_P4_MS_CCCR1: u32 = 0x00000365;
pub const MSR_P4_MS_CCCR2: u32 = 0x00000366;
pub const MSR_P4_MS_CCCR3: u32 = 0x00000367;
pub const MSR_P4_FLAME_CCCR0: u32 = 0x00000368;
pub const MSR_P4_FLAME_CCCR1: u32 = 0x00000369;
pub const MSR_P4_FLAME_CCCR2: u32 = 0x0000036a;
pub const MSR_P4_FLAME_CCCR3: u32 = 0x0000036b;
pub const MSR_P4_IQ_CCCR0: u32 = 0x0000036c;
pub const MSR_P4_IQ_CCCR1: u32 = 0x0000036d;
pub const MSR_P4_IQ_CCCR2: u32 = 0x0000036e;
pub const MSR_P4_IQ_CCCR3: u32 = 0x0000036f;
pub const MSR_P4_IQ_CCCR4: u32 = 0x00000370;
pub const MSR_P4_IQ_CCCR5: u32 = 0x00000371;
pub const MSR_P4_ALF_ESCR0: u32 = 0x000003ca;
pub const MSR_P4_ALF_ESCR1: u32 = 0x000003cb;
pub const MSR_P4_BPU_ESCR0: u32 = 0x000003b2;
pub const MSR_P4_BPU_ESCR1: u32 = 0x000003b3;
pub const MSR_P4_BSU_ESCR0: u32 = 0x000003a0;
pub const MSR_P4_BSU_ESCR1: u32 = 0x000003a1;
pub const MSR_P4_CRU_ESCR0: u32 = 0x000003b8;
pub const MSR_P4_CRU_ESCR1: u32 = 0x000003b9;
pub const MSR_P4_CRU_ESCR2: u32 = 0x000003cc;
pub const MSR_P4_CRU_ESCR3: u32 = 0x000003cd;
pub const MSR_P4_CRU_ESCR4: u32 = 0x000003e0;
pub const MSR_P4_CRU_ESCR5: u32 = 0x000003e1;
pub const MSR_P4_DAC_ESCR0: u32 = 0x000003a8;
pub const MSR_P4_DAC_ESCR1: u32 = 0x000003a9;
pub const MSR_P4_FIRM_ESCR0: u32 = 0x000003a4;
pub const MSR_P4_FIRM_ESCR1: u32 = 0x000003a5;
pub const MSR_P4_FLAME_ESCR0: u32 = 0x000003a6;
pub const MSR_P4_FLAME_ESCR1: u32 = 0x000003a7;
pub const MSR_P4_FSB_ESCR0: u32 = 0x000003a2;
pub const MSR_P4_FSB_ESCR1: u32 = 0x000003a3;
pub const MSR_P4_IQ_ESCR0: u32 = 0x000003ba;
pub const MSR_P4_IQ_ESCR1: u32 = 0x000003bb;
pub const MSR_P4_IS_ESCR0: u32 = 0x000003b4;
pub const MSR_P4_IS_ESCR1: u32 = 0x000003b5;
pub const MSR_P4_ITLB_ESCR0: u32 = 0x000003b6;
pub const MSR_P4_ITLB_ESCR1: u32 = 0x000003b7;
pub const MSR_P4_IX_ESCR0: u32 = 0x000003c8;
pub const MSR_P4_IX_ESCR1: u32 = 0x000003c9;
pub const MSR_P4_MOB_ESCR0: u32 = 0x000003aa;
pub const MSR_P4_MOB_ESCR1: u32 = 0x000003ab;
pub const MSR_P4_MS_ESCR0: u32 = 0x000003c0;
pub const MSR_P4_MS_ESCR1: u32 = 0x000003c1;
pub const MSR_P4_PMH_ESCR0: u32 = 0x000003ac;
pub const MSR_P4_PMH_ESCR1: u32 = 0x000003ad;
pub const MSR_P4_RAT_ESCR0: u32 = 0x000003bc;
pub const MSR_P4_RAT_ESCR1: u32 = 0x000003bd;
pub const MSR_P4_SAAT_ESCR0: u32 = 0x000003ae;
pub const MSR_P4_SAAT_ESCR1: u32 = 0x000003af;
pub const MSR_P4_SSU_ESCR0: u32 = 0x000003be;
pub const MSR_P4_SSU_ESCR1:u32 = 		0x000003bf	/* guess: not in manual */;

pub const MSR_P4_TBPU_ESCR0: u32 = 0x000003c2;
pub const MSR_P4_TBPU_ESCR1: u32 = 0x000003c3;
pub const MSR_P4_TC_ESCR0: u32 = 0x000003c4;
pub const MSR_P4_TC_ESCR1: u32 = 0x000003c5;
pub const MSR_P4_U2L_ESCR0: u32 = 0x000003b0;
pub const MSR_P4_U2L_ESCR1: u32 = 0x000003b1;

pub const MSR_P4_PEBS_MATRIX_VERT: u32 = 0x000003f2;

/* Intel Core-based CPU performance counters */
pub const MSR_CORE_PERF_FIXED_CTR0: u32 = 0x00000309;
pub const MSR_CORE_PERF_FIXED_CTR1: u32 = 0x0000030a;
pub const MSR_CORE_PERF_FIXED_CTR2: u32 = 0x0000030b;
pub const MSR_CORE_PERF_FIXED_CTR_CTRL: u32 = 0x0000038d;
pub const MSR_CORE_PERF_GLOBAL_STATUS: u32 = 0x0000038e;
pub const MSR_CORE_PERF_GLOBAL_CTRL: u32 = 0x0000038f;
pub const MSR_CORE_PERF_GLOBAL_OVF_CTRL: u32 = 0x00000390;

/* Geode defined MSRs */
pub const MSR_GEODE_BUSCONT_CONF0: u32 = 0x00001900;

/* Intel VT MSRs */
pub const MSR_IA32_VMX_BASIC: u32 = 0x00000480;
pub const MSR_IA32_VMX_PINBASED_CTLS: u32 = 0x00000481;
pub const MSR_IA32_VMX_PROCBASED_CTLS: u32 = 0x00000482;
pub const MSR_IA32_VMX_EXIT_CTLS: u32 = 0x00000483;
pub const MSR_IA32_VMX_ENTRY_CTLS: u32 = 0x00000484;
pub const MSR_IA32_VMX_MISC: u32 = 0x00000485;
pub const MSR_IA32_VMX_CR0_FIXED0: u32 = 0x00000486;
pub const MSR_IA32_VMX_CR0_FIXED1: u32 = 0x00000487;
pub const MSR_IA32_VMX_CR4_FIXED0: u32 = 0x00000488;
pub const MSR_IA32_VMX_CR4_FIXED1: u32 = 0x00000489;
pub const MSR_IA32_VMX_VMCS_ENUM: u32 = 0x0000048a;
pub const MSR_IA32_VMX_PROCBASED_CTLS2: u32 = 0x0000048b;
pub const MSR_IA32_VMX_EPT_VPID_CAP: u32 = 0x0000048c;
pub const MSR_IA32_VMX_TRUE_PINBASED_CTLS: u32 = 0x0000048d;
pub const MSR_IA32_VMX_TRUE_PROCBASED_CTLS: u32 = 0x0000048e;
pub const MSR_IA32_VMX_TRUE_EXIT_CTLS: u32 = 0x0000048f;
pub const MSR_IA32_VMX_TRUE_ENTRY_CTLS: u32 = 0x00000490;

/* VMX_BASIC bits and bitmasks */
pub const VMX_BASIC_VMCS_SIZE_SHIFT: u32 = 32;
pub const VMX_BASIC_64: u64 = (1 << 48);
pub const VMX_BASIC_MEM_TYPE_SHIFT: u32 = 50;
pub const VMX_BASIC_MEM_TYPE_MASK: u64 = (0xf << VMX_BASIC_MEM_TYPE_SHIFT);
pub const VMX_BASIC_MEM_TYPE_WB: u64 = 6;
pub const VMX_BASIC_INOUT: u64 = (1 << 54);
pub const VMX_BASIC_TRUE_CTLS: u64 = (1 << 55);

/* AMD-V MSRs */

pub const MSR_VM_CR: u32 = 0xc0010114;
pub const MSR_VM_IGNNE: u32 = 0xc0010115;
pub const MSR_VM_HSAVE_PA: u32 = 0xc0010117;

// additonal
pub const MSR_IA32_BIOS_SIGN_ID: u32 = 0x8b;
