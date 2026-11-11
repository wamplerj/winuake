# Phase 4 output — Sessions, tabs, splits

## Done

- **4a core** (`winuake-core`, Linux-tested, 101 new tests): `ids`, `sessions::{split_tree, session, session_stack, tab_list, workspace}`, `actions` (all 45 actions, Yakuake default shortcuts), `shortcut` (chords, parsing, virtual-key conversion). `Workspace` returns `WorkspaceEffect`s instead of Qt signals. `winuake-term` now reuses core's `TerminalId`; its `WindowShortcut` is gone (the shortcut table replaces it).
- **4b/4c app** (`winuake`): `workspace_host` applies effects (one ConPTY shell + `TerminalView` per terminal, margin-positioned panes in a Grid per session, draggable dividers, every action, quit/close confirmation via `MessageBoxW`). `chrome` builds the title bar and tab bar from XAML markup (`XamlReader`): tabs, `+`, close ×, middle-click close, right-click menu (Win32 popup), double-click inline rename, drag reorder, title text, keep-open pin, config menu, quit.
- New bindings: `XamlReader`, `FindName`, `Thickness`, `Visibility`, `TextBox`, `GotFocus/LostFocus`, `MessageBoxW`, menu check flags, tick/double-click time.

## Verified on the Win11 VM

New session (shortcut and `+`), tab switching by click, double-click rename (typed text goes to the box, not the shell), tab context menu, close × and shell `exit` closing sessions, last session closing retracts the window and the next open starts a fresh session, quit confirmation dialog (Cancel keeps running), `Ctrl+(` and `Ctrl+*` splits, divider drag, grow shortcuts, move-session-left.

## Not verified / known gaps

- Locked-session close dialog, tab drag reorder by mouse, `Ctrl+Shift+9/0` variants beyond those listed, `Ctrl+)`: Windows appeared not to deliver Ctrl+Shift+0 (probably reserved); rebindable in phase 5.
- New terminals start in `%USERPROFILE%`, not the active terminal's directory (needs OSC 9;9 from the shell; later).
- No divider cursor, no tab-drag drop indicator, no tab-bar skin (phase 6). Opening with no sessions starts one (deviation from Yakuake, which shows an empty window).
- Keep-open and settings changes are live only; persistence is phase 5.
- New views sometimes need a click before keys work when opened by `open_after_start`; focus is now retried after layout (fix not re-verified).
