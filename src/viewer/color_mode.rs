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
        Self::Normal
    }
}

impl PdfColorMode {
    /// Returns the standardized i18n dot-delimited translation key for this mode.
    pub fn translation_key(&self) -> &'static str {
        match self {
            Self::Normal => "viewer.normal",
            Self::AcademicDark => "viewer.dark",
            Self::Sepia => "viewer.sepia",
        }
    }

    /// Returns the localized display name according to the active interface locale.
    pub fn localized_name(&self) -> String {
        crate::i18n::tr(self.translation_key())
    }

    /// Returns human-readable label for UI settings and comboboxes (English fallback).
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Normal => "Light",
            Self::AcademicDark => "Dark",
            Self::Sepia => "Sepia",
        }
    }

    /// Returns the i18n translation key for this mode's explanatory description.
    pub fn description_key(&self) -> &'static str {
        match self {
            Self::Normal => "viewer.normal_desc",
            Self::AcademicDark => "viewer.dark_desc",
            Self::Sepia => "viewer.sepia_desc",
        }
    }

    /// Returns the localized description string according to the active interface locale.
    pub fn localized_description(&self) -> String {
        crate::i18n::tr(self.description_key())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdf_color_mode_i18n_localization() {
        assert_eq!(PdfColorMode::Normal.localized_name(), "Normal");
        assert_eq!(PdfColorMode::AcademicDark.localized_name(), "Academic Dark");
        assert_eq!(PdfColorMode::Sepia.localized_name(), "Sepia");
        let mut cat = crate::i18n::I18nCatalog::new();
        cat.load_defaults();
        cat.set_language("es");
        assert_eq!(cat.translate(PdfColorMode::Normal.translation_key()), "Normal");
        assert_eq!(cat.translate(PdfColorMode::AcademicDark.translation_key()), "Oscuro Académico");
        assert_eq!(cat.translate(PdfColorMode::Sepia.translation_key()), "Sepia");
    }
}
