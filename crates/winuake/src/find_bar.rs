//! The find bar: a small panel at the top right of the terminal area. Typing searches as you go; Enter and Shift+Enter step through the
//! matches, Escape closes it. The searching itself is `winuake_term`'s; the current match shows as the terminal's selection.

use crate::bindings::{FrameworkElement, TextBlock, TextBox, ToggleButton, UIElement, Visibility};
use crate::terminal_view;
use std::cell::{Cell, RefCell};
use windows_core::{HSTRING, Interface, Result};
use winuake_core::ids::TerminalId;
use winuake_term::emulator::{SearchOutcome, SearchQuery, SearchStep};

const VK_RETURN: i32 = 0x0D;
const VK_ESCAPE: i32 = 0x1B;
const ICON_FONT: &str = "Segoe MDL2 Assets";

/// Goes inside the root grid, over the terminal area.
pub fn markup() -> String {
    format!(
        r##"<Border x:Name="FindBar" Grid.Row="0" Visibility="Collapsed" HorizontalAlignment="Right" VerticalAlignment="Top" Margin="0,8,24,0" Padding="6" CornerRadius="4" Background="#F2252526" BorderBrush="#FF3F3F46" BorderThickness="1">
  <StackPanel Orientation="Horizontal">
    <TextBox x:Name="FindText" Width="220" PlaceholderText="Find"/>
    <ToggleButton x:Name="FindCase" Content="Aa" Margin="6,0,0,0" ToolTipService.ToolTip="Match case"/>
    <ToggleButton x:Name="FindRegex" Content=".*" Margin="4,0,0,0" ToolTipService.ToolTip="Regular expression"/>
    <Button x:Name="FindPrevious" Content="&#xE70E;" FontFamily="{ICON_FONT}" Margin="6,0,0,0" ToolTipService.ToolTip="Previous match (Shift+Enter)"/>
    <Button x:Name="FindNext" Content="&#xE70D;" FontFamily="{ICON_FONT}" Margin="4,0,0,0" ToolTipService.ToolTip="Next match (Enter)"/>
    <TextBlock x:Name="FindStatus" Foreground="#FFCCCCCC" VerticalAlignment="Center" Margin="8,0,0,0" MinWidth="90"/>
    <Button x:Name="FindClose" Content="&#xE711;" FontFamily="{ICON_FONT}" Margin="4,0,0,0" ToolTipService.ToolTip="Close (Escape)"/>
  </StackPanel>
</Border>"##
    )
}

struct FindBarViews {
    container: UIElement,
    text: TextBox,
    case_sensitive: ToggleButton,
    regex: ToggleButton,
    status: TextBlock,
}

thread_local! {
    static VIEWS: RefCell<Option<FindBarViews>> = const { RefCell::new(None) };
    /// The terminal the bar searches.
    static TARGET: Cell<Option<TerminalId>> = const { Cell::new(None) };
}

fn find<T: Interface>(root: &FrameworkElement, name: &str) -> Result<T> {
    root.FindName(&HSTRING::from(name))?.cast()
}

/// Wires the bar found in `root` (the grid built with `markup`).
pub fn connect(root: &FrameworkElement) -> Result<()> {
    let views = FindBarViews {
        container: find(root, "FindBar")?,
        text: find(root, "FindText")?,
        case_sensitive: find(root, "FindCase")?,
        regex: find(root, "FindRegex")?,
        status: find(root, "FindStatus")?,
    };

    let text_ui: UIElement = views.text.cast()?;
    std::mem::forget(text_ui.GotFocus(move |_, _| terminal_view::suspend_text_routing())?);
    std::mem::forget(text_ui.KeyDown(move |_, args| {
        terminal_view::suspend_text_routing();
        let Some(args) = args.as_ref() else { return };
        match args.Key().map(|key| key.0) {
            Ok(VK_RETURN) => {
                let _ = args.SetHandled(true);
                let step =
                    if terminal_view::current_modifiers().shift { SearchStep::Previous } else { SearchStep::Next };
                search(step);
            }
            Ok(VK_ESCAPE) => {
                let _ = args.SetHandled(true);
                close();
            }
            _ => {}
        }
    })?);
    std::mem::forget(views.text.TextChanged(move |_, _| search(SearchStep::Refine))?);

    let button = |name: &str| -> Result<UIElement> { find(root, name) };
    std::mem::forget(button("FindNext")?.PointerPressed(move |_, _| search(SearchStep::Next))?);
    std::mem::forget(button("FindPrevious")?.PointerPressed(move |_, _| search(SearchStep::Previous))?);
    std::mem::forget(button("FindClose")?.PointerPressed(move |_, _| close())?);
    // The options change what matches, so the current match is looked at again.
    std::mem::forget(views.case_sensitive.Click(move |_, _| search(SearchStep::Refine))?);
    std::mem::forget(views.regex.Click(move |_, _| search(SearchStep::Refine))?);

    VIEWS.with(|slot| *slot.borrow_mut() = Some(views));
    Ok(())
}

/// Shows the bar and puts the caret in it; it searches `terminal`.
pub fn show(terminal: TerminalId) {
    tracing::info!(terminal_id = %terminal, "showing the find bar");
    TARGET.with(|slot| slot.set(Some(terminal)));
    VIEWS.with(|slot| {
        let slot = slot.borrow();
        let Some(views) = slot.as_ref() else { return };
        let _ = views.container.SetVisibility(Visibility::Visible);
        let _ = views.status.SetText(&HSTRING::new());
        let _ = views.text.Focus(crate::bindings::FocusState::Programmatic);
        let _ = views.text.SelectAll();
    });
}

/// Hides the bar and gives the keyboard back to the terminal. The current match stays selected, so it can be copied.
fn close() {
    VIEWS.with(|slot| {
        if let Some(views) = slot.borrow().as_ref() {
            let _ = views.container.SetVisibility(Visibility::Collapsed);
        }
    });
    if let Some(view) = TARGET.with(Cell::get).and_then(terminal_view::find) {
        view.focus();
    }
}

fn search(step: SearchStep) {
    let Some(view) = TARGET.with(Cell::get).and_then(terminal_view::find) else { return };
    VIEWS.with(|slot| {
        let slot = slot.borrow();
        let Some(views) = slot.as_ref() else { return };
        let query = SearchQuery {
            text: views.text.Text().map(|text| text.to_string_lossy()).unwrap_or_default(),
            case_sensitive: views.case_sensitive.IsChecked().unwrap_or(false),
            regex: views.regex.IsChecked().unwrap_or(false),
        };
        let message = match view.find(&query, step) {
            SearchOutcome::Found => "",
            SearchOutcome::NotFound if query.text.is_empty() => "",
            SearchOutcome::NotFound => "Not found",
            SearchOutcome::InvalidPattern => "Invalid pattern",
        };
        let _ = views.status.SetText(&HSTRING::from(message));
    });
}
