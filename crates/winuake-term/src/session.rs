//! One terminal: a shell in a pseudo-terminal, its emulated screen, and the events it raises (Yakuake `Terminal`).

use crate::activity::{ActivityMonitor, MonitorNotification};
use crate::emulator::{DEFAULT_SCROLLBACK_LINES, TerminalEmulator, TerminalEvent};
use crate::pty::{ExitStatus, PtyBackend, PtyControl, PtyError, PtySpawnRequest, TerminalSize};
use std::io::{self, Read, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

const READ_BUFFER_BYTES: usize = 16 * 1024;

pub use winuake_core::ids::TerminalId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    /// New output was processed; the screen may have changed.
    OutputReceived,
    TitleChanged(String),
    TitleReset,
    BellRung,
    /// The shell exited. This is always the last event.
    Exited(ExitStatus),
}

struct SessionShared {
    emulator: Mutex<TerminalEmulator>,
    writer: Mutex<Box<dyn Write + Send>>,
    control: Arc<dyn PtyControl>,
    monitor: Mutex<ActivityMonitor>,
}

pub struct TerminalSession {
    id: TerminalId,
    shared: Arc<SessionShared>,
    /// Behind a mutex so the session can be shared (`Sync`) between the UI thread and the thread pumping events.
    events: Mutex<Receiver<SessionEvent>>,
}

impl TerminalSession {
    pub fn start(id: TerminalId, backend: &dyn PtyBackend, request: &PtySpawnRequest) -> Result<Self, PtyError> {
        tracing::info!(terminal_id = %id, columns = request.size.columns, rows = request.size.rows, "starting terminal session");
        let connection = backend
            .spawn(request)
            .inspect_err(|error| tracing::error!(terminal_id = %id, %error, "failed to spawn shell"))?;

        let shared = Arc::new(SessionShared {
            emulator: Mutex::new(TerminalEmulator::new(request.size, DEFAULT_SCROLLBACK_LINES)),
            writer: Mutex::new(connection.writer),
            control: connection.control,
            monitor: Mutex::new(ActivityMonitor::default()),
        });
        let (sender, events) = mpsc::channel();

        let reader_shared = Arc::clone(&shared);
        thread::Builder::new()
            .name(format!("terminal-{id}-reader"))
            .spawn(move || read_until_exit(id, connection.reader, &reader_shared, &sender))
            .map_err(|error| PtyError::OperationFailed {
                operation: "spawn reader thread",
                reason: error.to_string(),
            })?;

        Ok(Self { id, shared, events: Mutex::new(events) })
    }

    pub fn id(&self) -> TerminalId {
        self.id
    }

    /// Writes raw bytes (already VT-encoded keystrokes or pasted text) to the shell.
    pub fn send_input(&self, bytes: &[u8]) -> io::Result<()> {
        tracing::info!(terminal_id = %self.id, length = bytes.len(), "sending input to terminal");
        write_to_shell(&self.shared, bytes)
    }

    /// Types `command` and presses Enter (Yakuake `runCommand`).
    pub fn run_command(&self, command: &str) -> io::Result<()> {
        tracing::info!(terminal_id = %self.id, length = command.len(), "running command in terminal");
        let mut line = Vec::with_capacity(command.len() + 1);
        line.extend_from_slice(command.as_bytes());
        line.push(b'\r');
        write_to_shell(&self.shared, &line)
    }

    pub fn resize(&self, size: TerminalSize) -> Result<(), PtyError> {
        tracing::info!(terminal_id = %self.id, columns = size.columns, rows = size.rows, "resizing terminal");
        self.shared.control.resize(size)?;
        lock(&self.shared.emulator).resize(size);
        Ok(())
    }

    pub fn kill(&self) -> Result<(), PtyError> {
        tracing::info!(terminal_id = %self.id, "killing terminal process");
        self.shared.control.kill()
    }

    /// Runs `inspect` with the emulated screen locked; keep it short, output processing waits on it.
    pub fn with_emulator<R>(&self, inspect: impl FnOnce(&TerminalEmulator) -> R) -> R {
        inspect(&lock(&self.shared.emulator))
    }

    /// Runs `change` with the emulated screen locked for writing (scrolling, selection).
    pub fn update_emulator<R>(&self, change: impl FnOnce(&mut TerminalEmulator) -> R) -> R {
        change(&mut lock(&self.shared.emulator))
    }

    pub fn try_next_event(&self) -> Option<SessionEvent> {
        lock(&self.events).try_recv().ok()
    }

    /// Waits up to `timeout` for the next event. `None` on timeout or after the final `Exited` event was consumed.
    pub fn next_event_timeout(&self, timeout: Duration) -> Option<SessionEvent> {
        lock(&self.events).recv_timeout(timeout).ok()
    }

    pub fn set_monitoring_activity(&self, enabled: bool) {
        tracing::info!(terminal_id = %self.id, enabled, "setting activity monitoring");
        lock(&self.shared.monitor).set_watching_activity(enabled);
    }

    pub fn set_monitoring_silence(&self, enabled: bool) {
        tracing::info!(terminal_id = %self.id, enabled, "setting silence monitoring");
        lock(&self.shared.monitor).set_watching_silence(enabled);
    }

    pub fn is_monitoring_activity(&self) -> bool {
        lock(&self.shared.monitor).is_watching_activity()
    }

    pub fn is_monitoring_silence(&self) -> bool {
        lock(&self.shared.monitor).is_watching_silence()
    }

    pub fn acknowledge_monitor(&self) {
        lock(&self.shared.monitor).acknowledge();
    }

    pub fn poll_monitor_notification(&self, now: Instant) -> Option<MonitorNotification> {
        lock(&self.shared.monitor).poll(now)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        tracing::debug!(terminal_id = %self.id, "dropping terminal session, killing process");
        let _ = self.shared.control.kill();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn write_to_shell(shared: &SessionShared, bytes: &[u8]) -> io::Result<()> {
    let mut writer = lock(&shared.writer);
    writer.write_all(bytes)?;
    writer.flush()
}

fn read_until_exit(
    id: TerminalId,
    mut reader: Box<dyn Read + Send>,
    shared: &SessionShared,
    events: &Sender<SessionEvent>,
) {
    tracing::debug!(terminal_id = %id, "reader thread started");
    let mut buffer = vec![0u8; READ_BUFFER_BYTES];
    loop {
        let length = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(length) => length,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                tracing::debug!(terminal_id = %id, %error, "terminal output closed");
                break;
            }
        };

        let terminal_events = lock(&shared.emulator).advance(&buffer[..length]);
        lock(&shared.monitor).record_output(Instant::now());
        for terminal_event in terminal_events {
            match terminal_event {
                TerminalEvent::TitleChanged(title) => {
                    let _ = events.send(SessionEvent::TitleChanged(title));
                }
                TerminalEvent::TitleReset => {
                    let _ = events.send(SessionEvent::TitleReset);
                }
                TerminalEvent::BellRung => {
                    let _ = events.send(SessionEvent::BellRung);
                }
                TerminalEvent::ResponseForShell(bytes) => {
                    if let Err(error) = write_to_shell(shared, &bytes) {
                        tracing::warn!(terminal_id = %id, %error, length = bytes.len(), "failed to answer terminal query");
                    }
                }
            }
        }
        let _ = events.send(SessionEvent::OutputReceived);
    }

    let status = shared.control.wait_for_exit();
    tracing::info!(terminal_id = %id, exit_code = ?status.code, "terminal process exited");
    let _ = events.send(SessionEvent::Exited(status));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pty::PtyConnection;
    use std::sync::atomic::{AtomicBool, Ordering};

    const WAIT: Duration = Duration::from_secs(5);

    struct FakeShell {
        output: Option<Sender<Vec<u8>>>,
        written: Arc<Mutex<Vec<u8>>>,
        resizes: Arc<Mutex<Vec<TerminalSize>>>,
        killed: Arc<AtomicBool>,
        spawned_with: Arc<Mutex<Vec<PtySpawnRequest>>>,
    }

    impl FakeShell {
        fn print(&self, text: &str) {
            self.output.as_ref().expect("shell still running").send(text.as_bytes().to_vec()).unwrap();
        }

        fn exit(&mut self) {
            self.output = None;
        }

        fn received(&self) -> Vec<u8> {
            self.written.lock().unwrap().clone()
        }
    }

    struct ChannelReader {
        chunks: Receiver<Vec<u8>>,
        pending: Vec<u8>,
    }

    impl Read for ChannelReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if self.pending.is_empty() {
                match self.chunks.recv() {
                    Ok(chunk) => self.pending = chunk,
                    Err(_) => return Ok(0),
                }
            }
            let length = self.pending.len().min(buffer.len());
            buffer[..length].copy_from_slice(&self.pending[..length]);
            self.pending.drain(..length);
            Ok(length)
        }
    }

    struct SharedBufferWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedBufferWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct FakeControl {
        resizes: Arc<Mutex<Vec<TerminalSize>>>,
        killed: Arc<AtomicBool>,
        exit_code: i32,
    }

    impl PtyControl for FakeControl {
        fn resize(&self, size: TerminalSize) -> Result<(), PtyError> {
            self.resizes.lock().unwrap().push(size);
            Ok(())
        }

        fn wait_for_exit(&self) -> ExitStatus {
            ExitStatus { code: Some(self.exit_code) }
        }

        fn kill(&self) -> Result<(), PtyError> {
            self.killed.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    struct FakeBackend {
        connection: Mutex<Option<PtyConnection>>,
        spawned_with: Arc<Mutex<Vec<PtySpawnRequest>>>,
    }

    impl PtyBackend for FakeBackend {
        fn spawn(&self, request: &PtySpawnRequest) -> Result<PtyConnection, PtyError> {
            self.spawned_with.lock().unwrap().push(request.clone());
            self.connection.lock().unwrap().take().ok_or(PtyError::SpawnFailed {
                program: request.program.clone(),
                reason: "program not found".to_owned(),
            })
        }
    }

    fn request() -> PtySpawnRequest {
        PtySpawnRequest {
            program: "fake-shell".to_owned(),
            arguments: vec!["-x".to_owned()],
            working_directory: None,
            environment: vec![],
            size: TerminalSize::new(40, 10),
        }
    }

    fn start_fake_session(exit_code: i32) -> (TerminalSession, FakeShell) {
        let (output, chunks) = mpsc::channel();
        let written = Arc::new(Mutex::new(Vec::new()));
        let resizes = Arc::new(Mutex::new(Vec::new()));
        let killed = Arc::new(AtomicBool::new(false));
        let spawned_with = Arc::new(Mutex::new(Vec::new()));
        let connection = PtyConnection {
            reader: Box::new(ChannelReader { chunks, pending: Vec::new() }),
            writer: Box::new(SharedBufferWriter(Arc::clone(&written))),
            control: Arc::new(FakeControl { resizes: Arc::clone(&resizes), killed: Arc::clone(&killed), exit_code }),
        };
        let backend = FakeBackend { connection: Mutex::new(Some(connection)), spawned_with: Arc::clone(&spawned_with) };
        let session = TerminalSession::start(TerminalId(7), &backend, &request()).unwrap();
        (session, FakeShell { output: Some(output), written, resizes, killed, spawned_with })
    }

    fn wait_for(session: &TerminalSession, wanted: impl Fn(&SessionEvent) -> bool) -> SessionEvent {
        loop {
            let event = session.next_event_timeout(WAIT).expect("timed out waiting for session event");
            if wanted(&event) {
                return event;
            }
        }
    }

    fn wait_for_output(session: &TerminalSession) {
        wait_for(session, |event| *event == SessionEvent::OutputReceived);
    }

    #[test]
    fn given_new_session_when_started_then_backend_receives_the_spawn_request() {
        let (_session, shell) = start_fake_session(0);
        assert_eq!(*shell.spawned_with.lock().unwrap(), vec![request()]);
    }

    #[test]
    fn given_shell_output_when_received_then_screen_shows_it_and_output_event_is_raised() {
        let (session, shell) = start_fake_session(0);
        shell.print("PS C:\\> dir\r\nfile.txt");
        wait_for_output(&session);
        assert_eq!(session.with_emulator(|screen| screen.screen_text()), "PS C:\\> dir\nfile.txt");
    }

    #[test]
    fn given_title_sequence_when_received_then_title_changed_event_is_raised() {
        let (session, shell) = start_fake_session(0);
        shell.print("\x1b]0;build output\x07");
        assert_eq!(
            wait_for(&session, |event| matches!(event, SessionEvent::TitleChanged(_))),
            SessionEvent::TitleChanged("build output".to_owned())
        );
    }

    #[test]
    fn given_bell_when_received_then_bell_event_is_raised() {
        let (session, shell) = start_fake_session(0);
        shell.print("\x07");
        wait_for(&session, |event| *event == SessionEvent::BellRung);
    }

    #[test]
    fn given_terminal_query_when_received_then_the_answer_is_written_back_to_the_shell() {
        let (session, shell) = start_fake_session(0);
        shell.print("ab\x1b[6n");
        wait_for_output(&session);
        assert_eq!(shell.received(), b"\x1b[1;3R");
    }

    #[test]
    fn given_input_when_sent_then_shell_receives_exact_bytes() {
        let (session, shell) = start_fake_session(0);
        session.send_input(b"\x1b[A").unwrap();
        assert_eq!(shell.received(), b"\x1b[A");
    }

    #[test]
    fn given_command_when_run_then_shell_receives_it_followed_by_enter() {
        let (session, shell) = start_fake_session(0);
        session.run_command("git status").unwrap();
        assert_eq!(shell.received(), b"git status\r");
    }

    #[test]
    fn given_resize_when_requested_then_pty_and_screen_both_resize() {
        let (session, shell) = start_fake_session(0);
        session.resize(TerminalSize::new(100, 30)).unwrap();
        assert_eq!(*shell.resizes.lock().unwrap(), vec![TerminalSize::new(100, 30)]);
        assert_eq!(session.with_emulator(|screen| (screen.columns(), screen.rows())), (100, 30));
    }

    #[test]
    fn given_shell_exits_when_output_closes_then_exited_event_carries_the_exit_code() {
        let (session, mut shell) = start_fake_session(3);
        shell.exit();
        assert_eq!(
            wait_for(&session, |event| matches!(event, SessionEvent::Exited(_))),
            SessionEvent::Exited(ExitStatus { code: Some(3) })
        );
    }

    #[test]
    fn given_output_before_exit_when_shell_exits_then_output_is_processed_before_exited() {
        let (session, mut shell) = start_fake_session(0);
        shell.print("bye");
        shell.exit();
        wait_for(&session, |event| matches!(event, SessionEvent::Exited(_)));
        assert_eq!(session.with_emulator(|screen| screen.row_text(0)), "bye");
    }

    #[test]
    fn given_backend_cannot_start_the_program_when_starting_then_error_is_returned() {
        let backend = FakeBackend { connection: Mutex::new(None), spawned_with: Arc::default() };
        let result = TerminalSession::start(TerminalId(1), &backend, &request());
        assert!(matches!(result, Err(PtyError::SpawnFailed { .. })));
    }

    #[test]
    fn given_session_when_dropped_then_the_process_is_killed() {
        let (session, shell) = start_fake_session(0);
        drop(session);
        assert!(shell.killed.load(Ordering::SeqCst));
    }

    #[test]
    fn given_activity_monitoring_when_output_arrives_then_a_notification_can_be_polled() {
        let (session, shell) = start_fake_session(0);
        session.set_monitoring_activity(true);
        shell.print("x");
        wait_for_output(&session);
        assert_eq!(session.poll_monitor_notification(Instant::now()), Some(MonitorNotification::ActivityDetected));
        assert!(session.is_monitoring_activity());
        assert!(!session.is_monitoring_silence());
    }

    #[test]
    fn given_silence_monitoring_when_output_stops_then_silence_is_reported_after_the_timeout() {
        let (session, shell) = start_fake_session(0);
        session.set_monitoring_silence(true);
        shell.print("x");
        wait_for_output(&session);
        assert_eq!(session.poll_monitor_notification(Instant::now()), None);
        assert_eq!(
            session.poll_monitor_notification(Instant::now() + Duration::from_secs(11)),
            Some(MonitorNotification::SilenceDetected)
        );
    }
}
