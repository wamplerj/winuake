//! Dev tasks: `cargo xtask <task>`.

mod bindings;
mod icon;
mod package;
mod winappsdk;

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let workspace_root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask lives in workspace").to_path_buf();
    let result = match std::env::args().nth(1).as_deref() {
        Some("gen-bindings") => bindings::generate(&workspace_root),
        Some("fetch-runtime") => {
            winappsdk::ensure_package(&workspace_root, winappsdk::PACKAGE_RUNTIME).map(|directory| {
                println!("{}", directory.join("tools").join("MSIX").join("win10-x64").display());
            })
        }
        Some("gen-icon") => icon::generate(&workspace_root),
        Some("package") => package::run(&workspace_root, &std::env::args().skip(2).collect::<Vec<_>>()),
        Some("upstream-diff") => Err("upstream-diff: not implemented yet".into()),
        _ => {
            eprintln!("usage: cargo xtask <gen-bindings|gen-icon|fetch-runtime|package|upstream-diff>");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
