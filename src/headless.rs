//! CLI subcommands and headless HTTP server.

pub mod cli;
pub mod server;

pub use cli::CliArgs;
pub use server::{
    DocumentEditOperation, HeadlessServer, LiveEditorSnapshot, SharedAppState, StatefulUiCommand,
};
