# CLAUDE.md

Winuake: Windows 11 port of KDE Yakuake (drop-down terminal). Rust + WinUI 3. GPL-2.0-or-later.

Master plan: `docs/plans/winuake-port.md`. Parity matrix: `docs/parity.md`. Upstream sync state: `UPSTREAM.md`.

## Layout

Cargo workspace, `crates/` by feature. Each crate maps to Yakuake source (see master plan table). Pure-logic crates (`core`, `config`, `skin`, `ipc`, logic part of `term`) must build and test on Linux. Windows-only code behind `#[cfg(windows)]`.

## Rules

- Ported code carries header `// Ported from yakuake app/<file>.cpp @<sha>`.
- Any upstream-parity change updates `docs/parity.md` and `UPSTREAM.md`.
- Descriptive names. No `Manager`/`Helper`/`Provider`. Records-like structs for multi-value returns, no tuples.
- Enums cross wire (IPC, config) as names, never ordinals.
- Logging: `tracing` structured fields only. Never log terminal content or secrets; content = length only. Info at action/IPC entry, Debug internals, `error!` with identifying values on caught errors.
- Tests: BDD-style names (`given_x_when_y_then_z`), no real ConPTY/filesystem/network, PTY behind `PtyBackend` trait. Min 70% coverage, target 90% on logic crates. No Arrange/Act/Assert comments.
- Before commit: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Tests passing != working. Run the app on Windows and exercise the feature.
- Plans in `docs/plans/`. Phase plan before work, `<name>_output.md` summary after.
