use std::process::Command;
use std::{env, path::PathBuf};

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    println!("cargo:out_dir={}", out_dir);

    let target = env::var("TARGET").unwrap();

    if !target.contains("wasm32") {
        let out_dir = PathBuf::from(out_dir);

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
    }
}
