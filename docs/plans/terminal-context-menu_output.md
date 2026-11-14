# Terminal context menu and press-and-hold profile picker: output

Plan: `docs/plans/terminal-context-menu.md`. Five phases, one commit each. Verified on the Windows 11 VM (`win11-test`) after every phase.

## What was built

1. **Press and hold the `+` button** opens the profile menu (about 450 ms); a quick click still starts a session with the default profile; right click still works.
   - `winuake-core::press_and_hold` is the tested state machine; `hold_button.rs` wires it to pointer events and one timer. It covers the skinned `+`, the inline `+` and the plain `+`.
   - Verified: click added "Shell No. 2"; holding opened the menu, which stayed open after the button was released; choosing Git Bash started it.
2. **Terminal context menu** on right click (always; copy and paste moved to Ctrl+Shift+C / V and middle click). A program that asked for mouse events still gets right clicks; Shift+right click opens the menu.
   - `winuake-core::terminal_menu` builds the entries (tested); `popup_menu.rs` draws them as Win32 menus (submenus, check marks, shortcut text).
   - Items: Copy (only with a selection), Paste, Split View (left/right, top/bottom, automatic, close terminal), Read-Only, Allow Mouse Tracking, Edit Current Profile..., Close Session.
   - Read-Only is the existing keyboard-input switch of that terminal, so the tab menu's "Disable Keyboard Input" shows it too. It also blocks paste and mouse reports.
   - Verified: menu layout, Read-Only blocks typing and shows the check, split, edit-profile opens the Profile page on the terminal's profile, close, copy and paste.
3. **Switch Profile** applies another profile's font, size, colors, cursor and scrollback to the running terminal (the shell and screen stay; later splits use the new profile). **Scrollback** submenu: Clear Scrollback, Clear Scrollback and Reset (Ctrl+Shift+K, a full RIS), Adjust Scrollback....
   - New: `TerminalRenderer::set_font`, `TerminalEmulator::apply_defaults / clear_scrollback / reset_and_clear`, actions `clear-scrollback` and `clear-scrollback-and-reset`. The cursor blink thread now always runs and watches a flag.
   - Verified with a Git Bash profile changed to 16 pt One Half Dark: the live PowerShell terminal changed font size and colors and kept its output; both clear commands behave.
4. **Open File Manager / Open Folder With** (File Explorer, Visual Studio Code when `code.cmd` is on the PATH, New Winuake Session Here, Copy Path).
   - The shell's folder is read from OSC 7 and OSC 9;9 reports (`winuake-term::working_directory`, tested). Without a report the profile's starting folder, then the user folder, is used.
   - Verified: after `cd ..` with the prompt snippet below, Explorer opened `C:\Users`, Copy Path put `C:\Users` on the clipboard, and a new session started in `C:\Users`.
5. **Find** (Ctrl+Shift+F or the menu): a find bar at the top right of the terminal area. Search as you type, Enter / Shift+Enter, case and regular expression toggles, Escape closes. Searches screen and scrollback and wraps.
   - `TerminalEmulator::find` (tested, in `emulator/search.rs`) wraps alacritty's `RegexSearch`; the current match is the terminal's selection, so no renderer change was needed and Ctrl+Shift+C copies it.
   - Verified: typing highlights the match and scrolls to it, Enter steps to the next match, "Not found" shows, Escape returns the keyboard to the terminal.

Set Encoding is left out (ConPTY is always UTF-8); see `docs/parity.md`.

## Make PowerShell report its folder

Add this to the PowerShell profile (`$PROFILE`); oh-my-posh and starship already send it:

```powershell
function prompt {
    $escape = [char]27
    $bell = [char]7
    "$escape]9;9;`"$($PWD.ProviderPath)`"$bell" + "PS $($PWD.Path)> "
}
```

## Not done / limits

- Allow Mouse Tracking is only unit-covered at the pointer-routing level; it was not tried against a mouse-reporting program (vim, htop) on the VM.
- Find highlights only the current match, not every match, and shows no match count. Match case and regex changes re-run the search from the current match.
- Open File Manager and the other folder items use the folder the shell reported. For a WSL profile a Unix path maps into `\\wsl.localhost\<distro>`; New Winuake Session Here uses the default profile, so it does not start a WSL shell there.
- Visual Studio Code was not on the VM, so that item's launch was not exercised (it only shows when `code.cmd` is found).
- The skinned `+` button's press and hold was not exercised (the VM used the plain tab bar); it shares the same gesture code.
- `bindings.rs` is generated and not committed: run `cargo xtask gen-bindings` after pulling (new filters for `ComboBox`, `Selector` and `TextChanged`).

## VM changes left behind

Test data on the VM, not in the repo: `cursor_shape = "Bar"` and a changed Git Bash profile (16 pt, One Half Dark) in `%APPDATA%\Winuake\winuake.toml`, and the prompt snippet above in the PowerShell 7 profile.
