//! Title bar, tab bar and window borders. Ports the structure and behavior of Yakuake `titlebar.cpp` and `tabbar.cpp`; the look comes from
//! the selected skin (`skin_chrome`), with a plain built-in look if no skin loads.
//!
//! Both bars are XAML markup loaded at runtime. The tab strip is rebuilt from the workspace whenever it changes, and each tab's handlers
//! capture only its session id.

use crate::bindings::{
    ContentControl, FrameworkElement, Grid, Panel, PointerRoutedEventArgs, TextBlock, TextBox, Thickness, UIElement,
    Visibility, XamlReader,
};
use crate::skin_assets::AppSkin;
use crate::skin_chrome::{self, SkinButtonViews, TabLook};
use crate::window_bindings::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, GetDoubleClickTime, GetTickCount64, HMENU, HWND, LPARAM,
    MF_CHECKED, MF_GRAYED, MF_SEPARATOR, MF_STRING, POINT, PostMessageW, SetForegroundWindow, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TrackPopupMenuEx, WPARAM,
};
use crate::workspace_host::{self, WorkspaceHost};
use std::cell::{Cell, RefCell};
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

/// What the bars currently look like: a skin, or the built-in plain look when no skin loads.
struct Appearance {
    skin: Option<Rc<AppSkin>>,
    skin_id: String,
    hide_borders: bool,
    title: Option<TitleView>,
    tabs: Option<TabsView>,
}

struct TitleView {
    text: TextBlock,
    focus_button: Option<Rc<SkinButtonViews>>,
    plain_keep_open: Option<TextBlock>,
}

struct TabsView {
    strip: Panel,
    plain_new_tab: Option<TextBlock>,
    inline_plus: Option<UIElement>,
}

pub struct Chrome {
    root: Grid,
    border_host: ContentControl,
    content: ContentControl,
    tab_host: ContentControl,
    title_host: ContentControl,
    appearance: RefCell<Appearance>,
    /// The terminal title last shown; the skin caption is added when drawing.
    title: RefCell<String>,
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

fn plain_title_markup() -> String {
    format!(
        r##"<Grid xmlns="{XAML_NAMESPACE}" xmlns:x="{XAML_X_NAMESPACE}" Height="26" Background="#FF2B2B2B">
    <TextBlock x:Name="TitleText" Foreground="{COLOR_TEXT}" FontSize="12" VerticalAlignment="Center" HorizontalAlignment="Center" IsHitTestVisible="False"/>
    <StackPanel Orientation="Horizontal" HorizontalAlignment="Left" Margin="6,0,0,0">
      <TextBlock x:Name="KeepOpenButton" Text="&#xE718;" FontFamily="{ICON_FONT}" FontSize="12" Foreground="{COLOR_TEXT_DIM}" VerticalAlignment="Center" Padding="6,0,6,0"/>
    </StackPanel>
    <StackPanel Orientation="Horizontal" HorizontalAlignment="Right" Margin="0,0,6,0">
      <TextBlock x:Name="MenuButton" Text="&#xE713;" FontFamily="{ICON_FONT}" FontSize="12" Foreground="{COLOR_TEXT_DIM}" VerticalAlignment="Center" Padding="6,0,6,0"/>
      <TextBlock x:Name="QuitButton" Text="&#xE711;" FontFamily="{ICON_FONT}" FontSize="12" Foreground="{COLOR_TEXT_DIM}" VerticalAlignment="Center" Padding="6,0,6,0"/>
    </StackPanel>
  </Grid>"##
    )
}

fn plain_tab_bar_markup() -> String {
    format!(
        r##"<Grid xmlns="{XAML_NAMESPACE}" xmlns:x="{XAML_X_NAMESPACE}" Height="28" Background="#FF252525">
    <StackPanel x:Name="TabStrip" Orientation="Horizontal" HorizontalAlignment="Left"/>
  </Grid>"##
    )
}

fn press(element: &UIElement, action: impl Fn() + Send + 'static) -> Result<()> {
    std::mem::forget(element.PointerPressed(move |_, args| {
        if let Some(args) = args.as_ref() {
            let _ = args.SetHandled(true);
        }
        action();
    })?);
    Ok(())
}

impl Chrome {
    pub fn create() -> Result<Self> {
        let markup = format!(
            r##"<Grid xmlns="{XAML_NAMESPACE}" xmlns:x="{XAML_X_NAMESPACE}" Background="#00000000">
  <Grid.RowDefinitions>
    <RowDefinition Height="*"/><RowDefinition Height="Auto"/><RowDefinition Height="Auto"/>
  </Grid.RowDefinitions>
  <ContentControl x:Name="BorderHost" Grid.Row="0" HorizontalContentAlignment="Stretch" VerticalContentAlignment="Stretch" IsTabStop="False"/>
  <ContentControl x:Name="ContentHost" Grid.Row="0" HorizontalContentAlignment="Stretch" VerticalContentAlignment="Stretch" IsTabStop="False"/>
  <ContentControl x:Name="TabHost" Grid.Row="1" HorizontalContentAlignment="Stretch" IsTabStop="False" Padding="0"/>
  <ContentControl x:Name="TitleHost" Grid.Row="2" HorizontalContentAlignment="Stretch" IsTabStop="False" Padding="0"/>
</Grid>"##
        );
        let root_element = load(&markup)?;
        Ok(Self {
            root: root_element.cast()?,
            border_host: find(&root_element, "BorderHost")?,
            content: find(&root_element, "ContentHost")?,
            tab_host: find(&root_element, "TabHost")?,
            title_host: find(&root_element, "TitleHost")?,
            appearance: RefCell::new(Appearance {
                skin: None,
                skin_id: String::new(),
                hide_borders: false,
                title: None,
                tabs: None,
            }),
            title: RefCell::new(String::new()),
        })
    }

    pub fn root(&self) -> UIElement {
        self.root.cast().expect("a Grid is a UIElement")
    }

    pub fn content(&self) -> ContentControl {
        self.content.clone()
    }

    /// Called once after the host exists: draws the bars for the current settings.
    pub fn connect(&self, _host: &Rc<WorkspaceHost>) -> Result<()> {
        Ok(())
    }

    /// Draws the title bar, tab bar and borders for the skin in `settings`. Does nothing when neither the skin nor the border setting changed.
    pub fn apply_appearance(&self, settings: &winuake_config::settings::Settings, host: &WorkspaceHost) {
        let wanted = settings.appearance.skin.clone();
        let hide_borders = settings.appearance.hide_skin_borders;
        {
            let appearance = self.appearance.borrow();
            if appearance.title.is_some() && appearance.skin_id == wanted && appearance.hide_borders == hide_borders {
                return;
            }
        }
        let skin = AppSkin::load(&wanted).or_else(|| {
            tracing::warn!(skin = %wanted, "skin not found; using the default skin");
            AppSkin::load("default")
        });
        if let Err(error) = self.build(skin.map(Rc::new), &wanted, hide_borders) {
            tracing::error!(%error, "could not draw the skin; falling back to the plain look");
            if let Err(error) = self.build(None, &wanted, hide_borders) {
                tracing::error!(%error, "could not draw the plain look");
            }
        }
        self.refresh_tabs(host);
        let title = self.title.borrow().clone();
        self.set_title(&title);
    }

    fn build(&self, skin: Option<Rc<AppSkin>>, skin_id: &str, hide_borders: bool) -> Result<()> {
        crate::skin_chrome::clear_registry();
        let (title_element, tab_element) = match &skin {
            Some(skin) => (load(&skin_chrome::title_bar_markup(skin))?, load(&skin_chrome::tab_bar_markup(skin))?),
            None => (load(&plain_title_markup())?, load(&plain_tab_bar_markup())?),
        };
        let title_text: TextBlock = find(&title_element, "TitleText")?;
        let mut title = TitleView { text: title_text, focus_button: None, plain_keep_open: None };
        let mut tabs = TabsView { strip: find(&tab_element, "TabStrip")?, plain_new_tab: None, inline_plus: None };

        match &skin {
            Some(_) => {
                title.focus_button =
                    SkinButtonViews::connect(&title_element, "Focus", || workspace_host::run_action(Action::KeepOpen))?;
                SkinButtonViews::connect(&title_element, "Config", show_main_menu)?;
                SkinButtonViews::connect(&title_element, "Quit", workspace_host::request_quit)?;
                SkinButtonViews::connect(&tab_element, "Plus", || workspace_host::run_action(Action::NewSession))?;
                SkinButtonViews::connect(&tab_element, "Minus", workspace_host::close_selected_session)?;
            }
            None => {
                let keep_open: TextBlock = find(&title_element, "KeepOpenButton")?;
                press(&keep_open.cast()?, || workspace_host::run_action(Action::KeepOpen))?;
                title.plain_keep_open = Some(keep_open);
                press(&find(&title_element, "QuitButton")?, workspace_host::request_quit)?;
                press(&find(&title_element, "MenuButton")?, show_main_menu)?;
                let new_tab: TextBlock = load(&format!(
                    r#"<TextBlock xmlns="{XAML_NAMESPACE}" Text="&#xE710;" FontFamily="{ICON_FONT}" FontSize="12" Foreground="{COLOR_TEXT_DIM}" VerticalAlignment="Center" Padding="10,0,10,0"/>"#
                ))?
                .cast()?;
                press(&new_tab.cast()?, || workspace_host::run_action(Action::NewSession))?;
                tabs.plain_new_tab = Some(new_tab);
            }
        }
        if let Some(skin) = skin.as_ref().filter(|skin| skin.skin.tab_bar.plus_button_at_end_of_tabs) {
            let inline = load(&skin_chrome::plus_button_inline_markup(skin))?;
            SkinButtonViews::connect(&inline, "Plus", || workspace_host::run_action(Action::NewSession))?;
            tabs.inline_plus = Some(inline.cast()?);
        }
        crate::title_drag::connect(&title_element.cast()?)?;

        self.title_host.SetContent(&title_element)?;
        self.tab_host.SetContent(&tab_element)?;

        // Borders: the skin's color shows around the terminals, which sit inset by the border width.
        let border_width = match (&skin, hide_borders) {
            (Some(skin), false) => skin.skin.border.width.max(0),
            _ => 0,
        };
        let border_fill = match (&skin, hide_borders) {
            (Some(skin), false) => {
                format!(r##"<Rectangle xmlns="{XAML_NAMESPACE}" Fill="#FF{}"/>"##, &skin.skin.border.color.hex()[1..])
            }
            _ => format!(r##"<Rectangle xmlns="{XAML_NAMESPACE}" Fill="#00000000"/>"##),
        };
        self.border_host.SetContent(&load(&border_fill)?)?;
        let inset = f64::from(border_width);
        self.content.cast::<FrameworkElement>()?.SetMargin(Thickness {
            Left: inset,
            Top: 0.0,
            Right: inset,
            Bottom: inset,
        })?;

        let mut appearance = self.appearance.borrow_mut();
        appearance.skin = skin;
        appearance.skin_id = skin_id.to_owned();
        appearance.hide_borders = hide_borders;
        appearance.title = Some(title);
        appearance.tabs = Some(tabs);
        Ok(())
    }

    /// The text shown in the title bar: terminal title, then the skin's caption.
    fn display_title(&self, title: &str) -> String {
        let caption =
            self.appearance.borrow().skin.as_ref().map(|skin| skin.skin.title_bar.text.clone()).unwrap_or_default();
        match (title.is_empty(), caption.is_empty()) {
            (false, false) => format!("{title} - {caption}"),
            (true, false) => caption,
            _ => title.to_owned(),
        }
    }

    pub fn set_title(&self, title: &str) {
        *self.title.borrow_mut() = title.to_owned();
        let shown = self.display_title(title);
        let appearance = self.appearance.borrow();
        if let Some(view) = &appearance.title
            && let Err(error) = view.text.SetText(&HSTRING::from(shown))
        {
            tracing::warn!(%error, "could not set the title text");
        }
    }

    pub fn toggle_keep_open(&self) {
        crate::drop_down::toggle_keep_open();
    }

    /// `toggle-titlebar`: flips `ShowTitleBar` and saves it.
    pub fn toggle_title_bar(&self) {
        crate::settings_store::update(|settings| settings.window.show_title_bar = !settings.window.show_title_bar);
    }

    /// Shows or hides the title bar and tab bar and refreshes the keep-open indicator.
    pub fn apply_bar_visibility(&self, window: &winuake_config::window::WindowSettings) {
        let visibility = |shown: bool| if shown { Visibility::Visible } else { Visibility::Collapsed };
        if let Err(error) = self.title_host.SetVisibility(visibility(window.show_title_bar)) {
            tracing::warn!(%error, "could not change the title bar visibility");
        }
        if let Err(error) = self.tab_host.SetVisibility(visibility(window.show_tab_bar)) {
            tracing::warn!(%error, "could not change the tab bar visibility");
        }
        let appearance = self.appearance.borrow();
        if let Some(title) = &appearance.title {
            if let Some(button) = &title.focus_button {
                button.set_checked(window.keep_open);
            }
            if let Some(glyph) = &title.plain_keep_open {
                let text = if window.keep_open { "\u{E840}" } else { "\u{E718}" };
                let _ = glyph.SetText(&HSTRING::from(text));
            }
        }
    }

    /// Rebuilds the tab strip from the workspace.
    pub fn refresh_tabs(&self, host: &WorkspaceHost) {
        if let Err(error) = self.try_refresh_tabs(host) {
            tracing::error!(%error, "could not rebuild the tab bar");
        }
    }

    fn try_refresh_tabs(&self, host: &WorkspaceHost) -> Result<()> {
        let appearance = self.appearance.borrow();
        let Some(tabs) = &appearance.tabs else { return Ok(()) };
        let children = tabs.strip.Children()?;
        children.Clear()?;
        let renaming = RENAMING.with(Cell::get);
        let mut rename_box = None;
        {
            let workspace = host.workspace();
            let tab_list = workspace.tabs();
            let order = tab_list.order();
            let selected_index = tab_list.selected().and_then(|session| tab_list.index_of(session));
            for (index, session) in order.iter().enumerate() {
                let selected = tab_list.selected() == Some(*session);
                let title = tab_list.title(*session).unwrap_or_default();
                let locked = workspace.stack().session(*session).is_some_and(|candidate| !candidate.closable());
                let is_renaming = renaming == Some(*session);
                let (tab, rename) = match &appearance.skin {
                    Some(skin) => {
                        let look = TabLook {
                            title,
                            selected,
                            locked,
                            previous_is_selected: selected_index.is_some_and(|chosen| index == chosen + 1),
                            next_is_selected: selected_index.is_some_and(|chosen| chosen > 0 && index == chosen - 1),
                            renaming: is_renaming,
                        };
                        build_skin_tab(*session, &skin_chrome::tab_markup(skin, &look), is_renaming)?
                    }
                    None => build_tab(*session, title, selected, locked, is_renaming)?,
                };
                children.Append(&tab.cast::<UIElement>()?)?;
                if rename.is_some() {
                    rename_box = rename;
                }
            }
        }
        if let Some(plain) = &tabs.plain_new_tab {
            children.Append(&plain.cast::<UIElement>()?)?;
        }
        if let Some(inline) = &tabs.inline_plus {
            children.Append(inline)?;
        }
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

/// A tab drawn from a skin: pointer handling and the rename box, no per-tab close button (the skin has one close button).
fn build_skin_tab(session: SessionId, markup: &str, renaming: bool) -> Result<(FrameworkElement, Option<TextBox>)> {
    let tab = load(markup)?;
    let ui: UIElement = tab.cast()?;
    connect_tab_pointer(&ui, &tab, session)?;
    let rename_box = if renaming {
        let text_box: TextBox = find(&tab, "RenameBox")?;
        connect_rename_box(&text_box, session)?;
        Some(text_box)
    } else {
        None
    };
    Ok((tab, rename_box))
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
const MAIN_MENU_SETTINGS: usize = 4;
const MAIN_MENU_SHORTCUTS: usize = 5;

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
        let _ = AppendMenuW(menu, flags(true, false), MAIN_MENU_SETTINGS, w!("Settings..."));
        let _ = AppendMenuW(menu, flags(true, false), MAIN_MENU_SHORTCUTS, w!("Configure Shortcuts..."));
        let _ = AppendMenuW(menu, MF_SEPARATOR as u32, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, flags(true, false), MAIN_MENU_QUIT, w!("Quit"));
    });
    match chosen {
        MAIN_MENU_KEEP_OPEN => workspace_host::run_action(Action::KeepOpen),
        MAIN_MENU_TITLEBAR => workspace_host::run_action(Action::ToggleTitlebar),
        MAIN_MENU_SETTINGS => crate::settings_window::open(None),
        MAIN_MENU_SHORTCUTS => crate::settings_window::open_shortcuts(),
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
