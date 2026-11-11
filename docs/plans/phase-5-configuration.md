# Phase 5 — Configuration + settings UI

Parent: `winuake-port.md`. Ports `config/yakuake.kcfg`, `config/*settings.*`, `firstrundialog.*`.

## Behavior to port

- Every kcfg key (groups `Window`, `Behavior`, `Appearance`, `Animation`, `AutoOpen`, `Dialogs`) with Yakuake's defaults and ranges, stored in `%APPDATA%\Winuake\winuake.toml`. Skin keys are stored now, used in Phase 6; `UseWMAssist` is N/A.
- Settings apply live: edits in the UI and in the file take effect without restart. Changes made through actions (window width/height, keep open, title bar) are saved.
- Shortcuts: per-action overrides in `[shortcuts]`; a shortcut editor in the UI.
- Profiles replace Konsole profiles: shell, arguments, working directory, font, size, color scheme, scrollback, cursor. `manage-profiles` / `edit-profile` open the profile pages.
- Settings window with Window / Behavior / Appearance / Shortcuts / Profiles pages, mirroring `windowsettings.ui`, `behaviorsettings.ui`, `appearancesettings.ui`.
- First-run dialog (`Dialogs/FirstRun`): pick the toggle hotkey; saved as `toggle_hotkey`, registered at start.
- Color schemes: import Konsole `.colorscheme` and Windows Terminal JSON.

## Design

- `winuake-config` (pure): `Settings` (all groups), tolerant TOML parse (a bad group falls back to defaults with a reported problem, values clamped to kcfg ranges), serialization, `Profile`, color-scheme import. No file IO in the crate's logic; the app reads, writes and watches the file.
- Config shape: `[window] [behavior] [appearance] [animation] [auto_open] [dialogs] [shortcuts] [toggle_hotkey] [[profile]]`, snake_case keys, colors `"#RRGGBB"`, enums by name.
- `winuake-core`: shortcut overrides applied to `ShortcutMap`.
- App: settings store (load, atomic save, mtime poll reload), `SettingsChanged` fan-out to drop-down host, workspace, chrome, terminals; settings window built from XAML markup.

## Chunks

- **5a** `winuake-config`: `Settings`, TOML, ranges, shortcuts overrides, profiles model + tests; shortcut override support in core.
- **5b** App wiring: load at start, apply, save on action changes, live reload, configurable hotkey; `WINUAKE_WINDOW_SETTINGS` stays as an override.
- **5c** Profiles in use (shell, font, scheme, scrollback, cursor), color-scheme import.
- **5d** Settings window and shortcut editor.
- **5e** First-run hotkey dialog.

## Verification

Unit tests for parse/serialize/clamp/problems/overrides. On the VM: edit the file while running and watch the window change; change width via shortcut and see it saved; settings window edits apply live; first run shows the dialog once.
