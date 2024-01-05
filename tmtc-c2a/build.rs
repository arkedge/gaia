use std::process::Command;
use std::{env, path::PathBuf};

fn wasm_packages_root() -> PathBuf {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let wasm_outdir = out_dir.join("wasm_packages");
    wasm_outdir
}

fn wasm_pack(name: &str) {
    let pkg_outdir = wasm_packages_root().join(name).join("pkg");
    let status = Command::new("yarn")
        .current_dir("devtools_frontend")
        .arg("run")
        .arg("crate")
        .arg(name)
        .arg("--out-dir")
        .arg(&pkg_outdir)
        .status()
        .expect("failed to build frontend");
    assert!(status.success());
}

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    tonic_build::configure()
        .file_descriptor_set_path(out_dir.join("tmtc_generic_c2a.bin"))
        .compile(&["./proto/tmtc_generic_c2a.proto"], &["./proto"])
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));

    if std::env::var("SKIP_FRONTEND_BUILD").is_err() {
        println!("cargo:rerun-if-changed=devtools_frontend");
        let status = Command::new("yarn")
            .current_dir("devtools_frontend")
            .status()
            .expect("failed to build frontend");
        assert!(status.success());

        wasm_pack("wasm-interpolate");
        let devtools_out_dir = out_dir.join("devtools_dist");
        let status = Command::new("yarn")
            // vite.config.ts にwasmのビルド場所を教えるために環境変数を渡す
            .envs([("DEVTOOLS_CRATE_ROOT", wasm_packages_root())])
            .current_dir("devtools_frontend")
            .arg("run")
            .arg("build:vite")
            .arg("--")
            .arg("--outDir")
            .arg(&devtools_out_dir)
            .status()
            .expect("failed to build frontend");
        assert!(status.success());
    }
}
