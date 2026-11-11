// Ported from yakuake app/skin.cpp @21c6210
//! The skin model and loading it from `title.skin` and `tabs.skin`.

use crate::ini::IniFile;

pub const TITLE_FILE: &str = "title.skin";
pub const TABS_FILE: &str = "tabs.skin";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SkinColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl SkinColor {
    /// `#RRGGBB`, for XAML markup.
    pub fn hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.red, self.green, self.blue)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Left,
    Right,
}

/// Size of an image in device-independent pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}

/// An image a skin names, as a path relative to the skin directory (`title/back.png`) and its size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinImage {
    pub path: String,
    pub size: ImageSize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ButtonImages {
    pub up: Option<SkinImage>,
    pub over: Option<SkinImage>,
    pub down: Option<SkinImage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinButton {
    pub position: Point,
    pub anchor: Anchor,
    pub images: ButtonImages,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkinDescription {
    /// `Skin=`: the display name.
    pub name: String,
    pub author: String,
    pub email: String,
    pub icon: Option<SkinImage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinBorder {
    pub color: SkinColor,
    pub width: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleBarSkin {
    /// Caption appended to the title (`Text/text`).
    pub text: String,
    pub text_position: Point,
    pub text_color: SkinColor,
    pub text_bold: bool,
    pub text_centered: bool,
    pub background: Option<SkinImage>,
    pub left_corner: Option<SkinImage>,
    pub right_corner: Option<SkinImage>,
    pub focus_button: SkinButton,
    pub config_button: SkinButton,
    pub quit_button: SkinButton,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabBarSkin {
    pub position: Point,
    pub text_color: SkinColor,
    pub separator: Option<SkinImage>,
    pub unselected_background: Option<SkinImage>,
    pub selected_background: Option<SkinImage>,
    pub unselected_left_corner: Option<SkinImage>,
    pub unselected_right_corner: Option<SkinImage>,
    pub selected_left_corner: Option<SkinImage>,
    pub selected_right_corner: Option<SkinImage>,
    pub selected_text_bold: bool,
    pub prevent_closing_image: Option<SkinImage>,
    pub prevent_closing_position: Point,
    pub compact: bool,
    /// Compact skins: left and right inset of the tab strip inside the window.
    pub left: i32,
    pub right: i32,
    pub background: Option<SkinImage>,
    pub left_corner: Option<SkinImage>,
    pub right_corner: Option<SkinImage>,
    pub plus_button: SkinButton,
    pub plus_button_at_end_of_tabs: bool,
    pub minus_button: SkinButton,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skin {
    /// Directory name: the identifier stored in settings.
    pub id: String,
    pub description: SkinDescription,
    pub border: SkinBorder,
    pub title_bar: TitleBarSkin,
    pub tab_bar: TabBarSkin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkinError {
    /// `title.skin` or `tabs.skin` is missing: not a skin.
    MissingFile(&'static str),
}

impl std::fmt::Display for SkinError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkinError::MissingFile(name) => write!(formatter, "the skin has no {name}"),
        }
    }
}

impl std::error::Error for SkinError {}

/// A skin and the images it names that could not be found or read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedSkin {
    pub skin: Skin,
    pub missing_images: Vec<String>,
}

/// Reads files out of one skin directory. Paths are relative to it, with `/` separators.
pub trait SkinFiles {
    fn read_text(&self, relative_path: &str) -> Option<String>;
    /// Size of an image file in DIPs (PNG pixels, SVG user units), or `None` if it is absent or unreadable.
    fn image_size(&self, relative_path: &str) -> Option<ImageSize>;
}

struct Loader<'a> {
    files: &'a dyn SkinFiles,
    missing: Vec<String>,
}

impl Loader<'_> {
    /// Resolves an image entry (`/title/back.png`) to a relative path and its size. Empty entries mean "no image".
    fn image(&mut self, entry: &str) -> Option<SkinImage> {
        let path = entry.trim().trim_start_matches('/');
        if path.is_empty() {
            return None;
        }
        match self.files.image_size(path) {
            Some(size) => Some(SkinImage { path: path.to_owned(), size }),
            None => {
                self.missing.push(path.to_owned());
                None
            }
        }
    }

    fn color(group: &crate::ini::IniGroup<'_>) -> SkinColor {
        let channel = |key: &str| group.int(key, 0).clamp(0, 255) as u8;
        SkinColor { red: channel("red"), green: channel("green"), blue: channel("blue") }
    }

    fn button(&mut self, group: crate::ini::IniGroup<'_>) -> SkinButton {
        SkinButton {
            position: Point { x: group.int("x", 0), y: group.int("y", 0) },
            anchor: if group.text("anchor", "") == "left" { Anchor::Left } else { Anchor::Right },
            images: ButtonImages {
                up: self.image(&group.text("up_image", "")),
                over: self.image(&group.text("over_image", "")),
                down: self.image(&group.text("down_image", "")),
            },
        }
    }
}

impl Skin {
    /// Loads the skin in `files`, named `id`. Fails only when a `.skin` file is missing; absent images are listed in the result.
    pub fn load(id: &str, files: &dyn SkinFiles) -> Result<LoadedSkin, SkinError> {
        let title_text = files.read_text(TITLE_FILE).ok_or(SkinError::MissingFile(TITLE_FILE))?;
        let tabs_text = files.read_text(TABS_FILE).ok_or(SkinError::MissingFile(TABS_FILE))?;
        let title = IniFile::parse(&title_text);
        let tabs = IniFile::parse(&tabs_text);
        let mut loader = Loader { files, missing: Vec::new() };

        let description_group = title.group("Description");
        let description = SkinDescription {
            name: description_group.text("Skin", id),
            author: description_group.text("Author", ""),
            email: description_group.text("Email", ""),
            icon: loader.image(&description_group.text("Icon", "")),
        };

        let border_group = title.group("Border");
        let border = SkinBorder { color: Loader::color(&border_group), width: border_group.int("width", 1) };

        let text = title.group("Text");
        let background = title.group("Background");
        let title_bar = TitleBarSkin {
            text: text.text("text", ""),
            text_position: Point { x: text.int("x", 0), y: text.int("y", 0) },
            text_color: Loader::color(&text),
            text_bold: text.flag("bold", true),
            text_centered: text.flag("centered", false),
            background: loader.image(&background.text("back_image", "")),
            left_corner: loader.image(&background.text("left_corner", "")),
            right_corner: loader.image(&background.text("right_corner", "")),
            focus_button: loader.button(title.group("FocusButton")),
            config_button: loader.button(title.group("ConfigButton")),
            quit_button: loader.button(title.group("QuitButton")),
        };

        let tab_group = tabs.group("Tabs");
        let tab_background = tabs.group("Background");
        let plus_group = tabs.group("PlusButton");
        let mut plus_button = loader.button(plus_group);
        let mut minus_button = loader.button(tabs.group("MinusButton"));
        let mut position = Point { x: tab_group.int("x", 0), y: tab_group.int("y", 0) };
        let compact = tab_group.flag("compact", false);
        let at_end_of_tabs = plus_group.flag("at_end_of_tabs", false);
        let prevent_closing_image = loader.image(&tab_group.text("prevent_closing_image", ""));
        let (mut left, mut right) = (0, 0);
        if compact {
            // Skin.cpp: shift the strip so its left edge is the leftmost of the strip and the plus button; the close button
            // sits at the right edge and the strip stops before it.
            if at_end_of_tabs {
                left = position.x;
                position.x = 0;
            } else {
                left = position.x.min(plus_button.position.x);
                position.x -= left;
                plus_button.position.x -= left;
            }
            let close_width = minus_button.images.up.as_ref().map_or(0, |image| image.size.width as i32);
            right = minus_button.position.x - close_width;
            minus_button.position.x = close_width;
        }
        let tab_bar = TabBarSkin {
            position,
            text_color: Loader::color(&tab_group),
            separator: loader.image(&tab_group.text("separator_image", "")),
            unselected_background: loader.image(&tab_group.text("unselected_background", "")),
            selected_background: loader.image(&tab_group.text("selected_background", "")),
            unselected_left_corner: loader.image(&tab_group.text("unselected_left_corner", "")),
            unselected_right_corner: loader.image(&tab_group.text("unselected_right_corner", "")),
            selected_left_corner: loader.image(&tab_group.text("selected_left_corner", "")),
            selected_right_corner: loader.image(&tab_group.text("selected_right_corner", "")),
            selected_text_bold: tab_group.flag("selected_text_bold", true),
            prevent_closing_image,
            prevent_closing_position: Point {
                x: tab_group.int("prevent_closing_image_x", 0),
                y: tab_group.int("prevent_closing_image_y", 0),
            },
            compact,
            left,
            right,
            background: loader.image(&tab_background.text("back_image", "")),
            left_corner: loader.image(&tab_background.text("left_corner", "")),
            right_corner: loader.image(&tab_background.text("right_corner", "")),
            plus_button,
            plus_button_at_end_of_tabs: at_end_of_tabs,
            minus_button,
        };

        let mut missing_images = loader.missing;
        missing_images.sort();
        missing_images.dedup();
        Ok(LoadedSkin { skin: Skin { id: id.to_owned(), description, border, title_bar, tab_bar }, missing_images })
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;
    use std::collections::HashMap;

    /// An in-memory skin directory.
    #[derive(Default)]
    pub struct MemorySkinFiles {
        pub texts: HashMap<String, String>,
        pub images: HashMap<String, ImageSize>,
    }

    impl MemorySkinFiles {
        pub fn text(mut self, path: &str, text: &str) -> Self {
            self.texts.insert(path.to_owned(), text.to_owned());
            self
        }

        pub fn image(mut self, path: &str, width: u32, height: u32) -> Self {
            self.images.insert(path.to_owned(), ImageSize { width, height });
            self
        }
    }

    impl SkinFiles for MemorySkinFiles {
        fn read_text(&self, relative_path: &str) -> Option<String> {
            self.texts.get(relative_path).cloned()
        }

        fn image_size(&self, relative_path: &str) -> Option<ImageSize> {
            self.images.get(relative_path).copied()
        }
    }

    pub const TITLE: &str = "[Description]\nSkin=Test Skin\nAuthor=Tester\nEmail=t@example.com\nIcon=/icon.png\n\n[Border]\nred=61\ngreen=174\nblue=233\nwidth=2\n\n[Text]\nx=34\ny=20\nred=239\ngreen=240\nblue=241\ntext=Drop-Down Terminal\ncentered=true\n\n[Background]\nback_image=/title/back.png\nleft_corner=/title/left.png\nright_corner=/title/right.png\n\n[FocusButton]\nx=78\ny=0\nup_image=/title/focus_up.png\nover_image=/title/focus_over.png\ndown_image=/title/focus_down.png\n\n[ConfigButton]\nx=53\nanchor=left\nup_image=/title/config_up.png\n\n[QuitButton]\nx=30\nup_image=/title/quit_up.png\n";

    pub const TABS: &str = "[Tabs]\nx=22\ny=0\nred=239\ngreen=240\nblue=241\nseparator_image=/tabs/separator.png\nselected_background=/tabs/selected_back.png\nunselected_background=/tabs/unselected_back.png\nprevent_closing_image=/tabs/lock.png\nprevent_closing_image_x=3\nprevent_closing_image_y=4\n\n[Background]\nback_image=/tabs/back.png\n\n[PlusButton]\nx=0\ny=0\nup_image=/tabs/plus_up.png\n\n[MinusButton]\nx=100\ny=0\nup_image=/tabs/minus_up.png\n";

    pub fn complete_files() -> MemorySkinFiles {
        let mut files = MemorySkinFiles::default().text(TITLE_FILE, TITLE).text(TABS_FILE, TABS);
        for (path, width, height) in [
            ("icon.png", 32, 32),
            ("title/back.png", 1, 22),
            ("title/left.png", 8, 22),
            ("title/right.png", 8, 22),
            ("title/focus_up.png", 22, 22),
            ("title/focus_over.png", 22, 22),
            ("title/focus_down.png", 22, 22),
            ("title/config_up.png", 22, 22),
            ("title/quit_up.png", 22, 22),
            ("tabs/separator.png", 2, 20),
            ("tabs/selected_back.png", 1, 20),
            ("tabs/unselected_back.png", 1, 20),
            ("tabs/lock.png", 10, 10),
            ("tabs/back.png", 1, 22),
            ("tabs/plus_up.png", 20, 20),
            ("tabs/minus_up.png", 18, 20),
        ] {
            files = files.image(path, width, height);
        }
        files
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;

    #[test]
    fn given_a_complete_skin_when_loading_then_description_border_and_text_are_read() {
        let loaded = Skin::load("test", &complete_files()).expect("loads");
        assert!(loaded.missing_images.is_empty(), "{:?}", loaded.missing_images);
        let skin = loaded.skin;
        assert_eq!(skin.id, "test");
        assert_eq!(skin.description.name, "Test Skin");
        assert_eq!(skin.description.author, "Tester");
        assert_eq!(skin.description.icon.as_ref().map(|icon| icon.path.as_str()), Some("icon.png"));
        assert_eq!(skin.border, SkinBorder { color: SkinColor { red: 61, green: 174, blue: 233 }, width: 2 });
        assert_eq!(skin.border.color.hex(), "#3DAEE9");
        assert_eq!(skin.title_bar.text, "Drop-Down Terminal");
        assert_eq!(skin.title_bar.text_position, Point { x: 34, y: 20 });
        assert!(skin.title_bar.text_bold, "bold defaults to true");
        assert!(skin.title_bar.text_centered);
    }

    #[test]
    fn given_title_buttons_when_loading_then_positions_anchors_and_state_images_are_read() {
        let skin = Skin::load("test", &complete_files()).expect("loads").skin;
        let focus = &skin.title_bar.focus_button;
        assert_eq!(focus.position, Point { x: 78, y: 0 });
        assert_eq!(focus.anchor, Anchor::Right, "anchor defaults to right");
        assert_eq!(focus.images.over.as_ref().map(|image| image.path.as_str()), Some("title/focus_over.png"));
        assert_eq!(skin.title_bar.config_button.anchor, Anchor::Left);
        assert_eq!(skin.title_bar.config_button.images.over, None);
        assert_eq!(
            skin.title_bar.focus_button.images.up.as_ref().map(|image| image.size),
            Some(ImageSize { width: 22, height: 22 })
        );
    }

    #[test]
    fn given_tab_settings_when_loading_then_images_and_the_lock_position_are_read() {
        let skin = Skin::load("test", &complete_files()).expect("loads").skin;
        let tabs = &skin.tab_bar;
        assert_eq!(tabs.position, Point { x: 22, y: 0 });
        assert_eq!(tabs.text_color.red, 239);
        assert_eq!(tabs.separator.as_ref().map(|image| image.path.as_str()), Some("tabs/separator.png"));
        assert_eq!(tabs.prevent_closing_image.as_ref().map(|image| image.path.as_str()), Some("tabs/lock.png"));
        assert_eq!(tabs.prevent_closing_position, Point { x: 3, y: 4 });
        assert!(!tabs.compact);
        assert!(tabs.selected_text_bold);
        assert_eq!(tabs.minus_button.position, Point { x: 100, y: 0 });
    }

    #[test]
    fn given_images_that_do_not_exist_when_loading_then_they_are_reported_and_skipped() {
        let mut files = complete_files();
        files.images.remove("title/back.png");
        files.images.remove("tabs/lock.png");
        let loaded = Skin::load("test", &files).expect("still loads");
        assert_eq!(loaded.missing_images, vec!["tabs/lock.png".to_owned(), "title/back.png".to_owned()]);
        assert_eq!(loaded.skin.title_bar.background, None);
    }

    #[test]
    fn given_a_missing_skin_file_when_loading_then_it_is_not_a_skin() {
        let only_title = MemorySkinFiles::default().text(TITLE_FILE, TITLE);
        assert_eq!(Skin::load("x", &only_title), Err(SkinError::MissingFile(TABS_FILE)));
        let only_tabs = MemorySkinFiles::default().text(TABS_FILE, TABS);
        assert_eq!(Skin::load("x", &only_tabs), Err(SkinError::MissingFile(TITLE_FILE)));
        assert!(SkinError::MissingFile(TABS_FILE).to_string().contains("tabs.skin"));
    }

    #[test]
    fn given_empty_skin_files_when_loading_then_yakuakes_defaults_apply() {
        let files = MemorySkinFiles::default().text(TITLE_FILE, "").text(TABS_FILE, "");
        let skin = Skin::load("plain", &files).expect("loads").skin;
        assert_eq!(skin.description.name, "plain", "the directory name stands in for the display name");
        assert_eq!(skin.border.width, 1);
        assert_eq!(skin.border.color, SkinColor::default());
        assert!(skin.title_bar.text_bold && !skin.title_bar.text_centered);
        assert!(skin.title_bar.background.is_none());
    }

    #[test]
    fn given_color_channels_out_of_range_when_loading_then_they_are_clamped() {
        let files =
            MemorySkinFiles::default().text(TITLE_FILE, "[Border]\nred=999\ngreen=-5\nblue=7\n").text(TABS_FILE, "");
        let skin = Skin::load("c", &files).expect("loads").skin;
        assert_eq!(skin.border.color, SkinColor { red: 255, green: 0, blue: 7 });
    }

    const COMPACT_TABS: &str = "[Tabs]\ncompact=true\nx=40\n\n[PlusButton]\nx=10\nup_image=/plus.png\n\n[MinusButton]\nx=300\nup_image=/minus.png\n";

    #[test]
    fn given_a_compact_skin_when_loading_then_the_strip_and_buttons_are_shifted_like_yakuake() {
        let files = MemorySkinFiles::default()
            .text(TITLE_FILE, "")
            .text(TABS_FILE, COMPACT_TABS)
            .image("plus.png", 20, 20)
            .image("minus.png", 18, 20);
        let tabs = Skin::load("c", &files).expect("loads").skin.tab_bar;
        assert!(tabs.compact);
        assert_eq!(tabs.left, 10, "the plus button is left of the strip");
        assert_eq!(tabs.position.x, 30);
        assert_eq!(tabs.plus_button.position.x, 0);
        assert_eq!(tabs.right, 282);
        assert_eq!(tabs.minus_button.position.x, 18);
    }

    #[test]
    fn given_a_compact_skin_with_the_plus_button_at_the_end_when_loading_then_the_strip_starts_at_zero() {
        let text = COMPACT_TABS.replace("[PlusButton]\n", "[PlusButton]\nat_end_of_tabs=true\n");
        let files = MemorySkinFiles::default()
            .text(TITLE_FILE, "")
            .text(TABS_FILE, &text)
            .image("plus.png", 20, 20)
            .image("minus.png", 18, 20);
        let tabs = Skin::load("c", &files).expect("loads").skin.tab_bar;
        assert_eq!(tabs.left, 40);
        assert_eq!(tabs.position.x, 0);
        assert!(tabs.plus_button_at_end_of_tabs);
        assert_eq!(tabs.plus_button.position.x, 10, "the plus button keeps its own offset");
    }

    #[test]
    fn given_a_compact_skin_without_a_close_image_when_loading_then_the_close_width_is_zero() {
        let text = "[Tabs]\ncompact=true\n[MinusButton]\nx=50\n";
        let files = MemorySkinFiles::default().text(TITLE_FILE, "").text(TABS_FILE, text);
        let tabs = Skin::load("c", &files).expect("loads").skin.tab_bar;
        assert_eq!((tabs.right, tabs.minus_button.position.x), (50, 0));
    }
}
