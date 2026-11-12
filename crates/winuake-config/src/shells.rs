//! Finding the shells installed on this PC, so the user can add profiles for them in one click (Windows PowerShell, PowerShell 7,
//! Command Prompt, Git Bash, WSL distributions). Looking at the machine goes through `ShellEnvironment`, so the rules are testable.

use crate::profile::Profile;

/// What detection needs to know about the machine.
pub trait ShellEnvironment {
    fn file_exists(&self, path: &str) -> bool;
    fn environment_variable(&self, name: &str) -> Option<String>;
    /// Full path of `executable` when it is found on `PATH`.
    fn find_on_path(&self, executable: &str) -> Option<String>;
    /// Names of installed WSL distributions (empty when WSL is absent).
    fn wsl_distributions(&self) -> Vec<String>;
}

fn profile(name: &str, program: &str, arguments: &[&str]) -> Profile {
    Profile {
        name: name.to_owned(),
        program: Some(program.to_owned()),
        arguments: arguments.iter().map(|argument| (*argument).to_owned()).collect(),
        ..Profile::default()
    }
}

fn join(directory: &str, file: &str) -> String {
    format!("{}\\{}", directory.trim_end_matches(['\\', '/']), file)
}

/// Profiles for the shells found on this machine, in a stable order.
pub fn detect_shells(environment: &dyn ShellEnvironment) -> Vec<Profile> {
    let mut found = Vec::new();
    let variable = |name: &str| environment.environment_variable(name).filter(|value| !value.is_empty());

    let system_root = variable("SystemRoot").unwrap_or_else(|| r"C:\Windows".to_owned());
    let windows_powershell = join(&system_root, r"System32\WindowsPowerShell\v1.0\powershell.exe");
    if environment.file_exists(&windows_powershell) {
        found.push(profile("Windows PowerShell", &windows_powershell, &["-NoLogo"]));
    }

    let mut pwsh_candidates = Vec::new();
    if let Some(on_path) = environment.find_on_path("pwsh.exe") {
        pwsh_candidates.push(on_path);
    }
    if let Some(program_files) = variable("ProgramFiles") {
        pwsh_candidates.push(join(&program_files, r"PowerShell\7\pwsh.exe"));
    }
    if let Some(local) = variable("LocalAppData") {
        pwsh_candidates.push(join(&local, r"Microsoft\WindowsApps\pwsh.exe"));
    }
    if let Some(pwsh) = pwsh_candidates.into_iter().find(|path| environment.file_exists(path)) {
        found.push(profile("PowerShell 7", &pwsh, &["-NoLogo"]));
    }

    let command_prompt = variable("ComSpec").unwrap_or_else(|| join(&system_root, r"System32\cmd.exe"));
    if environment.file_exists(&command_prompt) {
        found.push(profile("Command Prompt", &command_prompt, &[]));
    }

    let mut bash_candidates = Vec::new();
    for variable_name in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Some(directory) = variable(variable_name) {
            bash_candidates.push(join(&directory, r"Git\bin\bash.exe"));
        }
    }
    if let Some(local) = variable("LocalAppData") {
        bash_candidates.push(join(&local, r"Programs\Git\bin\bash.exe"));
    }
    if let Some(bash) = bash_candidates.into_iter().find(|path| environment.file_exists(path)) {
        let mut git_bash = profile("Git Bash", &bash, &["--login", "-i"]);
        // Git for Windows' bash starts in the home folder it chooses itself when no directory is forced.
        git_bash.working_directory = variable("USERPROFILE");
        found.push(git_bash);
    }

    let wsl = join(&system_root, r"System32\wsl.exe");
    if environment.file_exists(&wsl) {
        for distribution in environment.wsl_distributions() {
            found.push(profile(&format!("WSL: {distribution}"), &wsl, &["-d", &distribution]));
        }
    }
    found
}

/// Reads the output of `wsl.exe --list --quiet`: UTF-16, one distribution per line. Docker's helper distributions are ignored.
pub fn parse_wsl_list(output: &[u8]) -> Vec<String> {
    let units: Vec<u16> = output.chunks_exact(2).map(|pair| u16::from_le_bytes([pair[0], pair[1]])).collect();
    let text = String::from_utf16_lossy(&units);
    text.lines()
        .map(|line| {
            line.trim_matches(|character: char| {
                character == '\u{feff}' || character.is_whitespace() || character == '\0'
            })
        })
        .filter(|line| !line.is_empty() && !line.starts_with("docker-desktop"))
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    #[derive(Default)]
    struct Fake {
        files: HashSet<String>,
        variables: HashMap<String, String>,
        path: HashMap<String, String>,
        distributions: Vec<String>,
    }

    impl ShellEnvironment for Fake {
        fn file_exists(&self, path: &str) -> bool {
            self.files.contains(path)
        }
        fn environment_variable(&self, name: &str) -> Option<String> {
            self.variables.get(name).cloned()
        }
        fn find_on_path(&self, executable: &str) -> Option<String> {
            self.path.get(executable).cloned()
        }
        fn wsl_distributions(&self) -> Vec<String> {
            self.distributions.clone()
        }
    }

    fn typical() -> Fake {
        let mut fake = Fake::default();
        for (name, value) in [
            ("SystemRoot", r"C:\Windows"),
            ("ComSpec", r"C:\Windows\System32\cmd.exe"),
            ("ProgramFiles", r"C:\Program Files"),
            ("LocalAppData", r"C:\Users\Jo\AppData\Local"),
            ("USERPROFILE", r"C:\Users\Jo"),
        ] {
            fake.variables.insert(name.to_owned(), value.to_owned());
        }
        for file in [
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
            r"C:\Windows\System32\cmd.exe",
            r"C:\Program Files\PowerShell\7\pwsh.exe",
            r"C:\Program Files\Git\bin\bash.exe",
            r"C:\Windows\System32\wsl.exe",
        ] {
            fake.files.insert(file.to_owned());
        }
        fake.distributions = vec!["Ubuntu".into(), "Debian".into()];
        fake
    }

    fn names(profiles: &[Profile]) -> Vec<&str> {
        profiles.iter().map(|profile| profile.name.as_str()).collect()
    }

    #[test]
    fn given_a_typical_machine_when_detecting_then_every_installed_shell_gets_a_profile_in_order() {
        let found = detect_shells(&typical());
        assert_eq!(
            names(&found),
            vec!["Windows PowerShell", "PowerShell 7", "Command Prompt", "Git Bash", "WSL: Ubuntu", "WSL: Debian"]
        );
    }

    #[test]
    fn given_git_bash_when_detecting_then_it_starts_as_a_login_interactive_shell_in_the_home_folder() {
        let found = detect_shells(&typical());
        let git = found.iter().find(|profile| profile.name == "Git Bash").expect("found");
        assert_eq!(git.program.as_deref(), Some(r"C:\Program Files\Git\bin\bash.exe"));
        assert_eq!(git.arguments, vec!["--login", "-i"]);
        assert_eq!(git.working_directory.as_deref(), Some(r"C:\Users\Jo"));
    }

    #[test]
    fn given_wsl_distributions_when_detecting_then_each_starts_with_wsl_dash_d() {
        let found = detect_shells(&typical());
        let debian = found.iter().find(|profile| profile.name == "WSL: Debian").expect("found");
        assert_eq!(debian.program.as_deref(), Some(r"C:\Windows\System32\wsl.exe"));
        assert_eq!(debian.arguments, vec!["-d", "Debian"]);
    }

    #[test]
    fn given_pwsh_on_the_path_when_detecting_then_that_copy_is_used() {
        let mut fake = typical();
        fake.path.insert("pwsh.exe".into(), r"D:\Tools\pwsh.exe".into());
        fake.files.insert(r"D:\Tools\pwsh.exe".into());
        let found = detect_shells(&fake);
        let pwsh = found.iter().find(|profile| profile.name == "PowerShell 7").expect("found");
        assert_eq!(pwsh.program.as_deref(), Some(r"D:\Tools\pwsh.exe"));
    }

    #[test]
    fn given_a_bare_machine_when_detecting_then_only_what_exists_is_listed() {
        let mut fake = Fake::default();
        fake.files.insert(r"C:\Windows\System32\cmd.exe".into());
        assert_eq!(names(&detect_shells(&fake)), vec!["Command Prompt"]);
        assert!(detect_shells(&Fake::default()).is_empty());
    }

    #[test]
    fn given_git_installed_per_user_or_in_x86_when_detecting_then_it_is_still_found() {
        let mut fake = Fake::default();
        fake.variables.insert("LocalAppData".into(), r"C:\Users\Jo\AppData\Local".into());
        fake.files.insert(r"C:\Users\Jo\AppData\Local\Programs\Git\bin\bash.exe".into());
        assert_eq!(names(&detect_shells(&fake)), vec!["Git Bash"]);
        let mut x86 = Fake::default();
        x86.variables.insert("ProgramFiles(x86)".into(), r"C:\Program Files (x86)".into());
        x86.files.insert(r"C:\Program Files (x86)\Git\bin\bash.exe".into());
        assert_eq!(names(&detect_shells(&x86)), vec!["Git Bash"]);
    }

    #[test]
    fn given_wsl_without_the_executable_when_detecting_then_no_distribution_profiles_appear() {
        let mut fake = typical();
        fake.files.remove(r"C:\Windows\System32\wsl.exe");
        assert!(!names(&detect_shells(&fake)).iter().any(|name| name.starts_with("WSL")));
    }

    fn utf16(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        bytes
    }

    #[test]
    fn given_wsl_list_output_when_parsing_then_distribution_names_come_back_without_docker_helpers() {
        let output = utf16("Ubuntu\r\ndocker-desktop\r\nDebian\r\n\r\n");
        assert_eq!(parse_wsl_list(&output), vec!["Ubuntu", "Debian"]);
        assert!(parse_wsl_list(&[]).is_empty());
    }
}
