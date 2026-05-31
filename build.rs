use std::path::Path;

fn main() {
    let root = env!("CARGO_MANIFEST_DIR");
    let dave_build = format!("{root}/third_party/libdave/cpp/build");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let triplet = match (target_os.as_str(), arch.as_str()) {
        ("macos", "x86_64") => "x64-osx",
        ("macos", _) => "arm64-osx",
        ("linux", "aarch64") => "arm64-linux",
        ("linux", _) => "x64-linux",
        _ => "arm64-osx",
    };
    let dave_libs = format!("{dave_build}/vcpkg_installed/{triplet}/lib");

    if !Path::new(&format!("{dave_build}/libdave.a")).exists() {
        panic!("libdave.a not found at {dave_build}; build third_party/libdave/cpp first");
    }

    println!("cargo:rustc-link-search=native={dave_build}");
    println!("cargo:rustc-link-search=native={dave_libs}");

    let vcpkg_libs = [
        "mlspp",
        "hpke",
        "mls_vectors",
        "mls_ds",
        "tls_syntax",
        "bytes",
        "ssl",
        "crypto",
    ];

    if target_os == "macos" {
        println!("cargo:rustc-link-lib=static=dave");
        for lib in vcpkg_libs {
            println!("cargo:rustc-link-lib=static={lib}");
        }
        println!("cargo:rustc-link-lib=dylib=c++");
        println!("cargo:rustc-link-lib=framework=VideoToolbox");
        println!("cargo:rustc-link-lib=framework=CoreMedia");
        println!("cargo:rustc-link-lib=framework=CoreVideo");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
    } else {
        println!("cargo:rustc-link-arg=-Wl,--start-group");
        println!("cargo:rustc-link-arg={dave_build}/libdave.a");
        for lib in vcpkg_libs {
            println!("cargo:rustc-link-arg={dave_libs}/lib{lib}.a");
        }
        println!("cargo:rustc-link-arg=-Wl,--end-group");
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={dave_build}/libdave.a");
}
