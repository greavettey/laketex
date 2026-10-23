//! # Automated Engine & Dependency Provisioner
//!
//! Provides automated provisioning for portable compiler engines (e.g., Tectonic).
//! On Windows, Tectonic is embedded directly within the application binary in compressed form
//! and extracted on demand with zero network downloads.

#![allow(dead_code)]

use crate::compiler::engine::CompilerEngine;
use crate::error::{LakeTexError, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(windows)]
static EMBEDDED_TECTONIC_GZ: &[u8] = include_bytes!("../../assets/tectonic.exe.gz");

/// Automatic provisioning manager for portable compiler binaries.
pub struct Provisioner;

impl Provisioner {
    /// Official Tectonic release tag used for reference / non-Windows builds
    pub const TECTONIC_RELEASE_TAG: &'static str = "tectonic@0.17.0";

    /// URL for prebuilt binary based on operating system architecture (fallback)
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

    /// Extracts the embedded Tectonic binary directly into the target path without internet access.
    #[cfg(windows)]
    pub fn unpack_embedded_tectonic(target_exe: &Path) -> Result<()> {
        use flate2::read::GzDecoder;
        use std::io::{BufWriter, Read, Write};

        if let Some(parent) = target_exe.parent() {
            fs::create_dir_all(parent).map_err(LakeTexError::Io)?;
        }

        let temp_exe = target_exe.with_extension("exe.tmp");
        {
            let mut decoder = GzDecoder::new(EMBEDDED_TECTONIC_GZ);
            let out_file = fs::File::create(&temp_exe).map_err(LakeTexError::Io)?;
            let mut writer = BufWriter::with_capacity(256 * 1024, out_file);
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let n = decoder.read(&mut buffer).map_err(LakeTexError::Io)?;
                if n == 0 {
                    break;
                }
                writer.write_all(&buffer[..n]).map_err(LakeTexError::Io)?;
            }
            writer.flush().map_err(LakeTexError::Io)?;
        }

        if target_exe.exists() {
            let _ = fs::remove_file(target_exe);
        }
        fs::rename(&temp_exe, target_exe).map_err(LakeTexError::Io)?;
        Ok(())
    }

    /// Automatically provisions Tectonic to the local application bin directory.
    /// On Windows, extracts directly from the embedded compressed binary payload (zero network download).
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
            if let Ok(meta) = target_exe.metadata() {
                if meta.len() > 1_000_000 {
                    progress_callback(1.0, "Tectonic is ready");
                    return Ok(target_exe);
                }
            }
        }

        #[cfg(windows)]
        {
            progress_callback(0.3, "Unpacking embedded Tectonic engine (zero-download)...");
            Self::unpack_embedded_tectonic(&target_exe)?;
            progress_callback(1.0, "Tectonic engine ready (embedded)");
            return Ok(target_exe);
        }

        #[cfg(not(windows))]
        {
            let url = Self::get_download_url().ok_or_else(|| {
                LakeTexError::Provisioning("Unsupported OS for auto-provisioning".to_string())
            })?;

            progress_callback(0.1, "Connecting to GitHub Releases...");

            let temp_archive = bin_dir.join("tectonic_download.tar.gz");

            progress_callback(0.3, "Downloading portable Tectonic engine...");
            let response = ureq::get(url)
                .call()
                .map_err(|e| LakeTexError::Network(format!("Failed to download Tectonic: {e}")))?;

            let mut reader = response.into_reader();
            let mut file = fs::File::create(&temp_archive).map_err(LakeTexError::Io)?;
            std::io::copy(&mut reader, &mut file).map_err(LakeTexError::Io)?;
            drop(file);

            progress_callback(0.7, "Extracting binary package...");

            let _ = Self::extract_archive(&temp_archive, &bin_dir)?;
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
    }

    #[cfg(not(windows))]
    fn extract_archive(archive: &Path, destination: &Path) -> Result<bool> {
        let status = crate::create_hidden_command("tar")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn test_embedded_tectonic_unpack() {
        let temp_dir = tempfile::tempdir().unwrap();
        let target_exe = temp_dir.path().join("tectonic_test.exe");
        let result = Provisioner::unpack_embedded_tectonic(&target_exe);
        assert!(result.is_ok(), "Failed to unpack embedded tectonic: {:?}", result.err());
        assert!(target_exe.is_file());
        let meta = target_exe.metadata().unwrap();
        assert!(meta.len() > 50_000_000, "Unpacked size was {} bytes, expected >50MB", meta.len());
    }
}
