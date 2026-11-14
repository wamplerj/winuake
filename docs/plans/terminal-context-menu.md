# Terminal context menu + press-and-hold profile picker

## Context

Yakuake shows Konsole's context menu when you right-click a terminal (see `docs/plans/Screenshot_20261113_162603.png`). The menu has Paste, Open File Manager, Open Folder With, Split View, Set Encoding, Find..., Scrollback, Read-Only, Allow Mouse Tracking, Switch Profile, Edit Current Profile and Close Session. Winuake has no terminal menu: right-click copies the selection, or pastes when nothing is selected (`crates/winuake/src/terminal_view/pointer.rs:86`). Winuake now supports multiple profiles. Today the only ways to pick one for a new session are right-clicking `+` or the title-bar menu. You want to press and hold `+` to get the profile menu.

Decisions (from the questions):
- Right-click on a terminal **always** opens the menu, with no setting. Copy/paste move to Ctrl+Shift+C/V and middle-click, which keeps pasting.
  - When a program has mouse reporting on, right-click goes to the program, and Shift+right-click opens the menu. This is how Konsole works.
- **Set Encoding is left out.** ConPTY always uses UTF-8. `docs/parity.md` records it as not applicable.
- **Open Folder With** lists File Explorer, VS Code (only when `code` is on PATH), New Winuake Session Here, and Copy Path.

Step 0, before any code: copy this plan to `docs/plans/terminal-context-menu.md`, as the project rules ask. When done, write `docs/plans/terminal-context-menu_output.md`. Update `docs/parity.md` and `UPSTREAM.md` in each phase that changes parity.

Each phase builds, passes `cargo fmt --check`, `clippy -D warnings` and `cargo test --workspace`, and gets its own commit.

---

## Phase 1: press and hold `+` for the profile menu

- New pure state machine, `winuake-core/src/press_and_hold.rs`:
  - `PressAndHold` takes the events `pressed`, `released`, `hold_elapsed` and `cancelled`.
  - It returns `HoldOutcome::{Click, Hold, Nothing}`.
  - BDD tests: a quick release is a click; a hold opens the menu and the later release does nothing; leaving the button cancels; right-click still opens the menu.
- `crates/winuake/src/skin_chrome.rs` `SkinButtonViews::connect_with_right_click`, and the plain `+` in `crates/winuake/src/chrome.rs` (`build`, around line 237):
  - A left press starts a Win32 `SetTimer` of about 450 ms. This follows the existing pattern in `flashes.rs` and `settings_store.rs`.
  - Release before the timer fires runs `NewSession`. The action moves from press to release.
  - When the timer fires, `show_profile_menu()` runs at the cursor (`chrome.rs:672`, reused unchanged).
  - Exit or capture loss cancels.
- Applies to the skinned `+`, the inline `+` (`plus_button_at_end_of_tabs`) and the plain `+`.
- Update the Profile page hint text in `settings_window.rs` `profile_page_markup` to say "press and hold or right-click".

## Phase 2: menu framework and the simple items

- New pure menu model, `winuake-core/src/terminal_menu.rs`:
  - `TerminalMenuState` is a record of `has_selection`, `read_only`, `mouse_tracking_allowed`, `profiles` and `current_profile`.
  - `terminal_menu(state) -> Vec<MenuEntry>`. A `MenuEntry` is an item, separator or submenu, with label, shortcut text, enabled, checked and a `TerminalMenuCommand`.
  - Tests cover what shows and when: Copy only with a selection; Read-Only and Mouse Tracking check marks; the current profile checked under Switch Profile.
  - Order follows the screenshot, minus Set Encoding.
- `chrome.rs`:
  - Generalize `track_menu` and `append_profile_submenu` into a small `render_menu(&[MenuEntry])` that builds nested `HMENU`s with `MF_POPUP`.
  - Shortcut text goes after `\t`, taken from the current `ShortcutMap`.
  - The existing tab, main and profile menus keep their behavior.
- `terminal_view/pointer.rs` `handle_pointer_pressed`:
  - Right button with no app mouse reporting, or Shift held: call `workspace_host::show_terminal_menu(id)`.
  - Middle button: paste.
  - Remove right-click copy/paste.
- `workspace_host.rs`: `show_terminal_menu(terminal)` builds the state, shows the menu and dispatches the command. Items in this phase:
  - **Copy / Paste**: reuse `copy_selection` and `paste_clipboard`.
  - **Split View** submenu: Split Left/Right, Split Top/Bottom, Split Automatically, Close Terminal. These map to the existing `Action::SplitLeftRight`, `SplitTopBottom`, `SplitAuto` and `CloseActiveTerminal`, run against the clicked terminal after focusing it.
  - **Read-Only**: the clicked terminal's existing `TerminalSwitch::KeyboardInput`, inverted, so it stays one switch that the tab menu's "Disable Keyboard Input" also reflects. It also blocks paste and mouse reports while set.
  - **Allow Mouse Tracking**: a new per-terminal flag on `TerminalView`, default on. When off, `application_wants_mouse()` returns false, so selection works inside programs like vim and htop.
  - **Edit Current Profile...**: open settings on the Profile page with this terminal's profile selected. `settings_window::select_profile` already exists, so expose it.
  - **Close Session**: closes the clicked terminal (Konsole's part is one terminal), using `close_active_terminal` and its existing confirmation rules.

## Phase 3: Switch Profile and Scrollback

- **Switch Profile** submenu: lists profiles with the clicked terminal's current one checked. Picking one re-applies the profile live to that terminal: font, size, color scheme, cursor shape and blink, and scrollback length. The running program stays, as in Konsole. Changes:
  - `winuake-render/src/gpu.rs`: add `TerminalRenderer::set_font(&FontSpec)`, following the rebuild path `set_dpi` uses.
  - `TerminalView`: `scheme` and `cursor_blink` become `RefCell`/`Cell`. Add `apply_profile(&Profile)`, which calls `apply_size()` so the grid resizes to the new cell size.
  - `winuake-term`: add `TerminalEmulator::set_default_cursor` and `set_scrollback_lines`.
  - `workspace_host`: update `terminal_profiles` so later splits inherit the new profile.
- **Scrollback** submenu:
  - Clear Scrollback: `TerminalEmulator::clear_history`, using alacritty `grid_mut().clear_history()`.
  - Clear Scrollback and Reset: also resets the terminal state and clears the screen, Ctrl+Shift+K like Konsole.
  - Adjust Scrollback...: opens the Profile page.
  - Emulator unit tests for both clear operations.
- New actions in `winuake-core/src/actions.rs`: `clear-scrollback` (no default) and `clear-scrollback-and-reset` (`Ctrl+Shift+K`).
  - They are Winuake additions, marked like `OpenSettings`. Update the action-count docs and tests.
  - They appear on the Shortcuts page automatically.

## Phase 4: current folder, Open File Manager, Open Folder With

- `winuake-term`: new `working_directory.rs`, a pure scanner over incoming bytes for OSC 7 (`file://host/path`, percent-decoded) and OSC 9;9 (Windows Terminal style). It runs before bytes reach alacritty in `TerminalEmulator::advance`. The emulator exposes `current_directory() -> Option<PathBuf>`.
  - Tests: split sequences across reads, BEL/ST terminators, percent-decoding, `/C:/...` → `C:\...`, WSL `/home/...` paths (kept as `\\wsl$\<distro>\...` when the profile is WSL, otherwise ignored).
- Fallback order: reported folder, then the profile's `working_directory`, then the user profile folder.
- Menu items:
  - **Open File Manager**: `ShellExecuteW("open", folder)`.
  - **Open Folder With** submenu:
    - File Explorer.
    - Visual Studio Code: shown when `code.cmd` is found on PATH, via a pure `find_on_path` with a fake-environment test.
    - New Winuake Session Here: the default profile with a working-directory override. Add an override parameter to `add_session_with_profile` and `profile_for_new_terminal`.
    - Copy Path: reuses `clipboard.rs`.
- Document in the output file how to make PowerShell report its folder: a prompt snippet that emits OSC 9;9, and oh-my-posh/starship, which already do.

## Phase 5: Find

- `winuake-term`: a search wrapper around alacritty's `RegexSearch`. The query is escaped unless regex mode is on. It supports a case-sensitive toggle, next/previous match from the current position, and scrolling the view to the match. The snapshot carries the match range so the renderer can highlight it.
  - Emulator tests for next/previous, wrap-around, case and regex.
- `winuake-render`: draw the match highlight, using the selection path with a distinct color.
- UI: a search bar overlaid at the bottom of the terminal pane, built from XAML markup like the rename box in `chrome.rs`. It has a TextBox, previous/next, a case toggle and a regex toggle (both `ToggleButton`, already bound), and close.
  - Enter goes to the next match, Shift+Enter to the previous, Esc closes and refocuses the terminal.
  - Uses `suspend_text_routing()` like the other text boxes.
- New action `find-in-terminal` (`Ctrl+Shift+F`) and the menu item **Find...**.

---

## Critical files

- `crates/winuake/src/chrome.rs`: menu rendering (`track_menu`, `append_profile_submenu`, `show_profile_menu`) and the plain `+`.
- `crates/winuake/src/skin_chrome.rs`: `SkinButtonViews::connect_with_right_click`, for press and hold.
- `crates/winuake/src/terminal_view.rs` and `terminal_view/pointer.rs`: right-click routing, mouse tracking flag, `apply_profile`, search bar host.
- `crates/winuake/src/workspace_host.rs`: `show_terminal_menu`, command dispatch, profile switching, folder override.
- `crates/winuake-core/src/` (new `press_and_hold.rs`, `terminal_menu.rs`) and `actions.rs`.
- `crates/winuake-term/src/emulator.rs` and new `working_directory.rs`, plus search.
- `crates/winuake-render/src/gpu.rs`: `set_font`, match highlight.
- `docs/parity.md`, `UPSTREAM.md`, `docs/plans/terminal-context-menu*.md`.

## Verification

- Linux, every phase: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo check`/`clippy -p winuake --target x86_64-pc-windows-msvc`. New pure logic (press and hold, menu model, folder scanner, search, scrollback) gets BDD tests. Logic crates stay at or above 70% coverage.
- On the VM, after each phase: run `VM_SSH=james@192.168.122.55 VM_DOMAIN=win11-test scripts/vm-run-app.sh`, drive it with `scripts/vm-mouse.py`/`vm-type.sh`, and check with `virsh screenshot` plus the app log and `winuake.toml`.
  1. Short click on `+` opens a default session. Holding opens the profile menu, and choosing Git Bash starts it. Right-click still works. Check on the plain and skinned bars.
  2. Right-click in PowerShell shows the menu.
     - Paste types the clipboard; Copy appears only with a selection.
     - The splits work.
     - Read-Only blocks typing and the tab menu shows Disable Keyboard Input checked.
     - With mouse tracking off, selection works in `vim` inside WSL/Git Bash; with it on, Shift+right-click shows the menu.
     - Edit Current Profile opens the right profile, and Close Session closes the pane.
  3. Switching profile changes font and colors on the live terminal while the shell keeps its history. Clear Scrollback empties the scrollback, and Ctrl+Shift+K also clears the screen.
  4. After `cd C:\Windows` with the OSC 9;9 prompt snippet, Open File Manager opens Explorer there. New Session Here starts in that folder, and Copy Path copies it.
  5. Ctrl+Shift+F opens Find. Next and previous step through matches in the scrollback with the highlight visible, and Esc returns focus.
- Write `docs/plans/terminal-context-menu_output.md` with what was verified on the VM and what wasn't.
