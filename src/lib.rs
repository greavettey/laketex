//! Laketex: a LaTeX editor and previewer.
//!
//! `engine` is the built-in draft typesetter, `compiler` runs Tectonic or a system
//! TeX install in the background, `document` holds the lexer/outline/SyncTeX bits,
//! `viewer` draws PDF pages, `ui` is the egui app, and `headless` is the CLI and
//! HTTP server. See docs/ARCHITECTURE.md for more.

pub mod compiler;
pub mod document;
pub mod engine;
pub mod error;
pub mod headless;
pub mod i18n;
pub mod ui;
pub mod viewer;

pub use error::{LakeTexError, Result};
pub use i18n::{current_locale, set_locale, tr, tr_args};

pub static MAIN_START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// Prints a startup timing checkpoint. Only active when `LAKETEX_LOG_STARTUP` is set.
pub fn log_startup(step: &str) {
    if std::env::var_os("LAKETEX_LOG_STARTUP").is_some() {
        if let Some(start) = MAIN_START.get() {
            use std::io::Write;
            let line = format!("[+{:>8.2?}] {}\n", start.elapsed(), step);
            eprint!("{}", line);
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("startup_timing.txt") {
                let _ = f.write_all(line.as_bytes());
            }
        }
    }
}

/// `Command::new` that doesn't flash a console window on Windows, with PATH extended
/// to include any Python/Conda dirs we found (needed for minted's pygmentize).
pub fn create_hidden_command<S: AsRef<std::ffi::OsStr>>(program: S) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    if let Some(aug_path) = crate::document::packages::PackageManager::augmented_path_env() {
        cmd.env("PATH", aug_path);
    }
    cmd
}
