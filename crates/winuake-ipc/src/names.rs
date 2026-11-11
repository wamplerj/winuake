//! Where the pipe lives.

/// Name of the pipe for a user: `\\.\pipe\winuake-<user>`. Characters Windows does not allow in pipe names become `_`.
pub fn pipe_path(user_name: &str) -> String {
    let safe: String = user_name
        .chars()
        .map(
            |character| {
                if character.is_alphanumeric() || matches!(character, '-' | '_' | '.') { character } else { '_' }
            },
        )
        .collect();
    format!(r"\\.\pipe\winuake-{}", if safe.is_empty() { "user" } else { &safe })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_a_user_name_when_building_the_pipe_path_then_it_is_prefixed() {
        assert_eq!(pipe_path("james"), r"\\.\pipe\winuake-james");
    }

    #[test]
    fn given_characters_pipes_cannot_hold_when_building_the_path_then_they_are_replaced() {
        assert_eq!(pipe_path(r"DOMAIN\Jo Doe"), r"\\.\pipe\winuake-DOMAIN_Jo_Doe");
        assert_eq!(pipe_path(""), r"\\.\pipe\winuake-user");
    }
}
