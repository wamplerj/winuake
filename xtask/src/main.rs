//! Dev tasks: `cargo xtask <task>`.

use std::process::ExitCode;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("gen-bindings") => {
            not_implemented("gen-bindings: generate WinUI 3 bindings from WinAppSDK .winmd (Phase 0 spike)")
        }
        Some("upstream-diff") => not_implemented("upstream-diff: list Yakuake commits since UPSTREAM.md SHA"),
        _ => {
            eprintln!("usage: cargo xtask <gen-bindings|upstream-diff>");
            ExitCode::from(2)
        }
    }
}

fn not_implemented(task: &str) -> ExitCode {
    eprintln!("not implemented yet: {task}");
    ExitCode::FAILURE
}
