//! Winuake application object and main window. Phase 2 shows one terminal in a plain window; Phase 3 turns it into the drop-down.

use crate::bindings::{
    Application, ApplicationInitializationCallback, TypeKind as BindingsTypeKind, TypeName as BindingsTypeName, Window,
    XamlControlsResources, XamlControlsXamlMetaDataProvider,
};
use crate::composition::{
    Application as ComposedApplication, IApplicationOverrides, IApplicationOverrides_Impl, IXamlMetadataProvider,
    IXamlMetadataProvider_Impl, IXamlType, LaunchActivatedEventArgs, TypeName, XmlnsDefinition,
};
use crate::drop_down::DropDownHost;
use crate::terminal_view::TerminalView;
use crate::window_native::IWindowNative;
use std::cell::RefCell;
use std::rc::Rc;
use windows_core::{Array, HSTRING, Interface, Ref, Result, implement};
use winuake_config::window::WindowSettings;
use winuake_render::color::ColorScheme;
use winuake_render::gpu::FontSpec;
use winuake_term::conpty::ConPtyBackend;
use winuake_term::pty::{EnvironmentVariable, PtySpawnRequest, TerminalSize};
use winuake_term::session::{TerminalId, TerminalSession};
use winuake_term::shell::{PathVariableLocator, ShellResolver};

thread_local! {
    /// Keeps the main window and its terminal alive for the life of the UI thread.
    static MAIN_WINDOW: RefCell<Option<(Window, Rc<TerminalView>)>> = const { RefCell::new(None) };
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

    let session = start_shell_session()?;
    let window_handle = window.cast::<IWindowNative>()?.handle()?;
    let view = TerminalView::create(session, &FontSpec::default(), ColorScheme::campbell(), window_handle)?;
    window.SetContent(view.element())?;

    let settings = window_settings();
    let open_after_start = settings.open_after_start;
    crate::tray::install(settings.show_tray_icon)?;
    let focus_view = Rc::clone(&view);
    let host = DropDownHost::install(window_handle, settings, Box::new(move || focus_view.focus()))?;

    // Show once, off screen, so XAML lays the terminal out at its real size; then hide until the first toggle.
    host.park_for_first_layout();
    window.Activate()?;
    host.hide_now();
    if open_after_start {
        host.toggle();
    }

    MAIN_WINDOW.with(|slot| *slot.borrow_mut() = Some((window, view)));
    Ok(())
}

/// Defaults, plus `WINUAKE_WINDOW_SETTINGS` overrides (for example `frames=50+keep_open=false`) until the settings file exists.
fn window_settings() -> WindowSettings {
    let Some(overrides) = std::env::var_os("WINUAKE_WINDOW_SETTINGS") else { return WindowSettings::default() };
    let (settings, problems) = WindowSettings::default().with_overrides(&overrides.to_string_lossy());
    for problem in problems {
        tracing::warn!(%problem, "ignored a window setting override");
    }
    settings
}

fn start_shell_session() -> Result<TerminalSession> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let shell = ShellResolver::new(PathVariableLocator::from_path_variable(&path), std::env::var("COMSPEC").ok())
        .resolve_default_shell();
    let request = PtySpawnRequest {
        program: shell.program,
        arguments: shell.arguments,
        working_directory: std::env::var_os("USERPROFILE").map(Into::into),
        environment: vec![EnvironmentVariable { name: "COLORTERM".to_owned(), value: "truecolor".to_owned() }],
        size: TerminalSize::new(80, 24),
    };
    TerminalSession::start(TerminalId(1), &ConPtyBackend, &request).map_err(|error| {
        tracing::error!(%error, "could not start the shell");
        windows_core::Error::new(windows_core::HRESULT(0x8000_4005_u32 as i32), error.to_string())
    })
}
