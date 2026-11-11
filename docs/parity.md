# Feature parity matrix

Status: `todo` / `partial` / `done` / `n/a` (with reason). Baseline: Yakuake `21c6210`.

## Settings (`config/yakuake.kcfg`)

| Group | Key | Status | Phase | Notes |
|---|---|---|---|---|
| Window | Screen | done | 3 | `Screen` 0 follows the pointer; N = Nth monitor (primary first, then left to right) |
| Window | Width | done | 3 | Percent of the work area; Alt+Shift+Right/Left adjust by 10 |
| Window | Height | done | 3 | Percent of the work area; Alt+Shift+Down/Up adjust by 10 |
| Window | Position | done | 3 |  |
| Window | ShowOnAllDesktops | partial | 3 | No public pin API: the window is moved to the current virtual desktop when opened instead |
| Window | KeepOpen | done | 3 | Retract on focus loss when off; the title bar toggle button comes with Phase 4/6 |
| Window | KeepOpenAfterLastSessionCloses | partial | 4 | Logic done; default only until settings persist (phase 5) |
| Window | ToggleToFocus | done | 3 |  |
| Window | KeepAbove | done | 3 | Topmost when `!KeepOpen \|\| KeepAbove` |
| Window | ShowTitleBar | todo | | |
| Window | ShowTabBar | todo | | |
| Window | DynamicTabTitles | partial | 4 | Logic done and tested; default off until settings persist (phase 5) |
| Window | ShowSystrayIcon | done | 3 | Stock icon until Phase 9 assets; left click toggles, right click menu |
| Behavior | FocusFollowsMouse | todo | | |
| Behavior | OpenAfterStart | done | 3 |  |
| Behavior | RememberFullscreen | done | 3 |  |
| Appearance | Skin | todo | | |
| Appearance | SkinInstalledWithKns | todo | | |
| Appearance | Translucency | todo | | |
| Appearance | BackgroundColor | todo | | |
| Appearance | BackgroundColorOpacity | todo | | |
| Appearance | TerminalHighlightOnManualActivation | todo | | |
| Appearance | HideSkinBorders | todo | | |
| Appearance | TerminalHighlightOpacity | todo | | |
| Appearance | TerminalHighlightDuration | todo | | |
| Appearance | KeyboardInputBlockIndicatorColor | todo | | |
| Appearance | KeyboardInputBlockIndicatorOpacity | todo | | |
| Appearance | KeyboardInputBlockIndicatorDuration | todo | | |
| Animation | UseWMAssist | n/a |  | KWin effect; the Windows slide uses a clipped window region |
| Animation | Frames | done | 3 | 10 ms per frame, linear, 0 = instant |
| AutoOpen | PollMouse | todo | | |
| AutoOpen | PollInterval | todo | | |
| Dialogs | FirstRun | todo | | |
| Dialogs | ConfirmQuit | partial | 4 | Dialog works; default only until settings persist (phase 5) |

## Actions (`mainwindow.cpp`)

| Action | Status | Phase | Notes |
|---|---|---|---|
| `close-active-terminal` | done | 4 | Ctrl+Shift+R; asks first if the session is locked |
| `close-session` | done | 4 | Ctrl+Shift+W, tab ×, middle click, menu |
| `decrease-window-height` | done | 3 | Alt+Shift+Up |
| `decrease-window-width` | done | 3 | Alt+Shift+Left |
| `edit-profile` | partial | 5 | Inert until profiles exist (phase 5) |
| `grow-terminal-bottom` | done | 4 | Ctrl+Alt+Down, 10 DIPs |
| `grow-terminal-left` | done | 4 | Ctrl+Alt+Left, 10 DIPs |
| `grow-terminal-right` | done | 4 | Ctrl+Alt+Right, 10 DIPs |
| `grow-terminal-top` | done | 4 | Ctrl+Alt+Up, 10 DIPs |
| `increase-window-height` | done | 3 | Alt+Shift+Down |
| `increase-window-width` | done | 3 | Alt+Shift+Right |
| `keep-open` | done | 4 | Title bar pin and menu; live only until settings persist (phase 5) |
| `manage-profiles` | partial | 5 | Inert until profiles exist (phase 5) |
| `move-session-left` | done | 4 | Ctrl+Shift+Left, tab menu |
| `move-session-right` | done | 4 | Ctrl+Shift+Right, tab menu |
| `new-session` | done | 4 | Ctrl+Shift+T, tab bar + |
| `new-session-quad` | done | 4 | No shortcut by default |
| `new-session-two-horizontal` | done | 4 | No shortcut by default |
| `new-session-two-vertical` | done | 4 | No shortcut by default |
| `next-session` | done | 4 | Shift+Right |
| `next-terminal` | done | 4 | Ctrl+Tab, id order like Yakuake |
| `previous-session` | done | 4 | Shift+Left |
| `previous-terminal` | done | 4 | Ctrl+Shift+Tab |
| `rename-session` | done | 4 | Ctrl+Alt+S, double click; inline edit |
| `split-auto` | done | 4 | Ctrl+* |
| `split-left-right` | done | 4 | Ctrl+( |
| `split-top-bottom` | done | 4 | Ctrl+); Windows may reserve Ctrl+Shift+0, so it can fail to arrive (rebind in phase 5) |
| `switch-to-session-%1` | done | 4 | Alt+1..9, Alt+0; by tab position |
| `toggle-session-keyboard-input` | partial | 4 | Input is blocked; the overlay indicator arrives in phase 7 |
| `toggle-session-monitor-activity` | partial | 4 | State kept; notifications arrive in phase 7 |
| `toggle-session-monitor-silence` | partial | 4 | State kept; notifications arrive in phase 7 |
| `toggle-session-prevent-closing` | done | 4 | Tab menu; lock icon on the tab |
| `toggle-titlebar` | done | 4 | Ctrl+Shift+M |
| `toggle-window-state` | done | 3 | Global hotkey (F12 if Windows allows it, else Win+`, Ctrl+`, Ctrl+Alt+`), tray icon, starting a second instance |
| `view-full-screen` | done | 3 | Ctrl+Shift+F11 |

## Scripting API (D-Bus `Q_SCRIPTABLE`)

| Method | Status | Phase | Notes |
|---|---|---|---|
| `activeSessionId` | todo | 8 | |
| `activeTerminalId` | todo | 8 | |
| `addSession` | todo | 8 | |
| `addSessionQuad` | todo | 8 | |
| `addSessionTwoHorizontal` | todo | 8 | |
| `addSessionTwoVertical` | todo | 8 | |
| `hasTerminalsWithKeyboardInputDisabled` | todo | 8 | |
| `hasTerminalsWithKeyboardInputEnabled` | todo | 8 | |
| `hasTerminalsWithMonitorActivityDisabled` | todo | 8 | |
| `hasTerminalsWithMonitorActivityEnabled` | todo | 8 | |
| `hasTerminalsWithMonitorSilenceDisabled` | todo | 8 | |
| `hasTerminalsWithMonitorSilenceEnabled` | todo | 8 | |
| `hasUnclosableSessions` | todo | 8 | |
| `isSessionClosable` | todo | 8 | |
| `isSessionKeyboardInputEnabled` | todo | 8 | |
| `isSessionMonitorActivityEnabled` | todo | 8 | |
| `isSessionMonitorSilenceEnabled` | todo | 8 | |
| `isTerminalKeyboardInputEnabled` | todo | 8 | |
| `isTerminalMonitorActivityEnabled` | todo | 8 | |
| `isTerminalMonitorSilenceEnabled` | todo | 8 | |
| `raiseSession` | todo | 8 | |
| `removeSession` | todo | 8 | |
| `removeTerminal` | todo | 8 | |
| `runCommand` | todo | 8 | |
| `runCommandInTerminal` | todo | 8 | |
| `sessionAtTab` | todo | 8 | |
| `sessionIdForTerminalId` | todo | 8 | |
| `sessionIdList` | todo | 8 | |
| `setSessionClosable` | todo | 8 | |
| `setSessionKeyboardInputEnabled` | todo | 8 | |
| `setSessionMonitorActivityEnabled` | todo | 8 | |
| `setSessionMonitorSilenceEnabled` | todo | 8 | |
| `setTabTitle` | todo | 8 | |
| `setTerminalKeyboardInputEnabled` | todo | 8 | |
| `setTerminalMonitorActivityEnabled` | todo | 8 | |
| `setTerminalMonitorSilenceEnabled` | todo | 8 | |
| `splitSessionAuto` | todo | 8 | |
| `splitSessionLeftRight` | todo | 8 | |
| `splitSessionTopBottom` | todo | 8 | |
| `splitTerminalLeftRight` | todo | 8 | |
| `splitTerminalTopBottom` | todo | 8 | |
| `tabTitle` | todo | 8 | |
| `terminalIdList` | todo | 8 | |
| `terminalIdsForSessionId` | todo | 8 | |
| `toggleWindowState` | todo | 8 | |
| `tryGrowTerminalBottom` | todo | 8 | |
| `tryGrowTerminalLeft` | todo | 8 | |
| `tryGrowTerminalRight` | todo | 8 | |
| `tryGrowTerminalTop` | todo | 8 | |

## Skin keys (`skin.cpp`, `data/skins/*`)

Populate in Phase 6.
