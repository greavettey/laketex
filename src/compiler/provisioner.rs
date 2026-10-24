//! # Automated Engine & Dependency Provisioner
//!
//! Provides automated provisioning for portable compiler engines (e.g., Tectonic).
//! On supported platforms (Windows x86_64, Linux x86_64, macOS x86_64/ARM64),
//! Tectonic is embedded directly within the application binary in compressed form
//! and extracted on demand with zero network downloads.

#![allow(dead_code)]

use crate::compiler::engine::CompilerEngine;
use crate::error::{LakeTexError, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

static PROVISION_MUTEX: Mutex<()> = Mutex::new(());
static UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(all(windows, target_arch = "x86_64"))]
static EMBEDDED_TECTONIC_GZ: &[u8] = include_bytes!("../../assets/tectonic.exe.gz");

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
static EMBEDDED_TECTONIC_GZ: &[u8] = include_bytes!("../../assets/tectonic-x86_64-unknown-linux-musl.gz");

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
static EMBEDDED_TECTONIC_GZ: &[u8] = include_bytes!("../../assets/tectonic-x86_64-apple-darwin.gz");

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
static EMBEDDED_TECTONIC_GZ: &[u8] = include_bytes!("../../assets/tectonic-aarch64-apple-darwin.gz");

#[cfg(not(any(
    all(windows, target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64"),
)))]
static EMBEDDED_TECTONIC_GZ: &[u8] = &[];

/// Automatic provisioning manager for portable compiler binaries.
pub struct Provisioner;

impl Provisioner {
    /// Official Tectonic release tag used for reference / non-embedded builds
    pub const TECTONIC_RELEASE_TAG: &'static str = "tectonic@0.17.0";

    /// URL for prebuilt binary based on operating system architecture (fallback)
    pub fn get_download_url() -> Option<&'static str> {
        if cfg!(target_os = "windows") {
            Some("https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic@0.17.0/tectonic-0.17.0-x86_64-pc-windows-msvc.zip")
        } else if cfg!(target_os = "linux") {
            Some("https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic@0.17.0/tectonic-0.17.0-x86_64-unknown-linux-musl.tar.gz")
        } else if cfg!(target_os = "macos") {
            if cfg!(target_arch = "aarch64") {
                Some("https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic@0.17.0/tectonic-0.17.0-aarch64-apple-darwin.tar.gz")
            } else {
                Some("https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic@0.17.0/tectonic-0.17.0-x86_64-apple-darwin.tar.gz")
            }
        } else {
            None
        }
    }

    /// Returns true if Tectonic is available either locally or in system PATH
    pub fn is_tectonic_ready() -> bool {
        CompilerEngine::find_engine_path(crate::compiler::engine::EngineKind::Tectonic).is_some()
    }

    /// Extracts the embedded Tectonic binary directly into the target path without internet access.
    pub fn unpack_embedded_tectonic(target_exe: &Path) -> Result<()> {
        use flate2::read::GzDecoder;
        use std::io::{BufWriter, Read, Write};

        if EMBEDDED_TECTONIC_GZ.is_empty() {
            return Err(LakeTexError::Provisioning(
                "No embedded Tectonic binary payload available for this target platform".to_string(),
            ));
        }

        // Fast path: if target_exe already exists with valid size, ensure permissions and return
        if target_exe.is_file() {
            if let Ok(meta) = target_exe.metadata() {
                if meta.len() > 1_000_000 {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let mut perms = meta.permissions();
                        if perms.mode() & 0o111 == 0 {
                            perms.set_mode(0o755);
                            let _ = fs::set_permissions(target_exe, perms);
                        }
                    }
                    return Ok(());
                }
            }
        }

        if let Some(parent) = target_exe.parent() {
            fs::create_dir_all(parent).map_err(LakeTexError::Io)?;
        }

        // Unique temporary file avoids race conditions across parallel threads or test processes
        let unique_id = format!(
            "tmp.{}.{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
            UNIQUE_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let temp_exe = target_exe.with_extension(unique_id);
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
            if let Ok(file) = writer.into_inner() {
                let _ = file.sync_all();
            }
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(&temp_exe) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = fs::set_permissions(&temp_exe, perms);
            }
            #[cfg(target_os = "macos")]
            {
                // Ensure ad-hoc code signature on Apple Silicon macOS
                let _ = crate::create_hidden_command("codesign")
                    .args(["--force", "--sign", "-", temp_exe.to_str().unwrap_or("")])
                    .status();
            }
        }

        #[cfg(unix)]
        {
            // On Unix, rename is atomic and overwrites target_exe cleanly without deleting it first.
            if let Err(e) = fs::rename(&temp_exe, target_exe) {
                let _ = fs::remove_file(&temp_exe);
                if target_exe.is_file() {
                    if let Ok(meta) = target_exe.metadata() {
                        if meta.len() > 1_000_000 {
                            return Ok(());
                        }
                    }
                }
                return Err(LakeTexError::Io(e));
            }
        }

        #[cfg(windows)]
        {
            if target_exe.exists() {
                let _ = fs::remove_file(target_exe);
            }
            if let Err(e) = fs::rename(&temp_exe, target_exe) {
                let _ = fs::remove_file(&temp_exe);
                if target_exe.is_file() {
                    if let Ok(meta) = target_exe.metadata() {
                        if meta.len() > 1_000_000 {
                            return Ok(());
                        }
                    }
                }
                return Err(LakeTexError::Io(e));
            }
        }

        Ok(())
    }

    /// Automatically provisions Tectonic to the local application bin directory.
    /// Extracts directly from the embedded compressed binary payload if available (zero network download).
    pub fn provision_tectonic<F: Fn(f32, &str)>(progress_callback: F) -> Result<PathBuf> {
        let _guard = PROVISION_MUTEX.lock().unwrap_or_else(|p| p.into_inner());

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

        // 1. Unpack embedded binary payload (zero network download) if compiled with payload
        if !EMBEDDED_TECTONIC_GZ.is_empty() {
            progress_callback(0.3, "Unpacking embedded Tectonic engine (zero-download)...");
            match Self::unpack_embedded_tectonic(&target_exe) {
                Ok(()) => {
                    progress_callback(1.0, "Tectonic engine ready (embedded)");
                    return Ok(target_exe);
                }
                Err(err) => {
                    eprintln!("[LakeTex Provisioner] Failed to unpack embedded Tectonic: {err}. Falling back to network download...");
                }
            }
        }

        // 2. Network download fallback for non-embedded target architectures
        let url = Self::get_download_url().ok_or_else(|| {
            LakeTexError::Provisioning("Unsupported OS / architecture for auto-provisioning".to_string())
        })?;

        progress_callback(0.1, "Connecting to GitHub Releases...");
        let unique_archive = format!(
            "tectonic_download_{}_{}.tar.gz",
            std::process::id(),
            UNIQUE_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let temp_archive = bin_dir.join(unique_archive);

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

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(&target_exe) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = fs::set_permissions(&target_exe, perms);
            }
            #[cfg(target_os = "macos")]
            {
                let _ = crate::create_hidden_command("codesign")
                    .args(["--force", "--sign", "-", target_exe.to_str().unwrap_or("")])
                    .status();
            }
        }

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
        let unique_dir = format!("extract_{}_{}", std::process::id(), UNIQUE_COUNTER.fetch_add(1, Ordering::Relaxed));
        let temp_extract_dir = destination.join(unique_dir);
        let _ = fs::create_dir_all(&temp_extract_dir);

        #[cfg(windows)]
        {
            let status = crate::create_hidden_command("tar")
                .arg("-xf")
                .arg(archive)
                .arg("-C")
                .arg(&temp_extract_dir)
                .status();

            let ok = match status {
                Ok(s) if s.success() => true,
                _ => {
                    let ps_status = crate::create_hidden_command("powershell")
                        .args(["-NoProfile", "-Command", "Expand-Archive", "-Path"])
                        .arg(archive)
                        .arg("-DestinationPath")
                        .arg(&temp_extract_dir)
                        .arg("-Force")
                        .status();
                    ps_status.map(|s| s.success()).unwrap_or(false)
                }
            };

            if ok {
                let extracted_bin = temp_extract_dir.join("tectonic.exe");
                let target_bin = destination.join("tectonic.exe");
                if extracted_bin.is_file() {
                    if target_bin.exists() {
                        let _ = fs::remove_file(&target_bin);
                    }
                    let _ = fs::rename(&extracted_bin, &target_bin);
                }
            }
            let _ = fs::remove_dir_all(&temp_extract_dir);
            Ok(ok)
        }

        #[cfg(not(windows))]
        {
            let status = crate::create_hidden_command("tar")
                .arg("-xf")
                .arg(archive)
                .arg("-C")
                .arg(&temp_extract_dir)
                .status();

            let ok = status.map(|s| s.success()).unwrap_or(false);
            if ok {
                let extracted_bin = temp_extract_dir.join("tectonic");
                let target_bin = destination.join("tectonic");
                if extracted_bin.is_file() {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(meta) = fs::metadata(&extracted_bin) {
                        let mut perms = meta.permissions();
                        perms.set_mode(0o755);
                        let _ = fs::set_permissions(&extracted_bin, perms);
                    }
                    #[cfg(target_os = "macos")]
                    {
                        let _ = crate::create_hidden_command("codesign")
                            .args(["--force", "--sign", "-", extracted_bin.to_str().unwrap_or("")])
                            .status();
                    }
                    let _ = fs::rename(&extracted_bin, &target_bin);
                }
            }
            let _ = fs::remove_dir_all(&temp_extract_dir);
            Ok(ok)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_tectonic_unpack() {
        if EMBEDDED_TECTONIC_GZ.is_empty() {
            return;
        }
        let temp_dir = tempfile::tempdir().unwrap();
        let exe_name = if cfg!(windows) {
            "tectonic_test.exe"
        } else {
            "tectonic_test"
        };
        let target_exe = temp_dir.path().join(exe_name);
        let result = Provisioner::unpack_embedded_tectonic(&target_exe);
        assert!(result.is_ok(), "Failed to unpack embedded tectonic: {:?}", result.err());
        assert!(target_exe.is_file());
        let meta = target_exe.metadata().unwrap();
        assert!(meta.len() > 20_000_000, "Unpacked size was {} bytes, expected >20MB", meta.len());
    }
}
