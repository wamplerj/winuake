# Phase 2 — output

One real terminal in a WinUI 3 window: PowerShell runs in a ConPTY, is drawn by Direct2D/DirectWrite into a `SwapChainPanel`, and takes keyboard and mouse input.

## Done

| Chunk | What | Where |
|---|---|---|
| 2a | Input encoding: keys + modifiers, cursor/application mode, F-keys, bracketed/sanitized paste, focus, mouse (normal/SGR, click/drag/motion), wheel, alternate scroll | `winuake-term::input` |
| 2b | `ScreenSnapshot`, color resolution (16/256/truecolor, bold-bright, dim, inverse, hidden, selection), selection (simple/block/word/line), scrolling, input-mode flags | `winuake-term::{emulator,snapshot}`, `winuake-render::color` |
| 2c | `TerminalRenderer`: D3D11 + D2D + DirectWrite, backgrounds, text runs, wide chars, underline/strikethrough, cursor shapes, DPI (`set_dpi`, inverse composition transform), resize, GPU read-back | `winuake-render::gpu` |
| 2d | `TerminalView`: focusable control + `SwapChainPanel`, shell session, resize -> grid -> PTY, cursor blink, coalesced redraws from session events | `winuake::terminal_view` |
| 2e | Mouse selection (click, double = word, triple = line), copy/paste, wheel scrollback, mouse reporting to applications (Shift bypasses), clipboard shortcuts | `winuake::terminal_view::pointer`, `winuake::clipboard`, `winuake-term::{pointer,keyboard}` |

Shortcuts: Ctrl+Shift+C / Ctrl+Insert copy; Ctrl+Shift+V / Shift+Insert paste; right-click copies a selection or pastes; middle-click pastes. Plain Ctrl+C / Ctrl+V reach the shell.

## Verification

- Linux: 122 term + 12 render unit tests; `fmt`, `clippy -D warnings` clean on Linux and Windows targets.
- Windows VM (Win11 build 26300): 9 ConPTY integration tests; 14 GPU pixel tests run in the console session (backgrounds, glyph pixels, colors, cursor shapes, selection, inverse, underline, wide chars, resize, DPI scaling, font fallback).
- App, driven on the VM with injected keyboard/pointer events and screenshots: PowerShell prompt and PSReadLine colors render; window-fit resize (80x24 -> 147x34); typing, Shift, Enter; drag selection highlighted; Ctrl+Shift+C then Ctrl+Shift+V pasted the selected text twice (log: `copying selection length=3`, `pasting length=3`); wheel scrolled the view back (lines 39-72 shown after 3 notches).
- Memory with one live terminal (debug build): 74 MB working set, 23 MB private.

## Findings

1. **XAML `CharacterReceived` is never raised** for this unpackaged WinUI 3 setup (not on the control, not on the panel). Text comes from `WM_CHAR`/`WM_SYSCHAR` through a thread `WH_GETMESSAGE` hook (`install_text_input_hook`); `KeyDown` handles special keys and Ctrl combinations. This also gives correct dead-key composition.
2. **Composition swap chains need an interactive desktop**: over SSH, `CreateSwapChainForComposition` fails with `DXGI_ERROR_NOT_CURRENTLY_AVAILABLE`. GPU tests skip loudly there; `scripts/vm-test-interactive.sh` runs them in the console session via a scheduled task.
3. **windows-bindgen 0.100 gaps**: no `new` for generic delegates (`TypedEventHandler`) -> hand-written in `bindings_extensions.rs`; the flat layout collides when both `Windows.UI.Composition` and `Microsoft.UI.Composition` are reachable (avoid `InputKeyboardSource`); factory/statics interfaces must be listed by hand.
4. WinUI event handlers must be `Send + 'static`: views register in a UI-thread registry and handlers capture only a `TerminalId`.
5. `alacritty_terminal` enables alternate-scroll by default.
6. `SwapChainPanel` needs stretch alignment on both the panel and its host control, or it collapses to zero size.

## Not done / follow-ups

- **URL hover / Ctrl+click** (listed in the plan for 2e): not implemented. Needs hyperlink detection on snapshot text and a hover overlay.
- **IME composition UI** (candidate window, in-place preview): only committed text arrives via `WM_CHAR`. Needs a TSF text store / `CoreTextEditContext`.
- **Mouse reporting to applications** is implemented and unit-tested at the encoding level, but I did not drive a real mouse-aware TUI (vim, htop) on the VM.
- **Combining characters** (zero-width) are not drawn; box-drawing glyphs are drawn per cell and may show seams with some fonts; bold/italic use font faces only (no synthetic bold when a face is missing).
- **Device loss** (GPU reset, RDP session switch) is not handled: a failed `Present` is logged, not recovered.
- **Fractional DPI** (150%) is implemented (font rescale + inverse composition transform) and unit-measured, but the VM runs at 100%, so it has not been checked visually.
- Font, scheme, scrollback size, cursor style/blink are hard-coded defaults until Phase 5.
- Scripts (`scripts/`) are the dev loop for the Linux host + Windows guest; no CI job runs the GPU tests.
