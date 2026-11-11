# Phase 1 — Terminal core (headless)

Parent: `winuake-port.md`. Crate: `winuake-term`. Replaces Konsole KPart used by Yakuake `terminal.cpp`.

## Chunks (each buildable + committable)

1. **Pure logic (Linux-testable)**
   - `pty`: `PtyBackend`, `PtyConnection` (reader/writer/control), `PtySpawnRequest`, `TerminalSize`, `ExitStatus`, `PtyError`.
   - `shell`: `ShellResolver` + `ExecutableLocator` — `pwsh.exe` -> `powershell.exe` -> `%COMSPEC%`/`cmd.exe`.
   - `emulator`: `TerminalEmulator` wrapping `alacritty_terminal::Term` + VT `Processor`; maps terminal events (title, bell, PTY responses); screen text, cursor, resize.
   - `activity`: `ActivityMonitor` (monitor activity / monitor silence, pure, clock passed in).
   - `session`: `TerminalSession` — reader thread -> emulator, input/`run_command`/resize/kill, `SessionEvent` channel. Fake PTY for tests.
2. **ConPTY backend (Windows)**: `CreatePseudoConsole`, anonymous pipes, `CreateProcessW` with pseudoconsole attribute, resize, exit wait, kill. Bindings generated into gitignored `conpty_bindings.rs` by `cargo xtask gen-bindings`.
3. **Windows integration test** (ignored by default, run on Win11 VM): spawn `cmd.exe /c echo`, assert text lands in emulator grid; resize; exit code.

## Out of scope here

Rendering, input encoding (keys -> VT), selection, clipboard/OSC 52, color requests (Phase 2); notifications UI (Phase 7).

## Verification

fmt, clippy `-D warnings`, `cargo test --workspace` on Linux; ConPTY integration test run on the VM; coverage on logic modules.
