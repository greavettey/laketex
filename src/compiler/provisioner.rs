//! # Automated Engine & Dependency Provisioner
//!
//! Downloads and provisions portable compiler engines (e.g., Tectonic) dynamically into
//! the local application cache when external TeX distributions are absent.

#![allow(dead_code)]

use crate::compiler::engine::CompilerEngine;
use crate::error::{LakeTexError, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Automatic provisioning manager for portable compiler binaries.
pub struct Provisioner;

impl Provisioner {
    /// Official Tectonic release tag used for automated downloads
    pub const TECTONIC_RELEASE_TAG: &'static str = "tectonic@0.17.0";

    /// URL for prebuilt binary based on operating system architecture
    pub fn get_download_url() -> Option<&'static str> {
        if cfg!(target_os = "windows") {
            Some("https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic@0.17.0/tectonic-0.17.0-x86_64-pc-windows-msvc.zip")
        } else if cfg!(target_os = "linux") {
            Some("https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic@0.17.0/tectonic-0.17.0-x86_64-unknown-linux-musl.tar.gz")
        } else if cfg!(target_os = "macos") {
            Some("https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic@0.17.0/tectonic-0.17.0-x86_64-apple-darwin.tar.gz")
        } else {
            None
        }
    }

    /// Returns true if Tectonic is available either locally or in system PATH
    pub fn is_tectonic_ready() -> bool {
        CompilerEngine::find_engine_path(crate::compiler::engine::EngineKind::Tectonic).is_some()
    }

    /// Automatically downloads and provisions Tectonic to the local application bin directory.
    pub fn provision_tectonic<F: Fn(f32, &str)>(progress_callback: F) -> Result<PathBuf> {
        let bin_dir = CompilerEngine::laketex_bin_dir().ok_or_else(|| {
            LakeTexError::Provisioning("Could not determine user cache directory".to_string())
        })?;

        fs::create_dir_all(&bin_dir).map_err(LakeTexError::Io)?;

        let exe_name = if cfg!(windows) {
            "tectonic.exe"
        } else {
            "tectonic"
        };
        let target_exe = bin_dir.join(exe_name);
        if target_exe.is_file() {
            progress_callback(1.0, "Tectonic is ready");
            return Ok(target_exe);
        }

        let url = Self::get_download_url().ok_or_else(|| {
            LakeTexError::Provisioning("Unsupported OS for auto-provisioning".to_string())
        })?;

        progress_callback(0.1, "Connecting to GitHub Releases...");

        let temp_archive = bin_dir.join(if cfg!(windows) {
            "tectonic_download.zip"
        } else {
            "tectonic_download.tar.gz"
        });

        // Download using ureq
        progress_callback(0.3, "Downloading portable Tectonic engine...");
        let response = ureq::get(url)
            .call()
            .map_err(|e| LakeTexError::Network(format!("Failed to download Tectonic: {e}")))?;

        let mut reader = response.into_reader();
        let mut file = fs::File::create(&temp_archive).map_err(LakeTexError::Io)?;
        std::io::copy(&mut reader, &mut file).map_err(LakeTexError::Io)?;
        drop(file);

        progress_callback(0.7, "Extracting binary package...");

        let extracted = Self::extract_archive(&temp_archive, &bin_dir)?;
        if !extracted {
            if cfg!(windows) {
                let status = Command::new("powershell")
                    .args([
                        "-NoProfile",
                        "-Command",
                        &format!(
                            "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                            temp_archive.display(),
                            bin_dir.display()
                        ),
                    ])
                    .status();
                if let Err(e) = status {
                    return Err(LakeTexError::Provisioning(format!(
                        "Extraction failed: {e}"
                    )));
                }
            }
        }

        let _ = fs::remove_file(&temp_archive);

        if target_exe.is_file() {
            progress_callback(1.0, "Tectonic provisioned successfully");
            Ok(target_exe)
        } else {
            Err(LakeTexError::Provisioning(format!(
                "Tectonic executable not found at {}",
                target_exe.display()
            )))
        }
    }

    fn extract_archive(archive: &Path, destination: &Path) -> Result<bool> {
        let status = Command::new("tar")
            .arg("-xf")
            .arg(archive)
            .arg("-C")
            .arg(destination)
            .status();

        match status {
            Ok(s) => Ok(s.success()),
            Err(_) => Ok(false),
        }
    }
}
