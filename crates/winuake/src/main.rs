//! Winuake: drop-down terminal for Windows 11 (WinUI 3). Port of KDE Yakuake.

#[cfg(windows)]
fn main() {
    // Phase 0 spike goes here: WinAppSDK bootstrap + WinUI 3 window.
    println!("winuake: spike not implemented yet");
}

#[cfg(not(windows))]
fn main() {
    eprintln!("winuake only runs on Windows 11");
    std::process::exit(1);
}
