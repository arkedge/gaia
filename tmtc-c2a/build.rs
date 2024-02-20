use std::process::Command;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

fn wasm_packages_root() -> PathBuf {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    out_dir.join("wasm_packages")
}

fn wasm_pack(name: &str, devtools_build_dir: &PathBuf) {
    let pkg_outdir = wasm_packages_root().join(name).join("pkg");
    let status = Command::new("pnpm")
        .current_dir(devtools_build_dir)
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

    notalawyer_build::build();
}
