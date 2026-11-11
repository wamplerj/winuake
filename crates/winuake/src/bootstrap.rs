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
pub struct WindowsAppRuntime;

impl WindowsAppRuntime {
    pub fn initialize() -> Result<Self> {
        unsafe {
            RoInitialize(RO_INIT_SINGLETHREADED).ok()?;
            MddBootstrapInitialize2(
                WINDOWS_APP_SDK_MAJOR_MINOR,
                core::ptr::null(),
                0,
                BOOTSTRAP_OPTION_ON_NO_MATCH_SHOW_UI | BOOTSTRAP_OPTION_ON_ERROR_FAIL_FAST,
            )
            .ok()?;
        }
        Ok(Self)
    }
}

impl Drop for WindowsAppRuntime {
    fn drop(&mut self) {
        unsafe { MddBootstrapShutdown() };
    }
}
