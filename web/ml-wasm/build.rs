use std::{env, fs, path::PathBuf};

use sha2::{Digest, Sha256};

const EXPECTED_SHA256: &str = "34af46eaea092f8e296fdf1af2cd246dde75367714eeea871f48c913e6109a1b";

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let model = manifest.join("assets/u2netp.bpk");
    println!("cargo:rerun-if-changed={}", model.display());
    let bytes = fs::read(&model).expect(
        "read U2-Net-P Burnpack weights; run `git lfs pull` if this checkout contains a pointer",
    );
    let observed = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(
        observed, EXPECTED_SHA256,
        "U2-Net-P Burnpack checksum mismatch; run `git lfs pull` and retry"
    );
}
