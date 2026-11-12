# Winuake 1.0 release notes (draft)

Windows 11 port of KDE Yakuake in Rust and WinUI 3. Builds for **x64** and **ARM64**.

## Highlights

Drop-down window with slide animation, global hotkey, tray icon and multi-monitor placement; tabs and nested splits with all of Yakuake's 45 actions and shortcuts; Yakuake skins (default Breeze, Legacy, Plastik) with install and remove; translucency; activity and silence notifications; keyboard-input lock; focus follows mouse; auto open at the top edge; live-reloaded TOML settings with a settings window and shortcut editor; profiles with color-scheme import (Windows Terminal, Konsole); scripting over a named pipe with `winuake-cli`; installer, MSIX and portable zip.

## Known limitations

- **ARM64 has not been run on ARM hardware.** It is cross-compiled and verified as an ARM64 build (machine type, matching bootstrap DLL, installer and MSIX produced); the shared logic is tested on x64. Report ARM-specific problems.
- Release signing: executables, installers and MSIX are signed when a code-signing certificate is supplied (`WINUAKE_SIGN_PFX`, see `docs/releasing.md`). Packages built without one are not trusted by Windows; SmartScreen may warn until the certificate has reputation.
- New terminals do not inherit the previous terminal's working directory (Windows shells do not report it without OSC 9;9 support).
- Skins: no rounded-corner window masks, compact tab bars are not inset, PNG skins look soft at high DPI.
- Display hot-plug handling is untested on real multi-monitor changes; virtual-desktop pinning has no public API ("show on all desktops" moves the window to the current desktop instead).
- Notifications are Windows tray balloons, not branded toasts, and need the tray icon enabled.
- Packaged (MSIX) autostart uses a startup task the user enables in Windows Settings; the Winuake setting only controls the unpackaged build.
