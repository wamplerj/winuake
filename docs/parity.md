# Feature parity matrix

Status: `todo` / `partial` / `done` / `n/a` (with reason). Baseline: Yakuake `21c6210`.

## Settings (`config/yakuake.kcfg`)

| Group | Key | Status | Phase | Notes |
|---|---|---|---|---|
| Window | Screen | todo | | |
| Window | Width | todo | | |
| Window | Height | todo | | |
| Window | Position | todo | | |
| Window | ShowOnAllDesktops | todo | | |
| Window | KeepOpen | todo | | |
| Window | KeepOpenAfterLastSessionCloses | todo | | |
| Window | ToggleToFocus | todo | | |
| Window | KeepAbove | todo | | |
| Window | ShowTitleBar | todo | | |
| Window | ShowTabBar | todo | | |
| Window | DynamicTabTitles | todo | | |
| Window | ShowSystrayIcon | todo | | |
| Behavior | FocusFollowsMouse | todo | | |
| Behavior | OpenAfterStart | todo | | |
| Behavior | RememberFullscreen | todo | | |
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
| Animation | UseWMAssist | todo | | |
| Animation | Frames | todo | | |
| AutoOpen | PollMouse | todo | | |
| AutoOpen | PollInterval | todo | | |
| Dialogs | FirstRun | todo | | |
| Dialogs | ConfirmQuit | todo | | |

## Actions (`mainwindow.cpp`)

| Action | Status | Phase | Notes |
|---|---|---|---|
| `close-active-terminal` | todo | | |
| `close-session` | todo | | |
| `decrease-window-height` | todo | | |
| `decrease-window-width` | todo | | |
| `edit-profile` | todo | | |
| `grow-terminal-bottom` | todo | | |
| `grow-terminal-left` | todo | | |
| `grow-terminal-right` | todo | | |
| `grow-terminal-top` | todo | | |
| `increase-window-height` | todo | | |
| `increase-window-width` | todo | | |
| `keep-open` | todo | | |
| `manage-profiles` | todo | | |
| `move-session-left` | todo | | |
| `move-session-right` | todo | | |
| `new-session` | todo | | |
| `new-session-quad` | todo | | |
| `new-session-two-horizontal` | todo | | |
| `new-session-two-vertical` | todo | | |
| `next-session` | todo | | |
| `next-terminal` | todo | | |
| `previous-session` | todo | | |
| `previous-terminal` | todo | | |
| `rename-session` | todo | | |
| `split-auto` | todo | | |
| `split-left-right` | todo | | |
| `split-top-bottom` | todo | | |
| `switch-to-session-%1` | todo | | |
| `toggle-session-keyboard-input` | todo | | |
| `toggle-session-monitor-activity` | todo | | |
| `toggle-session-monitor-silence` | todo | | |
| `toggle-session-prevent-closing` | todo | | |
| `toggle-titlebar` | todo | | |
| `toggle-window-state` | todo | | |
| `view-full-screen` | todo | | |

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
