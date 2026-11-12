//! Copies the Windows App SDK bootstrap DLL (fetched by `cargo xtask gen-bindings`) next to the built executable.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    embed_resources();

    let manifest_directory = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let workspace_root =
        manifest_directory.parent().and_then(|path| path.parent()).expect("crate lives in workspace/crates");
    // The bootstrap DLL must match the architecture being built.
    let runtime_directory = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "win-arm64",
        Ok("x86") => "win-x86",
        _ => "win-x64",
    };
    let bootstrap_dll = workspace_root
        .join("target")
        .join("winappsdk")
        .join("2.6.5")
        .join("Foundation")
        .join("runtimes")
        .join(runtime_directory)
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
    let destination = profile_directory.join("Microsoft.WindowsAppRuntime.Bootstrap.dll");
    // A running copy of the app keeps its DLL locked; an existing identical copy is fine.
    if let Err(error) = std::fs::copy(&bootstrap_dll, &destination) {
        let same_size = std::fs::metadata(&destination).ok().map(|copy| copy.len())
            == std::fs::metadata(&bootstrap_dll).ok().map(|source| source.len());
        if !same_size {
            panic!("copy bootstrap DLL: {error}");
        }
        println!("cargo:warning=bootstrap DLL is in use; kept the existing copy");
    }
}

/// Icon, version information and the application manifest (DPI awareness, long paths, UTF-8 code page) as a Windows resource.
/// Needs `rc.exe`, so it only runs when the build host is Windows (a Linux `cargo check` for the Windows target skips it).
fn embed_resources() {
    println!("cargo:rerun-if-changed=winuake.manifest");
    println!("cargo:rerun-if-changed=../../assets/icon/winuake.ico");
    if !cfg!(windows) {
        return;
    }
    let version = std::env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
    let mut numbers: Vec<u32> = version.split(['.', '-', '+']).filter_map(|part| part.parse().ok()).collect();
    numbers.resize(4, 0);
    let manifest_directory = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let icon = manifest_directory.join("../../assets/icon/winuake.ico");
    let manifest = manifest_directory.join("winuake.manifest");
    let escape = |path: &std::path::Path| path.display().to_string().replace('\\', "\\\\");
    let resource = format!(
        r#"1 ICON "{icon}"
1 24 "{manifest}"
1 VERSIONINFO
FILEVERSION {a},{b},{c},{d}
PRODUCTVERSION {a},{b},{c},{d}
FILEOS 0x40004
FILETYPE 0x1
{{
  BLOCK "StringFileInfo"
  {{
    BLOCK "040904B0"
    {{
      VALUE "CompanyName", "Winuake contributors"
      VALUE "FileDescription", "Winuake drop-down terminal"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "winuake"
      VALUE "LegalCopyright", "GPL-2.0-or-later. Port of KDE Yakuake."
      VALUE "OriginalFilename", "winuake.exe"
      VALUE "ProductName", "Winuake"
      VALUE "ProductVersion", "{version}"
    }}
  }}
  BLOCK "VarFileInfo"
  {{
    VALUE "Translation", 0x409, 1200
  }}
}}
"#,
        icon = escape(&icon),
        manifest = escape(&manifest),
        a = numbers[0],
        b = numbers[1],
        c = numbers[2],
        d = numbers[3],
    );
    let out_directory = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let resource_file = out_directory.join("winuake.rc");
    std::fs::write(&resource_file, resource).expect("write resource script");
    embed_resource::compile(&resource_file, embed_resource::NONE).manifest_required().expect("compile resources");
}
