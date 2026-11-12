//! Text for crash reports and log housekeeping. The files themselves are written by the app.

/// Everything a crash report holds. Never includes terminal content.
pub struct CrashDetails<'a> {
    pub version: &'a str,
    pub architecture: &'a str,
    pub os: &'a str,
    pub thread: &'a str,
    pub message: &'a str,
    /// `file:line:column`, when known.
    pub location: Option<&'a str>,
    pub backtrace: &'a str,
    /// Seconds since the Unix epoch.
    pub unix_time: u64,
}

pub fn crash_report(details: &CrashDetails<'_>) -> String {
    let location = details.location.unwrap_or("unknown location");
    format!(
        "Winuake crash report\nversion: {}\narchitecture: {}\nos: {}\ntime (unix seconds): {}\nthread: {}\n\nWinuake stopped because of an internal error.\n{}\nat {}\n\nbacktrace:\n{}\n",
        details.version,
        details.architecture,
        details.os,
        details.unix_time,
        details.thread,
        details.message,
        location,
        details.backtrace
    )
}

/// File name for a crash report made at `unix_time`.
pub fn crash_file_name(unix_time: u64) -> String {
    format!("crash-{unix_time}.txt")
}

/// The panic payload as text (panics carry a `&str` or a `String`).
pub fn panic_message(payload: &dyn std::any::Any) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "panic with a non-text payload".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn details() -> CrashDetails<'static> {
        CrashDetails {
            version: "1.0.0",
            architecture: "aarch64",
            os: "windows",
            thread: "main",
            message: "index out of bounds",
            location: Some("src/x.rs:1:2"),
            backtrace: "0: frame",
            unix_time: 1_700_000_000,
        }
    }

    #[test]
    fn given_crash_details_when_formatting_then_the_report_names_version_architecture_message_and_location() {
        let report = crash_report(&details());
        for expected in
            ["version: 1.0.0", "architecture: aarch64", "index out of bounds", "src/x.rs:1:2", "0: frame", "1700000000"]
        {
            assert!(report.contains(expected), "missing {expected} in {report}");
        }
    }

    #[test]
    fn given_no_location_when_formatting_then_the_report_says_unknown() {
        let report = crash_report(&CrashDetails { location: None, ..details() });
        assert!(report.contains("unknown location"));
    }

    #[test]
    fn given_a_time_when_naming_the_file_then_it_sorts_by_time() {
        assert_eq!(crash_file_name(5), "crash-5.txt");
    }

    #[test]
    fn given_panic_payloads_when_extracting_the_message_then_text_and_strings_work_and_others_fall_back() {
        assert_eq!(panic_message(&"static text"), "static text");
        assert_eq!(panic_message(&String::from("owned text")), "owned text");
        assert_eq!(panic_message(&42u32), "panic with a non-text payload");
    }
}
