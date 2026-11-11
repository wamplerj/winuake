//! Copies the Windows App SDK bootstrap DLL (fetched by `cargo xtask gen-bindings`) next to the built executable.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest_directory = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let workspace_root =
        manifest_directory.parent().and_then(|path| path.parent()).expect("crate lives in workspace/crates");
    let bootstrap_dll = workspace_root
        .join("target")
        .join("winappsdk")
        .join("2.6.5")
        .join("Foundation")
        .join("runtimes")
        .join("win-x64")
        .join("native")
        .join("Microsoft.WindowsAppRuntime.Bootstrap.dll");
    println!("cargo:rerun-if-changed={}", bootstrap_dll.display());

    if !bootstrap_dll.exists() {
        println!("cargo:warning=bootstrap DLL missing; run `cargo xtask gen-bindings` first");
        return;
    }

    // OUT_DIR is target/<profile>/build/<crate>-<hash>/out; the executable lives three levels up.
    let out_directory = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let profile_directory = out_directory.ancestors().nth(3).expect("OUT_DIR is nested under the profile directory");
    std::fs::copy(&bootstrap_dll, profile_directory.join("Microsoft.WindowsAppRuntime.Bootstrap.dll"))
        .expect("copy bootstrap DLL");
}
