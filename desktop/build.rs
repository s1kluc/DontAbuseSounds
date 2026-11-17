use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=DAS_CAPTURE_DLL");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("capture-effect.dll");
    if let Some(source) = env::var_os("DAS_CAPTURE_DLL") {
        let source = PathBuf::from(source);
        println!("cargo:rerun-if-changed={}", source.display());
        fs::copy(source, out).expect("copy the built capture effect");
    } else {
        fs::write(out, []).expect("mark capture setup as unavailable");
    }
    tauri_build::build();
}
