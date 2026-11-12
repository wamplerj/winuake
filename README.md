# Winuake

Drop-down terminal for Windows 11, a port of [KDE Yakuake](https://github.com/KDE/yakuake). Rust + WinUI 3, for x64 and ARM64.

Press the hotkey (F12 if Windows allows it) and a terminal slides down from the top of the screen; press it again and it slides away. Sessions in tabs, split panes, Yakuake's skins, scripting from the command line.

## Install

Releases carry, for each of **x64** and **ARM64**:

| File | What |
|---|---|
| `winuake-<version>-win-<arch>-setup.exe` | Installer. Installs the matching Windows App SDK runtime, adds `winuake-cli` to your PATH, no admin needed. |
| `winuake-<version>-win-<arch>.msix` | MSIX package (signed with the release certificate; the CI test build uses a throwaway one). Needs the Windows App SDK 2.6 runtime. |
| `winuake-<version>-win-<arch>.zip` | Portable. Needs the Windows App SDK 2.6 runtime installed (the installer includes it). |
| `SHA256SUMS.txt` | Checksums. |

Requires Windows 11 22H2 or later. With winget (once the manifest is published): `winget install Winuake.Winuake`.

First start shows a dialog to pick the global hotkey. Settings: title-bar menu > Settings, or `%APPDATA%\Winuake\winuake.toml` (edits apply live). Logs and crash reports: `%LOCALAPPDATA%\Winuake\logs`.

## Using it

- `docs/parity.md`: every Yakuake setting, action, scripting method and skin key and its status.
- `docs/scripting.md`: `winuake-cli`, the named-pipe protocol, `qdbus` equivalents.
- Skins: Settings > Skins (install `.zip`/`.tar.gz`). Shortcuts: Settings > Shortcuts.

## Build

```
cargo xtask gen-bindings   # fetch Windows App SDK metadata and generate WinUI bindings (once)
cargo build --workspace
cargo test --workspace
cargo xtask package        # Windows only: release builds, zips, installers, MSIX, winget manifest for x64 and ARM64
cargo xtask upstream-diff  # Yakuake changes since the synced commit
```

The app (`crates/winuake`) runs on Windows 11 only; the logic crates build and test on any OS. ARM64 is cross-compiled (MSVC ARM64 tools plus `rustup target add aarch64-pc-windows-msvc`); packaging needs Inno Setup 6 and the Windows SDK. Plans and per-phase notes are in `docs/plans/`.

## License

GPL-2.0-or-later, same as Yakuake. Bundled skins come from Yakuake (see `assets/skins/NOTICE.md`); the app icon is the Yakuake default skin icon until Winuake has its own.
