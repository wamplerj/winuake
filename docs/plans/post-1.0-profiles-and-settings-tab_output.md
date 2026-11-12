# Settings as a tab, multiple profiles

## Done

- **Settings tab**: Settings no longer opens a separate window. It is a page session (`SessionKind::Page`, no terminals) in the Winuake tab bar, titled "Settings"; opening it again raises the tab, closing it works like any tab. Opened from the title-bar menu, the tray menu (which also shows the window when hidden), and the `manage-profiles` / `edit-profile` actions (Profile page). `winuake-core`: empty split layout, `Workspace::open_settings_page`, `PageOpened` effect, tests.
- **Multiple profiles**: Profile page lists every profile (default marked); click to edit its shell, arguments, directory, font, size, color scheme, scrollback, cursor; Add, Duplicate, Delete (the last one cannot be deleted), Make default, **Add detected shells** (Windows PowerShell, PowerShell 7, Command Prompt, Git Bash, each WSL distribution; existing profiles are not duplicated). `winuake-config`: profile management, per-profile field read/write, shell detection rules (`shells`) with a fake-environment test suite.
- **Using profiles**: right-click the `+` button (skinned or plain) for a profile menu; the title-bar menu has "New Session with Profile". Splits keep the profile of the terminal they split. New sessions from `+`, shortcuts and scripting use the default profile.
- **Focus fix**: a terminal that cannot take focus yet (not in the tree) now keeps the request pending and takes focus when it gets its first size, so typing works right after creating a session without clicking.

## Verified on the VM

Settings opens as a tab and switches pages; detection found Windows PowerShell, Command Prompt and Git Bash; right-click on `+` showed the profile menu; a Git Bash session started with the One Half Dark scheme and accepted typing without a click; Duplicate and Delete on the Profile page saved to the settings file.

## Not done

- Profile and scheme changes still apply to new terminals only. No per-profile shortcut or icon. Scripting `addSession` always uses the default profile.
- PowerShell 7 and WSL detection are covered by tests but this VM has neither.
- Global shortcuts (such as Ctrl+Shift+T) do not work while a text box in the settings tab has focus.

## Drag to resize

Drag the window's left or right edge to change its width, the bottom-left or bottom-right corner for width and height together; the title bar still drags the height. The edge you hold follows the pointer and the opposite edge stays put (width and horizontal position are adjusted together; measured from where the drag began so rounding to whole percents cannot make the other edge creep). Size cursors on the grips. Live while dragging, saved when released. `winuake-core::drop_down::resize_by_dragging` is the tested pure logic. Verified on the VM: right edge, left edge and corner drags changed width/height/position in the settings file. Limits: sizes are whole percents of the work area (steps of about 25 px on a 2560 px screen); no drag while full screen; the 6 px edge strips sit over the outermost terminal pixels.
