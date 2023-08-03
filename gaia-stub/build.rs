use std::{env, path::PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    tonic_build::configure()
        .file_descriptor_set_path(out_dir.join("broker_descriptor.bin"))
        .compile(&["proto/broker.proto"], &["proto"])
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
    tonic_build::configure()
        .file_descriptor_set_path(out_dir.join("recorder_descriptor.bin"))
        .compile(&["proto/recorder.proto"], &["proto"])
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
