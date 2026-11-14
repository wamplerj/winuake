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
| `edit-profile` | done | 10 | Opens the settings tab on the Profile page |
| `grow-terminal-bottom` | done | 4 | Ctrl+Alt+Down, 10 DIPs |
| `grow-terminal-left` | done | 4 | Ctrl+Alt+Left, 10 DIPs |
| `grow-terminal-right` | done | 4 | Ctrl+Alt+Right, 10 DIPs |
| `grow-terminal-top` | done | 4 | Ctrl+Alt+Up, 10 DIPs |
| `increase-window-height` | done | 3 | Alt+Shift+Down |
| `increase-window-width` | done | 3 | Alt+Shift+Right |
| `keep-open` | done | 4 | Title bar pin and menu; saved |
| `clear-scrollback` | added | 11 | Winuake addition (Konsole's menu item); no default shortcut |
| `clear-scrollback-and-reset` | added | 11 | Winuake addition: Ctrl+Shift+K |
| `open-settings` | added | 10 | Winuake addition: Ctrl+Shift+, opens the settings tab; rebindable on the Shortcuts page |
| `manage-profiles` | done | 10 | Opens the settings tab on the Profile page: add, duplicate, delete, default, add detected shells (Windows PowerShell, PowerShell 7, Command Prompt, Git Bash, WSL) |
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

## Terminal context menu (Konsole's menu as Yakuake shows it)

Right click on a terminal opens it. Middle click pastes. A program that asked for mouse events gets right clicks instead; Shift+right click still opens the menu. Copy and paste moved from right click to Ctrl+Shift+C / Ctrl+Shift+V and middle click.

| Item | Status | Phase | Notes |
|---|---|---|---|
| Copy (only with a selection), Paste | done | 11 | |
| Split View: left/right, top/bottom, automatic, close terminal | done | 11 | Shows the current shortcuts |
| Read-Only | done | 11 | The terminal's keyboard-input switch; also blocks paste and mouse reports |
| Allow Mouse Tracking | done | 11 | Per terminal; not tried against a mouse-reporting program on the VM yet |
| Edit Current Profile... | done | 11 | Opens the settings tab on the Profile page with this terminal's profile selected |
| Close Session | done | 11 | Closes the clicked terminal |
| Set Encoding | n/a | | ConPTY is always UTF-8 |
| Terminal's current folder | partial | 11 | Windows cannot ask a console process where it is, so the shell must report it (OSC 7, or OSC 9;9 as Windows Terminal does; oh-my-posh and starship already do). Without a report the profile's starting folder is used, then the user folder. WSL shells map `/home/...` into `\\wsl.localhost\<distro>`; Git Bash's `/c/...` maps to `C:\...` |
| Switch Profile | done | 11 | Applies the profile's font, size, colors, cursor and scrollback to the running terminal; the shell and screen stay; later splits use the new profile |
| Scrollback: Clear Scrollback, Clear Scrollback and Reset, Adjust Scrollback... | done | 11 | Reset is a full terminal reset (RIS), so the prompt returns at the shell's next output; Adjust opens the Profile page |
| Open File Manager | done | 11 | File Explorer in the terminal's folder (see below for how the folder is known) |
| Open Folder With | done | 11 | File Explorer, Visual Studio Code (when `code.cmd` is on the PATH), New Winuake Session Here (default profile), Copy Path |
| Find... | planned | 11 | |

## Scripting API (D-Bus `Q_SCRIPTABLE`)

| Method | Status | Phase | Notes |
|---|---|---|---|
| `activeSessionId` | done | 8 | |
| `activeTerminalId` | done | 8 | |
| `addSession` | done | 8 | |
| `addSessionQuad` | done | 8 | |
| `addSessionTwoHorizontal` | done | 8 | |
| `addSessionTwoVertical` | done | 8 | |
| `hasTerminalsWithKeyboardInputDisabled` | done | 8 | |
| `hasTerminalsWithKeyboardInputEnabled` | done | 8 | |
| `hasTerminalsWithMonitorActivityDisabled` | done | 8 | |
| `hasTerminalsWithMonitorActivityEnabled` | done | 8 | |
| `hasTerminalsWithMonitorSilenceDisabled` | done | 8 | |
| `hasTerminalsWithMonitorSilenceEnabled` | done | 8 | |
| `hasUnclosableSessions` | done | 8 | |
| `isSessionClosable` | done | 8 | |
| `isSessionKeyboardInputEnabled` | done | 8 | |
| `isSessionMonitorActivityEnabled` | done | 8 | |
| `isSessionMonitorSilenceEnabled` | done | 8 | |
| `isTerminalKeyboardInputEnabled` | done | 8 | |
| `isTerminalMonitorActivityEnabled` | done | 8 | |
| `isTerminalMonitorSilenceEnabled` | done | 8 | |
| `raiseSession` | done | 8 | |
| `removeSession` | done | 8 | A locked session is refused with an error instead of a confirmation dialog |
| `removeTerminal` | done | 8 | |
| `runCommand` | done | 8 | Text plus Enter (CR); the CLI joins words, so quote when using `runCommandInTerminal` |
| `runCommandInTerminal` | done | 8 | |
| `sessionAtTab` | done | 8 | |
| `sessionIdForTerminalId` | done | 8 | |
| `sessionIdList` | done | 8 | |
| `setSessionClosable` | done | 8 | |
| `setSessionKeyboardInputEnabled` | done | 8 | |
| `setSessionMonitorActivityEnabled` | done | 8 | |
| `setSessionMonitorSilenceEnabled` | done | 8 | |
| `setTabTitle` | done | 8 | |
| `setTerminalKeyboardInputEnabled` | done | 8 | |
| `setTerminalMonitorActivityEnabled` | done | 8 | |
| `setTerminalMonitorSilenceEnabled` | done | 8 | |
| `splitSessionAuto` | done | 8 | |
| `splitSessionLeftRight` | done | 8 | |
| `splitSessionTopBottom` | done | 8 | |
| `splitTerminalLeftRight` | done | 8 | |
| `splitTerminalTopBottom` | done | 8 | |
| `tabTitle` | done | 8 | |
| `terminalIdList` | done | 8 | |
| `terminalIdsForSessionId` | done | 8 | |
| `toggleWindowState` | done | 8 | |
| `tryGrowTerminalBottom` | done | 8 | |
| `tryGrowTerminalLeft` | done | 8 | |
| `tryGrowTerminalRight` | done | 8 | |
| `tryGrowTerminalTop` | done | 8 | |

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
| `[PlusButton]` x, y, images, at_end_of_tabs | done | 6 | Click starts a session; right click or press and hold (about 450 ms) opens the profile menu (Winuake addition; Yakuake only has the right-click menu) |
| `[MinusButton]` x, y, images | done | 6 | Closes the selected tab |
| SVG images (default skin) | done | 6 | Rendered at 3x into `%LOCALAPPDATA%\Winuake\skin-cache` |
| Window masks for rounded skin corners | n/a | 6 | Not ported; corner images are drawn over the window background |
| Install from `.tar`, `.tar.gz`, `.zip`; remove | done | 6 | Replaces KNewStuff; bzip2 archives are not supported |
| Title bar drag changes height | done | 6 | |
