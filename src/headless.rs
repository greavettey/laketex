//! # Headless Execution, CLI & REST API Service
//!
//! Provides CLI automation commands (`compile`, `check`, `watch`, `compare`, `serve`) and
//! a lightweight asynchronous REST API daemon for programmatic interaction with LakeTex.

pub mod cli;
pub mod server;

pub use cli::CliArgs;
pub use server::{
    DocumentEditOperation, HeadlessServer, LiveEditorSnapshot, SharedAppState, StatefulUiCommand,
};
