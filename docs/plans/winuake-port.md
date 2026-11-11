# Winuake — phased port of Yakuake to Windows 11 (Rust + WinUI 3)

## Context

Yakuake (KDE, https://github.com/KDE/yakuake, synced at `21c6210`, 2026-11-09) = Quake-style drop-down terminal. ~6.6k LOC C++/Qt in `app/`, embeds Konsole KPart for terminals, D-Bus scripting API, INI+SVG skins in `data/skins/`. Goal: Windows 11 clone "Winuake" in Rust + WinUI 3, kept at feature parity long-term, so structure must map cleanly onto Yakuake source and make upstream changes easy to port.

`~/src/winuake` exists, empty, not a git repo.

Decisions (from user):
- **UI**: Rust + WinUI 3 everywhere (accepted ~60–100 MB baseline from WinAppSDK runtime).
- **License**: GPL-2.0-or-later (port code/logic/skins directly; keep KDE copyright headers on ported logic).
- **Terminal engine**: `alacritty_terminal` (VT parser + grid) + ConPTY. Own renderer (Direct2D/DirectWrite into WinUI `SwapChainPanel`).

## Architecture

Cargo workspace, crates organized by feature; each crate mirrors Yakuake source files for parity mapping.

| Crate | Purpose | Yakuake source mapped |
|---|---|---|
| `winuake-core` | Pure logic, no Win deps: `SessionStack`, `Session`, split tree, ids, actions enum, window geometry math, monitor-flag state | `sessionstack.*`, `session.*`, `splitter.*`, geometry parts of `mainwindow.cpp` |
| `winuake-config` | TOML config mirroring `yakuake.kcfg` groups/keys 1:1 (`Window`, `Behavior`, `Appearance`, `Animation`, `AutoOpen`, `Dialogs`), profiles, shortcuts | `config/yakuake.kcfg`, `config/*settings.cpp` |
| `winuake-term` | ConPTY spawn/resize/io (windows-rs) + `alacritty_terminal::Term`, OSC title, activity/silence detection | `terminal.*` (Konsole part replacement) |
| `winuake-skin` | `.skin` INI parser + layout model (Title/Tabs/Buttons/Border), skin discovery/install | `skin.*`, `data/skins/*` |
| `winuake-render` | D3D11 + D2D/DirectWrite glyph-atlas grid renderer, cursor, selection, overlays | Konsole drawing, `visualeventoverlay.*` |
| `winuake-ipc` | Named-pipe JSON-RPC server/client mirroring every `Q_SCRIPTABLE` method | D-Bus interface (`sessionstack.h`, `tabbar.h`, `mainwindow.h`) |
| `winuake` (bin) | WinUI 3 app: bootstrap, drop-down `Window`/`AppWindow`, title bar, tab bar, split panes, settings window, tray, hotkey | `mainwindow.*`, `titlebar.*`, `tabbar.*`, `firstrundialog.*`, `config/*` UI |
| `winuake-cli` (bin) | `winuake-cli add-session`, `run-command`, `toggle` … (qdbus equivalent) | — |
| `xtask` | Generate WinUI bindings from WinAppSDK `.winmd` via `windows-bindgen`; upstream-sync tooling | — |

Key tech:
- WinUI 3 from Rust: `windows-bindgen` over `Microsoft.UI.Xaml.winmd`/`Microsoft.UI.winmd` (from `Microsoft.WindowsAppSDK` NuGet), unpackaged bootstrap via `MddBootstrapInitialize2`, `Application::Start` + custom `IXamlMetadataProvider`. UI built in code (no XAML compiler); `XamlReader::Load` for static markup snippets where convenient.
- Drop-down window: `AppWindow` + `OverlappedPresenter` (no border/title bar, always-on-top, hidden from Alt+Tab via `WS_EX_TOOLWINDOW`), `MoveAndResize` per-frame slide animation driven by `Animation.Frames`; monitor via `DisplayArea`.
- Global hotkey (`RegisterHotKey`), tray (`Shell_NotifyIconW`), single instance (named mutex → forward to IPC) on hidden message-only HWND.
- Logging: `tracing` + `tracing-subscriber` (structured fields, never terminal content/secrets; content = length only). Info at action/IPC entry, Debug internals.
- Tests: `cargo nextest`, BDD-style names (`given_two_sessions_when_close_active_then_previous_raised`), `cargo llvm-cov` ≥70% (target 90%) on `core/config/skin/ipc/term` logic; WinUI/render crates excluded from coverage. No real ConPTY/filesystem in unit tests — PTY behind `PtyBackend` trait, faked in tests.
- Build/CI: GitHub Actions `windows-latest` (build, test, clippy, fmt, coverage). Dev box is Linux: pure crates build/test on Linux; app needs Windows (CI or VM) — `cargo xwin` for compile checks.

## Parity maintenance (set up in Phase 0, used forever)

- `UPSTREAM.md`: last-synced Yakuake SHA + date.
- `docs/parity.md`: matrix — every kcfg key, action (35 incl. `switch-to-session-N`), D-Bus method, skin key → Winuake status (done/partial/N-A with reason, e.g. KNewStuff → zip install, `UseWMAssist` → N/A).
- `cargo xtask upstream-diff`: clones/fetches Yakuake, lists commits since synced SHA touching `app/` or `data/`, skipping `GIT_SILENT`/po-only, prints diffs grouped by mapped crate (mapping table in xtask).
- Ported files carry `// Ported from yakuake app/<file>.cpp @<sha>` header.
- Optional: scheduled routine (monthly) running upstream-diff and opening an issue.
- Each sync = plan `docs/plans/upstream-sync-<date>.md` → `_output.md`.

## Phases

Each phase independently buildable + committable; save each as `docs/plans/phase-N-<name>.md` before implementing, `_output.md` after.

### Phase 0 — Scaffold + WinUI-from-Rust spike (go/no-go)
- `git init`, GPL-2.0-or-later `LICENSE`, `.gitignore` (Rust + VS), `rustfmt.toml`, `clippy.toml`, `.editorconfig`, `CLAUDE.md`, `README.md`, CI workflow, `UPSTREAM.md`, `docs/parity.md` skeleton, this plan as `docs/plans/winuake-port.md`.
- Workspace with all crates stubbed; `xtask gen-bindings`.
- Spike: unpackaged WinUI 3 window from Rust with a `SwapChainPanel` cleared by D3D11 + a `TabView` and `Button`. Measure working set.
- **Gate**: if WinUI bindings prove unworkable, revisit UI stack before Phase 2.

### Phase 1 — Terminal core (headless)
- `winuake-term`: `PtyBackend` trait, ConPTY impl (`CreatePseudoConsole`, pipes, resize, exit detection), reader thread → `alacritty_terminal::Term` event loop, title/bell/activity events, `runCommand` write.
- Default shell resolution (pwsh → powershell → cmd; WSL profile later).
- Tests with fake PTY: output parsing, resize, title change, activity/silence timers.

### Phase 2 — Renderer + single terminal in WinUI
- `winuake-render`: DirectWrite font fallback, glyph atlas, cell grid, cursor styles, colors (16/256/truecolor), bold/italic/underline, DPI scaling.
- Input: key → VT encoding (incl. modifiers, app cursor mode), IME/text input, mouse reporting, selection, copy/paste (Ctrl+Shift+C/V), scrollback wheel, URL hover.
- App shows one working terminal in a normal window. Verify: run pwsh, vim in WSL, `htop`-like TUIs, resize.

### Phase 3 — Drop-down window behavior
- Port `mainwindow.cpp` window logic: `Width`/`Height` %, `Position`, `Screen` (follow mouse = 0), slide in/out over `Frames`, `KeepOpen`, hide on focus loss, `ToggleToFocus`, `KeepAbove`, `OpenAfterStart`, `RememberFullscreen`, increase/decrease width/height actions.
- Global hotkey (default F12), tray icon + menu (`ShowSystrayIcon`), single instance, `ConfirmQuit`.
- Geometry math in `winuake-core` with unit tests (multi-monitor, taskbar work area, DPI).
- Note: `ShowOnAllDesktops` via `WS_EX_TOOLWINDOW` behavior; investigate pinning API, mark partial if needed.

### Phase 4 — Sessions, tabs, splits
- `winuake-core`: `SessionStack` (ids, add/raise/remove, closable, active tracking), `Session` split tree (left-right, top-bottom, auto, two-horizontal/vertical/quad, grow terminal by px, prev/next terminal, close terminal).
- App: custom tab bar (port `tabbar.cpp`: rename via double-click/inline edit, drag reorder, middle-click close, context menu, `+` button, `DynamicTabTitles`), title bar (`titlebar.cpp`: text, keep-open/config/quit buttons, `ShowTitleBar`, `ShowTabBar`).
- All 35 actions bound with Yakuake default shortcuts; `switch-to-session-1..N`.
- Extensive BDD tests on stack/tree logic (largest test surface).

### Phase 5 — Configuration + settings UI
- `winuake-config`: TOML at `%APPDATA%\Winuake\winuake.toml`, kcfg defaults/ranges ported 1:1, live reload, profiles (`[[profile]]`: shell cmd, args, cwd, font, size, color scheme, scrollback, cursor) replacing Konsole profiles; import Konsole `.colorscheme` + Windows Terminal JSON schemes.
- Settings window (WinUI `NavigationView`): Window / Behavior / Appearance pages mirroring `windowsettings.ui`, `behaviorsettings.ui`, `appearancesettings.ui`; profile editor ("manage-profiles", "edit-profile" actions); shortcut editor.
- First-run dialog (`firstrundialog.ui`: pick toggle hotkey).

### Phase 6 — Skins
- `winuake-skin`: parse `title.skin`/`tabs.skin` (all sections/keys), relative asset resolution, validation.
- Render title bar + tab bar from skin images (SVG via `SvgImageSource`, PNG via `BitmapImage`), `Border` color/width, `HideSkinBorders`, text positions/colors, button states (up/over/down), `prevent_closing_image`.
- Bundle `default`, `legacy`, `plastik_dark`, `plastik_light` (GPL/CC0 compatible); skin list in Appearance page (port `skinlistdelegate.cpp`); install/remove skin from `.zip`/`.tar.gz` (replaces KNewStuff).
- Skin parser tests use in-memory fixtures.

### Phase 7 — Advanced behavior
- `Translucency` + `BackgroundColor`/`BackgroundColorOpacity` (Mica/Acrylic backdrop option on top).
- Keyboard input lock per session/terminal + indicator overlay (`KeyboardInputBlockIndicator*`), prevent-closing toggle.
- Monitor activity/silence → Windows toast notifications (replaces `yakuake.notifyrc`).
- Terminal highlight on manual activation (`TerminalHighlight*`, port `visualeventoverlay.cpp`).
- `FocusFollowsMouse` between splits; `AutoOpen.PollMouse`/`PollInterval` (open when mouse hits screen edge).
- Monitor hot-plug / display order changes (port intent of `outputorderwatcher.cpp` via `WM_DISPLAYCHANGE`).

### Phase 8 — Scripting IPC + CLI
- `winuake-ipc`: named pipe `\\.\pipe\winuake-<user SID>` (ACL: current user only), JSON-RPC, method names = Yakuake D-Bus names (`addSession`, `splitTerminalLeftRight`, `runCommandInTerminal`, `setTabTitle`, `toggleWindowState`, …) for script portability.
- `winuake-cli` + PowerShell examples; doc mapping from `qdbus org.kde.yakuake /yakuake/sessions …` calls.
- Tests: protocol round-trip with in-memory transport; dispatcher against fake `SessionStack`.

### Phase 9 — Packaging + release
- MSIX (signed) with WinAppSDK framework dependency + unpackaged installer bundling WinAppSDK runtime; winget manifest.
- Autostart (StartupTask for MSIX / Run key unpackaged), app icon from Yakuake assets, crash logs to `%LOCALAPPDATA%\Winuake\logs`.
- Fill `docs/parity.md` fully; release v1.0.

## Risks
- WinUI 3 from Rust: no XAML compiler, custom `IXamlMetadataProvider`, threading (DispatcherQueue) — retired in Phase 0 spike.
- `SwapChainPanel` + acrylic/translucency composition; drop-down animation smoothness via `AppWindow.MoveAndResize`.
- Virtual-desktop pinning has no public API.
- ConPTY quirks (resize reflow, older Windows builds) — target Windows 11 22H2+ only.

## Verification
- Every phase: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo nextest run`, `cargo llvm-cov` ≥70% on logic crates — all green in Windows CI.
- Per CLAUDE.md, tests ≠ working: run app on Windows each phase and exercise feature (phase-specific checklist in each phase plan: e.g. P3 hotkey toggles, slide animation, multi-monitor; P4 every action/shortcut; P8 `winuake-cli` drives running instance).
- Parity: `docs/parity.md` row updated per feature; `cargo xtask upstream-diff` shows zero unreviewed upstream commits at each release.
