//! Terminal profiles: what Konsole profiles are to Yakuake.

use serde::{Deserialize, Serialize};

pub const SCROLLBACK_LINES_MAX: u32 = 1_000_000;
pub const FONT_SIZE_RANGE: (f32, f32) = (6.0, 72.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CursorShapeSetting {
    #[default]
    Block,
    Underline,
    Bar,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    /// Program to run; unset uses the default shell (pwsh, then powershell, then cmd).
    pub program: Option<String>,
    pub arguments: Vec<String>,
    /// Starting directory; unset uses the user profile folder.
    pub working_directory: Option<String>,
    pub font_family: String,
    pub font_size_points: f32,
    /// Name of a built-in or imported color scheme.
    pub color_scheme: String,
    pub scrollback_lines: u32,
    pub cursor_shape: CursorShapeSetting,
    pub cursor_blink: bool,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: "Default".to_owned(),
            program: None,
            arguments: Vec::new(),
            working_directory: None,
            font_family: "Cascadia Mono".to_owned(),
            font_size_points: 11.0,
            color_scheme: "Campbell".to_owned(),
            scrollback_lines: 10_000,
            cursor_shape: CursorShapeSetting::Block,
            cursor_blink: true,
        }
    }
}

impl Profile {
    pub fn clamped(mut self) -> Self {
        self.font_size_points = self.font_size_points.clamp(FONT_SIZE_RANGE.0, FONT_SIZE_RANGE.1);
        self.scrollback_lines = self.scrollback_lines.min(SCROLLBACK_LINES_MAX);
        self
    }

    /// The WSL distribution this profile starts (`wsl.exe -d <name>`), for mapping the shell's Unix paths to Windows ones.
    pub fn wsl_distribution(&self) -> Option<&str> {
        let program = self.program.as_deref()?;
        let file_name = program.rsplit(['\\', '/']).next()?;
        if !file_name.eq_ignore_ascii_case("wsl.exe") && !file_name.eq_ignore_ascii_case("wsl") {
            return None;
        }
        let position = self.arguments.iter().position(|argument| argument == "-d" || argument == "--distribution")?;
        self.arguments.get(position + 1).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_the_default_profile_then_it_matches_the_current_terminal_look() {
        let profile = Profile::default();
        assert_eq!(profile.font_family, "Cascadia Mono");
        assert_eq!(profile.font_size_points, 11.0);
        assert_eq!(profile.scrollback_lines, 10_000);
        assert_eq!(profile.color_scheme, "Campbell");
    }

    #[test]
    fn given_extreme_values_when_clamping_then_they_are_forced_into_range() {
        let profile = Profile { font_size_points: 500.0, scrollback_lines: u32::MAX, ..Profile::default() }.clamped();
        assert_eq!(profile.font_size_points, 72.0);
        assert_eq!(profile.scrollback_lines, SCROLLBACK_LINES_MAX);
        assert_eq!(Profile { font_size_points: 1.0, ..Profile::default() }.clamped().font_size_points, 6.0);
    }

    fn wsl_profile(program: &str, arguments: &[&str]) -> Profile {
        Profile {
            program: Some(program.to_owned()),
            arguments: arguments.iter().map(|argument| (*argument).to_owned()).collect(),
            ..Profile::default()
        }
    }

    #[test]
    fn given_a_wsl_profile_when_asking_for_its_distribution_then_the_dash_d_name_is_returned() {
        let profile = wsl_profile(r"C:\Windows\System32\wsl.exe", &["-d", "Ubuntu"]);
        assert_eq!(profile.wsl_distribution(), Some("Ubuntu"));
        assert_eq!(wsl_profile("wsl.exe", &["--distribution", "Debian"]).wsl_distribution(), Some("Debian"));
    }

    #[test]
    fn given_a_profile_that_is_not_wsl_when_asking_for_its_distribution_then_there_is_none() {
        assert_eq!(wsl_profile(r"C:\Program Files\Git\bin\bash.exe", &["-d", "x"]).wsl_distribution(), None);
        assert_eq!(wsl_profile("wsl.exe", &[]).wsl_distribution(), None);
        assert_eq!(Profile::default().wsl_distribution(), None);
    }
}
