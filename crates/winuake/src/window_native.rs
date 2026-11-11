//! `IWindowNative` (microsoft.ui.xaml.window.h): the Win32 handle behind a WinUI `Window`. Not in the WinUI metadata.

use core::ffi::c_void;
use windows_core::{HRESULT, IUnknown, Interface, Result};

windows_core::imp::define_interface!(IWindowNative, IWindowNative_Vtbl, 0xeecdbf0e_bae9_4cb6_a68e_9598e1cb57bb);
windows_core::imp::interface_hierarchy!(IWindowNative, windows_core::IUnknown);

impl core::ops::Deref for IWindowNative {
    type Target = IUnknown;

    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}

#[repr(C)]
pub struct IWindowNative_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub window_handle: unsafe extern "system" fn(this: *mut c_void, handle: *mut isize) -> HRESULT,
}

impl IWindowNative {
    pub fn handle(&self) -> Result<isize> {
        let mut handle = 0isize;
        unsafe { (Interface::vtable(self).window_handle)(Interface::as_raw(self), &mut handle).ok()? };
        Ok(handle)
    }
}
