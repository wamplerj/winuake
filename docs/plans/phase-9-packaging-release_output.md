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

## Not done / follow-ups

- Real signing certificate and the repository URL for the winget manifest (`OWNER` placeholders, `WINUAKE_RELEASE_BASE_URL`); installer itself is unsigned.
- A Winuake-specific icon.
- Crash report writing is unit-tested (formatting) but a real panic was not triggered on the VM.
- Three hung dev processes from Phase 3 remain on the VM (they block the single-instance lock, so the packaged test used `WINUAKE_ALLOW_MULTIPLE`); a VM reboot clears them.
- Version is still `0.1.0` in `Cargo.toml`; bump to 1.0.0 and tag when you decide to release.
