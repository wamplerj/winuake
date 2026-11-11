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
| Window | KeepOpenAfterLastSessionCloses | done | 5 | |
| Window | ToggleToFocus | done | 3 |  |
| Window | KeepAbove | done | 3 | Topmost when `!KeepOpen \|\| KeepAbove` |
| Window | ShowTitleBar | done | 5 | Toggle with Ctrl+Shift+M; saved |
| Window | ShowTabBar | done | 5 | Settings window |
| Window | DynamicTabTitles | done | 5 | Applies live |
| Window | ShowSystrayIcon | done | 3 | Stock icon until Phase 9 assets; left click toggles, right click menu |
| Behavior | FocusFollowsMouse | done | 7 | |
| Behavior | OpenAfterStart | done | 3 |  |
| Behavior | RememberFullscreen | done | 3 |  |
| Appearance | Skin | done | 6 | Skins page of the settings window; switches live |
| Appearance | SkinInstalledWithKns | n/a | 6 | KNewStuff does not exist on Windows; skins install from an archive file instead |
| Appearance | Translucency | done | 7 | Acrylic backdrop; terminals draw their default background with the chosen opacity |
| Appearance | BackgroundColor | done | 7 | |
| Appearance | BackgroundColorOpacity | done | 7 | |
| Appearance | TerminalHighlightOnManualActivation | done | 7 | Highlight color is Breeze blue, not the desktop palette |
| Appearance | HideSkinBorders | done | 6 | |
| Appearance | TerminalHighlightOpacity | done | 7 | |
| Appearance | TerminalHighlightDuration | done | 7 | |
| Appearance | KeyboardInputBlockIndicatorColor | done | 7 | |
| Appearance | KeyboardInputBlockIndicatorOpacity | done | 7 | |
| Appearance | KeyboardInputBlockIndicatorDuration | done | 7 | |
| Animation | UseWMAssist | n/a |  | KWin effect; the Windows slide uses a clipped window region |
| Animation | Frames | done | 3 | 10 ms per frame, linear, 0 = instant |
| AutoOpen | PollMouse | done | 7 | Pointer on the top row of the work area within the window span |
| AutoOpen | PollInterval | done | 7 | |
| Dialogs | FirstRun | done | 5 | Asks for the global hotkey once; saved as `toggle_hotkey` |
| Dialogs | ConfirmQuit | done | 5 | |

## Actions (`mainwindow.cpp`)

| Action | Status | Phase | Notes |
|---|---|---|---|
| `close-active-terminal` | done | 4 | Ctrl+Shift+R; asks first if the session is locked |
| `close-session` | done | 4 | Ctrl+Shift+W, tab ×, middle click, menu |
| `decrease-window-height` | done | 3 | Alt+Shift+Up |
| `decrease-window-width` | done | 3 | Alt+Shift+Left |
| `edit-profile` | done | 5 | Opens the Profile page of the settings window |
| `grow-terminal-bottom` | done | 4 | Ctrl+Alt+Down, 10 DIPs |
| `grow-terminal-left` | done | 4 | Ctrl+Alt+Left, 10 DIPs |
| `grow-terminal-right` | done | 4 | Ctrl+Alt+Right, 10 DIPs |
| `grow-terminal-top` | done | 4 | Ctrl+Alt+Up, 10 DIPs |
| `increase-window-height` | done | 3 | Alt+Shift+Down |
| `increase-window-width` | done | 3 | Alt+Shift+Right |
| `keep-open` | done | 4 | Title bar pin and menu; saved |
| `manage-profiles` | done | 5 | Opens the Profile page of the settings window |
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
| `toggle-session-keyboard-input` | done | 7 | Blocks input and flashes the indicator |
| `toggle-session-monitor-activity` | done | 7 | Tray balloon (toast); needs the tray icon enabled |
| `toggle-session-monitor-silence` | done | 7 | Tray balloon after 10 s of silence |
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

| Key | Status | Phase | Notes |
|---|---|---|---|
| `[Description]` Skin, Author, Email, Icon | done | 6 | Shown in the skin list; an `Icon` naming a missing PNG falls back to the SVG of the same name |
| `[Border]` red, green, blue, width | done | 6 | Left, right and bottom borders around the terminals; off with `HideSkinBorders` |
| `[Text]` x, y, red, green, blue, text, bold, centered | done | 6 | Title is `<terminal title> - <text>`; baseline of `y` approximated (about 12 px above) |
| `[Background]` back_image, left_corner, right_corner (title) | done | 6 | Middle stretched instead of tiled |
| `[FocusButton]` `[ConfigButton]` `[QuitButton]` x, y, anchor, up/over/down images | done | 6 | Right anchor measures the button's left edge from the right, like Qt |
| `[Tabs]` x, y, colors, separator, selected/unselected background and corners, selected_text_bold | done | 6 | Tab width follows the text (Qt adds 10 px) |
| `[Tabs]` prevent_closing_image and its x, y | done | 6 | Drawn on locked tabs |
| `[Tabs]` compact | partial | 6 | Positions are adjusted as in `skin.cpp`; the tab bar is not inset to the compact left/right (no bundled skin uses it) |
| `[Background]` back_image, left_corner, right_corner (tabs) | done | 6 | |
| `[PlusButton]` x, y, images, at_end_of_tabs | done | 6 | |
| `[MinusButton]` x, y, images | done | 6 | Closes the selected tab |
| SVG images (default skin) | done | 6 | Rendered at 3x into `%LOCALAPPDATA%\Winuake\skin-cache` |
| Window masks for rounded skin corners | n/a | 6 | Not ported; corner images are drawn over the window background |
| Install from `.tar`, `.tar.gz`, `.zip`; remove | done | 6 | Replaces KNewStuff; bzip2 archives are not supported |
| Title bar drag changes height | done | 6 | |
