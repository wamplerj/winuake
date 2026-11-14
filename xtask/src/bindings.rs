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
    "Microsoft.UI.Xaml.Application::{Start, get_Resources, get_Current, Exit}",
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
    "Microsoft.UI.Xaml.FrameworkElement::{put_Width, put_Height, put_HorizontalAlignment, put_VerticalAlignment, add_SizeChanged, remove_SizeChanged, get_ActualWidth, get_ActualHeight}",
    "Microsoft.UI.Xaml.SizeChangedEventHandler",
    "Microsoft.UI.Xaml.SizeChangedEventArgs::get_NewSize",
    "Microsoft.UI.Xaml.Controls.SwapChainPanel::{get_CompositionScaleX, get_CompositionScaleY, add_CompositionScaleChanged}",
    "Microsoft.UI.Xaml.UIElement::{add_KeyDown, add_PreviewKeyDown, Focus, add_PointerPressed, add_PointerMoved, add_PointerReleased, add_PointerWheelChanged, CapturePointer, ReleasePointerCapture}",
    "Microsoft.UI.Xaml.Input.PointerEventHandler",
    "Microsoft.UI.Xaml.Input.PointerRoutedEventArgs::{get_Pointer, GetCurrentPoint, put_Handled}",
    "Microsoft.UI.Input.IPointerPointStatics",
    "Microsoft.UI.Input.PointerPoint::{get_Position, get_Properties}",
    "Microsoft.UI.Input.PointerPointProperties::{get_IsLeftButtonPressed, get_IsMiddleButtonPressed, get_IsRightButtonPressed, get_MouseWheelDelta, get_PointerUpdateKind}",
    "Microsoft.UI.Xaml.Input.KeyEventHandler",
    "Microsoft.UI.Xaml.IRoutedEventArgsFactory",
    "Microsoft.UI.Xaml.Input.KeyRoutedEventArgs::{get_Key, put_Handled}",
    "Microsoft.UI.Xaml.Controls.ContentControl::CreateInstance",
    "Microsoft.UI.Xaml.Controls.Control::{put_IsTabStop, put_UseSystemFocusVisuals, put_HorizontalContentAlignment, put_VerticalContentAlignment}",
    "Microsoft.UI.Dispatching.DispatcherQueue::{GetForCurrentThread, TryEnqueue}",
    "Microsoft.UI.Dispatching.DispatcherQueueHandler",
    "Microsoft.UI.Xaml.XamlTypeInfo.XamlControlsXamlMetaDataProvider::CreateInstance",
    "Microsoft.UI.Xaml.XamlTypeInfo.IXamlControlsXamlMetaDataProviderStatics",
    "Microsoft.UI.Xaml.Markup.IXamlMetadataProvider::{GetXamlType, GetXamlTypeByFullName, GetXmlnsDefinitions}",
    "Windows.Foundation.PropertyValue::{CreateString, CreateBoolean}",
    "Windows.Foundation.Collections.IVector",
    "Windows.Foundation.Collections.IIterable",
    // Phase 4: tab bar, title bar and split panes are loaded from XAML markup and positioned with margins.
    "Microsoft.UI.Xaml.Markup.XamlReader::Load",
    "Microsoft.UI.Xaml.Markup.IXamlReaderStatics",
    "Microsoft.UI.Xaml.FrameworkElement::{FindName, put_Margin}",
    "Microsoft.UI.Xaml.Thickness",
    "Microsoft.UI.Xaml.UIElement::{put_Visibility, add_GotFocus, add_LostFocus, add_PointerEntered, add_PointerExited}",
    "Microsoft.UI.Xaml.Visibility",
    "Microsoft.UI.Xaml.RoutedEventHandler",
    "Microsoft.UI.Xaml.Controls.TextBox::{get_Text, put_Text, SelectAll, add_TextChanged}",
    "Microsoft.UI.Xaml.Controls.TextChangedEventHandler",
    "Microsoft.UI.Xaml.Controls.ITextBoxStatics",
    "Microsoft.UI.Xaml.Controls.ITextBoxFactory",
    // Phase 5: settings window controls.
    "Microsoft.UI.Xaml.Controls.Primitives.ToggleButton::{get_IsChecked, put_IsChecked}",
    "Microsoft.UI.Xaml.Controls.Primitives.ButtonBase::add_Click",
    "Microsoft.UI.Xaml.Controls.Primitives.IToggleButtonFactory",
    "Microsoft.UI.Xaml.Controls.Primitives.IToggleButtonStatics",
    "Microsoft.UI.Xaml.Window::{Close, add_Closed}",
    "Windows.Foundation.IReference",
    // Phase 7: translucent window backdrop.
    "Microsoft.UI.Xaml.Window::{get_SystemBackdrop, put_SystemBackdrop}",
    "Microsoft.UI.Xaml.Media.DesktopAcrylicBackdrop::CreateInstance",
    "Microsoft.UI.Xaml.Media.ISystemBackdropFactory",
    "Microsoft.UI.Xaml.Media.IDesktopAcrylicBackdropFactory",
    // Resize grips: a size cursor on the window edges.
    "Microsoft.UI.Xaml.UIElement::put_ProtectedCursor",
    "Microsoft.UI.Input.InputSystemCursor::Create",
    "Microsoft.UI.Input.InputSystemCursorShape",
    "Microsoft.UI.Input.InputCursor",
    "Microsoft.UI.Input.IInputSystemCursorStatics",
    "Microsoft.UI.Input.IInputCursorStatics",
    "Windows.UI.Core.ICoreCursorFactory",
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
    // Dropdown for fields with a fixed set of choices.
    "Microsoft.UI.Xaml.Controls.Primitives.Selector::{get_SelectedIndex, put_SelectedIndex, add_SelectionChanged}",
    "Microsoft.UI.Xaml.Controls.SelectionChangedEventHandler",
    "Microsoft.UI.Xaml.Controls.ComboBox::CreateInstance",
    "Microsoft.UI.Xaml.Controls.Primitives.ISelectorFactory",
    "Microsoft.UI.Xaml.Controls.Primitives.ISelectorStatics",
    "Microsoft.UI.Xaml.Controls.IComboBoxFactory",
    "Microsoft.UI.Xaml.Controls.IComboBoxStatics",
    "Microsoft.UI.Xaml.Controls.IItemsControlFactory",
    "Microsoft.UI.Xaml.Controls.IItemsControlStatics",
    "Microsoft.UI.Xaml.Controls.ISelectionChangedEventArgsFactory",
];

/// Win32 APIs used by the ConPTY backend in `winuake-term`.
const CONPTY_FILTERS: &[&str] = &[
    "CreatePipe",
    "CreatePseudoConsole",
    "ResizePseudoConsole",
    "ClosePseudoConsole",
    "InitializeProcThreadAttributeList",
    "UpdateProcThreadAttribute",
    "DeleteProcThreadAttributeList",
    "CreateProcessW",
    "WaitForSingleObject",
    "GetExitCodeProcess",
    "TerminateProcess",
    "CloseHandle",
    "STARTUPINFOEXW",
    "PROCESS_INFORMATION",
    "COORD",
    "HPCON",
    "EXTENDED_STARTUPINFO_PRESENT",
    "CREATE_UNICODE_ENVIRONMENT",
    "PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE",
    "INFINITE",
];

/// Direct3D 11 / DXGI / Direct2D / DirectWrite used by the terminal renderer in `winuake-render`.
const RENDER_FILTERS: &[&str] = &[
    "D3D11CreateDevice",
    "ID3D11Device",
    "ID3D11DeviceContext",
    "ID3D11Texture2D",
    "ID3D11Resource",
    "D3D11_TEXTURE2D_DESC",
    "D3D11_USAGE_STAGING",
    "D3D11_CPU_ACCESS_READ",
    "D3D11_MAP_READ",
    "D3D11_MAPPED_SUBRESOURCE",
    "D3D11_SDK_VERSION",
    "D3D11_CREATE_DEVICE_BGRA_SUPPORT",
    "D3D_DRIVER_TYPE_HARDWARE",
    "D3D_DRIVER_TYPE_WARP",
    "IDXGIDevice",
    "IDXGIAdapter",
    "IDXGIObject",
    "IDXGIFactory2",
    "IDXGISwapChain1",
    "IDXGISwapChain",
    "IDXGISwapChain2",
    "DXGI_MATRIX_3X2_F",
    "IDXGIDeviceSubObject",
    "IDXGISurface",
    "DXGI_SWAP_CHAIN_DESC1",
    "DXGI_USAGE_RENDER_TARGET_OUTPUT",
    "DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL",
    "DXGI_SCALING_STRETCH",
    "DXGI_ALPHA_MODE_PREMULTIPLIED",
    "DXGI_FORMAT_B8G8R8A8_UNORM",
    "D2D1CreateFactory",
    "ID2D1Factory1",
    "ID2D1Device",
    "ID2D1DeviceContext",
    "ID2D1RenderTarget",
    "ID2D1Bitmap1",
    "ID2D1SolidColorBrush",
    "ID2D1Brush",
    "D2D1_BITMAP_PROPERTIES1",
    "D2D1_FACTORY_TYPE_SINGLE_THREADED",
    "D2D1_DEVICE_CONTEXT_OPTIONS_NONE",
    "D2D1_BITMAP_OPTIONS_TARGET",
    "D2D1_BITMAP_OPTIONS_CANNOT_DRAW",
    "D2D1_ANTIALIAS_MODE_ALIASED",
    "D2D1_ANTIALIAS_MODE_PER_PRIMITIVE",
    "D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE",
    "D2D1_DRAW_TEXT_OPTIONS_NONE",
    "D2D1_DRAW_TEXT_OPTIONS_CLIP",
    "D2D_RECT_F",
    "D3DCOLORVALUE",
    "D2D1_PIXEL_FORMAT",
    "D2D1_ALPHA_MODE_PREMULTIPLIED",
    "D2D1_BRUSH_PROPERTIES",
    "DWriteCreateFactory",
    "IDWriteFactory",
    "IDWriteTextFormat",
    "IDWriteTextLayout",
    "IDWriteFontCollection",
    "DWRITE_FONT_WEIGHT_NORMAL",
    "DWRITE_FONT_WEIGHT_BOLD",
    "DWRITE_FONT_STYLE_NORMAL",
    "DWRITE_FONT_STYLE_ITALIC",
    "DWRITE_FONT_STRETCH_NORMAL",
    "DWRITE_FACTORY_TYPE_SHARED",
    "DWRITE_WORD_WRAPPING_NO_WRAP",
    "DWRITE_TEXT_METRICS",
    "DWRITE_LINE_METRICS",
];

/// Win32 window management used by the drop-down host in `winuake`.
const WINDOW_FILTERS: &[&str] = &[
    "GetDC",
    "ReleaseDC",
    "GetDeviceCaps",
    "LOGPIXELSY",
    "ChooseFontW",
    "CHOOSEFONTW",
    "LOGFONTW",
    "CF_SCREENFONTS",
    "CF_FIXEDPITCHONLY",
    "CF_INITTOLOGFONTSTRUCT",
    "CF_NOSCRIPTSEL",
    "CF_LIMITSIZE",
    "SHBrowseForFolderW",
    "BROWSEINFOW",
    "SHGetPathFromIDListW",
    "CoTaskMemFree",
    "BIF_RETURNONLYFSDIRS",
    "BIF_NEWDIALOGSTYLE",
    "BIF_USENEWUI",
    "MF_POPUP",
    "RegCreateKeyExW",
    "RegSetValueExW",
    "RegDeleteValueW",
    "RegGetValueW",
    "RegCloseKey",
    "HKEY",
    "HKEY_CURRENT_USER",
    "KEY_SET_VALUE",
    "KEY_QUERY_VALUE",
    "REG_SZ",
    "REG_OPTION_NON_VOLATILE",
    "RRF_RT_REG_SZ",
    "ERROR_SUCCESS",
    "ERROR_FILE_NOT_FOUND",
    "CreateNamedPipeW",
    "ConnectNamedPipe",
    "DisconnectNamedPipe",
    "ReadFile",
    "WriteFile",
    "FlushFileBuffers",
    "CloseHandle",
    "SECURITY_ATTRIBUTES",
    "ConvertStringSecurityDescriptorToSecurityDescriptorW",
    "ConvertSidToStringSidW",
    "LocalFree",
    "OpenProcessToken",
    "GetCurrentProcess",
    "GetTokenInformation",
    "TOKEN_USER",
    "TOKEN_QUERY",
    "TokenUser",
    "PIPE_ACCESS_DUPLEX",
    "PIPE_TYPE_BYTE",
    "PIPE_READMODE_BYTE",
    "PIPE_WAIT",
    "PIPE_UNLIMITED_INSTANCES",
    "FILE_FLAG_FIRST_PIPE_INSTANCE",
    "ERROR_PIPE_CONNECTED",
    "SDDL_REVISION_1",
    "INVALID_HANDLE_VALUE",
    "NIF_INFO",
    "NIM_MODIFY",
    "NIIF_INFO",
    "NIIF_NOSOUND",
    "GetOpenFileNameW",
    "OPENFILENAMEW",
    "OFN_FILEMUSTEXIST",
    "OFN_PATHMUSTEXIST",
    "OFN_NOCHANGEDIR",
    "MOD_ALT",
    "MOD_CONTROL",
    "MOD_SHIFT",
    "MF_CHECKED",
    "MF_GRAYED",
    "GetTickCount64",
    "GetDoubleClickTime",
    "MessageBoxW",
    "MB_OKCANCEL",
    "MB_ICONWARNING",
    "MB_DEFBUTTON2",
    "IDOK",
    "SetWindowPos",
    "GetWindowLongPtrW",
    "SetWindowLongPtrW",
    "ShowWindow",
    "SetWindowRgn",
    "CreateRectRgn",
    "SetForegroundWindow",
    "BringWindowToTop",
    "GetForegroundWindow",
    "GetCursorPos",
    "EnumDisplayMonitors",
    "GetMonitorInfoW",
    "MONITORINFO",
    "RegisterHotKey",
    "UnregisterHotKey",
    "SetTimer",
    "KillTimer",
    "GetWindowThreadProcessId",
    "AttachThreadInput",
    "GetCurrentThreadId",
    "GetCurrentProcessId",
    "CoCreateInstance",
    "CoInitializeEx",
    "IVirtualDesktopManager",
    "VirtualDesktopManager",
    "DwmSetWindowAttribute",
    "PostMessageW",
    "IsWindowVisible",
    "HWND_TOPMOST",
    "HWND_NOTOPMOST",
    "SWP_NOSIZE",
    "SWP_NOMOVE",
    "SWP_NOZORDER",
    "SWP_NOACTIVATE",
    "SWP_FRAMECHANGED",
    "SW_HIDE",
    "SW_SHOWNOACTIVATE",
    "WS_POPUP",
    "WS_CLIPCHILDREN",
    "WS_CLIPSIBLINGS",
    "WS_EX_TOOLWINDOW",
    "WS_EX_APPWINDOW",
    "GWL_STYLE",
    "GWL_EXSTYLE",
    "MOD_NOREPEAT",
    "WM_HOTKEY",
    "WM_CLOSE",
    "WM_APP",
    "DWMWA_WINDOW_CORNER_PREFERENCE",
    "DWMWCP_DONOTROUND",
    "POINT",
    "RECT",
    "HWND",
    "Shell_NotifyIconW",
    "NOTIFYICONDATAW",
    "NIM_ADD",
    "NIM_DELETE",
    "NIF_MESSAGE",
    "NIF_ICON",
    "NIF_TIP",
    "WNDCLASSEXW",
    "RegisterClassExW",
    "CreateWindowExW",
    "DefWindowProcW",
    "DestroyWindow",
    "HWND_MESSAGE",
    "LoadIconW",
    "IDI_APPLICATION",
    "GetModuleHandleW",
    "CreatePopupMenu",
    "AppendMenuW",
    "TrackPopupMenuEx",
    "DestroyMenu",
    "MF_STRING",
    "MF_SEPARATOR",
    "TPM_RIGHTBUTTON",
    "TPM_BOTTOMALIGN",
    "TPM_RETURNCMD",
    "WM_LBUTTONUP",
    "WM_RBUTTONUP",
    "WM_CONTEXTMENU",
    "WM_COMMAND",
    "WM_USER",
    "WM_DESTROY",
    "CreateMutexW",
    "GetLastError",
    "ERROR_ALREADY_EXISTS",
    "FindWindowExW",
    "RegisterWindowMessageW",
    "LRESULT",
    "WPARAM",
    "LPARAM",
    "HMENU",
    "HICON",
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
    ergonomic
        .output(&ergonomic_output)
        .filters(ERGONOMIC_FILTERS.iter().copied())
        .flat()
        .implement("Windows.Foundation.TypedEventHandler");
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

    let conpty_output = workspace_root.join("crates").join("winuake-term").join("src").join("conpty_bindings.rs");
    let mut conpty = windows_bindgen::builder();
    conpty.input_default().output(&conpty_output).filters(CONPTY_FILTERS.iter().copied()).flat();
    conpty.write();

    let window_output = source_directory.join("window_bindings.rs");
    let mut window = windows_bindgen::builder();
    window.input_default().output(&window_output).filters(WINDOW_FILTERS.iter().copied()).flat();
    window.write();

    let render_output = workspace_root.join("crates").join("winuake-render").join("src").join("gpu_bindings.rs");
    let mut render = windows_bindgen::builder();
    render.input_default().output(&render_output).filters(RENDER_FILTERS.iter().copied()).flat();
    render.write();

    eprintln!(
        "wrote bindings.rs, composition.rs, window_bindings.rs, conpty_bindings.rs, gpu_bindings.rs from {} winmd files",
        metadata.len()
    );
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
