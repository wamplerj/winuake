# Phase 0 — Scaffold + WinUI-from-Rust spike

Parent: `winuake-port.md`.

## Chunks (each committable)

1. **Scaffold** (done): workspace, crate stubs, configs, LICENSE (GPL-2.0), CI, `UPSTREAM.md`, `docs/parity.md` (generated from Yakuake kcfg/actions/D-Bus), `CLAUDE.md`, `README.md`.
2. **Toolchain verify**: install rustup, `cargo fmt/clippy/test` green on Linux.
3. **Bindings**: `cargo xtask gen-bindings` — download `Microsoft.WindowsAppSDK` NuGet, run `windows-bindgen` over Microsoft.UI.Xaml/Microsoft.UI winmd into `crates/winuake/src/bindings/` (gitignored).
4. **Spike** (Windows only): unpackaged bootstrap (`MddBootstrapInitialize2`), `Application::Start`, custom `IXamlMetadataProvider`, window with `TabView`, `Button`, `SwapChainPanel` cleared via D3D11.
5. **Measure + gate**: idle working set, startup time. Go/no-go on WinUI-from-Rust.

## Verification

- Chunks 1-2: fmt/clippy/test green.
- Chunks 3-5: must run on Windows 11 (VM or CI artifact); screenshot + working-set number recorded in `phase-0-scaffold_output.md`.
