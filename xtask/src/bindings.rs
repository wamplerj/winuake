//! Generate Rust bindings for WinUI 3 + the few Win32 APIs Winuake needs, via `windows-bindgen`.
//!
//! Two outputs, because composition (subclassing `Application`) only exists in bindgen's minimal mode,
//! which omits ergonomic wrappers:
//! - `bindings.rs`: full ergonomic bindings (windows, controls, statics such as `Application::Start`).
//! - `composition.rs`: minimal bindings used only to implement `Application` + `IXamlMetadataProvider`.

use crate::winappsdk;
use std::error::Error;
use std::path::{Path, PathBuf};

/// Types Winuake uses. Grow this list as phases need more; bindgen pulls in transitive dependencies.
const ERGONOMIC_FILTERS: &[&str] = &[
    "Microsoft.UI.Xaml.Application::{Start, get_Resources, get_Current}",
    "Microsoft.UI.Xaml.ApplicationInitializationCallback",
    "Microsoft.UI.Xaml.Window::{CreateInstance, Activate, put_Content, put_Title}",
    "Microsoft.UI.Xaml.ResourceDictionary::{CreateInstance, get_MergedDictionaries}",
    "Microsoft.UI.Xaml.Controls.XamlControlsResources::CreateInstance",
    "Microsoft.UI.Xaml.Controls.Panel::get_Children",
    "Microsoft.UI.Xaml.Controls.Grid::CreateInstance",
    "Microsoft.UI.Xaml.Controls.Button::CreateInstance",
    "Microsoft.UI.Xaml.Controls.ContentControl::put_Content",
    "Microsoft.UI.Xaml.Controls.TabView::{CreateInstance, get_TabItems}",
    "Microsoft.UI.Xaml.Controls.TabViewItem::{CreateInstance, put_Header}",
    "Microsoft.UI.Xaml.Controls.TextBlock::{CreateInstance, put_Text}",
    "Microsoft.UI.Xaml.Controls.SwapChainPanel::CreateInstance",
    "Microsoft.UI.Xaml.FrameworkElement::{put_Width, put_Height}",
    "Microsoft.UI.Xaml.XamlTypeInfo.XamlControlsXamlMetaDataProvider::CreateInstance",
    "Microsoft.UI.Xaml.XamlTypeInfo.IXamlControlsXamlMetaDataProviderStatics",
    "Microsoft.UI.Xaml.Markup.IXamlMetadataProvider::{GetXamlType, GetXamlTypeByFullName, GetXmlnsDefinitions}",
    "Windows.Foundation.PropertyValue::CreateString",
    "Windows.Foundation.Collections.IVector",
    "Windows.Foundation.Collections.IIterable",
    "Microsoft.UI.Xaml.IDependencyPropertyStatics",
    "Microsoft.UI.Xaml.IPropertyMetadataFactory",
    "Microsoft.UI.Xaml.IPropertyMetadataStatics",
    "Microsoft.UI.Xaml.Controls.ITabViewStatics2",
    // Factory/statics interfaces bindgen references from class wrappers but does not emit for member-filtered classes.
    "Microsoft.UI.Xaml.IApplicationFactory",
    "Microsoft.UI.Xaml.Controls.Primitives.IButtonBaseFactory",
    "Microsoft.UI.Xaml.Controls.Primitives.IButtonBaseStatics",
    "Microsoft.UI.Xaml.Controls.IButtonStatics",
    "Microsoft.UI.Xaml.Controls.IContentControlFactory",
    "Microsoft.UI.Xaml.Controls.IContentControlStatics",
    "Microsoft.UI.Xaml.Controls.IControlFactory",
    "Microsoft.UI.Xaml.Controls.IControlStatics",
    "Microsoft.UI.Xaml.IDependencyObjectFactory",
    "Microsoft.UI.Xaml.IFrameworkElementFactory",
    "Microsoft.UI.Xaml.IFrameworkElementStatics",
    "Microsoft.UI.Xaml.Controls.IGridStatics",
    "Microsoft.UI.Xaml.Controls.IListViewItemFactory",
    "Microsoft.UI.Xaml.Controls.IPanelFactory",
    "Microsoft.UI.Xaml.Controls.IPanelStatics",
    "Microsoft.UI.Xaml.Controls.Primitives.ISelectorItemFactory",
    "Microsoft.UI.Xaml.Controls.Primitives.ISelectorItemStatics",
    "Microsoft.UI.Xaml.Controls.ISwapChainPanelStatics",
    "Microsoft.UI.Xaml.Controls.ITabViewItemStatics",
    "Microsoft.UI.Xaml.Controls.ITabViewStatics",
    "Microsoft.UI.Xaml.Controls.ITextBlockStatics",
    "Microsoft.UI.Xaml.IUIElementStatics",
    "Microsoft.UI.Xaml.IWindowStatics",
    "Microsoft.UI.Xaml.Controls.IXamlControlsResourcesStatics",
];

/// Win32 graphics used to render into a `SwapChainPanel`.
const GRAPHICS_FILTERS: &[&str] = &[
    "D3D11CreateDevice",
    "ID3D11Device",
    "ID3D11DeviceContext",
    "ID3D11Texture2D",
    "ID3D11RenderTargetView",
    "D3D11_SDK_VERSION",
    "D3D11_CREATE_DEVICE_BGRA_SUPPORT",
    "D3D_DRIVER_TYPE_HARDWARE",
    "IDXGIDevice",
    "IDXGIAdapter",
    "IDXGIObject",
    "IDXGIFactory2",
    "IDXGISwapChain1",
    "IDXGISwapChain",
    "IDXGIDeviceSubObject",
    "DXGI_SWAP_CHAIN_DESC1",
    "DXGI_USAGE_RENDER_TARGET_OUTPUT",
    "DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL",
    "DXGI_SCALING_STRETCH",
    "DXGI_ALPHA_MODE_PREMULTIPLIED",
    "DXGI_FORMAT_B8G8R8A8_UNORM",
];

const COMPOSITION_FILTERS: &[&str] = &[
    "Microsoft.UI.Xaml.Application",
    "Microsoft.UI.Xaml.IApplicationFactory",
    "Microsoft.UI.Xaml.IApplicationOverrides",
    "Microsoft.UI.Xaml.LaunchActivatedEventArgs",
    "Microsoft.UI.Xaml.Markup.IXamlMetadataProvider",
];

pub fn generate(workspace_root: &Path) -> Result<(), Box<dyn Error>> {
    let foundation = winappsdk::ensure_package(workspace_root, winappsdk::PACKAGE_FOUNDATION)?;
    let interactive = winappsdk::ensure_package(workspace_root, winappsdk::PACKAGE_INTERACTIVE_EXPERIENCES)?;
    let winui = winappsdk::ensure_package(workspace_root, winappsdk::PACKAGE_WINUI)?;

    let mut metadata = winappsdk::winmd_files(&foundation.join("metadata"))?;
    metadata.extend(winappsdk::winmd_files(&interactive.join("metadata").join("10.0.18362.0"))?);
    metadata.extend(winappsdk::winmd_files(&winui.join("metadata"))?);

    let source_directory = workspace_root.join("crates").join("winuake").join("src");

    let ergonomic_output = source_directory.join("bindings.rs");
    let mut ergonomic = builder_with_inputs(&metadata);
    ergonomic.output(&ergonomic_output).filters(ERGONOMIC_FILTERS.iter().copied()).flat();
    ergonomic.write();

    let composition_output = source_directory.join("composition.rs");
    let mut composition = builder_with_inputs(&metadata);
    composition
        .output(&composition_output)
        .filters(COMPOSITION_FILTERS.iter().copied())
        .flat()
        .minimal()
        .compose("Microsoft.UI.Xaml.Application")
        .implement("Microsoft.UI.Xaml.IApplicationOverrides")
        .implement("Microsoft.UI.Xaml.Markup.IXamlMetadataProvider");
    composition.write();

    let graphics_output = source_directory.join("graphics.rs");
    let mut graphics = windows_bindgen::builder();
    graphics.input_default().output(&graphics_output).filters(GRAPHICS_FILTERS.iter().copied()).flat();
    graphics.write();

    eprintln!("wrote bindings.rs, composition.rs, graphics.rs from {} winmd files", metadata.len());
    Ok(())
}

fn builder_with_inputs(metadata: &[PathBuf]) -> windows_bindgen::Bindgen {
    let mut builder = windows_bindgen::builder();
    builder.input_default();
    for file in metadata {
        builder.input(file);
    }
    builder
}
