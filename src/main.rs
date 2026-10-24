//! # LakeTex Main Binary Entry Point
//!
//! Evaluates command-line arguments to either execute headless commands (`compile`, `check`,
//! `watch`, `compare`, `serve`) or initialize the `eframe` GUI desktop application with
//! an embedded background REST API daemon.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use laketex::headless::{CliArgs, HeadlessServer};
use laketex::ui::LakeTexApp;
use std::sync::atomic::Ordering;

#[cfg(windows)]
fn attach_parent_console() {
    unsafe extern "system" {
        fn AttachConsole(dwProcessId: u32) -> i32;
        fn GetStdHandle(nStdHandle: u32) -> *mut std::ffi::c_void;
        fn SetStdHandle(nStdHandle: u32, hHandle: *mut std::ffi::c_void) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;
    const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5;
    const STD_ERROR_HANDLE: u32 = 0xFFFFFFF4;
    const INVALID_HANDLE_VALUE: *mut std::ffi::c_void = -1isize as _;

    unsafe {
        let stdout_h = GetStdHandle(STD_OUTPUT_HANDLE);
        if stdout_h.is_null() || stdout_h == INVALID_HANDLE_VALUE {
            if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
                use std::fs::OpenOptions;
                use std::os::windows::io::IntoRawHandle;
                if let Ok(file) = OpenOptions::new().write(true).open("CONOUT$") {
                    let handle = file.into_raw_handle();
                    SetStdHandle(STD_OUTPUT_HANDLE, handle as _);
                    SetStdHandle(STD_ERROR_HANDLE, handle as _);
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    attach_parent_console();

    let args = CliArgs::parse();

    // If a CLI subcommand was provided, run headless and exit
    if let Some(cmd) = args.command {
        return CliArgs::execute_subcommand(cmd);
    }

    // Launch background headless REST API server for programmatic interaction
    let api_port = args.api_port;
    let _api_handle = HeadlessServer::start_background("127.0.0.1", api_port);

    // Launch egui / eframe GUI application
    let app_icon = laketex::ui::logo::app_icon_data();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Laketex - Lightweight LaTeX Studio")
            .with_icon(app_icon)
            .with_inner_size([1360.0, 840.0])
            .with_min_inner_size([800.0, 520.0])
            .with_active(true),
        ..Default::default()
    };

    let file_to_open = args.file;

    eframe::run_native(
        "Laketex",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(LakeTexApp::new(cc, file_to_open, api_port)))
        }),
    )?;

    if let Ok(handle) = _api_handle {
        handle.store(false, Ordering::SeqCst);
    }

    Ok(())
}
