//! Title bar and tab bar. Ports the structure and behavior of Yakuake `titlebar.cpp` and `tabbar.cpp`; the look is a plain placeholder until
//! skins arrive in Phase 6.
//!
//! Both bars are XAML markup loaded at runtime. The tab strip is rebuilt from the workspace whenever it changes, and each tab's handlers
//! capture only its session id.

use crate::bindings::{
    ContentControl, FrameworkElement, Grid, Panel, PointerRoutedEventArgs, TextBlock, TextBox, UIElement, Visibility,
    XamlReader,
};
use crate::window_bindings::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, GetDoubleClickTime, GetTickCount64, HMENU, HWND, LPARAM,
    MF_CHECKED, MF_GRAYED, MF_SEPARATOR, MF_STRING, POINT, PostMessageW, SetForegroundWindow, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TrackPopupMenuEx, WPARAM,
};
use crate::workspace_host::{self, WorkspaceHost};
use std::cell::Cell;
use std::rc::Rc;
use windows_core::{HSTRING, Interface, PCWSTR, Result, w};
use winuake_core::actions::Action;
use winuake_core::ids::SessionId;
use winuake_core::sessions::session::TerminalSwitch;

const XAML_NAMESPACE: &str = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
const XAML_X_NAMESPACE: &str = "http://schemas.microsoft.com/winfx/2006/xaml";
const ICON_FONT: &str = "Segoe MDL2 Assets";
const VK_RETURN: i32 = 0x0D;
const VK_ESCAPE: i32 = 0x1B;
/// Pointer travel (DIPs) before a tab press counts as a drag rather than a click.
const DRAG_THRESHOLD: f64 = 6.0;
const WM_NULL: u32 = 0;

const COLOR_TAB_SELECTED: &str = "#FF3C3C3C";
const COLOR_TAB_IDLE: &str = "#FF2B2B2B";
const COLOR_TEXT: &str = "#FFDDDDDD";
const COLOR_TEXT_DIM: &str = "#FF999999";
const COLOR_ACCENT: &str = "#FF4FA3FF";

thread_local! {
    static TAB_PRESS: Cell<Option<TabPress>> = const { Cell::new(None) };
    static LAST_TAB_CLICK: Cell<Option<TabClick>> = const { Cell::new(None) };
    static RENAMING: Cell<Option<SessionId>> = const { Cell::new(None) };
}

#[derive(Clone, Copy)]
struct TabPress {
    session: SessionId,
    start_x: f64,
}

#[derive(Clone, Copy)]
struct TabClick {
    session: SessionId,
    at_millis: u64,
}

pub struct Chrome {
    root: Grid,
    content: ContentControl,
    title_bar: UIElement,
    title_bar_hidden: Cell<bool>,
    title_text: TextBlock,
    keep_open_button: TextBlock,
    tab_strip: Panel,
    new_tab_button: TextBlock,
}

fn load(markup: &str) -> Result<FrameworkElement> {
    XamlReader::Load(&HSTRING::from(markup))?.cast()
}

fn find<T: Interface>(root: &FrameworkElement, name: &str) -> Result<T> {
    root.FindName(&HSTRING::from(name))?.cast()
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

impl Chrome {
    pub fn create() -> Result<Self> {
        let markup = format!(
            r##"<Grid xmlns="{XAML_NAMESPACE}" xmlns:x="{XAML_X_NAMESPACE}" Background="#FF1E1E1E">
  <Grid.RowDefinitions>
    <RowDefinition Height="Auto"/><RowDefinition Height="Auto"/><RowDefinition Height="*"/>
  </Grid.RowDefinitions>
  <Grid x:Name="TitleBar" Grid.Row="0" Height="26" Background="#FF2B2B2B">
    <TextBlock x:Name="TitleText" Foreground="{COLOR_TEXT}" FontSize="12" VerticalAlignment="Center" HorizontalAlignment="Center"/>
    <StackPanel Orientation="Horizontal" HorizontalAlignment="Left" Margin="6,0,0,0">
      <TextBlock x:Name="KeepOpenButton" Text="&#xE718;" FontFamily="{ICON_FONT}" FontSize="12" Foreground="{COLOR_TEXT_DIM}" VerticalAlignment="Center" Padding="6,0,6,0"/>
    </StackPanel>
    <StackPanel Orientation="Horizontal" HorizontalAlignment="Right" Margin="0,0,6,0">
      <TextBlock x:Name="MenuButton" Text="&#xE713;" FontFamily="{ICON_FONT}" FontSize="12" Foreground="{COLOR_TEXT_DIM}" VerticalAlignment="Center" Padding="6,0,6,0"/>
      <TextBlock x:Name="QuitButton" Text="&#xE711;" FontFamily="{ICON_FONT}" FontSize="12" Foreground="{COLOR_TEXT_DIM}" VerticalAlignment="Center" Padding="6,0,6,0"/>
    </StackPanel>
  </Grid>
  <Grid x:Name="TabBar" Grid.Row="1" Height="28" Background="#FF252525">
    <StackPanel x:Name="TabStrip" Orientation="Horizontal" HorizontalAlignment="Left"/>
  </Grid>
  <ContentControl x:Name="ContentHost" Grid.Row="2" HorizontalContentAlignment="Stretch" VerticalContentAlignment="Stretch" IsTabStop="False"/>
</Grid>"##
        );
        let root_element = load(&markup)?;
        let new_tab_button: TextBlock = load(&format!(
            r#"<TextBlock xmlns="{XAML_NAMESPACE}" Text="&#xE710;" FontFamily="{ICON_FONT}" FontSize="12" Foreground="{COLOR_TEXT_DIM}" VerticalAlignment="Center" Padding="10,0,10,0"/>"#
        ))?
        .cast()?;
        Ok(Self {
            root: root_element.cast()?,
            content: find(&root_element, "ContentHost")?,
            title_bar: find(&root_element, "TitleBar")?,
            title_bar_hidden: Cell::new(false),
            title_text: find(&root_element, "TitleText")?,
            keep_open_button: find(&root_element, "KeepOpenButton")?,
            tab_strip: find(&root_element, "TabStrip")?,
            new_tab_button,
        })
    }

    pub fn root(&self) -> UIElement {
        self.root.cast().expect("a Grid is a UIElement")
    }

    pub fn content(&self) -> ContentControl {
        self.content.clone()
    }

    /// Hooks up the title bar buttons and the new-tab button. Called once, after the host exists.
    pub fn connect(&self, _host: &Rc<WorkspaceHost>) -> Result<()> {
        let root: FrameworkElement = self.root.cast()?;
        let keep_open: UIElement = self.keep_open_button.cast()?;
        std::mem::forget(keep_open.PointerPressed(move |_, args| {
            if let Some(args) = args.as_ref() {
                let _ = args.SetHandled(true);
            }
            workspace_host::run_action(Action::KeepOpen);
        })?);
        let quit: UIElement = find(&root, "QuitButton")?;
        std::mem::forget(quit.PointerPressed(move |_, args| {
            if let Some(args) = args.as_ref() {
                let _ = args.SetHandled(true);
            }
            workspace_host::request_quit();
        })?);
        let menu: UIElement = find(&root, "MenuButton")?;
        std::mem::forget(menu.PointerPressed(move |_, args| {
            if let Some(args) = args.as_ref() {
                let _ = args.SetHandled(true);
            }
            show_main_menu();
        })?);
        let new_tab: UIElement = self.new_tab_button.cast()?;
        std::mem::forget(new_tab.PointerPressed(move |_, args| {
            if let Some(args) = args.as_ref() {
                let _ = args.SetHandled(true);
            }
            workspace_host::run_action(Action::NewSession);
        })?);
        self.update_keep_open_indicator();
        Ok(())
    }

    pub fn set_title(&self, title: &str) {
        if let Err(error) = self.title_text.SetText(&HSTRING::from(title)) {
            tracing::warn!(%error, "could not set the title text");
        }
    }

    pub fn toggle_keep_open(&self) {
        crate::drop_down::toggle_keep_open();
        self.update_keep_open_indicator();
    }

    /// A filled pin while the window stays open on focus loss, an outline pin otherwise.
    fn update_keep_open_indicator(&self) {
        let glyph = if crate::drop_down::keep_open() { "\u{E840}" } else { "\u{E718}" };
        let _ = self.keep_open_button.SetText(&HSTRING::from(glyph));
    }

    pub fn toggle_title_bar(&self) {
        let visible = !self.title_bar_hidden.get();
        let next = if visible { Visibility::Collapsed } else { Visibility::Visible };
        match self.title_bar.SetVisibility(next) {
            Ok(()) => self.title_bar_hidden.set(visible),
            Err(error) => tracing::warn!(%error, "could not toggle the title bar"),
        }
    }

    /// Rebuilds the tab strip from the workspace.
    pub fn refresh_tabs(&self, host: &WorkspaceHost) {
        if let Err(error) = self.try_refresh_tabs(host) {
            tracing::error!(%error, "could not rebuild the tab bar");
        }
    }

    fn try_refresh_tabs(&self, host: &WorkspaceHost) -> Result<()> {
        let children = self.tab_strip.Children()?;
        children.Clear()?;
        let renaming = RENAMING.with(Cell::get);
        let mut rename_box = None;
        {
            let workspace = host.workspace();
            let tabs = workspace.tabs();
            for session in tabs.order() {
                let selected = tabs.selected() == Some(*session);
                let title = tabs.title(*session).unwrap_or_default();
                let locked = workspace.stack().session(*session).is_some_and(|candidate| !candidate.closable());
                let (tab, rename) = build_tab(*session, title, selected, locked, renaming == Some(*session))?;
                children.Append(&tab.cast::<UIElement>()?)?;
                if rename.is_some() {
                    rename_box = rename;
                }
            }
        }
        children.Append(&self.new_tab_button.cast::<UIElement>()?)?;
        if let Some(rename_box) = rename_box {
            workspace_host::suspend_text_routing();
            let _ = rename_box.Focus(crate::bindings::FocusState::Programmatic);
            let _ = rename_box.SelectAll();
        }
        Ok(())
    }

    pub fn begin_rename(&self, host: &WorkspaceHost, session: SessionId) {
        RENAMING.with(|slot| slot.set(Some(session)));
        self.refresh_tabs(host);
    }
}

/// Builds one tab. Returns the element and, when it is being renamed, its text box.
fn build_tab(
    session: SessionId,
    title: &str,
    selected: bool,
    locked: bool,
    renaming: bool,
) -> Result<(FrameworkElement, Option<TextBox>)> {
    let background = if selected { COLOR_TAB_SELECTED } else { COLOR_TAB_IDLE };
    let accent = if selected { COLOR_ACCENT } else { background };
    let lock = if locked {
        format!(
            r#"<TextBlock Text="&#xE72E;" FontFamily="{ICON_FONT}" FontSize="10" Foreground="{COLOR_TEXT_DIM}" Margin="0,0,6,0" VerticalAlignment="Center"/>"#
        )
    } else {
        String::new()
    };
    let label = if renaming {
        format!(
            r#"<TextBox x:Name="RenameBox" Text="{}" MinWidth="90" FontSize="12" Padding="2" VerticalAlignment="Center"/>"#,
            escape(title)
        )
    } else {
        format!(
            r#"<TextBlock Text="{}" Foreground="{COLOR_TEXT}" FontSize="12" VerticalAlignment="Center"/>"#,
            escape(title)
        )
    };
    let markup = format!(
        r#"<Border xmlns="{XAML_NAMESPACE}" xmlns:x="{XAML_X_NAMESPACE}" Background="{background}" BorderBrush="{accent}" BorderThickness="0,2,0,0" Height="28" MinWidth="90" Padding="10,0,6,0">
  <StackPanel Orientation="Horizontal">{lock}{label}
    <TextBlock x:Name="CloseButton" Text="&#xE711;" FontFamily="{ICON_FONT}" FontSize="10" Foreground="{COLOR_TEXT_DIM}" Margin="10,0,0,0" VerticalAlignment="Center"/>
  </StackPanel>
</Border>"#
    );
    let tab = load(&markup)?;
    let ui: UIElement = tab.cast()?;
    connect_tab_pointer(&ui, &tab, session)?;
    let close: UIElement = find(&tab, "CloseButton")?;
    std::mem::forget(close.PointerPressed(move |_, args| {
        if let Some(args) = args.as_ref() {
            let _ = args.SetHandled(true);
        }
        workspace_host::close_session(session);
    })?);
    let rename_box = if renaming {
        let text_box: TextBox = find(&tab, "RenameBox")?;
        connect_rename_box(&text_box, session)?;
        Some(text_box)
    } else {
        None
    };
    Ok((tab, rename_box))
}

fn connect_rename_box(text_box: &TextBox, session: SessionId) -> Result<()> {
    let ui: UIElement = text_box.cast()?;
    let source = text_box.clone();
    std::mem::forget(ui.KeyDown(move |_, args| {
        workspace_host::suspend_text_routing();
        let Some(args) = args.as_ref() else { return };
        match args.Key().map(|key| key.0) {
            Ok(VK_RETURN) => {
                let _ = args.SetHandled(true);
                finish_rename(session, Some(&source));
            }
            Ok(VK_ESCAPE) => {
                let _ = args.SetHandled(true);
                finish_rename(session, None);
            }
            _ => {}
        }
    })?);
    let lost = text_box.clone();
    std::mem::forget(ui.LostFocus(move |_, _| finish_rename(session, Some(&lost)))?);
    Ok(())
}

/// Commits (`Some`) or cancels (`None`) a rename, once.
fn finish_rename(session: SessionId, text_box: Option<&TextBox>) {
    if RENAMING.with(Cell::take) != Some(session) {
        return;
    }
    let title = text_box.and_then(|text_box| text_box.Text().ok()).map(|text| text.to_string_lossy());
    workspace_host::finish_rename(session, title.as_deref());
}

fn now_millis() -> u64 {
    unsafe { GetTickCount64() }
}

fn connect_tab_pointer(ui: &UIElement, tab: &FrameworkElement, session: SessionId) -> Result<()> {
    let tab_for_position = tab.clone();
    let capture_target = ui.clone();
    std::mem::forget(ui.PointerPressed(move |_, args| {
        let Some(args) = args.as_ref() else { return };
        let Ok(point) = args.GetCurrentPoint(&tab_for_position) else { return };
        let Ok(properties) = point.Properties() else { return };
        if properties.IsMiddleButtonPressed().unwrap_or(false) {
            let _ = args.SetHandled(true);
            workspace_host::close_session(session);
        } else if properties.IsRightButtonPressed().unwrap_or(false) {
            let _ = args.SetHandled(true);
            show_tab_menu(session);
        } else {
            let start_x = point.Position().map(|position| f64::from(position.X)).unwrap_or(0.0);
            TAB_PRESS.with(|slot| slot.set(Some(TabPress { session, start_x })));
            if let Ok(pointer) = args.Pointer() {
                let _ = capture_target.CapturePointer(&pointer);
            }
            let _ = args.SetHandled(true);
        }
    })?);
    let release_target = ui.clone();
    let strip_relative = tab.clone();
    std::mem::forget(ui.PointerReleased(move |_, args| {
        let Some(args) = args.as_ref() else { return };
        if let Ok(pointer) = args.Pointer() {
            let _ = release_target.ReleasePointerCapture(&pointer);
        }
        let Some(press) = TAB_PRESS.with(Cell::take) else { return };
        if press.session != session {
            return;
        }
        release_tab(press, args, &strip_relative);
    })?);
    Ok(())
}

/// A left press on a tab ended: a short movement is a click (select, or rename on double click), a long one reorders.
fn release_tab(press: TabPress, args: &PointerRoutedEventArgs, tab: &FrameworkElement) {
    let Ok(point) = args.GetCurrentPoint(tab) else { return };
    let Ok(position) = point.Position() else { return };
    let travelled = f64::from(position.X) - press.start_x;
    if travelled.abs() >= DRAG_THRESHOLD {
        let tab_width = tab.ActualWidth().unwrap_or(1.0).max(1.0);
        let steps = (travelled / tab_width).round() as i64;
        workspace_host::drag_session_by(press.session, steps);
        return;
    }
    let now = now_millis();
    let double_click_window = u64::from(unsafe { GetDoubleClickTime() });
    let previous = LAST_TAB_CLICK.with(Cell::take);
    if previous.is_some_and(|click| {
        click.session == press.session && now.saturating_sub(click.at_millis) <= double_click_window
    }) {
        workspace_host::begin_rename(press.session);
        return;
    }
    LAST_TAB_CLICK.with(|slot| slot.set(Some(TabClick { session: press.session, at_millis: now })));
    workspace_host::raise_session(press.session);
}

// ---- menus ----

const TAB_MENU_MOVE_LEFT: usize = 1;
const TAB_MENU_MOVE_RIGHT: usize = 2;
const TAB_MENU_RENAME: usize = 3;
const TAB_MENU_CLOSE: usize = 4;
const TAB_MENU_PREVENT_CLOSING: usize = 5;
const TAB_MENU_KEYBOARD_INPUT: usize = 6;
const TAB_MENU_MONITOR_ACTIVITY: usize = 7;
const TAB_MENU_MONITOR_SILENCE: usize = 8;

fn flags(enabled: bool, checked: bool) -> u32 {
    let mut value = MF_STRING as u32;
    if !enabled {
        value |= MF_GRAYED as u32;
    }
    if checked {
        value |= MF_CHECKED as u32;
    }
    value
}

/// Tab context menu (Yakuake `TabBar::contextMenuEvent`): move, rename, close, and the per-session switches.
fn show_tab_menu(session: SessionId) {
    let Some(state) = workspace_host::tab_menu_state(session) else { return };
    let chosen = track_menu(|menu| unsafe {
        let _ = AppendMenuW(menu, flags(state.can_move_left, false), TAB_MENU_MOVE_LEFT, w!("Move Session Left"));
        let _ = AppendMenuW(menu, flags(state.can_move_right, false), TAB_MENU_MOVE_RIGHT, w!("Move Session Right"));
        let _ = AppendMenuW(menu, MF_SEPARATOR as u32, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, flags(true, false), TAB_MENU_RENAME, w!("Rename Session..."));
        let _ = AppendMenuW(menu, flags(true, false), TAB_MENU_CLOSE, w!("Close Session"));
        let _ = AppendMenuW(menu, MF_SEPARATOR as u32, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, flags(true, !state.closable), TAB_MENU_PREVENT_CLOSING, w!("Prevent Closing"));
        let _ = AppendMenuW(
            menu,
            flags(true, state.keyboard_input_disabled),
            TAB_MENU_KEYBOARD_INPUT,
            w!("Disable Keyboard Input"),
        );
        let _ = AppendMenuW(
            menu,
            flags(true, state.monitor_activity),
            TAB_MENU_MONITOR_ACTIVITY,
            w!("Monitor for Activity"),
        );
        let _ =
            AppendMenuW(menu, flags(true, state.monitor_silence), TAB_MENU_MONITOR_SILENCE, w!("Monitor for Silence"));
    });
    match chosen {
        TAB_MENU_MOVE_LEFT => workspace_host::move_session(session, -1),
        TAB_MENU_MOVE_RIGHT => workspace_host::move_session(session, 1),
        TAB_MENU_RENAME => workspace_host::begin_rename(session),
        TAB_MENU_CLOSE => workspace_host::close_session(session),
        TAB_MENU_PREVENT_CLOSING => workspace_host::toggle_prevent_closing(session),
        TAB_MENU_KEYBOARD_INPUT => workspace_host::toggle_switch(session, TerminalSwitch::KeyboardInput),
        TAB_MENU_MONITOR_ACTIVITY => workspace_host::toggle_switch(session, TerminalSwitch::MonitorActivity),
        TAB_MENU_MONITOR_SILENCE => workspace_host::toggle_switch(session, TerminalSwitch::MonitorSilence),
        _ => {}
    }
}

const MAIN_MENU_KEEP_OPEN: usize = 1;
const MAIN_MENU_TITLEBAR: usize = 2;
const MAIN_MENU_QUIT: usize = 3;

/// The title bar's menu button. Yakuake's full menu (settings, skins, help) arrives with the settings window.
fn show_main_menu() {
    let chosen = track_menu(|menu| unsafe {
        let _ = AppendMenuW(
            menu,
            flags(true, crate::drop_down::keep_open()),
            MAIN_MENU_KEEP_OPEN,
            w!("Keep window open when it loses focus"),
        );
        let _ = AppendMenuW(menu, flags(true, false), MAIN_MENU_TITLEBAR, w!("Toggle Titlebar"));
        let _ = AppendMenuW(menu, MF_SEPARATOR as u32, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, flags(true, false), MAIN_MENU_QUIT, w!("Quit"));
    });
    match chosen {
        MAIN_MENU_KEEP_OPEN => workspace_host::run_action(Action::KeepOpen),
        MAIN_MENU_TITLEBAR => workspace_host::run_action(Action::ToggleTitlebar),
        MAIN_MENU_QUIT => workspace_host::request_quit(),
        _ => {}
    }
}

/// Shows a popup menu at the cursor and returns the chosen command id (0 for none).
fn track_menu(fill: impl FnOnce(HMENU)) -> usize {
    let owner = HWND(workspace_host::window_handle() as *mut core::ffi::c_void);
    unsafe {
        let menu = CreatePopupMenu();
        fill(menu);
        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        // The menu only dismisses properly when its owner window is in the foreground.
        let _ = SetForegroundWindow(owner);
        let chosen = TrackPopupMenuEx(menu, (TPM_RIGHTBUTTON | TPM_RETURNCMD) as u32, cursor.x, cursor.y, owner, None).0
            as usize;
        let _ = PostMessageW(Some(owner), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        chosen
    }
}
