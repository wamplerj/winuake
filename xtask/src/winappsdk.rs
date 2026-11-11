//! Download and unpack Windows App SDK NuGet packages (winmd metadata, bootstrap DLL, runtime installer).

use std::error::Error;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

/// Pinned Windows App SDK version. Bump deliberately, then regenerate bindings.
pub const WINDOWS_APP_SDK_VERSION: &str = "2.6.5";

pub const PACKAGE_FOUNDATION: &str = "Foundation";
pub const PACKAGE_INTERACTIVE_EXPERIENCES: &str = "InteractiveExperiences";
pub const PACKAGE_WINUI: &str = "WinUI";
pub const PACKAGE_RUNTIME: &str = "Runtime";

pub fn cache_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join("target").join("winappsdk").join(WINDOWS_APP_SDK_VERSION)
}

/// Ensure `Microsoft.WindowsAppSDK.<package>` is unpacked under the cache; returns its directory.
pub fn ensure_package(workspace_root: &Path, package: &str) -> Result<PathBuf, Box<dyn Error>> {
    let destination = cache_root(workspace_root).join(package);
    if destination.join(".complete").exists() {
        return Ok(destination);
    }

    let package_id = format!("microsoft.windowsappsdk.{}", package.to_lowercase());
    let url = format!(
        "https://api.nuget.org/v3-flatcontainer/{package_id}/{WINDOWS_APP_SDK_VERSION}/{package_id}.{WINDOWS_APP_SDK_VERSION}.nupkg"
    );
    eprintln!("downloading {url}");
    let bytes = ureq::get(&url).call()?.into_body().into_with_config().limit(1 << 30).read_to_vec()?;

    if destination.exists() {
        fs::remove_dir_all(&destination)?;
    }
    fs::create_dir_all(&destination)?;
    zip::ZipArchive::new(Cursor::new(bytes))?.extract(&destination)?;
    fs::write(destination.join(".complete"), WINDOWS_APP_SDK_VERSION)?;
    Ok(destination)
}

/// All `.winmd` files directly inside `directory`.
pub fn winmd_files(directory: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "winmd") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}
