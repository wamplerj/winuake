# Phase 4 — Sessions, tabs, splits

Parent: `winuake-port.md`. Ports Yakuake `sessionstack.cpp`, `session.cpp`, `splitter.cpp`, the tab logic of `tabbar.cpp`, `titlebar.cpp`, and the action table in `mainwindow.cpp` (`setupActions`, `handleContextDependentAction`, `handleSwitchToAction`, `handleLastTabClosed`, `queryClose`).

## Behavior to port (from Yakuake)

- **Sessions**: ids from a global counter starting at 0. Four kinds: single, two horizontal, two vertical, quad (2x2 grid). A new session starts in the working directory of the active terminal. Session title = title of its active terminal. A session can be locked against closing (`closable`); closing a locked session or terminal asks first.
- **Terminals**: ids from a global counter. "Next/previous terminal" walks terminals in id order and wraps (not visual order). Closing the active terminal focuses the previous one (by id) first. Closing the last terminal of a session removes the session.
- **Split tree**: binary splitter tree. Splitting a terminal replaces it with a splitter of the old and new terminal, new one second, 50/50, and the new terminal becomes active. `split-auto` splits left/right when the pane is wider than tall, else top/bottom. New terminal inherits the working directory.
- **Grow**: `tryGrowTerminal(id, direction, pixels)` walks up to the nearest splitter of matching orientation where the terminal can move toward the neighbor, shifts the divider, returns the actual size change or -1.
- **Tabs**: tab order is independent of session ids. New tab is selected. Removing the selected tab selects the last tab; removing another keeps the selection; removing the final tab raises `lastTabClosed`, which retracts the window unless `KeepOpenAfterLastSessionCloses`. Next/previous wrap. Move left/right swap neighbors. Default titles "Shell", "Shell No. 2", ... first free. Rename (interactive) pins the title against dynamic updates; an empty rename unpins it. `DynamicTabTitles` feeds terminal titles into unpinned tabs.
- **Quit**: with `ConfirmQuit` and more than one session, or any locked session, ask "Really Quit?".
- **Actions**: every `setupActions` entry with its default shortcut, including `switch-to-session-1..10` (Alt+1..Alt+9, Alt+0 for the tenth, indexed by tab position).

## Design

- **Pure core** (`winuake-core`, Linux-testable):
  - `ids` — `SessionId`, `TerminalId` (also used by `winuake-term`).
  - `split_tree` — tree, layout to pixel rects, divider ratios, grow, close/collapse.
  - `session` — `Session`: tree, active terminal, id-ordered focus walk, title, closable.
  - `session_stack` — sessions, active session, id allocation, queries, close policy.
  - `tab_list` — `TabList`: order, selection, titles, pinned titles, move, wrap.
  - `workspace` — facade combining stack and tabs. Every operation returns `Vec<WorkspaceEffect>` (session added/removed/raised, terminal added/removed/focused, tab title changed, retract window) that the app applies, like Yakuake's signals. App owns the real terminals; core hands out ids.
  - `actions` + `shortcut` — `Action` enum (names as in Yakuake), default chords, chord parsing/matching, `ShortcutMap`.
- **App** (`winuake`, Windows):
  - One `SessionView` per session: a `Canvas` of `TerminalView` panes positioned from the core layout, plus draggable divider rectangles.
  - Workspace owner object executes effects: starts a ConPTY session and `TerminalView` per new terminal id, disposes on removal.
  - Tab bar and title bar built from XAML primitives (skins arrive in Phase 6, so the look here is a plain placeholder with the same structure: tabs, `+` button, close button, title text, keep-open toggle, quit button).
  - Keyboard: key-down looks chords up in `ShortcutMap` before terminal encoding.
- **Deviations**: Konsole-profile actions (`manage-profiles`, `edit-profile`) exist as actions but stay inert until Phase 5. Activity/silence/keyboard-lock toggles keep state in core but do nothing visible until Phase 7. `Ctrl+(`, `Ctrl+)`, `Ctrl+*` assume US layout positions (Shift+9, Shift+0, Shift+8) since Windows reports virtual keys, not symbols.

## Chunks

- **4a** Core: ids, split tree, session, stack, tab list, workspace, actions/shortcuts + BDD tests.
- **4b** App: multiple terminals and splits (session view, dividers, workspace owner), all session/terminal shortcuts, replace `WindowShortcut` with action lookup.
- **4c** Tab bar, title bar, context menu, rename, drag reorder, middle-click close, `+`, confirm dialogs, `ConfirmQuit`, `KeepOpenAfterLastSessionCloses`, parity update.

## Verification

Unit tests for all state transitions on Linux. On the VM: every shortcut driven by injected keys, screenshots of each session kind and nested splits, divider drag by pointer, grow shortcuts, tab operations by mouse, closing the last tab retracts, quit confirmation.
