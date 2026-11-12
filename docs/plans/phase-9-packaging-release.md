# Phase 9 — Packaging + release

Parent: `winuake-port.md`. Ships Winuake for **x64 (amd64) and ARM64 Windows 11**.

## Goals

- Release builds for `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc`, each with `winuake.exe`, `winuake-cli.exe`, the matching Windows App SDK bootstrap DLL, bundled skins, license and docs.
- Distribution: portable zip per architecture, an installer per architecture (Inno Setup; installs the matching Windows App SDK runtime when missing), a signed-ready MSIX per architecture, a winget manifest, SHA-256 checksums.
- Integration: application icon and version resource, manifest (DPI awareness, long paths), autostart (`HKCU\...\Run`, a setting), rolling log files and crash reports in `%LOCALAPPDATA%\Winuake\logs`.
- `docs/parity.md` filled in and reviewed for v1.0; `cargo xtask package` produces everything; CI workflow builds both architectures.

## Design

- **Architecture-aware build**: `build.rs` copies the bootstrap DLL for the target architecture (`win-x64` / `win-arm64`). Runtime installer and framework package come from the Windows App SDK runtime NuGet (`tools/MSIX/win10-x64|win10-arm64`).
- **Cross build**: ARM64 is built on x64 hosts with the MSVC ARM64 tools and the `aarch64-pc-windows-msvc` standard library. An ARM64 binary cannot run on the x64 test VM, so it is built, inspected (PE machine type), checked for the right bootstrap DLL, and unit-tested through the shared logic crates, but not launched. This is stated in the release notes until it is run on ARM hardware.
- **Packaged vs unpackaged**: unpackaged builds call the bootstrap; a packaged (MSIX) build already has the framework dependency and must skip it. `main` checks for package identity.
- **Logs and crashes**: `tracing` output also goes to `%LOCALAPPDATA%\Winuake\logs\winuake-<date>.log` (7 days kept); a panic hook writes `crash-<timestamp>.txt` (message, location, backtrace). Terminal content is never logged (existing rule).
- **Autostart**: setting `behavior.start_with_windows`, writes/removes the Run value for the unpackaged exe. For MSIX the manifest declares a startup task (user-enabled in Windows Settings).
- **Signing**: scripts accept a certificate (`WINUAKE_SIGN_PFX`); without one they make a throwaway self-signed certificate for local MSIX testing only. A real release needs the maintainer's code-signing certificate.

## Chunks

- **9a** Architecture-aware build, ARM64 toolchain on the VM, both targets build; PE check.
- **9b** Icon, manifest, version resource; log files and crash reports; autostart setting; package-identity check.
- **9c** `cargo xtask package`: zips, Inno installers, MSIX, checksums, winget manifest; CI workflow.
- **9d** Parity review, README, release notes; verify x64 install/uninstall on the VM.

## Verification

x64: install from the installer on the VM, launch, autostart entry appears/disappears, logs written, uninstall clean, MSIX installs (self-signed). ARM64: PE machine type `AA64`, bootstrap DLL is arm64, no x64 DLLs, installer/MSIX build.
