# Phase 5 output — Configuration + settings UI

## Done

- **5a** `winuake-config`: `Settings` with every kcfg group (Window, Behavior, Appearance, Animation, AutoOpen, Dialogs), shortcut overrides, `toggle_hotkey`, and profiles. TOML load never fails: a syntax error falls back to defaults and is reported, a bad group falls back alone, values are clamped to the kcfg ranges. `fields`: one table (page, label, kind, read/write with validation) drives the settings window. `winuake-core`: `ShortcutMap::with_overrides`, `Chord::hotkey`.
- **5b** App: `settings_store` reads/creates `%APPDATA%\Winuake\winuake.toml`, saves atomically, polls for outside edits every second and applies them live (window geometry, bars, tray icon, shortcuts, hotkey, workspace behavior). A file that does not parse mid-edit is ignored, keeping the running settings. Resize shortcuts, keep-open and title-bar toggles are saved. `toggle_hotkey` configures the global key. `WINUAKE_WINDOW_SETTINGS` still overrides window settings and stops writes while set.
- **5c** Profiles: program, arguments, directory, font, size, color scheme, scrollback, cursor shape and blink apply to new terminals. Color schemes: Campbell, One Half Dark, Solarized Dark/Light built in; Windows Terminal JSON and Konsole `.colorscheme` files in `%APPDATA%\Winuake\schemes` are imported.
- **5d** Settings window (Window / Behavior / Appearance / Shortcuts / Profile), from the title-bar menu or tray; edits are validated, saved and applied at once; shortcut boxes record the pressed keys (Backspace unbinds, Delete restores the default). `manage-profiles` / `edit-profile` open the Profile page.
- **5e** First-run dialog: records the global hotkey once, saves `toggle_hotkey` and `dialogs.first_run = false`.

## Verified on the Win11 VM

Defaults file written on first start; edits to the file applied live (width, title bar, hotkey, shortcut override); invalid TOML kept the running settings; Alt+Shift+Right saved the new width; profile with Solarized Dark, 14 pt and a bar cursor applied to a new session; settings window edits (width 70 applied at once, an out-of-range height refused with a message and restored); shortcut recording; first-run hotkey capture saved and registered.

## Not done / known gaps

- Profile and color-scheme changes affect new terminals only, not open ones. One active profile is edited in the UI; adding, deleting and switching profiles is done in the file (`[[profile]]`, `default_profile`).
- The settings window is plain markup: no skin list, no scheme picker (type the scheme name), no color pickers (hex text), light-themed text boxes in a dark window. The window ignores the screen DPI for its size.
- Settings the later phases use (skin, translucency, highlight, block indicator, FocusFollowsMouse, AutoOpen) are stored and editable but have no effect yet.
- `Win+...` hotkeys cannot be captured in the dialog.
- The settings window is not refreshed if the file changes while it is open.
