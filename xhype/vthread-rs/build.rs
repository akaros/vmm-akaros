fn main() {
	println!("cargo:rustc-link-lib=framework=Hypervisor");

	cc::Build::new().file("c_src/cpuid.c").compile("cpuid");
	cc::Build::new().file("c_src/hlt.s").compile("hlt");
}
