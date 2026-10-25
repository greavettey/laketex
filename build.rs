//! Build script for embedding platform-specific executable icons and application bundle metadata.
//!
//! - **Windows**: Uses `winresource` to compile `assets/laketex.ico` into the PE resource table
//!   of `laketex.exe`.
//! - **macOS**: Embeds `Info.plist` metadata into the Mach-O `__TEXT,__info_plist` section and
//!   populates the `LakeTeX.app` bundle structure (with `assets/laketex.icns`) in the Cargo
//!   target profile directory.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/assets/LaketexLogo.png");
    println!("cargo:rerun-if-changed=assets/laketex.ico");
    println!("cargo:rerun-if-changed=assets/laketex.icns");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    #[cfg(windows)]
    if target_os == "windows" {
        let ico_path = Path::new("assets/laketex.ico");
        if ico_path.is_file() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/laketex.ico");
            if let Err(err) = res.compile() {
                println!("cargo:warning=Failed to embed Windows icon resource: {err}");
            }
        }
    }

    if target_os == "macos" {
        configure_macos_app_bundle();
    }
}

fn configure_macos_app_bundle() {
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.2.1".to_string());
    let info_plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>LakeTeX</string>
    <key>CFBundleDisplayName</key>
    <string>LakeTeX</string>
    <key>CFBundleIdentifier</key>
    <string>com.laketex.app</string>
    <key>CFBundleVersion</key>
    <string>{version}</string>
    <key>CFBundleShortVersionString</key>
    <string>{version}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleExecutable</key>
    <string>laketex</string>
    <key>CFBundleIconFile</key>
    <string>laketex.icns</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
"#
    );

    if let Ok(out_dir) = env::var("OUT_DIR") {
        let out_path = PathBuf::from(&out_dir);
        let plist_out = out_path.join("Info.plist");
        if fs::write(&plist_out, &info_plist).is_ok() {
            println!(
                "cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__info_plist,{}",
                plist_out.display()
            );
        }

        // Locate the Cargo profile output directory (e.g., target/release or target/<triple>/release)
        // OUT_DIR is target/[<triple>/]<profile>/build/<pkg>-<hash>/out
        if let Some(profile_dir) = out_path
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
        {
            let app_dir = profile_dir.join("LakeTeX.app").join("Contents");
            let macos_dir = app_dir.join("MacOS");
            let resources_dir = app_dir.join("Resources");

            let _ = fs::create_dir_all(&macos_dir);
            let _ = fs::create_dir_all(&resources_dir);
            let _ = fs::write(app_dir.join("Info.plist"), &info_plist);
            let _ = fs::write(app_dir.join("PkgInfo"), b"APPL????");

            let icns_src = Path::new("assets/laketex.icns");
            if icns_src.is_file() {
                let _ = fs::copy(icns_src, resources_dir.join("laketex.icns"));
            }

            #[cfg(unix)]
            {
                let bin_link = macos_dir.join("laketex");
                if !bin_link.exists() {
                    let _ = std::os::unix::fs::symlink("../../../laketex", &bin_link);
                }
            }
        }
    }
}
