# Phase 1 — output

## Done (`crates/winuake-term`)

| Module | What |
|---|---|
| `pty` | `PtyBackend`, `PtyConnection` (reader / writer / `PtyControl`), `PtySpawnRequest`, `TerminalSize`, `ExitStatus`, `PtyError` |
| `shell` | `ShellResolver` + `ExecutableLocator` (pwsh -> powershell -> `COMSPEC`/`cmd.exe`), `PathVariableLocator` |
| `emulator` | `TerminalEmulator` over `alacritty_terminal` 0.26 (`Term` + VT `Processor`): screen text, cursor, scrollback count, resize; maps title / title reset / bell / terminal-query replies to `TerminalEvent` |
| `activity` | `ActivityMonitor`: monitor-activity and monitor-silence (10 s default), pure, clock passed in |
| `session` | `TerminalSession`: reader thread -> emulator, `send_input`, `run_command`, `resize`, `kill`, `SessionEvent` channel (`OutputReceived`, `TitleChanged`, `TitleReset`, `BellRung`, `Exited`), monitor polling; kills the child on drop |
| `process_launch` | Pure `CreateProcessW` encoding: argument quoting, command line, case-insensitive environment merge, UTF-16 environment block |
| `conpty` (Windows) | `ConPtyBackend`: `CreatePseudoConsole`, pipes, `CreateProcessW` with the pseudo-console attribute, resize, exit wait, kill |

`cargo xtask gen-bindings` now also writes `crates/winuake-term/src/conpty_bindings.rs` (gitignored).

## Verification

- Linux: 53 unit tests (BDD names, fake PTY, no external resources), fmt, clippy `-D warnings` clean.
- Windows target: clippy clean; 9 integration tests (`crates/winuake-term/tests/conpty.rs`) against real ConPTY on the Win11 VM (cmd echo, exit codes, interactive `run_command`, resize via `mode con`, environment override, working directory, kill, spawn failure, Windows PowerShell). 15 consecutive runs, no flakes.
- Diagnostic: `cargo run -p winuake-term --example conpty_dump` (Windows) prints the raw ConPTY byte stream.

## Findings

1. **Child inherits redirected std handles unless told otherwise.** With `bInheritHandles = FALSE` and no `STARTF_USESTDHANDLES`, a child launched from a process whose stdio is redirected (here: SSH) wrote its output to our stdout instead of the pseudo console. Fix: `STARTF_USESTDHANDLES` with `INVALID_HANDLE_VALUE` for all three handles. Needed in real use too if Winuake is ever started from a redirected parent.
2. **The output pipe only reports EOF after `ClosePseudoConsole`.** A watcher thread waits for the process and closes the console; the reader then drains remaining output and sees EOF. Verified no lost output for short commands on build 26300.
3. `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE` takes the `HPCON` value itself (not a pointer to it).
4. Ergonomic Win32 bindings accept bare names in filters (`CreatePipe`), and constants arrive as `i32`.

## Not done / follow-ups

- **Coverage not measured**: `cargo-llvm-cov` is not installed on the dev box. Logic modules are heavily unit-tested, but the 70% gate is unverified.
- Emulator ignores clipboard (OSC 52), color queries, and text-area-size requests; Phase 2.
- Key -> VT encoding, selection, mouse reporting: Phase 2.
- No WSL profile yet (plan: later); no profile/config plumbing for scrollback size (Phase 5).
- Parity matrix untouched: scripting methods (`runCommand`, monitor toggles) only get `done` status once exposed over IPC (Phase 8).
