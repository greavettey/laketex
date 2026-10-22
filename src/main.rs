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
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;
    extern "system" {
        fn AttachConsole(dwProcessId: u32) -> i32;
    }
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
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
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Laketex - Lightweight LaTeX Studio")
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
