//! Chooses the default shell: PowerShell 7, then Windows PowerShell, then `cmd.exe`.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

const PREFERRED_SHELLS: &[&str] = &["pwsh.exe", "powershell.exe"];
const FALLBACK_SHELL: &str = "cmd.exe";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellCommand {
    pub program: String,
    pub arguments: Vec<String>,
}

pub trait ExecutableLocator {
    /// Full path of `executable_name` if it can be found, otherwise `None`.
    fn locate(&self, executable_name: &str) -> Option<PathBuf>;
}

pub struct ShellResolver<L: ExecutableLocator> {
    locator: L,
    command_interpreter_override: Option<String>,
}

impl<L: ExecutableLocator> ShellResolver<L> {
    /// `command_interpreter_override` is the `COMSPEC` value, used instead of plain `cmd.exe` when no PowerShell is installed.
    pub fn new(locator: L, command_interpreter_override: Option<String>) -> Self {
        Self { locator, command_interpreter_override }
    }

    pub fn resolve_default_shell(&self) -> ShellCommand {
        for name in PREFERRED_SHELLS {
            if let Some(path) = self.locator.locate(name) {
                tracing::debug!(shell = *name, "resolved default shell");
                return ShellCommand {
                    program: path.to_string_lossy().into_owned(),
                    arguments: vec!["-NoLogo".to_owned()],
                };
            }
        }
        let program = self.command_interpreter_override.clone().unwrap_or_else(|| FALLBACK_SHELL.to_owned());
        tracing::debug!(shell = FALLBACK_SHELL, "no PowerShell found, falling back to command interpreter");
        ShellCommand { program, arguments: Vec::new() }
    }
}

/// Searches the directories of a `PATH`-style variable.
pub struct PathVariableLocator {
    directories: Vec<PathBuf>,
}

impl PathVariableLocator {
    pub fn from_path_variable(path_variable: &OsStr) -> Self {
        Self { directories: std::env::split_paths(path_variable).collect() }
    }
}

impl ExecutableLocator for PathVariableLocator {
    fn locate(&self, executable_name: &str) -> Option<PathBuf> {
        self.directories
            .iter()
            .map(|directory| directory.join(executable_name))
            .find(|candidate| Path::is_file(candidate))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    struct InstalledExecutables(HashSet<&'static str>);

    impl ExecutableLocator for InstalledExecutables {
        fn locate(&self, executable_name: &str) -> Option<PathBuf> {
            self.0.contains(executable_name).then(|| PathBuf::from(r"C:\tools").join(executable_name))
        }
    }

    fn resolver(installed: &[&'static str], comspec: Option<&str>) -> ShellResolver<InstalledExecutables> {
        ShellResolver::new(InstalledExecutables(installed.iter().copied().collect()), comspec.map(str::to_owned))
    }

    #[test]
    fn given_pwsh_and_powershell_installed_when_resolving_then_pwsh_wins() {
        let shell = resolver(&["pwsh.exe", "powershell.exe"], None).resolve_default_shell();
        assert!(shell.program.ends_with("pwsh.exe"), "{shell:?}");
        assert_eq!(shell.arguments, vec!["-NoLogo"]);
    }

    #[test]
    fn given_only_windows_powershell_when_resolving_then_powershell_is_used() {
        let shell = resolver(&["powershell.exe"], None).resolve_default_shell();
        assert!(shell.program.ends_with("powershell.exe"), "{shell:?}");
    }

    #[test]
    fn given_no_powershell_and_comspec_when_resolving_then_comspec_is_used_without_arguments() {
        let shell = resolver(&[], Some(r"C:\Windows\System32\cmd.exe")).resolve_default_shell();
        assert_eq!(shell, ShellCommand { program: r"C:\Windows\System32\cmd.exe".to_owned(), arguments: vec![] });
    }

    #[test]
    fn given_nothing_installed_and_no_comspec_when_resolving_then_plain_cmd_is_used() {
        let shell = resolver(&[], None).resolve_default_shell();
        assert_eq!(shell.program, "cmd.exe");
    }

    #[test]
    fn given_path_variable_when_executable_exists_in_second_directory_then_it_is_found() {
        let first = std::env::temp_dir().join("winuake-shell-test-first-nonexistent");
        let second = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
        let this_executable = std::env::current_exe().unwrap().file_name().unwrap().to_string_lossy().into_owned();
        let joined = std::env::join_paths([first, second.clone()]).unwrap();

        let locator = PathVariableLocator::from_path_variable(&joined);

        assert_eq!(locator.locate(&this_executable), Some(second.join(&this_executable)));
        assert_eq!(locator.locate("definitely-not-installed.exe"), None);
    }
}
