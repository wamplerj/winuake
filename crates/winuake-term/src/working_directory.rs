//! Follows the shell's current folder, as far as the shell says. Shells that report it (oh-my-posh, starship, a prompt snippet) send an
//! OSC 7 (`ESC ] 7 ; file://host/path`) or a Windows Terminal style OSC 9;9 (`ESC ] 9 ; 9 ; path`) at each prompt. Windows has no other way
//! to ask a console process where it is, so a shell that stays quiet leaves this empty.

use std::path::PathBuf;

/// More than this many bytes inside one unfinished OSC are not a folder report; the scanner gives up on it.
const MAX_OSC_BYTES: usize = 4096;
const ESCAPE: u8 = 0x1b;
const BELL: u8 = 0x07;

/// A folder as the shell wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportedDirectory {
    /// A Windows path (`C:\Users\James`, or a `\\wsl.localhost\...` share).
    Windows(PathBuf),
    /// A path in a Unix-style world: WSL, or the `/c/Users/...` form MSYS shells (Git Bash) use.
    Posix(String),
}

impl ReportedDirectory {
    /// The folder as a Windows path. A Unix path of a WSL shell (`wsl_distribution`) maps into that distribution's share; otherwise
    /// only the MSYS drive form (`/c/Users`) can be mapped, and anything else gives `None`.
    pub fn to_windows_path(&self, wsl_distribution: Option<&str>) -> Option<PathBuf> {
        match self {
            ReportedDirectory::Windows(path) => Some(path.clone()),
            ReportedDirectory::Posix(path) => match wsl_distribution {
                Some(distribution) => {
                    Some(PathBuf::from(format!(r"\\wsl.localhost\{distribution}{}", path.replace('/', r"\"))))
                }
                None => msys_drive_path(path),
            },
        }
    }
}

/// `/c/Users/James` to `C:\Users\James`.
fn msys_drive_path(path: &str) -> Option<PathBuf> {
    let rest = path.strip_prefix('/')?;
    let (drive, remainder) = match rest.split_once('/') {
        Some((drive, remainder)) => (drive, remainder),
        None => (rest, ""),
    };
    let mut letters = drive.chars();
    let letter = letters.next().filter(char::is_ascii_alphabetic)?;
    if letters.next().is_some() {
        return None;
    }
    Some(PathBuf::from(format!(r"{}:\{}", letter.to_ascii_uppercase(), remainder.replace('/', r"\"))))
}

/// Scans shell output for folder reports. Feed it every chunk in order; a report may be split across chunks.
#[derive(Debug, Default)]
pub struct WorkingDirectoryTracker {
    /// The unfinished tail of the output: an OSC that has begun but not ended.
    carry: Vec<u8>,
    current: Option<ReportedDirectory>,
}

enum Ending {
    /// The sequence ends at `body_length`, and its terminator takes `terminator_length` bytes.
    Terminated { body_length: usize, terminator_length: usize },
    /// Another escape sequence began inside this one, at `body_length`: this one was abandoned.
    Abandoned { body_length: usize },
    /// More output is needed to tell.
    Incomplete,
}

impl WorkingDirectoryTracker {
    pub fn current(&self) -> Option<&ReportedDirectory> {
        self.current.as_ref()
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        if self.carry.is_empty() && !bytes.contains(&ESCAPE) {
            return;
        }
        self.carry.extend_from_slice(bytes);
        loop {
            let Some(start) = self.carry.windows(2).position(|pair| pair == [ESCAPE, b']']) else {
                // A lone escape at the very end may be the start of an OSC whose `]` comes with the next chunk.
                let keep_escape = self.carry.last() == Some(&ESCAPE);
                self.carry.clear();
                if keep_escape {
                    self.carry.push(ESCAPE);
                }
                return;
            };
            let body_start = start + 2;
            match find_ending(&self.carry[body_start..]) {
                Ending::Terminated { body_length, terminator_length } => {
                    let body = self.carry[body_start..body_start + body_length].to_vec();
                    self.carry.drain(..body_start + body_length + terminator_length);
                    self.apply(&body);
                }
                Ending::Abandoned { body_length } => {
                    self.carry.drain(..body_start + body_length);
                }
                Ending::Incomplete => {
                    self.carry.drain(..start);
                    if self.carry.len() > MAX_OSC_BYTES {
                        self.carry.clear();
                    }
                    return;
                }
            }
        }
    }

    fn apply(&mut self, body: &[u8]) {
        let text = String::from_utf8_lossy(body);
        let Some((code, rest)) = text.split_once(';') else { return };
        let reported = match code {
            "7" => parse_file_url(rest),
            "9" => rest.strip_prefix("9;").and_then(parse_windows_terminal_path),
            _ => None,
        };
        if reported.is_some() {
            self.current = reported;
        }
    }
}

fn find_ending(body: &[u8]) -> Ending {
    for (index, byte) in body.iter().enumerate() {
        match *byte {
            BELL => return Ending::Terminated { body_length: index, terminator_length: 1 },
            ESCAPE => {
                return match body.get(index + 1) {
                    Some(b'\\') => Ending::Terminated { body_length: index, terminator_length: 2 },
                    Some(_) => Ending::Abandoned { body_length: index },
                    None => Ending::Incomplete,
                };
            }
            _ => {}
        }
    }
    Ending::Incomplete
}

/// OSC 9;9: the path, possibly in quotes.
fn parse_windows_terminal_path(text: &str) -> Option<ReportedDirectory> {
    let path = text.trim().trim_matches('"');
    if path.is_empty() { None } else { Some(ReportedDirectory::Windows(PathBuf::from(path))) }
}

/// OSC 7: `file://host/path` with percent-encoding. A `/C:/...` path is a Windows one.
fn parse_file_url(url: &str) -> Option<ReportedDirectory> {
    let after_scheme = url.strip_prefix("file://")?;
    let path_start = after_scheme.find('/')?;
    let path = percent_decode(&after_scheme[path_start..]);
    let windows_form = path.strip_prefix('/').filter(|rest| {
        let bytes = rest.as_bytes();
        bytes.len() >= 2
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && bytes.get(2).is_none_or(|next| *next == b'/')
    });
    match windows_form {
        Some(rest) => Some(ReportedDirectory::Windows(PathBuf::from(rest.replace('/', r"\")))),
        None if path.len() > 1 || path == "/" => Some(ReportedDirectory::Posix(path)),
        None => None,
    }
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = |digit: u8| (digit as char).to_digit(16);
        if bytes[index] == b'%'
            && let (Some(high), Some(low)) =
                (bytes.get(index + 1).and_then(|d| hex(*d)), bytes.get(index + 2).and_then(|d| hex(*d)))
        {
            decoded.push((high * 16 + low) as u8);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn windows(path: &str) -> Option<ReportedDirectory> {
        Some(ReportedDirectory::Windows(PathBuf::from(path)))
    }

    fn tracked(chunks: &[&[u8]]) -> Option<ReportedDirectory> {
        let mut tracker = WorkingDirectoryTracker::default();
        for chunk in chunks {
            tracker.feed(chunk);
        }
        tracker.current().cloned()
    }

    #[test]
    fn given_an_osc_7_with_a_drive_path_when_fed_then_the_windows_folder_is_decoded() {
        assert_eq!(tracked(&[b"\x1b]7;file://HOST/C:/Users/James%20Wampler\x07"]), windows(r"C:\Users\James Wampler"));
    }

    #[test]
    fn given_an_osc_7_ended_by_string_terminator_when_fed_then_the_folder_is_found() {
        assert_eq!(tracked(&[b"\x1b]7;file:///D:/work\x1b\\"]), windows(r"D:\work"));
    }

    #[test]
    fn given_an_osc_9_9_with_a_quoted_path_when_fed_then_the_quotes_are_dropped() {
        assert_eq!(tracked(&[b"\x1b]9;9;\"C:\\Program Files\"\x07"]), windows(r"C:\Program Files"));
    }

    #[test]
    fn given_a_report_split_across_chunks_when_fed_then_it_is_still_found() {
        assert_eq!(tracked(&[b"prompt\x1b]7;file:///C:/Us", b"ers/James\x07$ "]), windows(r"C:\Users\James"));
    }

    #[test]
    fn given_the_escape_and_bracket_in_different_chunks_when_fed_then_the_report_is_still_found() {
        assert_eq!(tracked(&[b"text\x1b", b"]9;9;C:\\x\x07"]), windows(r"C:\x"));
    }

    #[test]
    fn given_two_reports_when_fed_then_the_later_one_wins() {
        assert_eq!(tracked(&[b"\x1b]9;9;C:\\a\x07 ... \x1b]9;9;C:\\b\x07"]), windows(r"C:\b"));
    }

    #[test]
    fn given_other_osc_sequences_when_fed_then_the_folder_stays_unknown() {
        assert_eq!(tracked(&[b"\x1b]0;window title\x07\x1b]8;;https://example.com\x07"]), None);
    }

    #[test]
    fn given_plain_output_when_fed_then_the_folder_stays_unknown() {
        assert_eq!(tracked(&[b"PS C:\\Users\\James> dir\r\n"]), None);
    }

    #[test]
    fn given_an_abandoned_osc_followed_by_a_report_when_fed_then_the_report_is_found() {
        assert_eq!(tracked(&[b"\x1b]7;file:///C:/old\x1b[31m\x1b]9;9;C:\\new\x07"]), windows(r"C:\new"));
    }

    #[test]
    fn given_an_endless_osc_when_fed_then_the_scanner_gives_up_and_later_reports_work() {
        let mut tracker = WorkingDirectoryTracker::default();
        tracker.feed(b"\x1b]7;");
        tracker.feed(&vec![b'x'; MAX_OSC_BYTES + 10]);
        tracker.feed(b"\x1b]9;9;C:\\after\x07");
        assert_eq!(tracker.current().cloned(), windows(r"C:\after"));
    }

    #[test]
    fn given_a_unix_path_when_reported_then_it_stays_unix_until_mapped() {
        let reported = tracked(&[b"\x1b]7;file://box/home/me/src\x07"]).expect("reported");
        assert_eq!(reported, ReportedDirectory::Posix("/home/me/src".to_owned()));
    }

    #[test]
    fn given_a_unix_path_and_a_wsl_distribution_when_mapping_then_it_points_into_the_distribution_share() {
        let reported = ReportedDirectory::Posix("/home/me/src".to_owned());
        assert_eq!(
            reported.to_windows_path(Some("Ubuntu")),
            Some(PathBuf::from(r"\\wsl.localhost\Ubuntu\home\me\src"))
        );
    }

    #[test]
    fn given_an_msys_drive_path_without_wsl_when_mapping_then_it_becomes_a_drive_path() {
        let reported = ReportedDirectory::Posix("/c/Users/James".to_owned());
        assert_eq!(reported.to_windows_path(None), Some(PathBuf::from(r"C:\Users\James")));
        assert_eq!(ReportedDirectory::Posix("/d".to_owned()).to_windows_path(None), Some(PathBuf::from(r"D:\")));
    }

    #[test]
    fn given_a_unix_path_that_is_not_a_drive_and_no_wsl_when_mapping_then_there_is_no_windows_path() {
        assert_eq!(ReportedDirectory::Posix("/home/me".to_owned()).to_windows_path(None), None);
        assert_eq!(ReportedDirectory::Posix("/usr/lib".to_owned()).to_windows_path(None), None);
    }

    #[test]
    fn given_a_windows_folder_when_mapping_then_it_is_returned_as_is() {
        let reported = ReportedDirectory::Windows(PathBuf::from(r"C:\x"));
        assert_eq!(reported.to_windows_path(Some("Ubuntu")), Some(PathBuf::from(r"C:\x")));
    }
}
