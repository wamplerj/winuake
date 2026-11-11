# Phase 2 — Renderer + single terminal in WinUI

Parent: `winuake-port.md`. Crates: `winuake-term` (input, selection, snapshot), `winuake-render` (colors, snapshot -> pixels), `winuake` (terminal view).

## Design decisions

- **Renderer = Direct2D + DirectWrite onto the swap chain's DXGI surface**, not a custom D3D glyph atlas (deviation from the master plan). Fewer moving parts, lower memory, text quality/ClearType/fallback come from DirectWrite. Revisit only if profiling demands it.
- **Render model is pure and cross-platform** (`RenderSnapshot`: resolved RGB cells, cursor, selection, display offset) so colors, cursor, selection, scrollback logic are unit-tested on Linux. Only snapshot -> pixels is Windows-only.
- **Input encoding is pure** (key / paste / mouse / focus -> VT bytes, driven by terminal mode flags), xterm-compatible.

## Chunks (each buildable + committable)

- **2a Input encoding** (`winuake-term::input`): keys with modifiers, cursor/keypad modes, F-keys, paste (bracketed, sanitized), mouse (X10/normal, SGR, drag/motion), wheel (incl. alternate scroll), focus events. Emulator exposes `InputModes`.
- **2b Render model + selection + scrollback** (`winuake-term` emulator, `winuake-render::model`): color scheme (16 + 256 + truecolor, dim/bold/inverse), `RenderSnapshot`, selection (simple/word/line/block) with text extraction, display scrolling.
- **2c Direct2D/DirectWrite renderer** (`winuake-render::gpu`, Windows): font metrics -> cell size, background runs, text runs (bold/italic/underline/strikethrough), wide chars, cursor shapes, selection overlay, DPI. Integration test renders a snapshot offscreen and checks pixels on the VM.
- **2d Terminal view in the app**: `SwapChainPanel` + `TerminalSession` + renderer; size -> columns/rows -> resize buffers/session/pty; WinUI key/char events -> input encoder; output events -> redraw (coalesced); cursor blink. Verified by typing into the VM and screenshotting.
- **2e Interaction**: mouse selection, copy (Ctrl+Shift+C), paste (Ctrl+Shift+V), wheel scrollback, mouse reporting to apps, URL hover (open on Ctrl+click), focus reporting.

## Out of scope (later phases)

Tabs/splits (4), settings/fonts config (5), skins (6), translucency (7), IME composition polish (tracked as follow-up).

## Verification

fmt, clippy `-D warnings` on both targets; Linux unit tests for 2a/2b; VM: offscreen pixel test (2c), then interactive run: `pwsh`/`cmd`, `vim`-style full-screen TUI, resize, selection, scrollback, paste (2d/2e), screenshots recorded in `phase-2-renderer-input_output.md`.
