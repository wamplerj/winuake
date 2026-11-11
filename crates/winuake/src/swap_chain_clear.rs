//! Phase 0 spike: attach a Direct3D 11 composition swap chain to a XAML `SwapChainPanel` and clear it to a solid color.

use crate::bindings::SwapChainPanel;
use crate::graphics::{
    D3D_DRIVER_TYPE_HARDWARE, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC, DXGI_SCALING_STRETCH,
    DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE, DXGI_USAGE_RENDER_TARGET_OUTPUT, HMODULE,
    ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11Texture2D, IDXGIAdapter, IDXGIDevice,
    IDXGIFactory2, IDXGIOutput,
};
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

/// `ISwapChainPanelNative` (microsoft.ui.xaml.media.dxinterop.h) is not in the WinUI metadata, so it is declared by hand.
#[repr(C)]
pub struct ISwapChainPanelNative_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub set_swap_chain: unsafe extern "system" fn(this: *mut c_void, swap_chain: *mut c_void) -> HRESULT,
}

/// Creates a hardware D3D11 device and a composition swap chain of `width` x `height`, binds it to `panel`, and presents one
/// frame cleared to `color` (RGBA, 0..1).
pub fn attach_cleared_swap_chain(panel: &SwapChainPanel, width: u32, height: u32, color: [f32; 4]) -> Result<()> {
    unsafe {
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT as u32,
            None,
            D3D11_SDK_VERSION as u32,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .ok()?;
        let device = device.ok_or_else(windows_core::Error::empty)?;
        let context = context.ok_or_else(windows_core::Error::empty)?;

        let factory: IDXGIFactory2 = device.cast::<IDXGIDevice>()?.GetAdapter()?.GetParent()?;
        let description = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            Stereo: false.into(),
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            BufferUsage: DXGI_USAGE(DXGI_USAGE_RENDER_TARGET_OUTPUT),
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
            Flags: 0,
        };
        let swap_chain = factory.CreateSwapChainForComposition(&device, &description, None::<&IDXGIOutput>)?;

        let native: ISwapChainPanelNative = panel.cast()?;
        (Interface::vtable(&native).set_swap_chain)(Interface::as_raw(&native), Interface::as_raw(&swap_chain)).ok()?;

        let back_buffer: ID3D11Texture2D = swap_chain.GetBuffer(0)?;
        let mut render_target: Option<ID3D11RenderTargetView> = None;
        device.CreateRenderTargetView(&back_buffer, None, Some(&mut render_target)).ok()?;
        let render_target = render_target.ok_or_else(windows_core::Error::empty)?;
        context.ClearRenderTargetView(&render_target, &color);
        swap_chain.Present(1, Default::default()).ok()
    }
}
