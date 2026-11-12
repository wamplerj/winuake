//! `cargo xtask gen-icon`: renders the app icon (the default skin's `icon.svg`, a placeholder until Winuake has its own art) at the sizes
//! Windows asks for and writes `assets/icon/winuake.ico` (PNG-compressed entries).

use std::error::Error;
use std::path::Path;
use winuake_skin::raster::svg_to_png;

const SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

pub fn generate(workspace_root: &Path) -> Result<(), Box<dyn Error>> {
    let source = workspace_root.join("assets").join("skins").join("default").join("icon.svg");
    let svg = std::fs::read(&source)?;
    let natural = svg_to_png(&svg, 1)?.size;
    let mut images = Vec::new();
    for size in SIZES {
        // Render at an integer multiple, then the PNG is the requested size only when the SVG is square; scale to fit the largest side.
        let scale = size.div_ceil(natural.width.max(natural.height)).max(1);
        let rendered = svg_to_png(&svg, scale)?;
        images.push((size, rendered.png));
    }
    let directory = workspace_root.join("assets").join("icon");
    std::fs::create_dir_all(&directory)?;
    let target = directory.join("winuake.ico");
    std::fs::write(&target, ico_bytes(&images))?;
    eprintln!("wrote {} ({} sizes)", target.display(), images.len());
    Ok(())
}

/// ICO container: header, one directory entry per image, then the PNG data. A size of 256 is stored as 0.
fn ico_bytes(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = vec![0, 0, 1, 0];
    bytes.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (size, png) in images {
        let dimension = if *size >= 256 { 0 } else { *size as u8 };
        bytes.extend_from_slice(&[dimension, dimension, 0, 0]);
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&32u16.to_le_bytes());
        bytes.extend_from_slice(&(png.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in images {
        bytes.extend_from_slice(png);
    }
    bytes
}
