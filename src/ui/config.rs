//! # Application Configuration & Persistent Settings
//!
//! Handles saving and loading user preferences to `laketex_config.json` alongside
//! the application executable, with graceful fallback to user app data.

use crate::compiler::engine::EngineKind;
use crate::ui::viewer_panel::ViewMode;
use crate::viewer::PdfColorMode;
use egui::{Key, Modifiers};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application settings persisted to disk in `laketex_config.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    /// Active theme name (e.g. "Dracula (JetBrains)", "Slate Blue", "Nord Frost", "Paper White")
    pub theme_name: String,

    /// Default PDF preview color mode (Normal Daylight, Academic Dark, Sepia Eye-Care)
    pub pdf_color_mode: PdfColorMode,

    /// Default PDF raster resolution base width in pixels (e.g. 1000, 1200, 1600, 2000)
    pub pdf_render_width: u16,

    /// Default preview mode (Native Vector vs PDF Vector View)
    pub view_mode: ViewMode,

    /// Default typesetting engine (Native, Tectonic, pdfLaTeX, XeLaTeX, LuaLaTeX)
    pub default_engine: EngineKind,

    /// Whether auto-compilation on edit is enabled by default
    pub auto_compile: bool,

    /// Whether PDF preview automatically fits to panel width
    pub fit_to_width: bool,

    /// Active interface language (e.g. "en", "es")
    pub language: String,

    /// Editor font size in points (default: 13.0)
    pub font_size: f32,

    /// User configurable keyboard shortcuts
    pub shortcuts: KeyBindingConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme_name: "Dracula (JetBrains)".to_string(),
            pdf_color_mode: PdfColorMode::Normal,
            pdf_render_width: 1200,
            view_mode: ViewMode::PdfRaster,
            default_engine: EngineKind::Tectonic,
            auto_compile: true,
            fit_to_width: true,
            language: "en".to_string(),
            font_size: 13.0,
            shortcuts: KeyBindingConfig::default(),
        }
    }
}

impl AppConfig {
    /// Primary path to configuration file located in the same directory as the executable.
    pub fn config_path() -> PathBuf {
        if let Ok(mut exe) = std::env::current_exe() {
            exe.pop();
            return exe.join("laketex_config.json");
        }
        PathBuf::from("laketex_config.json")
    }

    /// Fallback directory in user app data if the executable directory is write-protected.
    pub fn fallback_config_path() -> PathBuf {
        if let Some(bin_dir) = crate::compiler::CompilerEngine::laketex_bin_dir() {
            if let Some(parent) = bin_dir.parent() {
                return parent.join("laketex_config.json");
            }
        }
        PathBuf::from("laketex_config.json")
    }

    /// Loads application configuration from disk.
    /// Checks next to exe first; then fallback directory; returns defaults if not found.
    pub fn load() -> Self {
        let mut cfg = Self::default();
        let primary = Self::config_path();
        if primary.is_file() {
            if let Ok(content) = std::fs::read_to_string(&primary) {
                if let Ok(loaded) = serde_json::from_str::<AppConfig>(&content) {
                    cfg = loaded;
                }
            }
        } else {
            let fallback = Self::fallback_config_path();
            if fallback.is_file() {
                if let Ok(content) = std::fs::read_to_string(&fallback) {
                    if let Ok(loaded) = serde_json::from_str::<AppConfig>(&content) {
                        cfg = loaded;
                    }
                }
            }
        }

        crate::i18n::set_locale(&cfg.language);
        cfg
    }

    /// Saves application configuration to disk.
    /// Attempts to write next to the executable first, falling back if not permitted.
    pub fn save(&self) -> std::io::Result<PathBuf> {
        let json_text = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let primary = Self::config_path();
        match std::fs::write(&primary, &json_text) {
            Ok(()) => Ok(primary),
            Err(primary_err) => {
                let fallback = Self::fallback_config_path();
                if let Some(parent) = fallback.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(&fallback, &json_text) {
                    Ok(()) => Ok(fallback),
                    Err(_) => Err(primary_err),
                }
            }
        }
    }
}

/// User customizable keyboard shortcuts configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct KeyBindingConfig {
    /// Recompile document shortcut (default: `"Ctrl+B"`).
    pub compile: String,
    /// Save active file shortcut (default: `"Ctrl+S"`).
    pub save: String,
    /// Open file dialog shortcut (default: `"Ctrl+O"`).
    pub open: String,
    /// Create new document shortcut (default: `"Ctrl+N"`).
    pub new_doc: String,
    /// Toggle project files drawer shortcut (default: `"Ctrl+P"`).
    pub project_files: String,
    /// Find search drawer shortcut (default: `"Ctrl+F"`).
    pub find: String,
    /// Find and replace drawer shortcut (default: `"Ctrl+H"`).
    pub replace: String,
    /// Open settings preferences modal shortcut (default: `"Ctrl+,"`).
    pub settings: String,
}

impl Default for KeyBindingConfig {
    fn default() -> Self {
        Self {
            compile: "Ctrl+B".to_string(),
            save: "Ctrl+S".to_string(),
            open: "Ctrl+O".to_string(),
            new_doc: "Ctrl+N".to_string(),
            project_files: "Ctrl+P".to_string(),
            find: "Ctrl+F".to_string(),
            replace: "Ctrl+H".to_string(),
            settings: "Ctrl+,".to_string(),
        }
    }
}

impl KeyBindingConfig {
    /// Checks whether the compile action key combination was pressed this frame.
    pub fn matches_compile(&self, ctx: &egui::Context) -> bool {
        matches_shortcut(&self.compile, ctx)
    }

    /// Checks whether the save file action key combination was pressed this frame.
    pub fn matches_save(&self, ctx: &egui::Context) -> bool {
        matches_shortcut(&self.save, ctx)
    }

    /// Checks whether the open file action key combination was pressed this frame.
    pub fn matches_open(&self, ctx: &egui::Context) -> bool {
        matches_shortcut(&self.open, ctx)
    }

    /// Checks whether the new document action key combination was pressed this frame.
    pub fn matches_new(&self, ctx: &egui::Context) -> bool {
        matches_shortcut(&self.new_doc, ctx)
    }

    /// Checks whether the project files modal action key combination was pressed this frame.
    pub fn matches_project_files(&self, ctx: &egui::Context) -> bool {
        matches_shortcut(&self.project_files, ctx)
    }

    /// Checks whether the find search action key combination was pressed this frame.
    pub fn matches_find(&self, ctx: &egui::Context) -> bool {
        matches_shortcut(&self.find, ctx)
    }

    /// Checks whether the find and replace action key combination was pressed this frame.
    pub fn matches_replace(&self, ctx: &egui::Context) -> bool {
        matches_shortcut(&self.replace, ctx)
    }

    /// Checks whether the settings preferences action key combination was pressed this frame.
    pub fn matches_settings(&self, ctx: &egui::Context) -> bool {
        matches_shortcut(&self.settings, ctx)
    }
}

/// Tests whether the keyboard state in the given context matches the shortcut string,
/// and consumes the key event if matched.
pub fn matches_shortcut(shortcut_str: &str, ctx: &egui::Context) -> bool {
    let Some((mods, key)) = parse_shortcut(shortcut_str) else {
        return false;
    };
    ctx.input_mut(|i| i.consume_key(mods, key))
}

/// Parses a human-readable shortcut string (e.g. "Ctrl+B", "Ctrl+Shift+S", "F5", "Ctrl+,")
/// into an `egui::Modifiers` and `egui::Key` pair.
pub fn parse_shortcut(input: &str) -> Option<(Modifiers, Key)> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut mods = Modifiers::NONE;
    let mut key: Option<Key> = None;

    // Handle single "+" or trailing "+" if any
    let parts: Vec<&str> = trimmed.split('+').map(|s| s.trim()).collect();
    for part in parts {
        let lower = part.to_lowercase();
        match lower.as_str() {
            "ctrl" | "control" => {
                mods.ctrl = true;
                mods.command = true;
            }
            "shift" => mods.shift = true,
            "alt" => mods.alt = true,
            "cmd" | "command" | "super" => mods.command = true,
            _ => {
                if let Some(parsed_k) = parse_key(part) {
                    key = Some(parsed_k);
                }
            }
        }
    }

    key.map(|k| (mods, k))
}

/// Parses a single key token into an `egui::Key`.
pub fn parse_key(token: &str) -> Option<Key> {
    let lower = token.trim().to_lowercase();
    match lower.as_str() {
        "a" => Some(Key::A),
        "b" => Some(Key::B),
        "c" => Some(Key::C),
        "d" => Some(Key::D),
        "e" => Some(Key::E),
        "f" => Some(Key::F),
        "g" => Some(Key::G),
        "h" => Some(Key::H),
        "i" => Some(Key::I),
        "j" => Some(Key::J),
        "k" => Some(Key::K),
        "l" => Some(Key::L),
        "m" => Some(Key::M),
        "n" => Some(Key::N),
        "o" => Some(Key::O),
        "p" => Some(Key::P),
        "q" => Some(Key::Q),
        "r" => Some(Key::R),
        "s" => Some(Key::S),
        "t" => Some(Key::T),
        "u" => Some(Key::U),
        "v" => Some(Key::V),
        "w" => Some(Key::W),
        "x" => Some(Key::X),
        "y" => Some(Key::Y),
        "z" => Some(Key::Z),
        "0" | "num0" => Some(Key::Num0),
        "1" | "num1" => Some(Key::Num1),
        "2" | "num2" => Some(Key::Num2),
        "3" | "num3" => Some(Key::Num3),
        "4" | "num4" => Some(Key::Num4),
        "5" | "num5" => Some(Key::Num5),
        "6" | "num6" => Some(Key::Num6),
        "7" | "num7" => Some(Key::Num7),
        "8" | "num8" => Some(Key::Num8),
        "9" | "num9" => Some(Key::Num9),
        "f1" => Some(Key::F1),
        "f2" => Some(Key::F2),
        "f3" => Some(Key::F3),
        "f4" => Some(Key::F4),
        "f5" => Some(Key::F5),
        "f6" => Some(Key::F6),
        "f7" => Some(Key::F7),
        "f8" => Some(Key::F8),
        "f9" => Some(Key::F9),
        "f10" => Some(Key::F10),
        "f11" => Some(Key::F11),
        "f12" => Some(Key::F12),
        "," | "comma" => Some(Key::Comma),
        "." | "period" => Some(Key::Period),
        ";" | "semicolon" => Some(Key::Semicolon),
        ":" | "colon" => Some(Key::Colon),
        "enter" | "return" => Some(Key::Enter),
        "space" => Some(Key::Space),
        "tab" => Some(Key::Tab),
        "escape" | "esc" => Some(Key::Escape),
        "backspace" => Some(Key::Backspace),
        "delete" | "del" => Some(Key::Delete),
        _ => None,
    }
}

/// Formats a key and modifier set into a clean shortcut string (e.g. "Ctrl+B", "Ctrl+Shift+S").
pub fn format_shortcut(mods: Modifiers, key: Key) -> String {
    let mut parts = Vec::new();
    if mods.ctrl || mods.command {
        parts.push("Ctrl");
    }
    if mods.alt {
        parts.push("Alt");
    }
    if mods.shift {
        parts.push("Shift");
    }

    let key_name = format_key(key);
    parts.push(&key_name);
    parts.join("+")
}

/// Formats a single `egui::Key` into a user-readable display name.
pub fn format_key(key: Key) -> String {
    match key {
        Key::Comma => ",".to_string(),
        Key::Period => ".".to_string(),
        Key::Semicolon => ";".to_string(),
        Key::Colon => ":".to_string(),
        Key::Num0 => "0".to_string(),
        Key::Num1 => "1".to_string(),
        Key::Num2 => "2".to_string(),
        Key::Num3 => "3".to_string(),
        Key::Num4 => "4".to_string(),
        Key::Num5 => "5".to_string(),
        Key::Num6 => "6".to_string(),
        Key::Num7 => "7".to_string(),
        Key::Num8 => "8".to_string(),
        Key::Num9 => "9".to_string(),
        k => format!("{k:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_format_shortcuts() {
        let (mods, key) = parse_shortcut("Ctrl+B").expect("parse Ctrl+B");
        assert!(mods.ctrl);
        assert_eq!(key, Key::B);
        assert_eq!(format_shortcut(mods, key), "Ctrl+B");

        let (mods, key) = parse_shortcut("Ctrl+Shift+S").expect("parse Ctrl+Shift+S");
        assert!(mods.ctrl);
        assert!(mods.shift);
        assert_eq!(key, Key::S);
        assert_eq!(format_shortcut(mods, key), "Ctrl+Shift+S");

        let (mods, key) = parse_shortcut("F5").expect("parse F5");
        assert!(!mods.ctrl);
        assert_eq!(key, Key::F5);
        assert_eq!(format_shortcut(mods, key), "F5");

        let (mods, key) = parse_shortcut("Ctrl+,").expect("parse Ctrl+,");
        assert!(mods.ctrl);
        assert_eq!(key, Key::Comma);
        assert_eq!(format_shortcut(mods, key), "Ctrl+,");
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let mut cfg = AppConfig::default();
        cfg.theme_name = "Slate Blue".to_string();
        cfg.pdf_color_mode = PdfColorMode::Sepia;
        cfg.pdf_render_width = 1600;
        cfg.shortcuts.compile = "F5".to_string();

        let json = serde_json::to_string_pretty(&cfg).expect("serialize");
        let decoded: AppConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cfg, decoded);
    }

    #[test]
    fn test_partial_config_defaults() {
        let partial_json = r#"{"theme_name": "Nord Frost", "pdf_render_width": 2000}"#;
        let cfg: AppConfig = serde_json::from_str(partial_json).expect("deserialize partial");
        assert_eq!(cfg.theme_name, "Nord Frost");
        assert_eq!(cfg.pdf_render_width, 2000);
        assert_eq!(cfg.pdf_color_mode, PdfColorMode::Normal);
        assert_eq!(cfg.shortcuts.compile, "Ctrl+B");
    }
}
