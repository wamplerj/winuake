//! Disk behavior of skin discovery, install and removal, on a throwaway directory. The only tests here that touch the filesystem.

use std::path::PathBuf;
use winuake_skin::archive::{ArchiveFile, SkinArchive};
use winuake_skin::directory::{SkinLocation, discover, install, locate, remove};

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("winuake-skin-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("creates");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write_skin(root: &std::path::Path, id: &str, name: &str) {
    let directory = root.join(id);
    std::fs::create_dir_all(&directory).expect("creates");
    std::fs::write(directory.join("title.skin"), format!("[Description]\nSkin={name}\nAuthor=A\n")).expect("writes");
    std::fs::write(directory.join("tabs.skin"), "").expect("writes");
}

fn archive(id: &str, title: &str) -> SkinArchive {
    SkinArchive {
        id: id.to_owned(),
        files: vec![
            ArchiveFile { path: "title.skin".into(), bytes: title.as_bytes().to_vec() },
            ArchiveFile { path: "tabs.skin".into(), bytes: Vec::new() },
            ArchiveFile { path: "title/back.png".into(), bytes: vec![1, 2, 3] },
        ],
    }
}

#[test]
fn given_user_and_bundled_skins_when_discovering_then_all_are_listed_by_name_and_user_hides_bundled_with_the_same_id() {
    let scratch = Scratch::new("discover");
    let (user, bundled) = (scratch.0.join("user"), scratch.0.join("bundled"));
    write_skin(&bundled, "default", "Breeze");
    write_skin(&bundled, "legacy", "Legacy");
    write_skin(&user, "legacy", "My Legacy");
    write_skin(&user, "zebra", "Aardvark");
    std::fs::create_dir_all(bundled.join("not-a-skin")).expect("creates");
    let skins = discover(&user, &bundled);
    let names: Vec<(&str, SkinLocation)> = skins.iter().map(|skin| (skin.name.as_str(), skin.location)).collect();
    assert_eq!(
        names,
        vec![("Aardvark", SkinLocation::User), ("Breeze", SkinLocation::Bundled), ("My Legacy", SkinLocation::User)]
    );
    assert_eq!(skins[0].author, "A");
}

#[test]
fn given_missing_folders_when_discovering_then_the_list_is_empty() {
    let scratch = Scratch::new("missing");
    assert!(discover(&scratch.0.join("a"), &scratch.0.join("b")).is_empty());
}

#[test]
fn given_a_skin_id_when_locating_then_the_user_folder_wins_and_unknown_ids_are_none() {
    let scratch = Scratch::new("locate");
    let (user, bundled) = (scratch.0.join("user"), scratch.0.join("bundled"));
    write_skin(&bundled, "default", "Breeze");
    write_skin(&user, "default", "Mine");
    write_skin(&bundled, "legacy", "Legacy");
    assert_eq!(locate(&user, &bundled, "default"), Some(user.join("default")));
    assert_eq!(locate(&user, &bundled, "legacy"), Some(bundled.join("legacy")));
    assert_eq!(locate(&user, &bundled, "nope"), None);
}

#[test]
fn given_an_archive_when_installing_then_files_land_under_the_id_and_a_reinstall_replaces_them() {
    let scratch = Scratch::new("install");
    let user = scratch.0.join("user");
    let directory = install(&archive("mine", "[Description]\nSkin=One\n"), &user).expect("installs");
    assert_eq!(std::fs::read(directory.join("title/back.png")).expect("reads"), vec![1, 2, 3]);
    std::fs::write(directory.join("stale.txt"), "x").expect("writes");
    install(&archive("mine", "[Description]\nSkin=Two\n"), &user).expect("reinstalls");
    assert!(!directory.join("stale.txt").exists());
    assert_eq!(discover(&user, &scratch.0.join("none"))[0].name, "Two");
}

#[test]
fn given_an_installed_skin_when_removing_then_it_is_gone_and_odd_ids_are_refused() {
    let scratch = Scratch::new("remove");
    let user = scratch.0.join("user");
    write_skin(&user, "mine", "Mine");
    remove(&user, "mine").expect("removes");
    assert!(!user.join("mine").exists());
    for bad in ["", "..", ".", "a/b", "a\\b"] {
        assert!(remove(&user, bad).is_err(), "{bad:?}");
    }
}

#[test]
fn given_the_bundled_skins_when_loading_then_all_four_load_without_missing_images() {
    use winuake_skin::directory::DirectorySkinFiles;
    use winuake_skin::model::Skin;
    let bundled = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/skins");
    for id in ["default", "legacy", "plastik_dark", "plastik_light"] {
        let files = DirectorySkinFiles::new(bundled.join(id));
        let loaded = Skin::load(id, &files).unwrap_or_else(|error| panic!("{id}: {error}"));
        // The default skin's title.skin names icon.png but ships icon.svg (an upstream quirk Yakuake tolerates).
        let unexpected: Vec<&String> =
            loaded.missing_images.iter().filter(|path| path.as_str() != "icon.png").collect();
        assert!(unexpected.is_empty(), "{id} is missing {unexpected:?}");
        assert!(loaded.skin.title_bar.focus_button.images.up.is_some(), "{id} has a focus button image");
        assert!(loaded.skin.tab_bar.selected_background.is_some(), "{id} has a selected tab background");
        assert!((0..=10).contains(&loaded.skin.border.width), "{id} border width");
    }
}

#[test]
fn given_the_default_skins_svg_images_when_rasterizing_then_every_one_renders() {
    use winuake_skin::raster::svg_to_png;
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/skins/default");
    let mut count = 0;
    for folder in ["title", "tabs"] {
        for entry in std::fs::read_dir(directory.join(folder)).expect("lists").flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|extension| extension == "svg") {
                let bytes = std::fs::read(&path).expect("reads");
                svg_to_png(&bytes, 3).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
                count += 1;
            }
        }
    }
    assert!(count >= 20, "expected the default skin's svg files, found {count}");
}
