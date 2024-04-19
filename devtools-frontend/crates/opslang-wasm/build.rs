use std::fs;
use std::process::Command;
use std::{env, path::PathBuf};

fn main() {
    // no build logic on wasm32 build (cargo build called from wasm-pack)
    let target = env::var("TARGET").unwrap();
    if target.contains("wasm32") {
        return;
    }

    // Just called cargo build (target may be x86_64 or aarch64)
    // But, now we build wasm by wasm-pack

    let out_dir = env::var("OUT_DIR").unwrap();

    // pass dist directory path to dependents crate (via package.links)
    // it can be used as DEP_OPSLANG_WASM_OUT_DIR in dependents build.rs
    println!("cargo:out_dir={}", out_dir);

    // Of course we think we should copy source dir into $OUT_DIR
    //  & build(wasm-pack build) in $OUT_DIR,
    //  we can't be happy with cargo-metadata called from wasm-pack build
    //  (from cargo build only.not from cargo package)
    let out_dir = PathBuf::from(out_dir);

    // Let's go
    let status = Command::new("wasm-pack")
        .arg("build")
        .arg("--weak-refs")
        .arg("--target")
        .arg("web")
        .arg("--release")
        .arg("--out-dir")
        .arg(out_dir)
        .status()
        .expect("failed to execute wasm-pack");
    assert!(status.success(), "failed to wasm-pack build");

    // wasm-pack build (cargo build --target wasm32) generates Cargo.lock
    // On cargo build, it's fine.
    // On cargo package, it cause a catastrophe!!! (it can't be exists in source directory)
    fs::remove_file("Cargo.lock").unwrap_or(());
}
