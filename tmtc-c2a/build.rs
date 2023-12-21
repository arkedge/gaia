use std::process::Command;
use std::{env, path::PathBuf};

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
        let devtools_out_dir = out_dir.join("devtools_dist");
        let status = Command::new("yarn")
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
