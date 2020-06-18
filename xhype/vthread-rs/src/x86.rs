/*
 * Basic CPU control in CR0
 */
pub const X86_CR0_PE: u64 = 0x00000001; /* Protection Enable */
pub const X86_CR0_MP: u64 = 0x00000002; /* Monitor Coprocessor */
pub const X86_CR0_EM: u64 = 0x00000004; /* Emulation */
pub const X86_CR0_TS: u64 = 0x00000008; /* Task Switched */
pub const X86_CR0_ET: u64 = 0x00000010; /* Extension Type */
pub const X86_CR0_NE: u64 = 0x00000020; /* Numeric Error */
pub const X86_CR0_WP: u64 = 0x00010000; /* Write Protect */
pub const X86_CR0_AM: u64 = 0x00040000; /* Alignment Mask */
pub const X86_CR0_NW: u64 = 0x20000000; /* Not Write-through */
pub const X86_CR0_CD: u64 = 0x40000000; /* Cache Disable */
pub const X86_CR0_PG: u64 = 0x80000000; /* Paging */

/*
 * Paging options in CR3
 */
pub const X86_CR3_PWT: u64 = 0x00000008; /* Page Write Through */
pub const X86_CR3_PCD: u64 = 0x00000010; /* Page Cache Disable */
pub const X86_CR3_PCID_MASK: u64 = 0x00000fff; /* PCID Mask */

/*
 * Intel CPU features in CR4
 */
pub const X86_CR4_VME: u64 = 0x00000001; /* enable vm86 extensions */
pub const X86_CR4_PVI: u64 = 0x00000002; /* virtual interrupts flag enable */
pub const X86_CR4_TSD: u64 = 0x00000004; /* disable time stamp at ipl 3 */
pub const X86_CR4_DE: u64 = 0x00000008; /* enable debugging extensions */
pub const X86_CR4_PSE: u64 = 0x00000010; /* enable page size extensions */
pub const X86_CR4_PAE: u64 = 0x00000020; /* enable physical address extensions */
pub const X86_CR4_MCE: u64 = 0x00000040; /* Machine check enable */
pub const X86_CR4_PGE: u64 = 0x00000080; /* enable global pages */
pub const X86_CR4_PCE: u64 = 0x00000100; /* enable performance counters at ipl 3 */
pub const X86_CR4_OSFXSR: u64 = 0x00000200; /* enable fast FPU save and restore */
pub const X86_CR4_OSXMMEXCPT: u64 = 0x00000400; /* enable unmasked SSE exceptions */
pub const X86_CR4_VMXE: u64 = 0x00002000; /* enable VMX virtualization */
pub const X86_CR4_RDWRGSFS: u64 = 0x00010000; /* enable RDWRGSFS support */
pub const X86_CR4_PCIDE: u64 = 0x00020000; /* enable PCID support */
pub const X86_CR4_OSXSAVE: u64 = 0x00040000; /* enable xsave and xrestore */
pub const X86_CR4_SMEP: u64 = 0x00100000; /* enable SMEP support */
pub const X86_CR4_SMAP: u64 = 0x00200000; /* enable SMAP support */

pub const X86_EFER_LME: u64 = 1 << 8;
pub const X86_EFER_LMA: u64 = 1 << 10;

/* AMD64 MSR's */
pub const MSR_EFER: u32 = 0xc0000080; /* extended features */
pub const MSR_STAR: u32 = 0xc0000081; /* legacy mode SYSCALL target/cs/ss */
pub const MSR_LSTAR: u32 = 0xc0000082; /* long mode SYSCALL target rip */
pub const MSR_CSTAR: u32 = 0xc0000083; /* compat mode SYSCALL target rip */
pub const MSR_SF_MASK: u32 = 0xc0000084; /* syscall flags mask */
pub const MSR_FSBASE: u32 = 0xc0000100; /* base address of the %fs "segment" */
pub const MSR_GSBASE: u32 = 0xc0000101; /* base address of the %gs "segment" */
pub const MSR_KGSBASE: u32 = 0xc0000102; /* base address of the kernel %gs */
pub const MSR_PERFEVSEL0: u32 = 0xc0010000;
pub const MSR_PERFEVSEL1: u32 = 0xc0010001;
pub const MSR_PERFEVSEL2: u32 = 0xc0010002;
pub const MSR_PERFEVSEL3: u32 = 0xc0010003;
pub const MSR_K7_PERFCTR0: u32 = 0xc0010004;
pub const MSR_K7_PERFCTR1: u32 = 0xc0010005;
pub const MSR_K7_PERFCTR2: u32 = 0xc0010006;
pub const MSR_K7_PERFCTR3: u32 = 0xc0010007;
pub const MSR_SYSCFG: u32 = 0xc0010010;
pub const MSR_HWCR: u32 = 0xc0010015;
pub const MSR_IORRBASE0: u32 = 0xc0010016;
pub const MSR_IORRMASK0: u32 = 0xc0010017;
pub const MSR_IORRBASE1: u32 = 0xc0010018;
pub const MSR_IORRMASK1: u32 = 0xc0010019;
pub const MSR_TOP_MEM: u32 = 0xc001001a; /* boundary for ram below 4G */
pub const MSR_TOP_MEM2: u32 = 0xc001001d; /* boundary for ram above 4G */
pub const MSR_NB_CFG1: u32 = 0xc001001f; /* NB configuration 1 */
pub const MSR_P_STATE_LIMIT: u32 = 0xc0010061; /* P-state Current Limit Register */
pub const MSR_P_STATE_CONTROL: u32 = 0xc0010062; /* P-state Control Register */
pub const MSR_P_STATE_STATUS: u32 = 0xc0010063; /* P-state Status Register */
// pub const  MSR_P_STATE_CONFIG(n) (0xc0010064 + (n)) /* P-state Config */
pub const MSR_SMM_ADDR: u32 = 0xc0010112; /* SMM TSEG base address */
pub const MSR_SMM_MASK: u32 = 0xc0010113; /* SMM TSEG address mask */
pub const MSR_IC_CFG: u32 = 0xc0011021; /* Instruction Cache Configuration */
pub const MSR_K8_UCODE_UPDATE: u32 = 0xc0010020; /* update microcode */
pub const MSR_MC0_CTL_MASK: u32 = 0xc0010044;
pub const MSR_VM_CR: u32 = 0xc0010114; /* SVM: feature control */
pub const MSR_VM_HSAVE_PA: u32 = 0xc0010117; /* SVM: host save area address */

pub const MSR_SYSENTER_CS_MSR: u32 = 0x174;
pub const MSR_SYSENTER_ESP_MSR: u32 = 0x175;
pub const MSR_SYSENTER_EIP_MSR: u32 = 0x176;

pub const MSR_TSC: u32 = 0x010;

pub const MSR_IA32_TSC_AUX: u32 = 0xc0000103;
