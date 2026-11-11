//! `ISwapChainPanelNative` (microsoft.ui.xaml.media.dxinterop.h) is not in the WinUI metadata, so it is declared by hand.

use core::ffi::c_void;
use windows_core::{HRESULT, IUnknown, Interface, Result};

windows_core::imp::define_interface!(
    ISwapChainPanelNative,
    ISwapChainPanelNative_Vtbl,
    0x63aad0b8_7c24_40ff_85a8_640d944cc325
);
windows_core::imp::interface_hierarchy!(ISwapChainPanelNative, windows_core::IUnknown);

impl core::ops::Deref for ISwapChainPanelNative {
    type Target = IUnknown;

    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}

#[repr(C)]
pub struct ISwapChainPanelNative_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub set_swap_chain: unsafe extern "system" fn(this: *mut c_void, swap_chain: *mut c_void) -> HRESULT,
}

impl ISwapChainPanelNative {
    /// Makes `swap_chain` (any `IDXGISwapChain1`) the content of the panel.
    pub fn attach(&self, swap_chain: &impl Interface) -> Result<()> {
        unsafe { (Interface::vtable(self).set_swap_chain)(Interface::as_raw(self), Interface::as_raw(swap_chain)).ok() }
    }
}
