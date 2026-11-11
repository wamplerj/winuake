//! Reading a skin out of an archive (tar, tar.gz or zip) without touching the disk, and checking it is safe to install.
//! Yakuake uses the archive's base directory name as the skin id; so does this.

use crate::model::{TABS_FILE, TITLE_FILE};
use std::io::Read;

const MAXIMUM_FILES: usize = 2_000;
const MAXIMUM_TOTAL_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveFile {
    /// Path inside the skin directory, `/` separated.
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinArchive {
    pub id: String,
    pub files: Vec<ArchiveFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveError {
    Unreadable(String),
    /// bzip2 archives are not supported.
    UnsupportedFormat,
    /// An entry would be written outside the skin directory.
    UnsafePath(String),
    /// Entries are not all inside one top-level directory.
    NoSingleSkinDirectory,
    /// `title.skin` or `tabs.skin` is missing.
    NotASkin,
    TooLarge,
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiveError::Unreadable(reason) => write!(formatter, "the archive cannot be read: {reason}"),
            ArchiveError::UnsupportedFormat => {
                formatter.write_str("bzip2 archives are not supported; use .tar.gz or .zip")
            }
            ArchiveError::UnsafePath(path) => write!(formatter, "the archive contains an unsafe path: {path}"),
            ArchiveError::NoSingleSkinDirectory => {
                formatter.write_str("the archive must hold exactly one skin directory")
            }
            ArchiveError::NotASkin => write!(formatter, "the archive has no {TITLE_FILE} and {TABS_FILE}"),
            ArchiveError::TooLarge => formatter.write_str("the archive is too large"),
        }
    }
}

impl std::error::Error for ArchiveError {}

struct RawEntry {
    path: String,
    bytes: Vec<u8>,
}

/// Reads and validates a skin archive. The format is detected from the content.
pub fn read_archive(bytes: &[u8]) -> Result<SkinArchive, ArchiveError> {
    let entries = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut tar_bytes = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .take(MAXIMUM_TOTAL_BYTES + 1)
            .read_to_end(&mut tar_bytes)
            .map_err(|error| ArchiveError::Unreadable(error.to_string()))?;
        read_tar(&tar_bytes)?
    } else if bytes.starts_with(b"PK") {
        read_zip(bytes)?
    } else if bytes.starts_with(b"BZh") {
        return Err(ArchiveError::UnsupportedFormat);
    } else {
        read_tar(bytes)?
    };
    into_skin(entries)
}

fn read_tar(bytes: &[u8]) -> Result<Vec<RawEntry>, ArchiveError> {
    let unreadable = |error: std::io::Error| ArchiveError::Unreadable(error.to_string());
    let mut archive = tar::Archive::new(bytes);
    let mut entries = Vec::new();
    let mut total = 0u64;
    for entry in archive.entries().map_err(unreadable)? {
        let mut entry = entry.map_err(unreadable)?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().map_err(unreadable)?.to_string_lossy().into_owned();
        total += entry.size();
        if total > MAXIMUM_TOTAL_BYTES || entries.len() >= MAXIMUM_FILES {
            return Err(ArchiveError::TooLarge);
        }
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents).map_err(unreadable)?;
        entries.push(RawEntry { path, bytes: contents });
    }
    Ok(entries)
}

fn read_zip(bytes: &[u8]) -> Result<Vec<RawEntry>, ArchiveError> {
    let unreadable = |error: String| ArchiveError::Unreadable(error);
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|error| unreadable(error.to_string()))?;
    let mut entries = Vec::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| unreadable(error.to_string()))?;
        if file.is_dir() {
            continue;
        }
        total += file.size();
        if total > MAXIMUM_TOTAL_BYTES || entries.len() >= MAXIMUM_FILES {
            return Err(ArchiveError::TooLarge);
        }
        let path = file.name().map(|name| name.into_owned()).map_err(|error| unreadable(error.to_string()))?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).map_err(|error| unreadable(error.to_string()))?;
        entries.push(RawEntry { path, bytes: contents });
    }
    Ok(entries)
}

/// Normalizes paths, rejects anything that could escape the skin directory, and splits off the skin id.
fn into_skin(entries: Vec<RawEntry>) -> Result<SkinArchive, ArchiveError> {
    let mut id: Option<String> = None;
    let mut files = Vec::new();
    for entry in entries {
        let normalized = entry.path.replace('\\', "/");
        let parts: Vec<&str> = normalized.split('/').filter(|part| !part.is_empty() && *part != ".").collect();
        let unsafe_path = normalized.starts_with('/')
            || parts.iter().any(|part| *part == ".." || part.contains(':'))
            || parts.is_empty();
        if unsafe_path {
            return Err(ArchiveError::UnsafePath(entry.path));
        }
        if parts.len() < 2 {
            return Err(ArchiveError::NoSingleSkinDirectory);
        }
        match &id {
            Some(existing) if existing != parts[0] => return Err(ArchiveError::NoSingleSkinDirectory),
            Some(_) => {}
            None => id = Some(parts[0].to_owned()),
        }
        files.push(ArchiveFile { path: parts[1..].join("/"), bytes: entry.bytes });
    }
    let id = id.ok_or(ArchiveError::NoSingleSkinDirectory)?;
    let has = |name: &str| files.iter().any(|file| file.path == name);
    if !has(TITLE_FILE) || !has(TABS_FILE) {
        return Err(ArchiveError::NotASkin);
    }
    Ok(SkinArchive { id, files })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tar_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for (path, contents) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            // `set_path` refuses unsafe names, so write the raw bytes for those cases.
            let raw = header.as_old_mut();
            raw.name[..path.len()].copy_from_slice(path.as_bytes());
            header.set_cksum();
            builder.append(&header, *contents).expect("appends");
        }
        builder.into_inner().expect("finishes")
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(bytes).expect("writes");
        encoder.finish().expect("finishes")
    }

    fn zip_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (path, contents) in entries {
            writer.start_file(*path, zip::write::SimpleFileOptions::default()).expect("starts");
            writer.write_all(contents).expect("writes");
        }
        writer.finish().expect("finishes").into_inner()
    }

    const GOOD: [(&str, &[u8]); 3] =
        [("mine/title.skin", b"[Border]"), ("mine/tabs.skin", b""), ("mine/title/back.png", b"png")];

    #[test]
    fn given_a_tar_when_reading_then_the_top_directory_is_the_id_and_paths_are_relative() {
        let skin = read_archive(&tar_with(&GOOD)).expect("reads");
        assert_eq!(skin.id, "mine");
        let paths: Vec<&str> = skin.files.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(paths, vec!["title.skin", "tabs.skin", "title/back.png"]);
        assert_eq!(skin.files[2].bytes, b"png");
    }

    #[test]
    fn given_a_gzip_compressed_tar_when_reading_then_it_is_detected_and_unpacked() {
        assert_eq!(read_archive(&gzip(&tar_with(&GOOD))).expect("reads").id, "mine");
    }

    #[test]
    fn given_a_zip_when_reading_then_it_is_detected_and_unpacked() {
        let skin = read_archive(&zip_with(&GOOD)).expect("reads");
        assert_eq!((skin.id.as_str(), skin.files.len()), ("mine", 3));
    }

    #[test]
    fn given_windows_style_paths_when_reading_then_they_are_normalized() {
        let archive = zip_with(&[("mine\\title.skin", b"x"), ("mine\\tabs.skin", b"y")]);
        let skin = read_archive(&archive).expect("reads");
        assert_eq!(skin.files[0].path, "title.skin");
    }

    #[test]
    fn given_a_path_that_climbs_out_when_reading_then_it_is_refused() {
        for bad in ["mine/../evil.txt", "/abs/title.skin", "mine/C:/x", "../mine/title.skin"] {
            let archive = zip_with(&[("mine/title.skin", b"x"), ("mine/tabs.skin", b"y"), (bad, b"z")]);
            assert_eq!(read_archive(&archive), Err(ArchiveError::UnsafePath(bad.to_owned())), "{bad}");
        }
    }

    #[test]
    fn given_files_in_several_top_directories_or_none_when_reading_then_it_is_refused() {
        let two = zip_with(&[("a/title.skin", b"x"), ("b/tabs.skin", b"y")]);
        assert_eq!(read_archive(&two), Err(ArchiveError::NoSingleSkinDirectory));
        let loose = zip_with(&[("title.skin", b"x"), ("tabs.skin", b"y")]);
        assert_eq!(read_archive(&loose), Err(ArchiveError::NoSingleSkinDirectory));
        let empty = zip_with(&[]);
        assert_eq!(read_archive(&empty), Err(ArchiveError::NoSingleSkinDirectory));
    }

    #[test]
    fn given_an_archive_without_both_skin_files_when_reading_then_it_is_not_a_skin() {
        let archive = zip_with(&[("mine/title.skin", b"x"), ("mine/readme.txt", b"y")]);
        assert_eq!(read_archive(&archive), Err(ArchiveError::NotASkin));
    }

    #[test]
    fn given_bzip2_or_garbage_when_reading_then_the_error_says_why() {
        assert_eq!(read_archive(b"BZh91AY&SY"), Err(ArchiveError::UnsupportedFormat));
        assert!(matches!(
            read_archive(b"this is not an archive at all, really"),
            Err(ArchiveError::NoSingleSkinDirectory | ArchiveError::Unreadable(_))
        ));
        assert!(matches!(read_archive(b"PK\x03\x04broken"), Err(ArchiveError::Unreadable(_))));
    }

    #[test]
    fn given_every_error_when_displayed_then_the_message_is_not_empty() {
        for error in [
            ArchiveError::Unreadable("x".into()),
            ArchiveError::UnsupportedFormat,
            ArchiveError::UnsafePath("p".into()),
            ArchiveError::NoSingleSkinDirectory,
            ArchiveError::NotASkin,
            ArchiveError::TooLarge,
        ] {
            assert!(!error.to_string().is_empty());
        }
    }
}
