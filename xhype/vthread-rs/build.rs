use std::env;

fn main() {
	println!("cargo:rustc-link-lib=framework=Hypervisor");
	let project_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

	println!("cargo:rustc-link-search={}", project_dir); // the "-L" flag
	println!("cargo:rustc-link-lib=hlt"); // the "-l" flag
}
