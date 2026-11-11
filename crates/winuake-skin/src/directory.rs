//! Skins on disk: reading one directory, listing the installed and bundled skins, installing an archive, removing a skin.

use crate::archive::SkinArchive;
use crate::image_info::{png_size, svg_size};
use crate::model::{ImageSize, Skin, SkinFiles, TABS_FILE, TITLE_FILE};
use std::path::{Path, PathBuf};

/// A skin directory read through `std::fs`.
pub struct DirectorySkinFiles {
    root: PathBuf,
}

impl DirectorySkinFiles {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn path_of(&self, relative_path: &str) -> PathBuf {
        self.root.join(relative_path)
    }
}

impl SkinFiles for DirectorySkinFiles {
    fn read_text(&self, relative_path: &str) -> Option<String> {
        std::fs::read_to_string(self.path_of(relative_path)).ok()
    }

    fn image_size(&self, relative_path: &str) -> Option<ImageSize> {
        let bytes = std::fs::read(self.path_of(relative_path)).ok()?;
        if relative_path.to_ascii_lowercase().ends_with(".svg") {
            svg_size(&String::from_utf8_lossy(&bytes))
        } else {
            png_size(&bytes)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinLocation {
    /// Installed by the user; can be removed.
    User,
    /// Shipped with the app.
    Bundled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinSummary {
    pub id: String,
    pub name: String,
    pub author: String,
    /// Icon file, when the skin names one that exists.
    pub icon: Option<PathBuf>,
    pub location: SkinLocation,
    pub directory: PathBuf,
}

fn summarize(directory: &Path, location: SkinLocation) -> Option<SkinSummary> {
    let id = directory.file_name()?.to_string_lossy().into_owned();
    let files = DirectorySkinFiles::new(directory);
    let loaded = Skin::load(&id, &files).ok()?;
    let icon = loaded.skin.description.icon.map(|icon| files.path_of(&icon.path));
    Some(SkinSummary {
        id,
        name: loaded.skin.description.name,
        author: loaded.skin.description.author,
        icon,
        location,
        directory: directory.to_path_buf(),
    })
}

fn list_in(root: &Path, location: SkinLocation) -> Vec<SkinSummary> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter_map(|path| summarize(&path, location))
        .collect()
}

/// Every skin (a directory holding `title.skin` and `tabs.skin`) in the user and bundled folders, sorted by display name.
/// A user skin hides a bundled one with the same id.
pub fn discover(user_root: &Path, bundled_root: &Path) -> Vec<SkinSummary> {
    let mut skins = list_in(user_root, SkinLocation::User);
    for bundled in list_in(bundled_root, SkinLocation::Bundled) {
        if !skins.iter().any(|skin| skin.id == bundled.id) {
            skins.push(bundled);
        }
    }
    skins.sort_by_key(|skin| skin.name.to_lowercase());
    skins
}

/// Where to find skin `id`: the user folder first, then the bundled one.
pub fn locate(user_root: &Path, bundled_root: &Path, id: &str) -> Option<PathBuf> {
    [user_root, bundled_root]
        .into_iter()
        .map(|root| root.join(id))
        .find(|directory| directory.join(TITLE_FILE).is_file() && directory.join(TABS_FILE).is_file())
}

/// Writes a validated archive into `user_root/<id>`, replacing an earlier install of the same skin. Returns the directory.
pub fn install(archive: &SkinArchive, user_root: &Path) -> std::io::Result<PathBuf> {
    let target = user_root.join(&archive.id);
    if target.exists() {
        std::fs::remove_dir_all(&target)?;
    }
    for file in &archive.files {
        let path = target.join(&file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &file.bytes)?;
    }
    Ok(target)
}

/// Removes an installed skin. Bundled skins are never touched.
pub fn remove(user_root: &Path, id: &str) -> std::io::Result<()> {
    if id.is_empty() || id.contains(['/', '\\']) || id == ".." || id == "." {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a skin id"));
    }
    std::fs::remove_dir_all(user_root.join(id))
}
