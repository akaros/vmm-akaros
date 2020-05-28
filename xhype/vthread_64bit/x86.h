
/* Various flags defined: can be included from assembler. */

/*
 * EFLAGS bits
 */
#define X86_EFLAGS_CF 0x00000001   /* Carry Flag */
#define X86_EFLAGS_BIT1 0x00000002 /* Bit 1 - always on */
#define X86_EFLAGS_PF 0x00000004   /* Parity Flag */
#define X86_EFLAGS_AF 0x00000010   /* Auxiliary carry Flag */
#define X86_EFLAGS_ZF 0x00000040   /* Zero Flag */
#define X86_EFLAGS_SF 0x00000080   /* Sign Flag */
#define X86_EFLAGS_TF 0x00000100   /* Trap Flag */
#define X86_EFLAGS_IF 0x00000200   /* Interrupt Flag */
#define X86_EFLAGS_DF 0x00000400   /* Direction Flag */
#define X86_EFLAGS_OF 0x00000800   /* Overflow Flag */
#define X86_EFLAGS_IOPL 0x00003000 /* IOPL mask */
#define X86_EFLAGS_NT 0x00004000   /* Nested Task */
#define X86_EFLAGS_RF 0x00010000   /* Resume Flag */
#define X86_EFLAGS_VM 0x00020000   /* Virtual Mode */
#define X86_EFLAGS_AC 0x00040000   /* Alignment Check */
#define X86_EFLAGS_VIF 0x00080000  /* Virtual Interrupt Flag */
#define X86_EFLAGS_VIP 0x00100000  /* Virtual Interrupt Pending */
#define X86_EFLAGS_ID 0x00200000   /* CPUID detection flag */

/*
 * Basic CPU control in CR0
 */
#define X86_CR0_PE 0x00000001 /* Protection Enable */
#define X86_CR0_MP 0x00000002 /* Monitor Coprocessor */
#define X86_CR0_EM 0x00000004 /* Emulation */
#define X86_CR0_TS 0x00000008 /* Task Switched */
#define X86_CR0_ET 0x00000010 /* Extension Type */
#define X86_CR0_NE 0x00000020 /* Numeric Error */
#define X86_CR0_WP 0x00010000 /* Write Protect */
#define X86_CR0_AM 0x00040000 /* Alignment Mask */
#define X86_CR0_NW 0x20000000 /* Not Write-through */
#define X86_CR0_CD 0x40000000 /* Cache Disable */
#define X86_CR0_PG 0x80000000 /* Paging */

/*
 * Paging options in CR3
 */
#define X86_CR3_PWT 0x00000008       /* Page Write Through */
#define X86_CR3_PCD 0x00000010       /* Page Cache Disable */
#define X86_CR3_PCID_MASK 0x00000fff /* PCID Mask */

/*
 * Intel CPU features in CR4
 */
#define X86_CR4_VME 0x00000001        /* enable vm86 extensions */
#define X86_CR4_PVI 0x00000002        /* virtual interrupts flag enable */
#define X86_CR4_TSD 0x00000004        /* disable time stamp at ipl 3 */
#define X86_CR4_DE 0x00000008         /* enable debugging extensions */
#define X86_CR4_PSE 0x00000010        /* enable page size extensions */
#define X86_CR4_PAE 0x00000020        /* enable physical address extensions */
#define X86_CR4_MCE 0x00000040        /* Machine check enable */
#define X86_CR4_PGE 0x00000080        /* enable global pages */
#define X86_CR4_PCE 0x00000100        /* enable performance counters at ipl 3 */
#define X86_CR4_OSFXSR 0x00000200     /* enable fast FPU save and restore */
#define X86_CR4_OSXMMEXCPT 0x00000400 /* enable unmasked SSE exceptions */
#define X86_CR4_VMXE 0x00002000       /* enable VMX virtualization */
#define X86_CR4_RDWRGSFS 0x00010000   /* enable RDWRGSFS support */
#define X86_CR4_PCIDE 0x00020000      /* enable PCID support */
#define X86_CR4_OSXSAVE 0x00040000    /* enable xsave and xrestore */
#define X86_CR4_SMEP 0x00100000       /* enable SMEP support */
#define X86_CR4_SMAP 0x00200000       /* enable SMAP support */

/* EFER bits: */
#define _EFER_SCE 0    /* SYSCALL/SYSRET */
#define _EFER_LME 8    /* Long mode enable */
#define _EFER_LMA 10   /* Long mode active (read-only) */
#define _EFER_NX 11    /* No execute enable */
#define _EFER_SVME 12  /* Enable virtualization */
#define _EFER_LMSLE 13 /* Long Mode Segment Limit Enable */
#define _EFER_FFXSR 14 /* Enable Fast FXSAVE/FXRSTOR */

#define EFER_SCE (1 << _EFER_SCE)
#define EFER_LME (1 << _EFER_LME)
#define EFER_LMA (1 << _EFER_LMA)
#define EFER_NX (1 << _EFER_NX)
#define EFER_SVME (1 << _EFER_SVME)
#define EFER_LMSLE (1 << _EFER_LMSLE)
#define EFER_FFXSR (1 << _EFER_FFXSR)

/* AMD64 MSR's */
#define MSR_EFER 0xc0000080    /* extended features */
#define MSR_STAR 0xc0000081    /* legacy mode SYSCALL target/cs/ss */
#define MSR_LSTAR 0xc0000082   /* long mode SYSCALL target rip */
#define MSR_CSTAR 0xc0000083   /* compat mode SYSCALL target rip */
#define MSR_SF_MASK 0xc0000084 /* syscall flags mask */
#define MSR_FSBASE 0xc0000100  /* base address of the %fs "segment" */
#define MSR_GSBASE 0xc0000101  /* base address of the %gs "segment" */
#define MSR_KGSBASE 0xc0000102 /* base address of the kernel %gs */
#define MSR_PERFEVSEL0 0xc0010000
#define MSR_PERFEVSEL1 0xc0010001
#define MSR_PERFEVSEL2 0xc0010002
#define MSR_PERFEVSEL3 0xc0010003
#define MSR_K7_PERFCTR0 0xc0010004
#define MSR_K7_PERFCTR1 0xc0010005
#define MSR_K7_PERFCTR2 0xc0010006
#define MSR_K7_PERFCTR3 0xc0010007
#define MSR_SYSCFG 0xc0010010
#define MSR_HWCR 0xc0010015
#define MSR_IORRBASE0 0xc0010016
#define MSR_IORRMASK0 0xc0010017
#define MSR_IORRBASE1 0xc0010018
#define MSR_IORRMASK1 0xc0010019
#define MSR_TOP_MEM 0xc001001a         /* boundary for ram below 4G */
#define MSR_TOP_MEM2 0xc001001d        /* boundary for ram above 4G */
#define MSR_NB_CFG1 0xc001001f         /* NB configuration 1 */
#define MSR_P_STATE_LIMIT 0xc0010061   /* P-state Current Limit Register */
#define MSR_P_STATE_CONTROL 0xc0010062 /* P-state Control Register */
#define MSR_P_STATE_STATUS 0xc0010063  /* P-state Status Register */
#define MSR_P_STATE_CONFIG(n) (0xc0010064 + (n)) /* P-state Config */
#define MSR_SMM_ADDR 0xc0010112                  /* SMM TSEG base address */
#define MSR_SMM_MASK 0xc0010113                  /* SMM TSEG address mask */
#define MSR_IC_CFG 0xc0011021          /* Instruction Cache Configuration */
#define MSR_K8_UCODE_UPDATE 0xc0010020 /* update microcode */
#define MSR_MC0_CTL_MASK 0xc0010044
#define MSR_VM_CR 0xc0010114       /* SVM: feature control */
#define MSR_VM_HSAVE_PA 0xc0010117 /* SVM: host save area address */

/*
 * Model-specific registers for the i386 family
 */
#define MSR_P5_MC_ADDR 0x000
#define MSR_P5_MC_TYPE 0x001
#define MSR_TSC 0x010
#define MSR_P5_CESR 0x011
#define MSR_P5_CTR0 0x012
#define MSR_P5_CTR1 0x013
#define MSR_IA32_PLATFORM_ID 0x017
#define MSR_APICBASE 0x01b
#define MSR_EBL_CR_POWERON 0x02a
#define MSR_TEST_CTL 0x033
#define MSR_IA32_FEATURE_CONTROL 0x03a
#define MSR_BIOS_UPDT_TRIG 0x079
#define MSR_BBL_CR_D0 0x088
#define MSR_BBL_CR_D1 0x089
#define MSR_BBL_CR_D2 0x08a
#define MSR_BIOS_SIGN 0x08b
#define MSR_PERFCTR0 0x0c1
#define MSR_PERFCTR1 0x0c2
#define MSR_PLATFORM_INFO 0x0ce
#define MSR_MPERF 0x0e7
#define MSR_APERF 0x0e8
#define MSR_IA32_EXT_CONFIG 0x0ee /* Undocumented. Core Solo/Duo only */
#define MSR_MTRRcap 0x0fe
#define MSR_BBL_CR_ADDR 0x116
#define MSR_BBL_CR_DECC 0x118
#define MSR_BBL_CR_CTL 0x119
#define MSR_BBL_CR_TRIG 0x11a
#define MSR_BBL_CR_BUSY 0x11b
#define MSR_BBL_CR_CTL3 0x11e
#define MSR_SYSENTER_CS_MSR 0x174
#define MSR_SYSENTER_ESP_MSR 0x175
#define MSR_SYSENTER_EIP_MSR 0x176
#define MSR_MCG_CAP 0x179
#define MSR_MCG_STATUS 0x17a
#define MSR_MCG_CTL 0x17b
#define MSR_EVNTSEL0 0x186
#define MSR_EVNTSEL1 0x187
#define MSR_THERM_CONTROL 0x19a
#define MSR_THERM_INTERRUPT 0x19b
#define MSR_THERM_STATUS 0x19c
#define MSR_IA32_MISC_ENABLE 0x1a0
#define MSR_IA32_TEMPERATURE_TARGET 0x1a2
#define MSR_TURBO_RATIO_LIMIT 0x1ad
#define MSR_TURBO_RATIO_LIMIT1 0x1ae
#define MSR_DEBUGCTLMSR 0x1d9
#define MSR_LASTBRANCHFROMIP 0x1db
#define MSR_LASTBRANCHTOIP 0x1dc
#define MSR_LASTINTFROMIP 0x1dd
#define MSR_LASTINTTOIP 0x1de
#define MSR_ROB_CR_BKUPTMPDR6 0x1e0
#define MSR_MTRRVarBase 0x200
#define MSR_MTRR64kBase 0x250
#define MSR_MTRR16kBase 0x258
#define MSR_MTRR4kBase 0x268
#define MSR_PAT 0x277
#define MSR_MC0_CTL2 0x280
#define MSR_MTRRdefType 0x2ff
#define MSR_MC0_CTL 0x400
#define MSR_MC0_STATUS 0x401
#define MSR_MC0_ADDR 0x402
#define MSR_MC0_MISC 0x403
#define MSR_MC1_CTL 0x404
#define MSR_MC1_STATUS 0x405
#define MSR_MC1_ADDR 0x406
#define MSR_MC1_MISC 0x407
#define MSR_MC2_CTL 0x408
#define MSR_MC2_STATUS 0x409
#define MSR_MC2_ADDR 0x40a
#define MSR_MC2_MISC 0x40b
#define MSR_MC3_CTL 0x40c
#define MSR_MC3_STATUS 0x40d
#define MSR_MC3_ADDR 0x40e
#define MSR_MC3_MISC 0x40f
#define MSR_MC4_CTL 0x410
#define MSR_MC4_STATUS 0x411
#define MSR_MC4_ADDR 0x412
#define MSR_MC4_MISC 0x413
#define MSR_RAPL_POWER_UNIT 0x606
#define MSR_PKG_ENERGY_STATUS 0x611
#define MSR_DRAM_ENERGY_STATUS 0x619
#define MSR_PP0_ENERGY_STATUS 0x639
#define MSR_PP1_ENERGY_STATUS 0x641

#define MSR_IA32_TSC_AUX 0xc0000103