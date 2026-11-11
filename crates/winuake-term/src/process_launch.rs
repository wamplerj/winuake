//! Pure encoding of what `CreateProcessW` needs: a quoted command line and a sorted UTF-16 environment block.

use crate::pty::EnvironmentVariable;

/// Quotes one argument following the Windows `CommandLineToArgvW` rules.
pub fn quote_argument(argument: &str) -> String {
    let needs_quotes = argument.is_empty() || argument.contains([' ', '\t', '\n', '\u{b}', '"']);
    if !needs_quotes {
        return argument.to_owned();
    }

    let mut quoted = String::with_capacity(argument.len() + 2);
    quoted.push('"');
    let mut pending_backslashes = 0usize;
    for character in argument.chars() {
        match character {
            '\\' => pending_backslashes += 1,
            '"' => {
                quoted.extend(std::iter::repeat_n('\\', pending_backslashes * 2 + 1));
                quoted.push('"');
                pending_backslashes = 0;
            }
            other => {
                quoted.extend(std::iter::repeat_n('\\', pending_backslashes));
                quoted.push(other);
                pending_backslashes = 0;
            }
        }
    }
    quoted.extend(std::iter::repeat_n('\\', pending_backslashes * 2));
    quoted.push('"');
    quoted
}

pub fn build_command_line(program: &str, arguments: &[String]) -> String {
    let mut command_line = quote_argument(program);
    for argument in arguments {
        command_line.push(' ');
        command_line.push_str(&quote_argument(argument));
    }
    command_line
}

/// Windows environment names are case-insensitive: overrides replace base entries regardless of case.
pub fn merge_environment(base: &[EnvironmentVariable], overrides: &[EnvironmentVariable]) -> Vec<EnvironmentVariable> {
    let mut merged: Vec<EnvironmentVariable> = base
        .iter()
        .filter(|variable| !overrides.iter().any(|replacement| replacement.name.eq_ignore_ascii_case(&variable.name)))
        .cloned()
        .collect();
    merged.extend(overrides.iter().cloned());
    merged.sort_by_key(|variable| variable.name.to_ascii_uppercase());
    merged
}

/// `NAME=VALUE\0NAME=VALUE\0\0` as UTF-16, the layout `CreateProcessW` expects with `CREATE_UNICODE_ENVIRONMENT`.
pub fn encode_environment_block(variables: &[EnvironmentVariable]) -> Vec<u16> {
    let mut block = Vec::new();
    for variable in variables {
        block.extend(format!("{}={}", variable.name, variable.value).encode_utf16());
        block.push(0);
    }
    if block.is_empty() {
        block.push(0);
    }
    block.push(0);
    block
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variable(name: &str, value: &str) -> EnvironmentVariable {
        EnvironmentVariable { name: name.to_owned(), value: value.to_owned() }
    }

    #[test]
    fn given_simple_argument_when_quoting_then_it_is_unchanged() {
        assert_eq!(quote_argument("-NoLogo"), "-NoLogo");
    }

    #[test]
    fn given_empty_argument_when_quoting_then_empty_quotes_are_produced() {
        assert_eq!(quote_argument(""), "\"\"");
    }

    #[test]
    fn given_argument_with_spaces_when_quoting_then_it_is_wrapped() {
        assert_eq!(quote_argument(r"C:\Program Files\x"), r#""C:\Program Files\x""#);
    }

    #[test]
    fn given_embedded_quote_when_quoting_then_it_is_escaped() {
        assert_eq!(quote_argument(r#"say "hi""#), r#""say \"hi\"""#);
    }

    #[test]
    fn given_trailing_backslashes_in_quoted_argument_when_quoting_then_they_are_doubled() {
        assert_eq!(quote_argument(r"C:\my dir\"), r#""C:\my dir\\""#);
    }

    #[test]
    fn given_backslashes_before_a_quote_when_quoting_then_they_are_doubled_plus_escape() {
        assert_eq!(quote_argument(r#"a\"b"#), r#""a\\\"b""#);
    }

    #[test]
    fn given_program_and_arguments_when_building_command_line_then_each_part_is_quoted_and_space_separated() {
        let line = build_command_line(
            r"C:\Program Files\PowerShell\7\pwsh.exe",
            &["-NoLogo".to_owned(), "-Command".to_owned(), "echo hi".to_owned()],
        );
        assert_eq!(line, r#""C:\Program Files\PowerShell\7\pwsh.exe" -NoLogo -Command "echo hi""#);
    }

    #[test]
    fn given_override_with_different_case_when_merging_then_it_replaces_the_base_entry() {
        let merged = merge_environment(&[variable("Path", "old"), variable("HOME", "h")], &[variable("PATH", "new")]);
        assert_eq!(merged, vec![variable("HOME", "h"), variable("PATH", "new")]);
    }

    #[test]
    fn given_unsorted_variables_when_merging_then_result_is_sorted_case_insensitively() {
        let merged = merge_environment(&[variable("b", "1"), variable("A", "2")], &[variable("c", "3")]);
        let names: Vec<&str> = merged.iter().map(|variable| variable.name.as_str()).collect();
        assert_eq!(names, vec!["A", "b", "c"]);
    }

    #[test]
    fn given_variables_when_encoding_block_then_entries_are_nul_separated_and_double_nul_terminated() {
        let block = encode_environment_block(&[variable("A", "1"), variable("B", "2")]);
        assert_eq!(String::from_utf16(&block).unwrap(), "A=1\0B=2\0\0");
    }

    #[test]
    fn given_no_variables_when_encoding_block_then_it_is_still_double_nul_terminated() {
        assert_eq!(encode_environment_block(&[]), vec![0, 0]);
    }

    #[test]
    fn given_non_ascii_value_when_encoding_block_then_utf16_is_used() {
        let block = encode_environment_block(&[variable("NAME", "世")]);
        assert_eq!(String::from_utf16(&block).unwrap(), "NAME=世\0\0");
    }
}
