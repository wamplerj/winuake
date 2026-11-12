//! Log files and crash reports in `%LOCALAPPDATA%\Winuake\logs`.
//!
//! Logs roll daily and the last seven days are kept. A panic writes `crash-<time>.txt` with the message, location and a backtrace, then lets
//! the default handler run. Terminal content is never logged (the rule applies to every `tracing` call in the app).

use std::path::PathBuf;
use std::sync::Mutex;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use winuake_core::diagnostics::{CrashDetails, crash_file_name, crash_report, panic_message};

const LOG_FILES_KEPT: usize = 7;

/// Keeps the background log writer alive for the life of the process.
static LOG_GUARD: Mutex<Option<WorkerGuard>> = Mutex::new(None);

pub fn logs_directory() -> PathBuf {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("Winuake").join("logs")
}

/// Sets up logging to the console and a rolling file, and installs the crash-report panic hook.
pub fn initialize() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "winuake=info,winuake_term=info".into());
    let directory = logs_directory();
    let file_layer = std::fs::create_dir_all(&directory).ok().and_then(|()| {
        tracing_appender::rolling::Builder::new()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .filename_prefix("winuake")
            .filename_suffix("log")
            .max_log_files(LOG_FILES_KEPT)
            .build(&directory)
            .ok()
    });
    let registry = tracing_subscriber::registry().with(filter).with(tracing_subscriber::fmt::layer());
    match file_layer {
        Some(appender) => {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            if let Ok(mut slot) = LOG_GUARD.lock() {
                *slot = Some(guard);
            }
            registry.with(tracing_subscriber::fmt::layer().with_ansi(false).with_writer(writer)).init();
        }
        None => registry.init(),
    }
    install_panic_hook(directory);
}

/// Flushes pending log lines. The process ends with `TerminateProcess`, which would otherwise drop what the writer thread still holds.
pub fn shutdown() {
    if let Ok(mut slot) = LOG_GUARD.lock() {
        slot.take();
    }
}

fn install_panic_hook(directory: PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = panic_message(info.payload());
        let location =
            info.location().map(|location| format!("{}:{}:{}", location.file(), location.line(), location.column()));
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let unix_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0);
        let thread = std::thread::current();
        let report = crash_report(&CrashDetails {
            version: env!("CARGO_PKG_VERSION"),
            architecture: std::env::consts::ARCH,
            os: std::env::consts::OS,
            thread: thread.name().unwrap_or("unnamed"),
            message: &message,
            location: location.as_deref(),
            backtrace: &backtrace,
            unix_time,
        });
        tracing::error!(%message, location = location.as_deref().unwrap_or("unknown"), "winuake panicked");
        let _ = std::fs::create_dir_all(&directory);
        let _ = std::fs::write(directory.join(crash_file_name(unix_time)), report);
        previous(info);
    }));
}
