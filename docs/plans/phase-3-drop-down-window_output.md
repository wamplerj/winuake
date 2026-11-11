# Phase 3 — output

Winuake is now a drop-down terminal: hidden at start, summoned from the top of the screen with a slide, retracted again, with Yakuake's window rules ported.

## Done

- **Pure core** (`winuake-core::drop_down`, 54 tests): geometry with Yakuake's integer formula, screen selection, linear 10 ms slide frames, the toggle decision table, and a `DropDownController` state machine (events in, window commands out). `winuake-config::window::WindowSettings` carries the kcfg keys, defaults and ranges (8 tests), plus a tested `key=value` override parser for development.
- **Win32 host** (`winuake::drop_down`): borderless popup style, tool window (no taskbar button / Alt+Tab), square corners, placement on the monitor work area, slide implemented by moving the full-size window partly above the work area and clipping the overhang with a window region, topmost rules, move-to-current-virtual-desktop (`IVirtualDesktopManager`), foreground activation, focus-loss polling, shared `WH_GETMESSAGE` hook (`message_hook`).
- **Toggle sources**: global hotkey, tray icon (left click toggles, right-click menu: Open/Retract, Quit), and starting a second instance (single-instance lock + message to a hidden message-only window).
- **Shortcuts**: Alt+Shift+Right/Left/Down/Up resize by 10%, Ctrl+Shift+F11 full screen, Ctrl+Shift+Q quit.
- **Quit** ends the app and kills the shell.
- Parity matrix updated (`docs/parity.md`).

## Verification (Win11 VM, display 2560x1440 at 150%)

- Placement matches the formula in physical pixels: 90% x 50% centered -> `left=128 top=0 width=2304 height=684` on a 1368-px work area; Alt+Shift shortcuts gave 100% (x=0, 2560 wide), height 60% = 820, width 90% -> 2304 at x=128, 80% -> 2048 at x=256; fullscreen -> 0,0 2560x1440.
- Slide captured mid-animation with `frames=50`: lower part of the window visible and clipped at the top edge; settled frame matches placement.
- Retract on focus loss (`keep_open=false`): clicking another window hides Winuake; verified after fixing the controller bug below.
- Second instance toggles the first and exits (one process left). Tray icon visible; menu shows both entries; Open/Retract hides an open window.
- `open_after_start=true` opens at startup. Quit (Ctrl+Shift+Q) removes the process and its shell.
- Unit/lint: 125 term, 54 core, 12 render, 8 config tests; `fmt` and `clippy -D warnings` clean on Linux and Windows targets.

## Findings and fixes

1. **Not DPI-aware.** The placement log showed `width=1536` on a 2560-px screen: Windows was virtualizing coordinates and bitmap-scaling the window. Phase 2's note that the VM runs at 100% was wrong (it runs at 150%). `main` now sets per-monitor-v2 awareness before any window exists. The 150% path (font rescale, composition scale) is therefore exercised for the first time and looks right (178 columns x 26 rows in a 2304x684 window).
2. **F12 is refused by Windows** (reserved for debuggers; `ERROR_HOTKEY_ALREADY_REGISTERED`), and Win+` was taken by Windows Terminal. The host tries F12, Win+`, Ctrl+`, Ctrl+Alt+` and logs which one it got. The first-run dialog (Phase 5) will let users choose.
3. **Controller bug found on the VM**: focus events during the opening animation were ignored, so the window was never marked as having been focused and never retracted. Fixed, with three regression tests.
4. **Process exit hangs.** After `Application.Exit()` (and even after skipping the Windows App SDK shutdown) `ExitProcess` hung in its DLL-detach phase: a dead process that kept its memory, handles, hotkeys, log file and single-instance lock. Cleanup is now explicit (tray removed, session dropped) followed by `TerminateProcess`, which exits cleanly.
5. **Windows Terminal owns Win+`** and the VM had other tools running; hotkey registration must always be allowed to fail gracefully.
6. Dev-loop gotchas fixed in `scripts/`: `cmd`'s `if ... & next` makes the rest of the line conditional (tar extraction was skipped); a hung old copy locks the exe (rename it before building), holds the log file (fresh log per run) and keeps its scheduled task "running" (unique task name per run). `WINUAKE_WINDOW_SETTINGS` overrides settings for tests; `WINUAKE_ALLOW_MULTIPLE` skips the instance check.

## Not done / follow-ups

- **Multi-monitor not exercised**: the VM has one display. Screen selection and monitor ordering are unit-tested only. Pointer-based edge opening (`AutoOpen`) is Phase 7.
- **Virtual desktops not exercised**: `ShowOnAllDesktops=off` + `MoveToCurrentDesktop` is implemented but untested (one desktop on the VM).
- Settings are not persisted yet (width/height changes are logged only); the hotkey is not configurable; the tray icon is the stock application icon. Phases 5 and 9.
- `ConfirmQuit` and `KeepOpenAfterLastSessionCloses` need sessions (Phase 4). The title bar's keep-open button comes with the title bar.
- Three hung copies from before the exit fix remain on the VM until it is rebooted; they hold the F12 alternatives, so hotkey tests there use the second-instance toggle.
