//! Terminal rendering. `color` is pure; `gpu` (Windows) draws a `ScreenSnapshot` with Direct2D/DirectWrite.

pub mod color;
pub mod scheme_catalog;

#[cfg(windows)]
pub mod gpu;
#[cfg(windows)]
#[allow(warnings, clippy::all)]
mod gpu_bindings;
