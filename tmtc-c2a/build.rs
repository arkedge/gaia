use std::process::Command;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    tonic_build::configure()
        .file_descriptor_set_path(out_dir.join("tmtc_generic_c2a.bin"))
        .compile(&["./proto/tmtc_generic_c2a.proto"], &["./proto"])
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));

    if std::env::var("SKIP_FRONTEND_BUILD").is_err() {
        println!("cargo:rerun-if-changed=devtools_frontend");

        // copy frontend source into OUT_DIR
        let devtools_build_dir = out_dir.join("devtools_frontend");
        copy_dir_all("devtools_frontend", &devtools_build_dir).unwrap();

        let status = Command::new("yarn")
            .current_dir(&devtools_build_dir)
            .status()
            .expect("failed to build frontend");
        assert!(status.success());

        let devtools_out_dir = out_dir.join("devtools_dist");
        let status = Command::new("yarn")
            .current_dir(&devtools_build_dir)
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

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}
