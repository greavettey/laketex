//! # Document Preview Color Modes
//!
//! Defines color rendering modes supported by both the pure-Rust native vector renderer
//! and the Google PDFium rasterizer.

use serde::{Deserialize, Serialize};

/// Color rendering mode for PDF preview and native vector pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PdfColorMode {
    /// Standard daylight paper (black text on white background)
    Normal,
    /// Inverted academic dark mode (white text on dark background)
    AcademicDark,
    /// Warm eye-care paper tone (soft vintage parchment)
    Sepia,
}

impl Default for PdfColorMode {
    fn default() -> Self {
        Self::AcademicDark
    }
}

impl PdfColorMode {
    /// Returns human-readable label for UI settings and comboboxes.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Normal => "Daylight (White Paper)",
            Self::AcademicDark => "Academic Dark (Inverted)",
            Self::Sepia => "Sepia (Eye-Care Parchment)",
        }
    }

    /// Returns true if this color mode represents an inverted dark theme.
    pub fn is_dark(&self) -> bool {
        matches!(self, Self::AcademicDark)
    }

    /// Returns all available color modes for selection menus.
    pub fn all() -> [Self; 3] {
        [Self::Normal, Self::AcademicDark, Self::Sepia]
    }
}
