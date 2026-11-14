//! The settings page, shown in a tab of the Winuake window: Window / Behavior / Appearance / Skins / Shortcuts / Profile pages, built from `winuake_config::fields` and the action
//! table. Every edit is checked, saved and applied at once through `settings_store`; there is no Apply button.
//!
//! The window is XAML markup with named controls. Text boxes commit when they lose focus; check boxes commit when clicked; shortcut
//! boxes record the key combination pressed in them.

use crate::bindings::{
    Button, FrameworkElement, Panel, Selector, TextBlock, TextBox, ToggleButton, UIElement, Visibility, XamlReader,
};
use crate::settings_store;
use crate::window_bindings::HWND;
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
pub const WINDOW_PAGE: &str = "Window";
pub const SHORTCUTS_PAGE: &str = "Shortcuts";
pub const PROFILE_PAGE: &str = "Profile";
const SKINS_PAGE: &str = "Skins";
const PAGES: [&str; 6] = [WINDOW_PAGE, "Behavior", "Appearance", SKINS_PAGE, SHORTCUTS_PAGE, PROFILE_PAGE];

thread_local! {
    static OPEN: RefCell<Option<Rc<SettingsWindow>>> = const { RefCell::new(None) };
    /// The profile whose settings the Profile page shows.
    static SELECTED_PROFILE: RefCell<String> = const { RefCell::new(String::new()) };
}

struct SettingsWindow {
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

/// A dropdown listing `choices` with `current` selected; `extra` holds the namespace declarations a standalone row needs.
fn choice_markup(label: &str, name: &str, extra: &str, choices: &[&str], current: &str) -> String {
    let selected = choices.iter().position(|choice| *choice == current).unwrap_or(0);
    let items: String =
        choices.iter().map(|choice| format!(r#"<ComboBoxItem Content="{}"/>"#, escape(choice))).collect();
    format!(
        r#"<StackPanel{extra} Margin="0,10,0,0"><TextBlock Text="{label}" Foreground="{DIM}" FontSize="12"/><ComboBox x:Name="{name}" SelectedIndex="{selected}" MinWidth="200">{items}</ComboBox></StackPanel>"#
    )
}

fn field_markup(descriptor: &FieldDescriptor, settings: &Settings) -> String {
    let name = control_name("F", descriptor.key);
    let label = escape(descriptor.label);
    let value = fields::read(settings, descriptor.key);
    match (descriptor.kind, value) {
        (FieldKind::Toggle, Some(FieldValue::Bool(checked))) => format!(
            r#"<CheckBox x:Name="{name}" Content="{label}" IsChecked="{checked}" Foreground="{TEXT}" Margin="0,6,0,0"/>"#
        ),
        (FieldKind::Choice(choices), Some(FieldValue::Text(current))) => {
            choice_markup(&label, &name, "", choices, &current)
        }
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

fn profile_page_markup() -> String {
    let rows = r##"<TextBlock Text="Profiles: shell, font, colors and more for each kind of terminal. Click one to edit it. New sessions use the default profile; press and hold or right-click the + button (or use the menu) to pick another." Foreground="#FF999999" FontSize="12" TextWrapping="Wrap"/><StackPanel x:Name="ProfileList" Margin="0,10,0,0"/><StackPanel Orientation="Horizontal" Margin="0,12,0,0"><Button x:Name="AddProfileButton" Content="Add"/><Button x:Name="DuplicateProfileButton" Content="Duplicate" Margin="8,0,0,0"/><Button x:Name="DeleteProfileButton" Content="Delete" Margin="8,0,0,0"/><Button x:Name="DefaultProfileButton" Content="Make default" Margin="8,0,0,0"/><Button x:Name="DetectShellsButton" Content="Add detected shells" Margin="8,0,0,0"/></StackPanel><StackPanel x:Name="ProfileFields" Margin="0,8,0,0"/>"##;
    page_wrapper(PROFILE_PAGE, rows)
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
                r##"<Grid x:Name="Nav_{title}" Background="#00000000"><Border x:Name="NavOff_{title}"><TextBlock Text="{title}" Foreground="{TEXT}" FontSize="14" Padding="14,10,12,10"/></Border><Border x:Name="NavOn_{title}" Background="#FF2D3A48" BorderBrush="#FF4FA3FF" BorderThickness="3,0,0,0" Visibility="Collapsed"><TextBlock Text="{title}" Foreground="#FFFFFFFF" FontWeight="SemiBold" FontSize="14" Padding="11,10,12,10"/></Border></Grid>"##
            )
        })
        .collect();
    let pages: String = [Page::Window, Page::Behavior, Page::Appearance]
        .into_iter()
        .map(|page| page_markup(page, settings))
        .chain([skins_markup(), shortcuts_markup(settings), profile_page_markup()])
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

/// Builds the settings UI for the settings tab. Call once per tab.
pub fn build_page() -> Result<FrameworkElement> {
    let settings = settings_store::current();
    let root: FrameworkElement = XamlReader::Load(&HSTRING::from(window_markup(&settings)))
        .inspect_err(|error| tracing::error!(%error, "settings page: markup failed"))?
        .cast()?;
    let shown = Rc::new(SettingsWindow { root: root.clone() });
    shown.connect(&settings).inspect_err(|error| tracing::error!(%error, "settings page: connect failed"))?;
    OPEN.with(|slot| *slot.borrow_mut() = Some(Rc::clone(&shown)));
    refresh_skin_list();
    refresh_profile_page();
    shown.show_page(WINDOW_PAGE)?;
    Ok(root)
}

/// The settings tab was closed.
pub fn closed() {
    OPEN.with(|slot| slot.borrow_mut().take());
}

/// Switches the open settings page to `name` (one of the `*_PAGE` names or "Behavior" / "Appearance").
pub fn show_page(name: &str) {
    if let Some(window) = OPEN.with(|slot| slot.borrow().clone())
        && let Err(error) = window.show_page(name)
    {
        tracing::warn!(%error, page = name, "could not switch settings page");
    }
}

impl SettingsWindow {
    fn show_page(&self, name: &str) -> Result<()> {
        for title in PAGES {
            let page: UIElement = find(&self.root, &format!("Page_{title}"))?;
            let visible = title == name;
            page.SetVisibility(if visible { Visibility::Visible } else { Visibility::Collapsed })?;
            // The page list on the left shows which page is open: a highlighted, bold entry with an accent bar.
            let shown = |on: bool| if on { Visibility::Visible } else { Visibility::Collapsed };
            find::<UIElement>(&self.root, &format!("NavOn_{title}"))?.SetVisibility(shown(visible))?;
            find::<UIElement>(&self.root, &format!("NavOff_{title}"))?.SetVisibility(shown(!visible))?;
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
        for descriptor in fields::descriptors().into_iter().filter(|descriptor| descriptor.page != Page::Profile) {
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
        connect_profile_buttons(&self.root)?;
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
            FieldKind::Choice(choices) => {
                let drop_down: Selector = find(&self.root, &name)?;
                let source = drop_down.clone();
                std::mem::forget(drop_down.SelectionChanged(move |_, _| {
                    if let Some(choice) = selected_choice(&source, choices) {
                        commit(key, FieldValue::Text(choice));
                    }
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
        // Preview (tunneling) so the text box's own handling of Delete and Backspace does not swallow the key first.
        std::mem::forget(ui.PreviewKeyDown(move |_, args| {
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
    pick_file("Install skin", "Skin archives\0*.zip;*.tar;*.tar.gz;*.tgz\0All files\0*.*\0\0")
}

/// Shows the Windows "open file" dialog. `filter` is the double-NUL separated filter list of the Win32 dialog.
fn pick_file(title: &str, filter: &str) -> Option<std::path::PathBuf> {
    use crate::window_bindings::{
        GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    let mut buffer = vec![0u16; 1024];
    let filter: Vec<u16> = filter.encode_utf16().collect();
    let title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let owner = crate::workspace_host::window_handle();
    let mut dialog = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: HWND(owner as *mut core::ffi::c_void),
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

/// A font chosen in the font dialog.
struct ChosenFont {
    family: String,
    size_points: i32,
}

/// Shows the Windows font dialog with fixed-width screen fonts, starting at `family` and `size_points`.
fn pick_font(family: &str, size_points: i32) -> Option<ChosenFont> {
    use crate::window_bindings::{
        CF_FIXEDPITCHONLY, CF_INITTOLOGFONTSTRUCT, CF_LIMITSIZE, CF_NOSCRIPTSEL, CF_SCREENFONTS, CHOOSEFONTW,
        ChooseFontW, GetDC, GetDeviceCaps, LOGFONTW, LOGPIXELSY, ReleaseDC,
    };
    // The dialog converts the height to points with the screen device context's resolution, so use the same one.
    let dots_per_inch = unsafe {
        let screen = GetDC(None);
        let dots = GetDeviceCaps(Some(screen), LOGPIXELSY);
        ReleaseDC(None, screen);
        if dots > 0 { dots } else { 96 }
    };
    let mut log_font =
        LOGFONTW { lfHeight: -(size_points * dots_per_inch / 72), lfWeight: 400, lfCharSet: 1, ..LOGFONTW::default() };
    for (slot, unit) in log_font.lfFaceName.iter_mut().zip(family.encode_utf16().take(31)) {
        *slot = unit;
    }
    let mut dialog = CHOOSEFONTW {
        lStructSize: size_of::<CHOOSEFONTW>() as u32,
        hwndOwner: HWND(crate::workspace_host::window_handle() as *mut core::ffi::c_void),
        lpLogFont: &mut log_font,
        iPointSize: size_points * 10,
        Flags: (CF_SCREENFONTS | CF_FIXEDPITCHONLY | CF_INITTOLOGFONTSTRUCT | CF_NOSCRIPTSEL | CF_LIMITSIZE) as u32,
        nSizeMin: 6,
        nSizeMax: 72,
        ..CHOOSEFONTW::default()
    };
    if !unsafe { ChooseFontW(&mut dialog) }.as_bool() {
        return None;
    }
    let length = log_font.lfFaceName.iter().position(|unit| *unit == 0).unwrap_or(log_font.lfFaceName.len());
    let family = String::from_utf16_lossy(&log_font.lfFaceName[..length]);
    (!family.is_empty()).then(|| ChosenFont { family, size_points: ((dialog.iPointSize + 5) / 10).clamp(6, 72) })
}

/// Shows the Windows folder picker.
fn pick_folder(title: &str) -> Option<std::path::PathBuf> {
    use crate::window_bindings::{
        BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS, BROWSEINFOW, CoTaskMemFree, SHBrowseForFolderW, SHGetPathFromIDListW,
    };
    let title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let mut display = vec![0u16; 260];
    let browse = BROWSEINFOW {
        hwndOwner: HWND(crate::workspace_host::window_handle() as *mut core::ffi::c_void),
        pszDisplayName: windows_core::PWSTR(display.as_mut_ptr()),
        lpszTitle: windows_core::PCWSTR(title.as_ptr()),
        ulFlags: (BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE) as u32,
        ..BROWSEINFOW::default()
    };
    let item = unsafe { SHBrowseForFolderW(&browse) };
    if item.is_null() {
        return None;
    }
    let mut path = vec![0u16; 1024];
    let found = unsafe { SHGetPathFromIDListW(item, windows_core::PWSTR(path.as_mut_ptr())) }.as_bool();
    unsafe { CoTaskMemFree(item.cast()) };
    if !found {
        return None;
    }
    let length = path.iter().position(|unit| *unit == 0).unwrap_or(path.len());
    Some(std::path::PathBuf::from(String::from_utf16_lossy(&path[..length])))
}

// ---- profiles page ----

/// The profile being edited: the remembered selection if it still exists, else the default.
fn selected_profile(settings: &Settings) -> String {
    let remembered = SELECTED_PROFILE.with(|slot| slot.borrow().clone());
    if settings.profile(&remembered).is_some() { remembered } else { settings.default_profile_name() }
}

fn select_profile(name: &str) {
    SELECTED_PROFILE.with(|slot| *slot.borrow_mut() = name.to_owned());
}

/// Makes the Profile page show `name`, if the settings tab is open ("Edit Current Profile" in a terminal's menu).
pub fn show_profile(name: &str) {
    select_profile(name);
    refresh_profile_page();
}

fn profile_row_markup(profile: &winuake_config::profile::Profile, is_default: bool, selected: bool) -> String {
    let background = if selected { "#FF3A4A5A" } else { "#00000000" };
    let program = profile.program.clone().unwrap_or_else(|| "default shell".to_owned());
    let tag = if is_default { "  (default)" } else { "" };
    format!(
        r#"<Border xmlns="{XAML_NAMESPACE}" Background="{background}" Padding="8" Margin="0,3,0,0" CornerRadius="4"><StackPanel><TextBlock Text="{}{tag}" Foreground="{TEXT}" FontSize="14"/><TextBlock Text="{}" Foreground="{DIM}" FontSize="12"/></StackPanel></Border>"#,
        escape(&profile.name),
        escape(&program),
    )
}

fn profile_field_markup(descriptor: &FieldDescriptor, settings: &Settings, profile: &str) -> String {
    let label = escape(descriptor.label);
    let Some(value) = fields::read_profile_field(settings, profile, descriptor.key) else { return String::new() };
    match (descriptor.kind, value) {
        (FieldKind::Toggle, FieldValue::Bool(checked)) => format!(
            r#"<CheckBox xmlns="{XAML_NAMESPACE}" x:Name="Field" xmlns:x="{XAML_X_NAMESPACE}" Content="{label}" IsChecked="{checked}" Foreground="{TEXT}" Margin="0,6,0,0"/>"#
        ),
        (FieldKind::Choice(choices), FieldValue::Text(current)) => {
            let namespaces = format!(r#" xmlns="{XAML_NAMESPACE}" xmlns:x="{XAML_X_NAMESPACE}""#);
            choice_markup(&label, "Field", &namespaces, choices, &current)
        }
        (kind, value) => {
            let text = match value {
                FieldValue::Number(number) => number.to_string(),
                FieldValue::Text(text) => text,
                FieldValue::Bool(flag) => flag.to_string(),
            };
            let browse = match kind {
                FieldKind::FilePath | FieldKind::FolderPath => {
                    r#"<Button x:Name="Browse" Content="Browse..." Margin="8,0,0,0"/>"#
                }
                FieldKind::FontFamily => r#"<Button x:Name="Browse" Content="Choose font..." Margin="8,0,0,0"/>"#,
                _ => "",
            };
            format!(
                r#"<StackPanel xmlns="{XAML_NAMESPACE}" xmlns:x="{XAML_X_NAMESPACE}" Margin="0,10,0,0"><TextBlock Text="{label}" Foreground="{DIM}" FontSize="12"/><StackPanel Orientation="Horizontal"><TextBox x:Name="Field" Text="{}" MinWidth="320" HorizontalAlignment="Left"/>{browse}</StackPanel></StackPanel>"#,
                escape(&text)
            )
        }
    }
}

/// Rebuilds the profile list and the fields of the selected profile.
fn refresh_profile_page() {
    let Some(window) = OPEN.with(|slot| slot.borrow().clone()) else { return };
    let result = (|| -> Result<()> {
        let settings = settings_store::current();
        let selected = selected_profile(&settings);
        select_profile(&selected);
        let default_name = settings.default_profile_name();

        let list: Panel = find(&window.root, "ProfileList")?;
        let rows = list.Children()?;
        rows.Clear()?;
        for profile in &settings.profiles {
            let row: FrameworkElement = XamlReader::Load(&HSTRING::from(profile_row_markup(
                profile,
                profile.name == default_name,
                profile.name == selected,
            )))?
            .cast()?;
            let name = profile.name.clone();
            let ui: UIElement = row.cast()?;
            std::mem::forget(ui.PointerPressed(move |_, _| {
                select_profile(&name);
                refresh_profile_page();
            })?);
            rows.Append(&ui)?;
        }

        let panel: Panel = find(&window.root, "ProfileFields")?;
        let children = panel.Children()?;
        children.Clear()?;
        for descriptor in fields::descriptors().into_iter().filter(|descriptor| descriptor.page == Page::Profile) {
            let markup = profile_field_markup(&descriptor, &settings, &selected);
            if markup.is_empty() {
                continue;
            }
            let row: FrameworkElement = XamlReader::Load(&HSTRING::from(markup))?.cast()?;
            connect_profile_field(&row, &descriptor, &selected)?;
            children.Append(&row.cast::<UIElement>()?)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        tracing::error!(%error, "could not show the profiles");
    }
}

/// The choice currently picked in a dropdown, if any.
fn selected_choice(drop_down: &Selector, choices: &[&str]) -> Option<String> {
    let index = usize::try_from(drop_down.SelectedIndex().ok()?).ok()?;
    choices.get(index).map(|choice| (*choice).to_owned())
}

fn connect_profile_field(row: &FrameworkElement, descriptor: &FieldDescriptor, profile: &str) -> Result<()> {
    let key = descriptor.key;
    let profile = profile.to_owned();
    match descriptor.kind {
        FieldKind::Toggle => {
            let check_box: ToggleButton = row.cast()?;
            let source = check_box.clone();
            std::mem::forget(check_box.Click(move |_, _| {
                commit_profile(&profile, key, FieldValue::Bool(source.IsChecked().unwrap_or(false)));
            })?);
        }
        FieldKind::Choice(choices) => {
            let drop_down: Selector = find(row, "Field")?;
            let source = drop_down.clone();
            std::mem::forget(drop_down.SelectionChanged(move |_, _| {
                if let Some(choice) = selected_choice(&source, choices) {
                    commit_profile(&profile, key, FieldValue::Text(choice));
                }
            })?);
        }
        kind => {
            let text_box: TextBox = find(row, "Field")?;
            let source = text_box.clone();
            let ui: UIElement = text_box.cast()?;
            std::mem::forget(ui.GotFocus(move |_, _| crate::terminal_view::suspend_text_routing())?);
            let lost_focus_profile = profile.clone();
            std::mem::forget(ui.LostFocus(move |_, _| {
                let profile = lost_focus_profile.clone();
                let text = source.Text().map(|text| text.to_string_lossy()).unwrap_or_default();
                let value = match kind {
                    FieldKind::Number { .. } => match text.trim().parse::<i64>() {
                        Ok(number) => FieldValue::Number(number),
                        Err(_) => {
                            report(key, "must be a whole number");
                            restore_profile_field(&source, &profile, key);
                            return;
                        }
                    },
                    _ => FieldValue::Text(text),
                };
                if !commit_profile(&profile, key, value) {
                    restore_profile_field(&source, &profile, key);
                }
            })?);
            if matches!(kind, FieldKind::FilePath | FieldKind::FolderPath | FieldKind::FontFamily) {
                connect_browse_button(row, kind, key, &profile)?;
            }
        }
    }
    Ok(())
}

/// Opens the Windows font dialog (fixed-width fonts only, since a terminal needs them) and saves the chosen family and size.
fn choose_profile_font(profile: &str) {
    let current = fields::read_profile_field(&settings_store::current(), profile, "profile.font_family");
    let current_size = fields::read_profile_field(&settings_store::current(), profile, "profile.font_size_points");
    let family = match current {
        Some(FieldValue::Text(text)) => text,
        _ => String::new(),
    };
    let size = match current_size {
        Some(FieldValue::Number(number)) => number as i32,
        _ => 11,
    };
    let Some(choice) = pick_font(&family, size) else { return };
    commit_profile(profile, "profile.font_family", FieldValue::Text(choice.family));
    commit_profile(profile, "profile.font_size_points", FieldValue::Number(i64::from(choice.size_points)));
    refresh_profile_page();
}

/// The Browse... button of a path field: asks for a file or folder, shows it in the text box, and saves it.
fn connect_browse_button(row: &FrameworkElement, kind: FieldKind, key: &'static str, profile: &str) -> Result<()> {
    let browse: Button = find(row, "Browse")?;
    let text_box: TextBox = find(row, "Field")?;
    let profile = profile.to_owned();
    std::mem::forget(browse.Click(move |_, _| {
        if kind == FieldKind::FontFamily {
            choose_profile_font(&profile);
            return;
        }
        let picked = match kind {
            FieldKind::FolderPath => pick_folder("Choose the starting directory"),
            _ => pick_file("Choose the program to run", "Programs\0*.exe;*.cmd;*.bat;*.com\0All files\0*.*\0\0"),
        };
        let Some(path) = picked else { return };
        let text = path.display().to_string();
        let _ = text_box.SetText(&HSTRING::from(text.as_str()));
        if !commit_profile(&profile, key, FieldValue::Text(text)) {
            restore_profile_field(&text_box, &profile, key);
        }
    })?);
    Ok(())
}

fn restore_profile_field(text_box: &TextBox, profile: &str, key: &str) {
    let Some(value) = fields::read_profile_field(&settings_store::current(), profile, key) else { return };
    let text = match value {
        FieldValue::Number(number) => number.to_string(),
        FieldValue::Text(text) => text,
        FieldValue::Bool(flag) => flag.to_string(),
    };
    let _ = text_box.SetText(&HSTRING::from(text));
}

/// Checks, saves and applies one profile field. Renaming moves the selection to the new name.
fn commit_profile(profile: &str, key: &'static str, value: FieldValue) -> bool {
    let mut candidate = settings_store::current();
    if let Err(error) = fields::write_profile_field(&mut candidate, profile, key, value.clone()) {
        report(key, &error.message);
        return false;
    }
    let renamed_to = match (key, &value) {
        ("profile.name", FieldValue::Text(text)) => Some(text.trim().to_owned()),
        _ => None,
    };
    let profile_name = profile.to_owned();
    settings_store::update(move |settings| {
        let _ = fields::write_profile_field(settings, &profile_name, key, value);
    });
    if let Some(new_name) = renamed_to {
        select_profile(&new_name);
        refresh_profile_page();
    }
    report(key, "");
    true
}

fn profile_status(message: &str) {
    if let Some(window) = OPEN.with(|slot| slot.borrow().clone()) {
        window.set_status(message);
    }
}

fn connect_profile_buttons(root: &FrameworkElement) -> Result<()> {
    let add: Button = find(root, "AddProfileButton")?;
    std::mem::forget(add.Click(move |_, _| {
        let name = settings_store::current().unique_profile_name("New profile");
        let created = name.clone();
        settings_store::update(move |settings| {
            settings.add_profile(&created);
        });
        select_profile(&name);
        refresh_profile_page();
    })?);
    let duplicate: Button = find(root, "DuplicateProfileButton")?;
    std::mem::forget(duplicate.Click(move |_, _| {
        let selected = selected_profile(&settings_store::current());
        let mut preview = settings_store::current();
        let Some(copy) = preview.duplicate_profile(&selected) else { return };
        settings_store::update(move |settings| {
            settings.duplicate_profile(&selected);
        });
        select_profile(&copy);
        refresh_profile_page();
    })?);
    let delete: Button = find(root, "DeleteProfileButton")?;
    std::mem::forget(delete.Click(move |_, _| {
        let selected = selected_profile(&settings_store::current());
        if settings_store::current().profiles.len() <= 1 {
            profile_status("The last profile cannot be deleted.");
            return;
        }
        profile_status("");
        settings_store::update(move |settings| {
            settings.delete_profile(&selected);
        });
        refresh_profile_page();
    })?);
    let default: Button = find(root, "DefaultProfileButton")?;
    std::mem::forget(default.Click(move |_, _| {
        let selected = selected_profile(&settings_store::current());
        settings_store::update(move |settings| {
            settings.set_default_profile(&selected);
        });
        refresh_profile_page();
    })?);
    let detect: Button = find(root, "DetectShellsButton")?;
    std::mem::forget(detect.Click(move |_, _| {
        let found = crate::shell_detection::detect();
        let mut preview = settings_store::current();
        let added = preview.merge_detected_profiles(found.clone());
        settings_store::update(move |settings| {
            settings.merge_detected_profiles(found);
        });
        profile_status(&match added {
            0 => "No new shells found; profiles for the shells on this PC already exist.".to_owned(),
            1 => "Added 1 profile.".to_owned(),
            count => format!("Added {count} profiles."),
        });
        refresh_profile_page();
    })?);
    Ok(())
}
