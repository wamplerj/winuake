//! Finding skins on disk and turning their images into something XAML can show: PNGs by `file:///` URI, SVGs rasterized once at 3x into a
//! cache folder (so they stay sharp at 150-300% DPI).

use std::path::{Path, PathBuf};
use winuake_skin::directory::{self, DirectorySkinFiles, SkinSummary};
use winuake_skin::model::{Skin, SkinImage};
use winuake_skin::raster::svg_to_png;

const SVG_SCALE: u32 = 3;

/// `%APPDATA%\Winuake\skins`: skins the user installed.
pub fn user_skins_directory() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("Winuake").join("skins")
}

/// Skins shipped with the app: `skins` beside the executable, or `assets/skins` in the source tree when running from a build folder.
pub fn bundled_skins_directory() -> PathBuf {
    let executable = std::env::current_exe().unwrap_or_default();
    let mut folder = executable.parent();
    while let Some(directory) = folder {
        for candidate in [directory.join("skins"), directory.join("assets").join("skins")] {
            if candidate.join("default").join("title.skin").is_file() {
                return candidate;
            }
        }
        folder = directory.parent();
    }
    PathBuf::from("skins")
}

fn cache_directory() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Winuake")
        .join("skin-cache")
}

/// Skins available to choose from.
pub fn list() -> Vec<SkinSummary> {
    directory::discover(&user_skins_directory(), &bundled_skins_directory())
}

/// An image ready for XAML.
#[derive(Debug, Clone)]
pub struct ShownImage {
    pub uri: String,
    /// Size in DIPs at 1x.
    pub width: u32,
    pub height: u32,
}

/// A loaded skin and where its files live.
#[derive(Debug)]
pub struct AppSkin {
    pub skin: Skin,
    directory: PathBuf,
}

impl AppSkin {
    /// Loads skin `id`, user folder first. `None` when it is missing or not a skin.
    pub fn load(id: &str) -> Option<Self> {
        let directory = directory::locate(&user_skins_directory(), &bundled_skins_directory(), id)?;
        let files = DirectorySkinFiles::new(&directory);
        match Skin::load(id, &files) {
            Ok(loaded) => {
                for path in &loaded.missing_images {
                    tracing::warn!(skin = id, image = %path, "skin image is missing");
                }
                Some(Self { skin: loaded.skin, directory })
            }
            Err(error) => {
                tracing::warn!(skin = id, %error, "could not load skin");
                None
            }
        }
    }

    /// Prepares `image` for display. SVGs are rendered into the cache the first time (or when the source is newer).
    pub fn show(&self, image: &SkinImage) -> Option<ShownImage> {
        let source = self.directory.join(&image.path);
        let is_svg = image.path.to_ascii_lowercase().ends_with(".svg");
        if !is_svg {
            return Some(ShownImage { uri: file_uri(&source), width: image.size.width, height: image.size.height });
        }
        let cached = cache_directory()
            .join(&self.skin.id)
            .join(format!("{}@{SVG_SCALE}x.png", image.path.replace(['/', '\\'], "_")));
        let stale = match (
            std::fs::metadata(&source).and_then(|meta| meta.modified()),
            std::fs::metadata(&cached).and_then(|meta| meta.modified()),
        ) {
            (Ok(source_time), Ok(cached_time)) => source_time > cached_time,
            (_, Err(_)) => true,
            _ => false,
        };
        if stale && !render_svg(&source, &cached) {
            return None;
        }
        Some(ShownImage { uri: file_uri(&cached), width: image.size.width, height: image.size.height })
    }
}

/// An icon file for the skin list as a `file:///` URI. SVG icons are rendered once into the cache.
pub fn icon_uri(summary: &SkinSummary) -> Option<String> {
    // The default skin names icon.png but ships icon.svg, so try the vector twin of a missing icon.
    let icon = summary.icon.as_ref().and_then(|path| {
        [path.clone(), path.with_extension("svg")].into_iter().find(|candidate| candidate.is_file())
    })?;
    let icon = &icon;
    if !icon.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("svg")) {
        return Some(file_uri(icon));
    }
    let cached = cache_directory().join("_icons").join(format!("{}.png", summary.id));
    (cached.is_file() || render_svg(icon, &cached)).then(|| file_uri(&cached))
}

/// Removes cached renderings of a skin, so a reinstalled skin is rendered again.
pub fn clear_cache(id: &str) {
    let _ = std::fs::remove_dir_all(cache_directory().join(id));
}

fn render_svg(source: &Path, target: &Path) -> bool {
    let result = (|| -> Result<(), String> {
        let bytes = std::fs::read(source).map_err(|error| error.to_string())?;
        let rendered = svg_to_png(&bytes, SVG_SCALE).map_err(|error| error.to_string())?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::write(target, rendered.png).map_err(|error| error.to_string())
    })();
    if let Err(reason) = &result {
        tracing::warn!(source = %source.display(), %reason, "could not render skin svg");
    }
    result.is_ok()
}

/// `C:\a b\c.png` as `file:///C:/a%20b/c.png`.
pub fn file_uri(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let encoded: String = text
        .chars()
        .map(|character| match character {
            ' ' => "%20".to_owned(),
            '#' => "%23".to_owned(),
            '%' => "%25".to_owned(),
            '&' => "%26".to_owned(),
            other => other.to_string(),
        })
        .collect();
    format!("file:///{}", encoded.trim_start_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_a_windows_path_with_special_characters_when_making_a_uri_then_it_is_escaped() {
        assert_eq!(file_uri(Path::new(r"C:\Users\Jo Doe\a#b.png")), "file:///C:/Users/Jo%20Doe/a%23b.png");
    }
}
