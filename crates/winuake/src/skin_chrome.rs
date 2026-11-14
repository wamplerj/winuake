//! XAML markup for the title bar and tab bar drawn from a skin, plus the up/over/down button behavior.
//!
//! Everything is absolute positioning from the skin's coordinates: each image is an `Image` with a margin, left or right aligned.

use crate::bindings::{FrameworkElement, UIElement, Visibility};
use crate::skin_assets::{AppSkin, ShownImage};
use std::cell::Cell;
use std::rc::Rc;
use windows_core::{HSTRING, Interface, Result};
use winuake_skin::model::{Anchor, SkinButton, SkinColor, SkinImage};

pub const XAML_NAMESPACE: &str = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
pub const XAML_X_NAMESPACE: &str = "http://schemas.microsoft.com/winfx/2006/xaml";
const FALLBACK_BAR_HEIGHT: u32 = 22;

pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

struct Placement {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl Placement {
    fn at(left: i32, top: i32) -> Self {
        Self { left, top, right: 0, bottom: 0 }
    }

    fn text(&self) -> String {
        format!("{},{},{},{}", self.left, self.top, self.right, self.bottom)
    }
}

fn show(skin: &AppSkin, image: &Option<SkinImage>) -> Option<ShownImage> {
    image.as_ref().and_then(|image| skin.show(image))
}

fn image_element(name: &str, shown: &ShownImage, alignment: &str, placement: &Placement, collapsed: bool) -> String {
    let visibility = if collapsed { r#" Visibility="Collapsed""# } else { "" };
    let name_attribute = if name.is_empty() { String::new() } else { format!(r#" x:Name="{name}""#) };
    format!(
        r#"<Image{name_attribute} Source="{}" Width="{}" Height="{}" Stretch="Fill" HorizontalAlignment="{alignment}" VerticalAlignment="Top" Margin="{}"{visibility}/>"#,
        shown.uri,
        shown.width,
        shown.height,
        placement.text()
    )
}

/// The bar background: tiled (here stretched) middle between a left and right corner.
fn background_markup(
    back: &Option<ShownImage>,
    left: &Option<ShownImage>,
    right: &Option<ShownImage>,
    height: u32,
) -> String {
    let mut markup = String::new();
    let left_width = left.as_ref().map_or(0, |image| image.width) as i32;
    let right_width = right.as_ref().map_or(0, |image| image.width) as i32;
    if let Some(back) = back {
        markup += &format!(
            r#"<Image Source="{}" Height="{height}" Stretch="Fill" HorizontalAlignment="Stretch" VerticalAlignment="Top" Margin="{left_width},0,{right_width},0"/>"#,
            back.uri
        );
    }
    if let Some(left) = left {
        markup += &image_element("", left, "Left", &Placement::at(0, 0), false);
    }
    if let Some(right) = right {
        markup += &image_element("", right, "Right", &Placement::at(0, 0), false);
    }
    markup
}

fn button_extent(shown: &Option<ShownImage>) -> (u32, u32) {
    shown.as_ref().map_or((0, 0), |image| (image.width, image.height))
}

/// A button: three stacked images (up, over, down) of which one shows. Missing states fall back to the up image.
fn button_markup(prefix: &str, button: &SkinButton, skin: &AppSkin) -> String {
    let up = show(skin, &button.images.up);
    let Some(up_image) = up.clone() else { return String::new() };
    let over = show(skin, &button.images.over).unwrap_or_else(|| up_image.clone());
    let down = show(skin, &button.images.down).unwrap_or_else(|| up_image.clone());
    let (width, height) = button_extent(&up);
    let (alignment, placement) = match button.anchor {
        Anchor::Left => ("Left", Placement::at(button.position.x, button.position.y)),
        // Qt places the button's left edge at `window width - x`.
        Anchor::Right => {
            ("Right", Placement { left: 0, top: button.position.y, right: button.position.x - width as i32, bottom: 0 })
        }
    };
    format!(
        r##"<Grid x:Name="{prefix}Button" Width="{width}" Height="{height}" Background="#00000000" HorizontalAlignment="{alignment}" VerticalAlignment="Top" Margin="{}">{}{}{}</Grid>"##,
        placement.text(),
        image_element(&format!("{prefix}Up"), &up_image, "Left", &Placement::at(0, 0), false),
        image_element(&format!("{prefix}Over"), &over, "Left", &Placement::at(0, 0), true),
        image_element(&format!("{prefix}Down"), &down, "Left", &Placement::at(0, 0), true),
    )
}

pub fn title_bar_height(skin: &AppSkin) -> u32 {
    let bar = &skin.skin.title_bar;
    [&bar.background, &bar.left_corner, &bar.right_corner]
        .into_iter()
        .flatten()
        .map(|image| image.size.height)
        .max()
        .unwrap_or(FALLBACK_BAR_HEIGHT)
}

pub fn tab_bar_height(skin: &AppSkin) -> u32 {
    let bar = &skin.skin.tab_bar;
    [&bar.background, &bar.left_corner, &bar.right_corner]
        .into_iter()
        .flatten()
        .map(|image| image.size.height)
        .max()
        .unwrap_or(FALLBACK_BAR_HEIGHT)
}

fn color(color: SkinColor) -> String {
    format!("#FF{}", &color.hex()[1..])
}

/// Title bar: background, text, and the focus, config and quit buttons.
pub fn title_bar_markup(skin: &AppSkin) -> String {
    let bar = &skin.skin.title_bar;
    let height = title_bar_height(skin);
    let background = background_markup(
        &show(skin, &bar.background),
        &show(skin, &bar.left_corner),
        &show(skin, &bar.right_corner),
        height,
    );
    let weight = if bar.text_bold { "Bold" } else { "Normal" };
    // Qt draws text with its baseline at `y`; a 12px font puts the top about 12px higher.
    let text_top = (bar.text_position.y - 12).max(0);
    let (alignment, margin) = if bar.text_centered {
        ("Center", Placement::at(0, text_top))
    } else {
        ("Left", Placement::at(bar.text_position.x, text_top))
    };
    format!(
        r##"<Grid xmlns="{XAML_NAMESPACE}" xmlns:x="{XAML_X_NAMESPACE}" x:Name="TitleBar" Height="{height}" Background="#00000000">{background}<TextBlock x:Name="TitleText" FontSize="12" FontWeight="{weight}" Foreground="{}" IsHitTestVisible="False" HorizontalAlignment="{alignment}" VerticalAlignment="Top" Margin="{}"/>{}{}{}</Grid>"##,
        color(bar.text_color),
        margin.text(),
        button_markup("Focus", &bar.focus_button, skin),
        button_markup("Config", &bar.config_button, skin),
        button_markup("Quit", &bar.quit_button, skin),
    )
}

/// Tab bar shell: background, the strip that holds the tabs, and the plus and minus buttons. Tabs are added at run time.
pub fn tab_bar_markup(skin: &AppSkin) -> String {
    let bar = &skin.skin.tab_bar;
    let height = tab_bar_height(skin);
    let background = background_markup(
        &show(skin, &bar.background),
        &show(skin, &bar.left_corner),
        &show(skin, &bar.right_corner),
        height,
    );
    // Tab bar buttons have no `anchor` key: the plus button's x is from the left edge, the close button's from the right.
    let mut plus_button = bar.plus_button.clone();
    plus_button.anchor = Anchor::Left;
    let plus = if bar.plus_button_at_end_of_tabs { String::new() } else { button_markup("Plus", &plus_button, skin) };
    let minus = button_markup("Minus", &minus_button_right_anchored(bar.minus_button.clone()), skin);
    format!(
        r##"<Grid xmlns="{XAML_NAMESPACE}" xmlns:x="{XAML_X_NAMESPACE}" x:Name="TabBar" Height="{height}" Background="#00000000">{background}<StackPanel x:Name="TabStrip" Orientation="Horizontal" HorizontalAlignment="Left" VerticalAlignment="Top" Margin="{},{},0,0"/>{plus}{minus}</Grid>"##,
        bar.position.x, bar.position.y
    )
}

/// The close button's `x` is measured from the right edge (`width - x`), whatever the anchor field says.
fn minus_button_right_anchored(mut button: SkinButton) -> SkinButton {
    button.anchor = Anchor::Right;
    button
}

/// The plus button as a strip element, for skins that put it after the last tab.
pub fn plus_button_inline_markup(skin: &AppSkin) -> String {
    let bar = &skin.skin.tab_bar;
    let mut button = bar.plus_button.clone();
    button.anchor = Anchor::Left;
    // Position is an offset from the end of the tabs here, so it becomes a margin on the element.
    button_markup("Plus", &button, skin)
}

/// One tab. `previous_selected`/`next_selected` say whether the neighbor is the selected tab, which decides separators like Yakuake.
pub struct TabLook<'a> {
    pub title: &'a str,
    pub selected: bool,
    pub locked: bool,
    pub previous_is_selected: bool,
    pub next_is_selected: bool,
    pub renaming: bool,
}

pub fn tab_markup(skin: &AppSkin, look: &TabLook<'_>) -> String {
    let bar = &skin.skin.tab_bar;
    let height = tab_bar_height(skin).saturating_sub(bar.position.y.max(0) as u32);
    let (background, left_corner, right_corner) = if look.selected {
        (&bar.selected_background, &bar.selected_left_corner, &bar.selected_right_corner)
    } else {
        (&bar.unselected_background, &bar.unselected_left_corner, &bar.unselected_right_corner)
    };
    let separator = show(skin, &bar.separator);
    let left = show(skin, left_corner).or_else(|| (!look.previous_is_selected).then(|| separator.clone()).flatten());
    let right = show(skin, right_corner).or_else(|| (!look.next_is_selected).then(|| separator.clone()).flatten());
    let piece = |image: &Option<ShownImage>| {
        image.as_ref().map(|shown| image_element("", shown, "Left", &Placement::at(0, 0), false)).unwrap_or_default()
    };
    let lock = if look.locked {
        show(skin, &bar.prevent_closing_image)
            .map(|shown| {
                let position = bar.prevent_closing_position;
                format!(
                    r#"<Grid Width="{}" Height="{height}">{}</Grid>"#,
                    position.x.max(0) as u32 + shown.width,
                    image_element("", &shown, "Left", &Placement::at(position.x, position.y), false)
                )
            })
            .unwrap_or_default()
    } else {
        String::new()
    };
    let weight = if look.selected && bar.selected_text_bold { "Bold" } else { "Normal" };
    let label = if look.renaming {
        format!(
            r#"<TextBox x:Name="RenameBox" Text="{}" MinWidth="90" FontSize="12" Padding="2" VerticalAlignment="Center"/>"#,
            escape(look.title)
        )
    } else {
        format!(
            r#"<TextBlock Text="{}" Foreground="{}" FontSize="12" FontWeight="{weight}" VerticalAlignment="Center" Margin="5,0,5,0"/>"#,
            escape(look.title),
            color(bar.text_color)
        )
    };
    let back = show(skin, background)
        .map(|shown| {
            format!(
                r#"<Image Source="{}" Stretch="Fill" HorizontalAlignment="Stretch" VerticalAlignment="Stretch"/>"#,
                shown.uri
            )
        })
        .unwrap_or_default();
    format!(
        r##"<StackPanel xmlns="{XAML_NAMESPACE}" xmlns:x="{XAML_X_NAMESPACE}" Orientation="Horizontal" Height="{height}" Background="#00000000">{}{lock}<Grid>{back}{label}</Grid>{}</StackPanel>"##,
        piece(&left),
        piece(&right)
    )
}

// ---- button behavior ----

/// Shows one of up / over / down according to pointer state; `checked` keeps the down image (the focus button).
pub struct SkinButtonViews {
    up: UIElement,
    over: UIElement,
    down: UIElement,
    inside: Cell<bool>,
    pressed: Cell<bool>,
    checked: Cell<bool>,
}

impl SkinButtonViews {
    /// Wires `prefix`Button in `root`. Returns `None` when the skin has no image for that button.
    pub fn connect(
        root: &FrameworkElement,
        prefix: &str,
        on_click: impl Fn() + Send + 'static,
    ) -> Result<Option<Rc<Self>>> {
        Self::connect_with_menu(root, prefix, on_click, None)
    }

    /// Like `connect`, and with `on_menu` (the plus button's profile menu) the button also opens it on a right click or when held down.
    pub fn connect_with_menu(
        root: &FrameworkElement,
        prefix: &str,
        on_click: impl Fn() + Send + 'static,
        on_menu: Option<Box<dyn Fn() + Send>>,
    ) -> Result<Option<Rc<Self>>> {
        let find = |name: String| -> Result<UIElement> { root.FindName(&HSTRING::from(name))?.cast() };
        let Ok(hit): Result<UIElement> = find(format!("{prefix}Button")) else { return Ok(None) };
        let views = Rc::new(Self {
            up: find(format!("{prefix}Up"))?,
            over: find(format!("{prefix}Over"))?,
            down: find(format!("{prefix}Down"))?,
            inside: Cell::new(false),
            pressed: Cell::new(false),
            checked: Cell::new(false),
        });
        // Handlers must be Send; the views live on the UI thread, so each handler reaches them through a thread-local registry index.
        let index = register(Rc::clone(&views));
        std::mem::forget(hit.PointerEntered(move |_, _| {
            with_views(index, |views| {
                views.inside.set(true);
                views.refresh();
            });
        })?);
        std::mem::forget(hit.PointerExited(move |_, _| {
            with_views(index, |views| {
                views.inside.set(false);
                views.pressed.set(false);
                views.refresh();
            });
        })?);
        match on_menu {
            Some(on_menu) => {
                // Click, hold and right click go through the shared gesture; the handlers here only draw the pressed look.
                let on_menu: Rc<dyn Fn()> = Rc::from(on_menu as Box<dyn Fn()>);
                crate::hold_button::attach(&hit, on_click, move || {
                    // The menu takes the pointer, so the button would otherwise stay drawn as pressed.
                    with_views(index, |views| {
                        views.pressed.set(false);
                        views.refresh();
                    });
                    on_menu();
                })?;
                std::mem::forget(hit.PointerPressed(move |_, _| {
                    with_views(index, |views| {
                        views.pressed.set(true);
                        views.refresh();
                    });
                })?);
                std::mem::forget(hit.PointerReleased(move |_, _| {
                    with_views(index, |views| {
                        views.pressed.set(false);
                        views.refresh();
                    });
                })?);
            }
            None => {
                std::mem::forget(hit.PointerPressed(move |_, args| {
                    if let Some(args) = args.as_ref() {
                        let _ = args.SetHandled(true);
                    }
                    with_views(index, |views| {
                        views.pressed.set(true);
                        views.refresh();
                    });
                })?);
                std::mem::forget(hit.PointerReleased(move |_, args| {
                    if let Some(args) = args.as_ref() {
                        let _ = args.SetHandled(true);
                    }
                    let clicked = with_views(index, |views| {
                        let clicked = views.pressed.replace(false) && views.inside.get();
                        views.refresh();
                        clicked
                    });
                    if clicked == Some(true) {
                        on_click();
                    }
                })?);
            }
        }
        views.refresh();
        Ok(Some(views))
    }

    pub fn set_checked(&self, checked: bool) {
        self.checked.set(checked);
        self.refresh();
    }

    fn refresh(&self) {
        let state = if self.pressed.get() || self.checked.get() {
            2
        } else if self.inside.get() {
            1
        } else {
            0
        };
        let visibility = |shown: bool| if shown { Visibility::Visible } else { Visibility::Collapsed };
        let _ = self.up.SetVisibility(visibility(state == 0));
        let _ = self.over.SetVisibility(visibility(state == 1));
        let _ = self.down.SetVisibility(visibility(state == 2));
    }
}

/// Whether this press is the right mouse button.
pub fn is_right_button(args: &crate::bindings::PointerRoutedEventArgs, relative_to: &UIElement) -> bool {
    args.GetCurrentPoint(relative_to)
        .and_then(|point| point.Properties())
        .and_then(|properties| properties.IsRightButtonPressed())
        .unwrap_or(false)
}

thread_local! {
    static REGISTRY: std::cell::RefCell<Vec<Rc<SkinButtonViews>>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn register(views: Rc<SkinButtonViews>) -> usize {
    REGISTRY.with(|registry| {
        let mut registry = registry.borrow_mut();
        registry.push(views);
        registry.len() - 1
    })
}

fn with_views<T>(index: usize, action: impl FnOnce(&SkinButtonViews) -> T) -> Option<T> {
    let views = REGISTRY.with(|registry| registry.borrow().get(index).cloned());
    views.map(|views| action(&views))
}

/// Drops registered buttons from a bar that was replaced. (Handlers on the old elements die with them.)
pub fn clear_registry() {
    crate::hold_button::clear_registry();
    REGISTRY.with(|registry| registry.borrow_mut().clear());
}
