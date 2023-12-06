use std::{env, path::PathBuf};
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    tonic_build::configure()
        .file_descriptor_set_path(out_dir.join("tmtc_generic_c2a.bin"))
        .compile(&["./proto/tmtc_generic_c2a.proto"], &["./proto"])
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));

    if std::env::var("SKIP_FRONTEND_BUILD").is_err() {
        let mut env_vars: Vec<(&str, &str)> = vec![];
        #[cfg(feature = "prefer_self_port")]
        {
            env_vars.push(("VITE_PREFER_SELF_PORT", "1"));
        }
        let status = Command::new("yarn")
            .current_dir("devtools_frontend")
            .status()
            .expect("failed to build frontend");
        assert!(status.success());
        let status = Command::new("yarn")
            .current_dir("devtools_frontend")
            .arg("build")
            .envs(env_vars)
            .status()
            .expect("failed to build frontend");
        assert!(status.success());
    }
}
