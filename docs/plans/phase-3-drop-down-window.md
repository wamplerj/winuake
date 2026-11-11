# Phase 3 — Drop-down window behavior

Parent: `winuake-port.md`. Ports the window logic of Yakuake `mainwindow.cpp` (`applyWindowGeometry`, `setWindowGeometry`, `_toggleWindowState`, `xshape*Window` animation, `wmActiveWindowChanged`, `getScreen`, `getDesktopGeometry`, width/height actions) onto Win32.

## Behavior to port (from Yakuake)

- **Geometry**: work area of the chosen screen; width = work.w x Width%, height = work.h x Height%, x = work.x + work.w x Position% x (100 - Width%) / 10000, y = work.y. Full screen: 100% x 100% of the whole screen (taskbar covered).
- **Screen**: `Screen = 0` follows the pointer (active output), `N` = N-th screen.
- **Slide**: linear, `Frames` frames at 10 ms each (default 17, 0 = instant). Toggles during an animation are ignored.
- **Toggle**: hidden -> open; open and active -> retract; open but inactive with `KeepOpen`: focus it if `ToggleToFocus`, or (when not `ShowOnAllDesktops` and on another desktop) bring it to the current desktop and focus; otherwise retract.
- **Focus loss**: with `KeepOpen` off the window retracts when it loses focus (not when focus moves to one of its own windows); the focus change caused by opening is ignored.
- **KeepAbove**: topmost when `!KeepOpen || KeepAbove`. Hidden from taskbar and Alt+Tab.
- **Width/height actions**: +/-10 within 10..=100 (`Alt+Shift+Right/Left` width, `Alt+Shift+Down/Up` height). Fullscreen toggle (`Ctrl+Shift+F11`), `RememberFullscreen`.
- **Global hotkey** `F12`, tray icon (`ShowSystrayIcon`), `OpenAfterStart`, starting a second instance toggles the first.

## Design

- **Pure core** (Linux-testable): `winuake-config::window::WindowSettings` (kcfg keys/defaults/ranges) and `winuake-core::drop_down` — `Rect`/`Monitor`, `window_rect`, `select_screen`, linear slide frames, `decide_toggle`, and a `DropDownController` state machine (`Hidden -> Opening -> Open -> Closing`) that turns events (toggle, animation tick, focus change, adjust width/height, fullscreen) into commands (position/clip, show, hide, activate, topmost, move to current desktop, timers). All Yakuake decisions live here and are unit-tested.
- **Win32 glue** (`winuake::drop_down`): executes commands. Borderless popup style, `WS_EX_TOOLWINDOW`, `SetWindowPos` + `SetWindowRgn` clip for the slide (window sits partly above the work area; the clip keeps it from showing on a monitor above), `SetTimer` animation, `RegisterHotKey` (thread hotkey, seen by the existing message hook), `GetForegroundWindow` polling for focus, `IVirtualDesktopManager` (documented) for "show on current desktop", monitor enumeration with work areas.
- **Tray + instance**: hidden message-only window owns the notification icon and receives "toggle" posted by a second instance.
- **Deviation**: Yakuake's `UseWMAssist` (KWin effect) is N/A; `ShowOnAllDesktops` is emulated by moving the window to the current virtual desktop on open (no public pin API).

## Chunks

- **3a** Pure core + `WindowSettings` + tests.
- **3b** Window styling, monitors/geometry, slide animation, topmost, desktop handling, global hotkey, width/height/fullscreen shortcuts, hidden start.
- **3c** Tray icon + menu, single-instance toggle, focus-loss retract, `OpenAfterStart`, parity matrix update.

## Verification

Unit tests for all decisions on Linux. On the VM: hotkey toggles (injected F12), slide frames captured by repeated screenshots, geometry at several Width/Height/Position values vs. the formula, focus-loss retract (`KeepOpen` off), tray menu, second instance toggles the first.
