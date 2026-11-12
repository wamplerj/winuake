//! Unpackaged-app bootstrap for the Windows App SDK runtime (MddBootstrap) and COM apartment setup.

use windows_core::{HRESULT, Result};

/// Windows App SDK 2.6 -> `major << 16 | minor`.
const WINDOWS_APP_SDK_MAJOR_MINOR: u32 = 0x0002_0006;
const BOOTSTRAP_OPTION_ON_ERROR_FAIL_FAST: i32 = 0x0004;
const BOOTSTRAP_OPTION_ON_NO_MATCH_SHOW_UI: i32 = 0x0008;
const RO_INIT_SINGLETHREADED: i32 = 0;

windows_link::link!("Microsoft.WindowsAppRuntime.Bootstrap.dll" "system" fn MddBootstrapInitialize2(
    major_minor_version: u32,
    version_tag: *const u16,
    minimum_version: u64,
    options: i32
) -> HRESULT);
windows_link::link!("Microsoft.WindowsAppRuntime.Bootstrap.dll" "system" fn MddBootstrapShutdown());
windows_link::link!("combase.dll" "system" fn RoInitialize(init_type: i32) -> HRESULT);

/// Keeps the Windows App SDK runtime loaded; shuts it down on drop.
pub struct WindowsAppRuntime {
    /// Whether this process called `MddBootstrapInitialize2` (and so must shut it down).
    bootstrapped: bool,
}

windows_link::link!("kernel32.dll" "system" fn GetCurrentPackageFullName(length: *mut u32, name: *mut u16) -> i32);

/// `APPMODEL_ERROR_NO_PACKAGE`: the process has no package identity.
const APPMODEL_ERROR_NO_PACKAGE: i32 = 15700;

/// True when running from an MSIX package, which already declares its Windows App SDK dependency.
pub fn is_packaged() -> bool {
    let mut length = 0u32;
    unsafe { GetCurrentPackageFullName(&mut length, core::ptr::null_mut()) != APPMODEL_ERROR_NO_PACKAGE }
}

impl WindowsAppRuntime {
    /// Unpackaged builds load the runtime through the bootstrapper; a packaged build gets it from its framework dependency.
    pub fn initialize() -> Result<Self> {
        unsafe { RoInitialize(RO_INIT_SINGLETHREADED).ok()? };
        if is_packaged() {
            tracing::info!("running packaged; the Windows App SDK comes from the package");
            return Ok(Self { bootstrapped: false });
        }
        unsafe {
            MddBootstrapInitialize2(
                WINDOWS_APP_SDK_MAJOR_MINOR,
                core::ptr::null(),
                0,
                BOOTSTRAP_OPTION_ON_NO_MATCH_SHOW_UI | BOOTSTRAP_OPTION_ON_ERROR_FAIL_FAST,
            )
            .ok()?;
        }
        Ok(Self { bootstrapped: true })
    }
}

impl Drop for WindowsAppRuntime {
    fn drop(&mut self) {
        if self.bootstrapped {
            unsafe { MddBootstrapShutdown() };
        }
    }
}
