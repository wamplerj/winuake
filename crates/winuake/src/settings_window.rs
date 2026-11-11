//! The settings window: Window / Behavior / Appearance / Shortcuts / Profile pages, built from `winuake_config::fields` and the action
//! table. Every edit is checked, saved and applied at once through `settings_store`; there is no Apply button.
//!
//! The window is XAML markup with named controls. Text boxes commit when they lose focus; check boxes commit when clicked; shortcut
//! boxes record the key combination pressed in them.

use crate::bindings::{
    Button, FrameworkElement, Panel, TextBlock, TextBox, ToggleButton, UIElement, Visibility, Window, XamlReader,
};
use crate::settings_store;
use crate::window_bindings::{BringWindowToTop, HWND, HWND_TOPMOST, SWP_NOMOVE, SetForegroundWindow, SetWindowPos};
use crate::window_native::IWindowNative;
use std::cell::RefCell;
use std::rc::Rc;
use windows_core::{HSTRING, Interface, Result};
use winuake_config::fields::{self, FieldDescriptor, FieldKind, FieldValue, Page};
use winuake_config::settings::Settings;
use winuake_core::actions::{Action, ShortcutMap};
use winuake_core::shortcut::{Chord, ChordModifiers};

const XAML_NAMESPACE: &str = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
const XAML_X_NAMESPACE: &str = "http://schemas.microsoft.com/winfx/2006/xaml";
const TEXT: &str = "#FFDDDDDD";
const DIM: &str = "#FF999999";
const WINDOW_WIDTH: i32 = 1100;
const WINDOW_HEIGHT: i32 = 900;
const SHORTCUTS_PAGE: &str = "Shortcuts";
const SKINS_PAGE: &str = "Skins";
const PAGES: [&str; 6] = ["Window", "Behavior", "Appearance", SKINS_PAGE, SHORTCUTS_PAGE, "Profile"];

thread_local! {
    static OPEN: RefCell<Option<Rc<SettingsWindow>>> = const { RefCell::new(None) };
}

struct SettingsWindow {
    window: Window,
    root: FrameworkElement,
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Control names cannot contain dots or dashes.
fn control_name(prefix: &str, key: &str) -> String {
    format!("{prefix}_{}", key.replace(['.', '-'], "_"))
}

fn find<T: Interface>(root: &FrameworkElement, name: &str) -> Result<T> {
    root.FindName(&HSTRING::from(name))?.cast()
}

fn action_label(action: Action) -> String {
    if action == Action::Quit {
        return "Quit".to_owned();
    }
    let name = action.name().replace(['-', '_'], " ");
    let mut characters = name.chars();
    characters.next().map(|first| first.to_uppercase().collect::<String>() + characters.as_str()).unwrap_or(name)
}

fn field_markup(descriptor: &FieldDescriptor, settings: &Settings) -> String {
    let name = control_name("F", descriptor.key);
    let label = escape(descriptor.label);
    let value = fields::read(settings, descriptor.key);
    match (descriptor.kind, value) {
        (FieldKind::Toggle, Some(FieldValue::Bool(checked))) => format!(
            r#"<CheckBox x:Name="{name}" Content="{label}" IsChecked="{checked}" Foreground="{TEXT}" Margin="0,6,0,0"/>"#
        ),
        (_, Some(value)) => {
            let text = match value {
                FieldValue::Number(number) => number.to_string(),
                FieldValue::Text(text) => text,
                FieldValue::Bool(flag) => flag.to_string(),
            };
            format!(
                r#"<StackPanel Margin="0,10,0,0"><TextBlock Text="{label}" Foreground="{DIM}" FontSize="12"/><TextBox x:Name="{name}" Text="{}" MinWidth="320" HorizontalAlignment="Left"/></StackPanel>"#,
                escape(&text)
            )
        }
        (_, None) => String::new(),
    }
}

fn page_markup(page: Page, settings: &Settings) -> String {
    let rows: String = fields::descriptors()
        .iter()
        .filter(|descriptor| descriptor.page == page)
        .map(|descriptor| field_markup(descriptor, settings))
        .collect();
    page_wrapper(page.title(), &rows)
}

fn skins_markup() -> String {
    let rows = r##"<StackPanel x:Name="SkinList"/><StackPanel Orientation="Horizontal" Margin="0,16,0,0"><Button x:Name="InstallSkinButton" Content="Install skin from file..." /><Button x:Name="RemoveSkinButton" Content="Remove selected skin" Margin="10,0,0,0"/></StackPanel><TextBlock Margin="0,12,0,0" Foreground="#FF999999" FontSize="12" TextWrapping="Wrap" Text="Skins are folders with title.skin and tabs.skin. Install a .zip, .tar or .tar.gz archive that holds one skin folder."/>"##;
    page_wrapper(SKINS_PAGE, rows)
}

fn shortcuts_markup(settings: &Settings) -> String {
    let map = ShortcutMap::with_overrides(&settings.shortcuts).map;
    let rows: String = Action::all()
        .into_iter()
        .map(|action| {
            let name = control_name("S", &action.name());
            let chord = map.shortcut_for(action).map(|chord| chord.to_string()).unwrap_or_default();
            format!(
                r#"<StackPanel Margin="0,8,0,0"><TextBlock Text="{}" Foreground="{DIM}" FontSize="12"/><TextBox x:Name="{name}" Text="{chord}" IsReadOnly="True" PlaceholderText="None" MinWidth="320" HorizontalAlignment="Left"/></StackPanel>"#,
                escape(&action_label(action))
            )
        })
        .collect();
    let hint = r##"<TextBlock Text="Click a box and press the keys. Backspace removes the shortcut; Delete restores Yakuake's default." Foreground="#FF999999" FontSize="12" TextWrapping="Wrap"/>"##;
    page_wrapper(SHORTCUTS_PAGE, &format!("{hint}{rows}"))
}

fn page_wrapper(title: &str, rows: &str) -> String {
    format!(
        r#"<ScrollViewer x:Name="Page_{title}" Grid.Row="1" Visibility="Collapsed"><StackPanel Margin="24,8,24,24">{rows}</StackPanel></ScrollViewer>"#
    )
}

fn window_markup(settings: &Settings) -> String {
    let navigation: String = PAGES
        .iter()
        .map(|title| {
            format!(
                r#"<TextBlock x:Name="Nav_{title}" Text="{title}" Foreground="{TEXT}" FontSize="14" Padding="12,10,12,10"/>"#
            )
        })
        .collect();
    let pages: String = [Page::Window, Page::Behavior, Page::Appearance]
        .into_iter()
        .map(|page| page_markup(page, settings))
        .chain([skins_markup(), shortcuts_markup(settings), page_markup(Page::Profile, settings)])
        .collect();
    format!(
        r##"<Grid xmlns="{XAML_NAMESPACE}" xmlns:x="{XAML_X_NAMESPACE}" Background="#FF202020">
  <Grid.ColumnDefinitions><ColumnDefinition Width="190"/><ColumnDefinition Width="*"/></Grid.ColumnDefinitions>
  <StackPanel Grid.Column="0" Background="#FF1A1A1A" Padding="0,12,0,0">{navigation}</StackPanel>
  <Grid Grid.Column="1">
    <Grid.RowDefinitions><RowDefinition Height="Auto"/><RowDefinition Height="*"/></Grid.RowDefinitions>
    <TextBlock x:Name="Status" Grid.Row="0" Margin="24,12,24,0" Foreground="#FFFF9A9A" FontSize="12" TextWrapping="Wrap"/>
    {pages}
  </Grid>
</Grid>"##
    )
}

/// Opens the settings window (or brings the open one forward) on `page`.
pub fn open(page: Option<Page>) {
    let existing = OPEN.with(|slot| slot.borrow().clone());
    let result = match existing {
        Some(window) => window.show_page(page_name(page)).map(|()| window),
        None => create(page_name(page)),
    };
    match result {
        Ok(window) => {
            window.bring_forward();
            OPEN.with(|slot| *slot.borrow_mut() = Some(window));
        }
        Err(error) => tracing::error!(%error, "could not open the settings window"),
    }
}

fn page_name(page: Option<Page>) -> &'static str {
    page.map_or("Window", Page::title)
}

/// Opens the shortcut editor page.
pub fn open_shortcuts() {
    let existing = OPEN.with(|slot| slot.borrow().clone());
    match existing {
        Some(window) => {
            let _ = window.show_page(SHORTCUTS_PAGE);
            window.bring_forward();
        }
        None => match create(SHORTCUTS_PAGE) {
            Ok(window) => {
                window.bring_forward();
                OPEN.with(|slot| *slot.borrow_mut() = Some(window));
            }
            Err(error) => tracing::error!(%error, "could not open the settings window"),
        },
    }
}

fn create(initial_page: &str) -> Result<Rc<SettingsWindow>> {
    let settings = settings_store::current();
    let root: FrameworkElement = XamlReader::Load(&HSTRING::from(window_markup(&settings)))
        .inspect_err(|error| tracing::error!(%error, "settings window: markup failed"))?
        .cast()?;
    let window = Window::new()?;
    window.SetTitle(&HSTRING::from("Winuake Settings"))?;
    window.SetContent(&root)?;
    let shown = Rc::new(SettingsWindow { window: window.clone(), root: root.clone() });
    shown.connect(&settings).inspect_err(|error| tracing::error!(%error, "settings window: connect failed"))?;
    OPEN.with(|slot| *slot.borrow_mut() = Some(Rc::clone(&shown)));
    refresh_skin_list();
    shown.show_page(initial_page).inspect_err(|error| tracing::error!(%error, "settings window: show_page failed"))?;
    std::mem::forget(window.Closed(move |_, _| {
        OPEN.with(|slot| slot.borrow_mut().take());
    })?);
    window.Activate()?;
    Ok(shown)
}

impl SettingsWindow {
    fn bring_forward(&self) {
        let Ok(native) = self.window.cast::<IWindowNative>() else { return };
        let Ok(handle) = native.handle() else { return };
        let hwnd = HWND(handle as *mut core::ffi::c_void);
        unsafe {
            // Above the topmost drop-down, and sized once.
            let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, WINDOW_WIDTH, WINDOW_HEIGHT, SWP_NOMOVE as u32);
            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);
        }
        let _ = self.window.Activate();
    }

    fn show_page(&self, name: &str) -> Result<()> {
        for title in PAGES {
            let page: UIElement = find(&self.root, &format!("Page_{title}"))?;
            let visible = title == name;
            page.SetVisibility(if visible { Visibility::Visible } else { Visibility::Collapsed })?;
        }
        self.set_status("");
        Ok(())
    }

    fn set_status(&self, message: &str) {
        if let Ok(status) = find::<TextBlock>(&self.root, "Status") {
            let _ = status.SetText(&HSTRING::from(message));
        }
    }

    fn connect(self: &Rc<Self>, settings: &Settings) -> Result<()> {
        for title in PAGES {
            let nav: UIElement = find(&self.root, &format!("Nav_{title}"))
                .inspect_err(|error| tracing::error!(%error, title, "nav find failed"))?;
            let title = title.to_owned();
            std::mem::forget(nav.PointerPressed(move |_, _| {
                if let Some(window) = OPEN.with(|slot| slot.borrow().clone()) {
                    let _ = window.show_page(&title);
                }
            })?);
        }
        for descriptor in fields::descriptors() {
            self.connect_field(&descriptor)
                .inspect_err(|error| tracing::error!(%error, key = descriptor.key, "connect_field failed"))?;
        }
        for action in Action::all() {
            self.connect_shortcut_box(action)
                .inspect_err(|error| tracing::error!(%error, action = %action.name(), "connect_shortcut_box failed"))?;
        }
        let _ = settings;
        let install: Button = find(&self.root, "InstallSkinButton")?;
        std::mem::forget(install.Click(move |_, _| install_skin_from_file())?);
        let remove: Button = find(&self.root, "RemoveSkinButton")?;
        std::mem::forget(remove.Click(move |_, _| remove_selected_skin())?);
        Ok(())
    }

    fn connect_field(&self, descriptor: &FieldDescriptor) -> Result<()> {
        let name = control_name("F", descriptor.key);
        let key = descriptor.key;
        match descriptor.kind {
            FieldKind::Toggle => {
                let check_box: ToggleButton = find(&self.root, &name)?;
                let source = check_box.clone();
                std::mem::forget(check_box.Click(move |_, _| {
                    let checked = source.IsChecked().unwrap_or(false);
                    commit(key, FieldValue::Bool(checked));
                })?);
            }
            kind => {
                let text_box: TextBox = find(&self.root, &name)?;
                let source = text_box.clone();
                let ui: UIElement = text_box.cast()?;
                std::mem::forget(ui.GotFocus(move |_, _| crate::terminal_view::suspend_text_routing())?);
                std::mem::forget(ui.LostFocus(move |_, _| {
                    let text = source.Text().map(|text| text.to_string_lossy()).unwrap_or_default();
                    let value = match kind {
                        FieldKind::Number { .. } => match text.trim().parse::<i64>() {
                            Ok(number) => FieldValue::Number(number),
                            Err(_) => {
                                report(key, "must be a whole number");
                                restore(&source, key);
                                return;
                            }
                        },
                        _ => FieldValue::Text(text),
                    };
                    if !commit(key, value) {
                        restore(&source, key);
                    }
                })?);
            }
        }
        Ok(())
    }

    fn connect_shortcut_box(&self, action: Action) -> Result<()> {
        let text_box: TextBox = find(&self.root, &control_name("S", &action.name()))?;
        let ui: UIElement = text_box.cast()?;
        let source = text_box.clone();
        std::mem::forget(ui.GotFocus(move |_, _| crate::terminal_view::suspend_text_routing())?);
        std::mem::forget(ui.KeyDown(move |_, args| {
            crate::terminal_view::suspend_text_routing();
            let Some(args) = args.as_ref() else { return };
            let Ok(key) = args.Key() else { return };
            let _ = args.SetHandled(true);
            const VK_BACK: i32 = 0x08;
            const VK_DELETE: i32 = 0x2E;
            if key.0 == VK_BACK {
                set_shortcut(action, Some("none"));
            } else if key.0 == VK_DELETE {
                set_shortcut(action, None);
            } else {
                let modifiers = crate::terminal_view::current_modifiers();
                let chord_modifiers =
                    ChordModifiers { control: modifiers.control, shift: modifiers.shift, alt: modifiers.alt };
                if let Some(chord) = Chord::from_virtual_key(key.0, chord_modifiers) {
                    set_shortcut(action, Some(&chord.to_string()));
                }
            }
            refresh_shortcut_boxes(&source);
        })?);
        Ok(())
    }
}

/// Validates against the field's kind, then saves and applies. False (with the reason shown) when the value was refused.
fn commit(key: &'static str, value: FieldValue) -> bool {
    let mut candidate = settings_store::current();
    if let Err(error) = fields::write(&mut candidate, key, value.clone()) {
        report(key, &error.message);
        return false;
    }
    settings_store::update(|settings| {
        let _ = fields::write(settings, key, value);
    });
    report(key, "");
    true
}

fn report(key: &str, message: &str) {
    let Some(window) = OPEN.with(|slot| slot.borrow().clone()) else { return };
    let label = fields::descriptor(key).map_or(key, |descriptor| descriptor.label);
    window.set_status(&if message.is_empty() { String::new() } else { format!("{label}: {message}") });
}

/// Puts the saved value back after a refused edit.
fn restore(text_box: &TextBox, key: &str) {
    let Some(value) = fields::read(&settings_store::current(), key) else { return };
    let text = match value {
        FieldValue::Number(number) => number.to_string(),
        FieldValue::Text(text) => text,
        FieldValue::Bool(flag) => flag.to_string(),
    };
    let _ = text_box.SetText(&HSTRING::from(text));
}

/// `Some(text)` sets the override (`none` unbinds); `None` removes the override so the default applies again.
fn set_shortcut(action: Action, chord: Option<&str>) {
    settings_store::update(|settings| match chord {
        Some(text) => {
            let is_default = action.default_shortcut().is_some_and(|default| default.to_string() == text);
            if is_default {
                settings.shortcuts.remove(&action.name());
            } else {
                settings.shortcuts.insert(action.name(), text.to_owned());
            }
        }
        None => {
            settings.shortcuts.remove(&action.name());
        }
    });
}

/// Shows every action's effective chord again, since taking a chord from another action changes that action's box too.
fn refresh_shortcut_boxes(any_box: &TextBox) {
    let map = ShortcutMap::with_overrides(&settings_store::current().shortcuts).map;
    let Some(window) = OPEN.with(|slot| slot.borrow().clone()) else { return };
    let _ = any_box;
    for action in Action::all() {
        if let Ok(text_box) = find::<TextBox>(&window.root, &control_name("S", &action.name())) {
            let text = map.shortcut_for(action).map(|chord| chord.to_string()).unwrap_or_default();
            let _ = text_box.SetText(&HSTRING::from(text));
        }
    }
}

// ---- skins page ----

fn skin_row_markup(summary: &winuake_skin::directory::SkinSummary, selected: bool) -> String {
    let background = if selected { "#FF3A4A5A" } else { "#00000000" };
    let icon = crate::skin_assets::icon_uri(summary)
        .map(|uri| format!(r#"<Image Source="{uri}" Width="32" Height="32" Margin="0,0,12,0"/>"#))
        .unwrap_or_else(|| r#"<Border Width="32" Height="32" Margin="0,0,12,0"/>"#.to_owned());
    let origin = match summary.location {
        winuake_skin::directory::SkinLocation::User => "installed",
        winuake_skin::directory::SkinLocation::Bundled => "bundled",
    };
    format!(
        r#"<Border xmlns="{XAML_NAMESPACE}" Background="{background}" Padding="8" Margin="0,4,0,0" CornerRadius="4"><StackPanel Orientation="Horizontal">{icon}<StackPanel><TextBlock Text="{}" Foreground="{TEXT}" FontSize="14"/><TextBlock Text="{}" Foreground="{DIM}" FontSize="12"/></StackPanel></StackPanel></Border>"#,
        escape(&summary.name),
        escape(&format!("{} ({origin})", if summary.author.is_empty() { "unknown author" } else { &summary.author })),
    )
}

/// Rebuilds the skin rows from what is on disk, highlighting the skin in use.
fn refresh_skin_list() {
    let Some(window) = OPEN.with(|slot| slot.borrow().clone()) else { return };
    let result = (|| -> Result<()> {
        let list: Panel = find(&window.root, "SkinList")?;
        let children = list.Children()?;
        children.Clear()?;
        let current = settings_store::current().appearance.skin;
        for summary in crate::skin_assets::list() {
            let row: FrameworkElement =
                XamlReader::Load(&HSTRING::from(skin_row_markup(&summary, summary.id == current)))?.cast()?;
            let id = summary.id.clone();
            let ui: UIElement = row.cast()?;
            std::mem::forget(ui.PointerPressed(move |_, _| {
                let id = id.clone();
                settings_store::update(move |settings| settings.appearance.skin = id);
                refresh_skin_list();
            })?);
            children.Append(&ui)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        tracing::error!(%error, "could not list skins");
    }
}

fn skin_status(message: &str) {
    if let Some(window) = OPEN.with(|slot| slot.borrow().clone()) {
        window.set_status(message);
    }
}

fn install_skin_from_file() {
    let Some(path) = pick_archive() else { return };
    let outcome = (|| -> std::result::Result<String, String> {
        let bytes = std::fs::read(&path).map_err(|error| format!("cannot read the file: {error}"))?;
        let archive = winuake_skin::archive::read_archive(&bytes).map_err(|error| error.to_string())?;
        winuake_skin::directory::install(&archive, &crate::skin_assets::user_skins_directory())
            .map_err(|error| error.to_string())?;
        crate::skin_assets::clear_cache(&archive.id);
        Ok(archive.id)
    })();
    match outcome {
        Ok(id) => {
            tracing::info!(skin = %id, "skin installed");
            skin_status("");
            settings_store::update(move |settings| settings.appearance.skin = id);
            refresh_skin_list();
        }
        Err(message) => {
            tracing::warn!(%message, "could not install the skin");
            skin_status(&format!("Install failed: {message}"));
        }
    }
}

fn remove_selected_skin() {
    let current = settings_store::current().appearance.skin;
    let installed = crate::skin_assets::list()
        .into_iter()
        .find(|summary| summary.id == current && summary.location == winuake_skin::directory::SkinLocation::User);
    let Some(skin) = installed else {
        skin_status("Only skins you installed can be removed.");
        return;
    };
    match winuake_skin::directory::remove(&crate::skin_assets::user_skins_directory(), &skin.id) {
        Ok(()) => {
            crate::skin_assets::clear_cache(&skin.id);
            skin_status("");
            settings_store::update(|settings| settings.appearance.skin = "default".to_owned());
            refresh_skin_list();
        }
        Err(error) => skin_status(&format!("Could not remove the skin: {error}")),
    }
}

/// Shows the Windows "open file" dialog for skin archives.
fn pick_archive() -> Option<std::path::PathBuf> {
    use crate::window_bindings::{
        GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    let mut buffer = vec![0u16; 1024];
    let filter: Vec<u16> = "Skin archives\0*.zip;*.tar;*.tar.gz;*.tgz\0All files\0*.*\0\0".encode_utf16().collect();
    let title: Vec<u16> = "Install skin\0".encode_utf16().collect();
    let owner = OPEN
        .with(|slot| slot.borrow().clone())
        .and_then(|window| window.window.cast::<IWindowNative>().ok()?.handle().ok());
    let mut dialog = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: HWND(owner.unwrap_or(0) as *mut core::ffi::c_void),
        lpstrFilter: windows_core::PCWSTR(filter.as_ptr()),
        nFilterIndex: 1,
        lpstrFile: windows_core::PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        lpstrTitle: windows_core::PCWSTR(title.as_ptr()),
        Flags: (OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR) as u32,
        ..OPENFILENAMEW::default()
    };
    if !unsafe { GetOpenFileNameW(&mut dialog) }.as_bool() {
        return None;
    }
    let length = buffer.iter().position(|unit| *unit == 0).unwrap_or(buffer.len());
    Some(std::path::PathBuf::from(String::from_utf16_lossy(&buffer[..length])))
}
