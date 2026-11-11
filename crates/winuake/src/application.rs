//! Winuake application object and main window: the drop-down host plus the workspace of sessions inside it.

use crate::bindings::{
    Application, ApplicationInitializationCallback, TypeKind as BindingsTypeKind, TypeName as BindingsTypeName, Window,
    XamlControlsResources, XamlControlsXamlMetaDataProvider,
};
use crate::composition::{
    Application as ComposedApplication, IApplicationOverrides, IApplicationOverrides_Impl, IXamlMetadataProvider,
    IXamlMetadataProvider_Impl, IXamlType, LaunchActivatedEventArgs, TypeName, XmlnsDefinition,
};
use crate::drop_down::DropDownHost;
use crate::window_native::IWindowNative;
use crate::workspace_host::WorkspaceHost;
use std::cell::RefCell;
use windows_core::{Array, HSTRING, Interface, Ref, Result, implement};
use winuake_core::sessions::session::SessionKind;

thread_local! {
    /// Keeps the main window and its terminal alive for the life of the UI thread.
    static MAIN_WINDOW: RefCell<Option<Window>> = const { RefCell::new(None) };
}

/// Application subclass. Delegates XAML type lookup for built-in WinUI controls to the framework's own provider.
#[implement(IXamlMetadataProvider, IApplicationOverrides)]
pub struct WinuakeApplication {
    controls_metadata: XamlControlsXamlMetaDataProvider,
}

impl IApplicationOverrides_Impl for WinuakeApplication_Impl {
    fn OnLaunched(&self, _args: Ref<LaunchActivatedEventArgs>) -> Result<()> {
        tracing::info!("application launched");
        merge_control_resources()?;
        crate::message_hook::install()?;
        open_main_window()
    }
}

impl IXamlMetadataProvider_Impl for WinuakeApplication_Impl {
    fn GetXamlType(&self, type_name: &TypeName) -> Result<IXamlType> {
        let framework_type_name =
            BindingsTypeName { Name: type_name.name.clone(), Kind: BindingsTypeKind(type_name.kind.0) };
        self.controls_metadata.GetXamlType(&framework_type_name)?.cast()
    }

    fn GetXamlTypeByFullName(&self, full_name: &HSTRING) -> Result<IXamlType> {
        self.controls_metadata.GetXamlTypeByFullName(full_name)?.cast()
    }

    fn GetXmlnsDefinitions(&self) -> Result<Array<XmlnsDefinition>> {
        let framework_definitions = self.controls_metadata.GetXmlnsDefinitions()?;
        let definitions: Vec<XmlnsDefinition> = framework_definitions
            .iter()
            .map(|definition| XmlnsDefinition {
                xml_namespace: definition.XmlNamespace.clone(),
                namespace: definition.Namespace.clone(),
            })
            .collect();
        Ok(Array::from_slice(&definitions))
    }
}

pub fn run() -> Result<()> {
    Application::Start(&ApplicationInitializationCallback::new(|_| {
        let composed = ComposedApplication::compose(WinuakeApplication {
            controls_metadata: XamlControlsXamlMetaDataProvider::new()?,
        })?;
        let application: Application = composed.cast()?;
        // The application must outlive the message loop.
        std::mem::forget(application);
        Ok(())
    }))
}

/// Ends the application: drops the window and terminal (which kills the shell) and leaves the XAML message loop.
pub fn quit() {
    tracing::info!("quitting");
    crate::settings_store::stop_watching();
    crate::workspace_host::shutdown();
    MAIN_WINDOW.with(|slot| drop(slot.borrow_mut().take()));
    if let Err(error) = Application::Current().and_then(|application| application.Exit()) {
        tracing::error!(%error, "Application.Exit failed");
    }
}

/// `XamlControlsResources` must be merged inside `OnLaunched`; doing it earlier crashes XAML.
fn merge_control_resources() -> Result<()> {
    Application::Current()?.Resources()?.MergedDictionaries()?.Append(&XamlControlsResources::new()?)
}

fn open_main_window() -> Result<()> {
    let window = Window::new()?;
    window.SetTitle(&HSTRING::from("Winuake"))?;

    let settings = crate::settings_store::initialize();
    let window_handle = window.cast::<IWindowNative>()?.handle()?;
    let workspace = WorkspaceHost::install(window_handle, &settings)?;
    window.SetContent(&workspace.root())?;
    workspace.add_session(SessionKind::Single);

    let open_after_start = settings.window.open_after_start;
    crate::tray::install(settings.window.show_tray_icon)?;
    let host = DropDownHost::install(
        window_handle,
        settings.window.clone(),
        settings.toggle_hotkey.as_deref(),
        Box::new(crate::workspace_host::focus_active_terminal),
    )?;
    crate::settings_store::start_watching();

    // Show once, off screen, so XAML lays the terminal out at its real size; then hide until the first toggle.
    host.park_for_first_layout();
    window.Activate()?;
    host.hide_now();
    if open_after_start {
        host.toggle();
    }

    MAIN_WINDOW.with(|slot| *slot.borrow_mut() = Some(window));
    Ok(())
}
