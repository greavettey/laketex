//! Runs a headless CLI subcommand if one is given, otherwise starts the GUI
//! (optionally with the REST API in the background).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use laketex::headless::{CliArgs, HeadlessServer, SharedAppState};
use laketex::ui::LakeTexApp;
use std::sync::atomic::Ordering;
use std::sync::Arc;

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

use laketex::{log_startup, MAIN_START};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    MAIN_START.get_or_init(std::time::Instant::now);
    log_startup("main() started");

    #[cfg(windows)]
    attach_parent_console();
    log_startup("attach_parent_console done");

    let args = CliArgs::parse();
    log_startup("CliArgs::parse done");

    if let Some(cmd) = args.command {
        return CliArgs::execute_subcommand(cmd);
    }

    if args.with_state && !args.with_api {
        eprintln!(
            "[Laketex] Warning: --with-state requires --with-api to enable the stateful REST API server."
        );
    }
    let config = laketex::ui::config::AppConfig::load();
    log_startup("AppConfig::load done");
    let api_port = args.with_api.then_some(args.api_port);
    let shared_api_state =
        (args.with_api && args.with_state).then(|| Arc::new(SharedAppState::default()));
    let api_handle = api_port.map(|port| {
        HeadlessServer::start_background_with_state(
            "127.0.0.1",
            port,
            config.allow_shell_escape,
            shared_api_state.clone(),
        )
    });

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

    log_startup("eframe::run_native about to be called");

    eframe::run_native(
        "Laketex",
        native_options,
        Box::new(move |cc| {
            log_startup("eframe closure started -> calling LakeTexApp::new_with_state");
            let app = LakeTexApp::new_with_state(
                cc,
                file_to_open,
                api_port,
                shared_api_state,
            );
            log_startup("LakeTexApp::new_with_state finished");
            Ok(Box::new(app))
        }),
    )?;

    if let Some(Ok(handle)) = api_handle {
        handle.store(false, Ordering::SeqCst);
    }

    Ok(())
}
