use std::env;

fn main() {
    let target = env::var("TARGET").unwrap();
    if target == "armv7-unknown-linux-gnueabihf" {
        println!("cargo::rustc-link-lib=wayland-client");
        println!("cargo::rustc-link-lib=wayland-cursor");
        println!("cargo::rustc-link-lib=gstwayland-1.0");
        println!("cargo::rustc-link-lib=gstwaylandsink");
        println!("cargo::rustc-link-lib=xkbcommon");
    }
}
