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

    // copy frontend source into OUT_DIR
    let devtools_build_dir = out_dir.join("devtools_frontend");
    copy_devtools_dir(".", &devtools_build_dir).unwrap();

    let status = Command::new("corepack")
        .arg("enable")
        .current_dir(&devtools_build_dir)
        .status()
        .expect("failed to execute corepack");
    assert!(status.success(), "failed to install pnpm via corepack");

    let status = Command::new("pnpm")
        .arg("install")
        .current_dir(&devtools_build_dir)
        .status()
        .expect("failed to execute pnpm");
    assert!(status.success(), "failed to install deps for frontend");

    // parepare crate dir
    let crate_root_dir = out_dir.join("crate_root");

    // copy wasm-opslang dist
    {
        let opslang_pkg_dir = env::var("DEP_WASM_OPSLANG_OUT_DIR").unwrap();
        let opslang_dist_dir = crate_root_dir.join("wasm-opslang").join("pkg");

        copy_devtools_dir(opslang_pkg_dir, opslang_dist_dir).unwrap();
    }

    let devtools_out_dir = out_dir.join("devtools_dist");
    let status = Command::new("pnpm")
        .current_dir(&devtools_build_dir)
        // vite.config.ts にwasmのビルド場所を教えるために環境変数を渡す
        .envs([("DEVTOOLS_CRATE_ROOT", crate_root_dir)])
        .arg("run")
        .arg("build:vite")
        .arg("--outDir")
        .arg(&devtools_out_dir)
        .status()
        .expect("failed to execute yarn");
    assert!(status.success(), "failed to build frontend");
}

fn copy_devtools_dir(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            if entry.file_name().to_str() == Some("node_modules") {
                continue;
            }
            // In `cargo package`, each crate source files are copied to
            // target/package/crate-<version> & threre are target dir
            if entry.file_name().to_str() == Some("target") {
                continue;
            }
            copy_devtools_dir(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}
