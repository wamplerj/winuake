//! Command-line client for a running Winuake: the `qdbus org.kde.yakuake ...` of Windows.
//!
//! ```text
//! winuake-cli <method> [argument ...]     any scripting method, for example: winuake-cli splitSessionLeftRight 0
//! winuake-cli toggle                      toggleWindowState
//! winuake-cli add-session                 addSession
//! winuake-cli run <command ...>           runCommand with the words joined by spaces
//! ```

use serde_json::Value;
use winuake_ipc::client::argument_from_text;

const USAGE: &str = "usage: winuake-cli <method> [argument ...]\n       winuake-cli toggle | add-session | run <command ...>\n\
Methods are Yakuake's scripting names (addSession, splitSessionLeftRight, runCommandInTerminal, sessionIdList, ...).";

#[derive(Debug, PartialEq)]
struct Call {
    method: String,
    params: Vec<Value>,
}

/// Turns command-line words into a call. `Err` carries a message for the user.
fn build_call(arguments: &[String]) -> Result<Call, String> {
    let Some((first, rest)) = arguments.split_first() else { return Err(USAGE.to_owned()) };
    match first.as_str() {
        "-h" | "--help" | "help" => Err(USAGE.to_owned()),
        "toggle" => Ok(Call { method: "toggleWindowState".to_owned(), params: Vec::new() }),
        "add-session" => Ok(Call { method: "addSession".to_owned(), params: Vec::new() }),
        "run" if rest.is_empty() => Err("run needs a command".to_owned()),
        "run" => Ok(Call { method: "runCommand".to_owned(), params: vec![Value::String(rest.join(" "))] }),
        method => {
            Ok(Call { method: method.to_owned(), params: rest.iter().map(|word| argument_from_text(word)).collect() })
        }
    }
}

#[cfg(windows)]
fn run(call: &Call) -> Result<Option<String>, String> {
    let path = winuake_ipc::names::pipe_path(&std::env::var("USERNAME").unwrap_or_default());
    let pipe = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|error| format!("Winuake is not running or cannot be reached ({path}): {error}"))?;
    let result = winuake_ipc::client::call(pipe, &call.method, &call.params).map_err(|error| error.to_string())?;
    Ok(winuake_ipc::client::display_result(&result))
}

#[cfg(not(windows))]
fn run(_call: &Call) -> Result<Option<String>, String> {
    Err("winuake-cli only works on Windows".to_owned())
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let call = match build_call(&arguments) {
        Ok(call) => call,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    match run(&call) {
        Ok(Some(text)) => println!("{text}"),
        Ok(None) => {}
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn words(text: &str) -> Vec<String> {
        text.split(' ').map(str::to_owned).collect()
    }

    #[test]
    fn given_a_method_with_arguments_when_building_the_call_then_numbers_and_flags_keep_their_types() {
        let call = build_call(&words("setSessionClosable 2 false")).expect("builds");
        assert_eq!(call, Call { method: "setSessionClosable".into(), params: vec![json!(2), json!(false)] });
    }

    #[test]
    fn given_the_shortcut_words_when_building_the_call_then_they_map_to_yakuake_methods() {
        assert_eq!(build_call(&words("toggle")).expect("builds").method, "toggleWindowState");
        assert_eq!(build_call(&words("add-session")).expect("builds").method, "addSession");
        let run = build_call(&words("run echo hello world")).expect("builds");
        assert_eq!(run, Call { method: "runCommand".into(), params: vec![json!("echo hello world")] });
    }

    #[test]
    fn given_no_arguments_or_help_or_a_bare_run_when_building_the_call_then_the_message_explains() {
        assert!(build_call(&[]).unwrap_err().contains("usage"));
        assert!(build_call(&words("--help")).unwrap_err().contains("usage"));
        assert!(build_call(&words("run")).unwrap_err().contains("needs a command"));
    }

    #[test]
    fn given_a_method_without_arguments_when_building_the_call_then_the_parameter_list_is_empty() {
        assert!(build_call(&words("sessionIdList")).expect("builds").params.is_empty());
    }
}
