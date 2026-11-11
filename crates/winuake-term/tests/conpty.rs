//! Integration tests against the real Windows ConPTY. They launch `cmd.exe`, so they only exist on Windows.
#![cfg(windows)]

use std::time::{Duration, Instant};
use winuake_term::conpty::ConPtyBackend;
use winuake_term::pty::{PtyError, PtySpawnRequest, TerminalSize};
use winuake_term::session::{SessionEvent, TerminalId, TerminalSession};

const PATIENCE: Duration = Duration::from_secs(20);

fn cmd_request(arguments: &[&str]) -> PtySpawnRequest {
    PtySpawnRequest {
        program: "cmd.exe".to_owned(),
        arguments: arguments.iter().map(|argument| (*argument).to_owned()).collect(),
        working_directory: None,
        environment: vec![],
        size: TerminalSize::new(80, 25),
    }
}

fn start(request: &PtySpawnRequest) -> TerminalSession {
    TerminalSession::start(TerminalId(1), &ConPtyBackend, request).expect("cmd.exe starts in a pseudo console")
}

/// Consumes events until the shell exits and returns its exit code.
fn wait_for_exit_code(session: &TerminalSession) -> Option<i32> {
    let deadline = Instant::now() + PATIENCE;
    while Instant::now() < deadline {
        if let Some(SessionEvent::Exited(status)) = session.next_event_timeout(Duration::from_millis(200)) {
            return status.code;
        }
    }
    panic!(
        "shell did not exit within {PATIENCE:?}; screen was:\n{}",
        session.with_emulator(|screen| screen.screen_text())
    );
}

fn wait_for_screen_containing(session: &TerminalSession, wanted: &str) {
    let deadline = Instant::now() + PATIENCE;
    while Instant::now() < deadline {
        if session.with_emulator(|screen| screen.screen_text()).contains(wanted) {
            return;
        }
        session.next_event_timeout(Duration::from_millis(100));
    }
    panic!("screen never contained {wanted:?}; it was:\n{}", session.with_emulator(|screen| screen.screen_text()));
}

#[test]
fn given_a_command_that_prints_when_it_finishes_then_its_output_is_on_screen_and_exit_code_is_zero() {
    let session = start(&cmd_request(&["/c", "echo", "hello-from-conpty"]));

    let exit_code = wait_for_exit_code(&session);

    assert_eq!(exit_code, Some(0));
    assert!(session.with_emulator(|screen| screen.screen_text()).contains("hello-from-conpty"));
}

#[test]
fn given_a_command_that_fails_when_it_finishes_then_the_exit_code_is_reported() {
    let session = start(&cmd_request(&["/c", "exit", "7"]));

    assert_eq!(wait_for_exit_code(&session), Some(7));
}

#[test]
fn given_an_interactive_shell_when_a_command_is_run_then_its_output_appears_and_exit_ends_the_session() {
    let session = start(&cmd_request(&[]));

    session.run_command("echo marker-42").unwrap();
    wait_for_screen_containing(&session, "marker-42");
    session.run_command("exit 3").unwrap();

    assert_eq!(wait_for_exit_code(&session), Some(3));
}

#[test]
fn given_a_resized_terminal_when_the_shell_reports_its_size_then_it_matches() {
    let session = start(&cmd_request(&[]));
    wait_for_screen_containing(&session, ">");

    session.resize(TerminalSize::new(100, 30)).unwrap();
    session.run_command("mode con").unwrap();

    wait_for_screen_containing(&session, "100");
    assert_eq!(session.with_emulator(|screen| (screen.columns(), screen.rows())), (100, 30));
}

#[test]
fn given_environment_overrides_when_the_shell_prints_a_variable_then_the_override_is_visible() {
    let mut request = cmd_request(&["/c", "echo", "%WINUAKE_TEST_VALUE%"]);
    request.environment = vec![winuake_term::pty::EnvironmentVariable {
        name: "WINUAKE_TEST_VALUE".to_owned(),
        value: "from-test".to_owned(),
    }];
    let session = start(&request);

    wait_for_exit_code(&session);

    assert!(session.with_emulator(|screen| screen.screen_text()).contains("from-test"));
}

#[test]
fn given_a_working_directory_when_the_shell_prints_it_then_it_is_the_requested_one() {
    let mut request = cmd_request(&["/c", "cd"]);
    request.working_directory = Some(std::env::temp_dir());
    let session = start(&request);

    wait_for_exit_code(&session);

    let expected = std::env::temp_dir().to_string_lossy().trim_end_matches('\\').to_lowercase();
    let screen = session.with_emulator(|screen| screen.screen_text()).to_lowercase();
    assert!(screen.contains(&expected), "expected {expected:?} in {screen:?}");
}

#[test]
fn given_a_long_running_shell_when_killed_then_the_session_ends() {
    let session = start(&cmd_request(&[]));
    wait_for_screen_containing(&session, ">");

    session.kill().unwrap();

    wait_for_exit_code(&session);
}

#[test]
fn given_a_program_that_does_not_exist_when_starting_then_spawning_fails() {
    let mut request = cmd_request(&[]);
    request.program = "definitely-not-a-real-program.exe".to_owned();

    let result = TerminalSession::start(TerminalId(2), &ConPtyBackend, &request);

    assert!(matches!(result, Err(PtyError::SpawnFailed { .. })));
}

#[test]
fn given_windows_powershell_when_a_command_prints_then_its_output_arrives_through_the_terminal() {
    let request = PtySpawnRequest {
        program: "powershell.exe".to_owned(),
        arguments: ["-NoLogo", "-NoProfile", "-Command", "Write-Output 'ps-output-ok'"]
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect(),
        working_directory: None,
        environment: vec![],
        size: TerminalSize::new(120, 30),
    };
    let session = start(&request);

    assert_eq!(wait_for_exit_code(&session), Some(0));

    assert!(session.with_emulator(|screen| screen.screen_text()).contains("ps-output-ok"));
}
