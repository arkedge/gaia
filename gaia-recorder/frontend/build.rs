use std::process::Command;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed=package.json");
    println!("cargo:rerun-if-changed=pnpm-lock.yaml");
    println!("cargo:rerun-if-changed=index.html");
    println!("cargo:rerun-if-changed=src/");

    // Copy frontend source into OUT_DIR
    let frontend_build_dir = out_dir.join("frontend");
    copy_frontend_dir(".", &frontend_build_dir).unwrap();

    let status = Command::new("corepack")
        .arg("enable")
        .current_dir(&frontend_build_dir)
        .status()
        .expect("failed to execute corepack");
    assert!(status.success(), "failed to install pnpm via corepack");

    let status = Command::new("pnpm")
        .arg("install")
        .current_dir(&frontend_build_dir)
        .status()
        .expect("failed to execute pnpm");
    assert!(status.success(), "failed to install deps for frontend");

    let frontend_out_dir = out_dir.join("frontend_dist");
    let status = Command::new("pnpm")
        .current_dir(&frontend_build_dir)
        .arg("run")
        .arg("build")
        .arg("--outDir")
        .arg(&frontend_out_dir)
        .status()
        .expect("failed to execute pnpm build");
    assert!(status.success(), "failed to build frontend");
}

fn copy_frontend_dir(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            if entry.file_name().to_str() == Some("node_modules") {
                continue;
            }
            if entry.file_name().to_str() == Some("target") {
                continue;
            }
            if entry.file_name().to_str() == Some("dist") {
                continue;
            }
            copy_frontend_dir(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}
