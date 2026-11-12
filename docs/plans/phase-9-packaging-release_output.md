# Phase 9 output — Packaging + release

## Done

- **9a** Architecture-aware build: `build.rs` copies the bootstrap DLL for the target architecture. ARM64 toolchain set up on the x64 test VM (MSVC ARM64 tools, `aarch64-pc-windows-msvc` std). Both `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc` release builds succeed; PE machine types and the bootstrap DLL architecture are checked (and enforced by the packager).
- **9b** Application icon (rendered from the default skin's SVG into `assets/icon/winuake.ico`), version resource and manifest (PerMonitorV2 DPI, long paths, UTF-8) embedded; rolling log files and crash reports in `%LOCALAPPDATA%\Winuake\logs`; `start_with_windows` setting (Run key); package-identity check so a packaged build skips the bootstrapper.
- **9c** `cargo xtask package [--arch x64|arm64|all] [--version X]` builds and produces, per architecture: portable zip, Inno Setup installer (bundles and installs the matching Windows App SDK runtime, optional PATH entry, uninstall cleanup), signed MSIX (framework dependency, startup task, `winuake-cli` execution alias), plus `SHA256SUMS.txt` and a winget manifest. CI: ARM64 clippy/build added to `ci.yml`; `release.yml` packages both architectures on tags.
- **9d** Parity matrix reviewed (two partial rows, both explained), README rewritten, release notes drafted.

## Verified on the Win11 VM (x64)

Installer: silent install, files, PATH entry, `winuake-cli` works from the installed copy, silent uninstall removes files and (after a fix) the PATH entry. MSIX: signed with a throwaway certificate, installed, launched from `WindowsApps`, log says "running packaged", scripting pipe answered through the `winuake-cli` alias, autostart correctly skipped; removed afterwards. Autostart Run key written and removed live; log file written; version resource present.

## ARM64

Built, PE machine type `AA64`, bootstrap DLL arm64, installer and MSIX produced and signed. **Not run**: the VM is x64 and cannot execute ARM64 binaries.

## Open items, follow-up pass

- **Icon**: Winuake now has its own icon (`assets/icon/winuake.svg`, rendered by `cargo xtask gen-icon` into `winuake.ico` and the MSIX logos).
- **Signing**: with `WINUAKE_SIGN_PFX` the packager signs `winuake.exe`, `winuake-cli.exe`, each installer and each MSIX (SHA-256, RFC 3161 timestamp). Verified on the VM with the throwaway certificate: all four signed artifact kinds carry a signature and timestamp for both architectures (status "untrusted root", as expected for a self-signed certificate). A real certificate is still needed for a public release; `docs/releasing.md` explains the steps.
- **winget manifest**: `--repo OWNER/NAME` (or `WINUAKE_REPOSITORY`) fills the installer and locale URLs; the default is `wamplerj/winuake`. Checked in a test package run.
- **Crash report**: `WINUAKE_TEST_PANIC=1 winuake.exe` panics at startup; the VM produced `crash-<time>.txt` with version, architecture, message and location, and the log line.
- Minor: the manifest already sets DPI awareness, so the runtime call's harmless "access denied" is no longer printed.

## Still open

- Real code-signing certificate.
- Run ARM64 on ARM hardware.
- Version bump and tag (maintainer).
- The VM was rebooted and signed in again: the hung Phase 3 processes are gone. The real global hotkey (Ctrl+`, since F12 and Win+` are taken by Windows) toggled the window, and the signed 0.9.1 MSIX installed, ran as the only instance (no `WINUAKE_ALLOW_MULTIPLE`), answered `winuake-cli`, and was removed again.
