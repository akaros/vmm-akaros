/* SPDX-License-Identifier: GPL-2.0-only */

fn main() {
    cc::Build::new().file("c_src/cpuid.c").compile("cpuid");
    println!("cargo:rustc-link-lib=framework=Hypervisor");
}
