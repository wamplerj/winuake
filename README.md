# Winuake

Drop-down terminal for Windows 11, a port of [KDE Yakuake](https://github.com/KDE/yakuake). Rust + WinUI 3.

Status: Phase 0 (scaffold). See `docs/plans/winuake-port.md` for the phased plan and `docs/parity.md` for feature parity.

## Build

```
cargo build --workspace
cargo test --workspace
cargo xtask upstream-diff
```

The app itself (`crates/winuake`) runs on Windows 11 only. Logic crates build on any OS.

## License

GPL-2.0-or-later, same as Yakuake.
