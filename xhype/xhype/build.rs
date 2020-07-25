/* SPDX-License-Identifier: GPL-2.0-only */

fn main() {
    cc::Build::new().file("c_src/cpuid.c").compile("cpuid");
    cc::Build::new().file("c_src/utils.c").compile("utils");
    cc::Build::new().file("c_src/hlt.s").compile("hlt");
    cc::Build::new()
        .file("c_src/serial_c.c")
        .compile("serial_c");

    println!("cargo:rustc-link-lib=framework=Hypervisor");
}
