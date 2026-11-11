# Phase 6 — Skins

Parent: `winuake-port.md`. Ports `skin.cpp`, `titlebar.cpp`, the painting parts of `tabbar.cpp` and `mainwindow.cpp` (borders), `skinlistdelegate.cpp`, `appearancesettings.ui` (skin list, install/remove, `HideSkinBorders`).

## Behavior to port

- A skin is a directory with `title.skin` and `tabs.skin` (INI) plus images (PNG, or SVG in the default skin). Directory name = id. `[Description]` gives Skin (display name), Author, Email, Icon.
- `title.skin`: `[Border]` color and width, `[Text]` position/color/bold/centered/default caption, `[Background]` back/left/right images, `[FocusButton] [ConfigButton] [QuitButton]` with position, `anchor` (left, else right: x measured from the right edge) and up/over/down images.
- `tabs.skin`: `[Tabs]` position, text color, separator, selected/unselected background and corner images, `selected_text_bold`, `prevent_closing_image` (+x/y), `compact`; `[Background]`; `[PlusButton]` (`at_end_of_tabs`); `[MinusButton]`. Compact mode shifts the tab strip using the plus/minus positions (`skin.cpp`).
- Window layout (Yakuake): terminals on top, tab bar below, **title bar at the bottom**; the title bar is dragged to change the window height. Left, right and bottom borders in the skin border color and width unless `HideSkinBorders`. Title text is `<terminal title> - <skin caption>`.
- Skin list in Appearance shows name, author, icon; install from a tar/tar.gz archive (Yakuake) and zip; remove user skins. Bundled: default, legacy, plastik_dark, plastik_light.

## Design

- `winuake-skin` (pure, Linux-tested): INI reader, `Skin` model with every key above and Yakuake defaults, compact-tab math, image references with their size in DIPs (PNG header, SVG `viewBox`/size), problems for missing images, discovery over directory listings, archive entry validation (no path traversal, must contain both `.skin` files), SVG rasterization to PNG at 3x (`resvg`) for display. File access goes through small traits so tests use in-memory data.
- Skins are searched in `%APPDATA%\Winuake\skins` (user) then `skins` beside the exe or `assets/skins` in the repo.
- App chrome is rebuilt from the skin as XAML markup (images by `file:///` URI, positioned with margins, three stacked images per button for up/over/down). Without a loadable skin the Phase 4 plain chrome stays as fallback.
- Deviation: Qt window masks for rounded skin corners are not ported; corner images are drawn over the window background.

## Chunks

- **6a** `winuake-skin`: parser, model, discovery, image sizes, archive validation, SVG raster, tests.
- **6b** Bundled skins in `assets/skins`; skin-driven title bar (moved to the bottom, drag resizes), tab bar, borders; live skin switching via the `skin` setting.
- **6c** Skin list in the settings window, install from file, remove.

## Verification

Unit tests with in-memory skin fixtures plus the real bundled skins parsed in a test. On the VM: each bundled skin renders; buttons show hover/pressed images; switching skins applies live; title bar drag resizes; install an archive.
