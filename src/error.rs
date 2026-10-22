//! # Error Handling for LakeTex
//!
//! Defines the centralized [`LakeTexError`] enumeration and convenient [`Result`] alias
//! used throughout LakeTex subsystems.

#![allow(dead_code)]

use thiserror::Error;

/// Central error type encompassing all failure modes across the LakeTex system.
#[derive(Error, Debug)]
pub enum LakeTexError {
    /// Standard file or system I/O failure
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization or deserialization failure
    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// LaTeX compiler engine invocation failure
    #[error("Engine execution error: {0}")]
    Engine(String),

    /// LaTeX compilation completed with errors
    #[error("Compilation failed with {0} errors")]
    CompilationFailed(usize),

    /// Requested compiler backend executable was not found on PATH
    #[error("Engine not found: {0}")]
    EngineNotFound(String),

    /// CTAN package downloading or provisioning failure
    #[error("Provisioning error: {0}")]
    Provisioning(String),

    /// PDF rasterization or rendering failure
    #[error("PDF rendering error: {0}")]
    PdfRender(String),

    /// Network request failure during package download
    #[error("Network error: {0}")]
    Network(String),

    /// LaTeX AST or math syntax parsing failure
    #[error("Document parse error: {0}")]
    Parse(String),

    /// Package external tool dependency or permission error
    #[error("Dependency error: {0}")]
    Dependency(String),

    /// Unspecified general error
    #[error("General error: {0}")]
    General(String),
}

/// Convenience alias for results returned by LakeTex functions
pub type Result<T> = std::result::Result<T, LakeTexError>;
