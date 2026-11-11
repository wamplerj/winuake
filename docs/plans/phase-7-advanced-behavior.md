# Phase 7 — Advanced behavior

Parent: `winuake-port.md`. Ports `visualeventoverlay.cpp`, the monitor handlers of `mainwindow.cpp` (`handleTerminalActivity`, `handleTerminalSilence`), `pollMouse`, `Terminal::eventFilter` (focus follows mouse, input lock), `updateUseTranslucency`, `outputorderwatcher.cpp`.

## Behavior to port

- **Monitor for activity / silence** (per session, from the tab menu or shortcuts): output in a monitored terminal raises one "Activity detected in monitored terminal in session "<tab>"." notification until the terminal is looked at again; silence after output (10 s) raises "Silence detected ...". Yakuake uses KNotification; Windows gets a tray balloon (shown as a toast), only when that terminal is not the focused one in an open window.
- **Keyboard input lock**: typing without modifiers into a locked terminal flashes the blocked-input indicator over that terminal (`KeyboardInputBlockIndicatorColor/Opacity/Duration`).
- **Terminal highlight**: with `TerminalHighlightOnManualActivation`, activating a terminal by hand (mouse, Tab) in a split session flashes a highlight over it (`TerminalHighlightOpacity/Duration`).
- **Focus follows mouse**: pointer movement over a terminal focuses it.
- **Auto open**: `PollMouse` checks the pointer every `PollInterval` ms while the window is hidden; pointer at the top edge of the screen (y = 0) within the window's horizontal span opens it.
- **Translucency**: `Translucency` with `BackgroundColor` and `BackgroundColorOpacity` makes the window background see-through (Acrylic backdrop plus an alpha terminal background).
- **Display changes**: monitor hot-plug or reordering re-places the window (`WM_DISPLAYCHANGE`, replaces `outputorderwatcher`).

## Design

- Pure in `winuake-core`: `notifications` (message text, whether to notify), `auto_open` (edge hit test from window geometry), flash timeline (visible until deadline).
- App: poll monitor notifications on each terminal's event thread and forward to the UI thread; overlay rectangles added to the session grid for flashes and removed by one shared timer; tray balloon notification; pointer polling timer; `WM_DISPLAYCHANGE` hook in the existing message hook; backdrop via `Window.SystemBackdrop`.

## Chunks

- **7a** Monitor toggles reach the terminals, activity/silence notifications, input-lock indicator flash.
- **7b** Terminal highlight, focus follows mouse, auto open, display-change replace.
- **7c** Translucency and background color.

## Verification

Unit tests for the pure parts. On the VM: monitored session notifies when output arrives and after silence; blocked typing flashes red; highlight on click in a split; focus follows the pointer; pointer at the top edge opens the hidden window; translucent window shows the desktop through.
