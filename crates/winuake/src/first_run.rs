//! First-run dialog (Yakuake `firstrundialog.ui`): asks for the global key that opens and closes the window. Shown once; the answer is
//! saved as `toggle_hotkey` and `dialogs.first_run` is switched off.

use crate::bindings::{Button, FrameworkElement, TextBox, UIElement, Window, XamlReader};
use crate::settings_store;
use crate::window_bindings::{BringWindowToTop, HWND, HWND_TOPMOST, SWP_NOMOVE, SetForegroundWindow, SetWindowPos};
use crate::window_native::IWindowNative;
use std::cell::{Cell, RefCell};
use windows_core::{HSTRING, Interface, Result};
use winuake_core::shortcut::{Chord, ChordModifiers};

const XAML_NAMESPACE: &str = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
const XAML_X_NAMESPACE: &str = "http://schemas.microsoft.com/winfx/2006/xaml";

thread_local! {
    static OPEN: RefCell<Option<Window>> = const { RefCell::new(None) };
    static CHOSEN: RefCell<Option<String>> = const { RefCell::new(None) };
    static FINISHED: Cell<bool> = const { Cell::new(false) };
}

/// Shows the dialog when the settings say this is the first run.
pub fn show_if_first_run() {
    if !settings_store::current().dialogs.first_run {
        return;
    }
    // A test run with overrides never saves, so it would ask every time.
    if std::env::var_os("WINUAKE_WINDOW_SETTINGS").is_some() {
        return;
    }
    if let Err(error) = show() {
        tracing::error!(%error, "could not show the first-run dialog");
    }
}

fn show() -> Result<()> {
    let markup = format!(
        r##"<Grid xmlns="{XAML_NAMESPACE}" xmlns:x="{XAML_X_NAMESPACE}" Background="#FF202020" Padding="28">
  <StackPanel>
    <TextBlock Text="Welcome to Winuake" Foreground="#FFEEEEEE" FontSize="22"/>
    <TextBlock Margin="0,12,0,0" TextWrapping="Wrap" Foreground="#FFCCCCCC" Text="Pick the key combination that opens and closes the window from anywhere. Click the box, then press the keys. You can change this later in Settings."/>
    <TextBox x:Name="HotkeyBox" Margin="0,18,0,0" IsReadOnly="True" PlaceholderText="Press the keys (for example F12 or Ctrl+Alt+Q)" Width="360" HorizontalAlignment="Left"/>
    <TextBlock x:Name="Hint" Margin="0,8,0,0" Foreground="#FF999999" FontSize="12" Text="Leave it empty to use F12, or Win+` if Windows refuses F12."/>
    <StackPanel Orientation="Horizontal" Margin="0,22,0,0">
      <Button x:Name="OkButton" Content="OK" MinWidth="90"/>
      <Button x:Name="SkipButton" Content="Use default" Margin="10,0,0,0" MinWidth="110"/>
    </StackPanel>
  </StackPanel>
</Grid>"##
    );
    let root: FrameworkElement = XamlReader::Load(&HSTRING::from(markup))?.cast()?;
    let window = Window::new()?;
    window.SetTitle(&HSTRING::from("Welcome to Winuake"))?;
    window.SetContent(&root)?;
    FINISHED.with(|flag| flag.set(false));
    CHOSEN.with(|slot| slot.borrow_mut().take());

    let hotkey_box: TextBox = root.FindName(&HSTRING::from("HotkeyBox"))?.cast()?;
    let capture_target = hotkey_box.clone();
    let capture_ui: UIElement = hotkey_box.cast()?;
    std::mem::forget(capture_ui.GotFocus(move |_, _| crate::terminal_view::suspend_text_routing())?);
    std::mem::forget(capture_ui.KeyDown(move |_, args| {
        crate::terminal_view::suspend_text_routing();
        let Some(args) = args.as_ref() else { return };
        let Ok(key) = args.Key() else { return };
        let _ = args.SetHandled(true);
        const VK_BACK: i32 = 0x08;
        const VK_DELETE: i32 = 0x2E;
        if key.0 == VK_BACK || key.0 == VK_DELETE {
            CHOSEN.with(|slot| slot.borrow_mut().take());
            let _ = capture_target.SetText(&HSTRING::new());
            return;
        }
        let modifiers = crate::terminal_view::current_modifiers();
        let chord_modifiers = ChordModifiers { control: modifiers.control, shift: modifiers.shift, alt: modifiers.alt };
        if let Some(chord) = Chord::from_virtual_key(key.0, chord_modifiers) {
            let text = chord.to_string();
            let _ = capture_target.SetText(&HSTRING::from(text.as_str()));
            CHOSEN.with(|slot| *slot.borrow_mut() = Some(text));
        }
    })?);

    let ok: Button = root.FindName(&HSTRING::from("OkButton"))?.cast()?;
    let ok_window = window.clone();
    std::mem::forget(ok.Click(move |_, _| finish(&ok_window, true))?);
    let skip: Button = root.FindName(&HSTRING::from("SkipButton"))?.cast()?;
    let skip_window = window.clone();
    std::mem::forget(skip.Click(move |_, _| finish(&skip_window, false))?);
    let closing_window = window.clone();
    std::mem::forget(window.Closed(move |_, _| {
        // Closing with the X counts as "use the default".
        if !FINISHED.with(Cell::get) {
            finish(&closing_window, false);
        }
        OPEN.with(|slot| slot.borrow_mut().take());
    })?);

    window.Activate()?;
    if let Ok(native) = window.cast::<IWindowNative>()
        && let Ok(handle) = native.handle()
    {
        let hwnd = HWND(handle as *mut core::ffi::c_void);
        unsafe {
            let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 900, 620, SWP_NOMOVE as u32);
            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);
        }
    }
    OPEN.with(|slot| *slot.borrow_mut() = Some(window));
    Ok(())
}

/// Saves the answer (when `use_chosen`) and the fact that the dialog was seen, then closes it.
fn finish(window: &Window, use_chosen: bool) {
    if FINISHED.with(|flag| flag.replace(true)) {
        return;
    }
    let chosen = if use_chosen { CHOSEN.with(|slot| slot.borrow().clone()) } else { None };
    tracing::info!(custom_hotkey = chosen.is_some(), "first-run dialog finished");
    settings_store::update(|settings| {
        settings.dialogs.first_run = false;
        if chosen.is_some() {
            settings.toggle_hotkey = chosen;
        }
    });
    let _ = window.Close();
}
