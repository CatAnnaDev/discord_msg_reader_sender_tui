use std::path::Path;

fn main() {
    let root = env!("CARGO_MANIFEST_DIR");
    let dave_build = format!("{root}/third_party/libdave/cpp/build");
    let triplet = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86_64") => "x64-osx",
        _ => "arm64-osx",
    };
    let dave_libs = format!("{dave_build}/vcpkg_installed/{triplet}/lib");

    if !Path::new(&format!("{dave_build}/libdave.a")).exists() {
        panic!("libdave.a not found at {dave_build}; build third_party/libdave/cpp first");
    }

    println!("cargo:rustc-link-search=native={dave_build}");
    println!("cargo:rustc-link-search=native={dave_libs}");

    println!("cargo:rustc-link-lib=static=dave");
    println!("cargo:rustc-link-lib=static=mlspp");
    println!("cargo:rustc-link-lib=static=hpke");
    println!("cargo:rustc-link-lib=static=mls_vectors");
    println!("cargo:rustc-link-lib=static=mls_ds");
    println!("cargo:rustc-link-lib=static=tls_syntax");
    println!("cargo:rustc-link-lib=static=bytes");
    println!("cargo:rustc-link-lib=static=ssl");
    println!("cargo:rustc-link-lib=static=crypto");

    println!("cargo:rustc-link-lib=dylib=c++");

    println!("cargo:rustc-link-lib=framework=VideoToolbox");
    println!("cargo:rustc-link-lib=framework=CoreMedia");
    println!("cargo:rustc-link-lib=framework=CoreVideo");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={dave_build}/libdave.a");
}
