//! Looks at this PC for installed shells (see `winuake_config::shells` for the rules).

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;
use winuake_config::profile::Profile;
use winuake_config::shells::{ShellEnvironment, detect_shells, parse_wsl_list};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const WSL_TIMEOUT: Duration = Duration::from_secs(5);

struct ThisPc;

impl ShellEnvironment for ThisPc {
    fn file_exists(&self, path: &str) -> bool {
        Path::new(path).is_file()
    }

    fn environment_variable(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }

    fn find_on_path(&self, executable: &str) -> Option<String> {
        let path = std::env::var_os("PATH")?;
        std::env::split_paths(&path)
            .map(|directory| directory.join(executable))
            .find(|candidate| candidate.is_file())
            .map(|found| found.display().to_string())
    }

    fn wsl_distributions(&self) -> Vec<String> {
        use std::os::windows::process::CommandExt;
        let (sender, receiver) = mpsc::channel();
        // `wsl.exe --list` can stall when the WSL service is cold, so it runs on a thread with a time limit.
        std::thread::spawn(move || {
            let output = Command::new("wsl.exe")
                .args(["--list", "--quiet"])
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .output();
            let _ = sender.send(output);
        });
        match receiver.recv_timeout(WSL_TIMEOUT) {
            Ok(Ok(output)) if output.status.success() => parse_wsl_list(&output.stdout),
            _ => Vec::new(),
        }
    }
}

/// Profiles for the shells found on this PC.
pub fn detect() -> Vec<Profile> {
    let found = detect_shells(&ThisPc);
    tracing::info!(count = found.len(), "detected shells");
    found
}
