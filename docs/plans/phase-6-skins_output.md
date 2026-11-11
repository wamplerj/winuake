# Phase 6 output — Skins

## Done

- **6a** `winuake-skin` (Linux-tested, 40 tests): INI reader for the `KConfig::SimpleConfig` dialect, the full skin model with Yakuake defaults and the compact-tab math from `skin.cpp`, image sizes from PNG headers and SVG `viewBox`/size, discovery over user and bundled folders, archive reading (tar, tar.gz, zip) with path-traversal and size checks, install/remove, SVG to PNG at 3x with `resvg`. The four upstream skins are bundled in `assets/skins` (GPL, see `NOTICE.md`) and a test loads each and renders every SVG of the default skin.
- **6b** The window now has Yakuake's layout: terminals, tab bar, **title bar at the bottom** (dragging it sets the window height, saved). Title bar, tab bar and borders are drawn from the selected skin as XAML markup: background and corners, positioned buttons with up/over/down images (click on release), title text with the skin caption, skinned tabs with separators, selected/unselected images, the lock image, plus and minus buttons, borders in the skin color unless `HideSkinBorders`. Skins switch live when `appearance.skin` changes. If no skin loads the plain Phase 4 look is the fallback. The keep-open button shows the pressed image while keep-open is on.
- **6c** Skins page in the settings window: list with icon, name, author and origin; click selects and applies; Install skin from file (Windows open dialog, archive validated and installed under `%APPDATA%\Winuake\skins`); Remove for installed skins.
- Fixed: the drop-down no longer re-asserts "topmost" on every settings change, which had lifted it above the settings window.

## Verified on the Win11 VM

Default (Breeze, SVG) skin: title bar with icon, caption, buttons; tabs with selected state; plus button creates sessions; borders. Live switch to Legacy, Plastik Dark and Plastik Light by editing the settings file. Title-bar drag resized the window (height saved). Settings > Skins: list with icons, install of a `.tar.gz` through the file dialog (applied immediately), remove (skin returned to default).

## Not done / known gaps

- Hover and pressed images were checked only by eye on the title-bar buttons in the default skin; the minus (close) button and the plus hover state were not exercised separately for each skin.
- No window masks for rounded corners; compact tab bars are not inset (no bundled skin uses `compact`); tab backgrounds are stretched rather than tiled; the title text baseline is approximate; images are 1x PNGs scaled by DPI for the PNG skins (soft at 150%).
- The plain fallback look was not re-tested after the layout change; bzip2 archives are unsupported; the skin list has no preview image.
- Tests of the app-side skin code (`skin_assets`, `skin_chrome`) are limited to one URI test because the app crate builds only on Windows.
