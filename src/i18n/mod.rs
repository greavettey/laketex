//! # Internationalization & Localization Subsystem (i18n)
//!
//! Provides a standardized localization catalog and translation engine for LakeTex.
//! All user-facing strings are mapped to standard dot-delimited translation keys
//! (e.g., `toolbar.compile`, `editor.find`, `settings.title`).
//!
//! Features:
//! - Standardized JSON locale resource format (`locales/en.json`, `locales/es.json`).
//! - Bundled compile-time embedded fallback catalogs.
//! - Dynamic filesystem loading for external community translations in `./locales/`
//!   or alongside the executable.
//! - Variable interpolation (`{current}`, `{total}`, `{ms}`, etc.).
//! - Thread-safe global accessor and convenient `t!` macro.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

const DEFAULT_EN_LOCALE: &str = include_str!("../../locales/en.json");
const BUNDLED_ES_LOCALE: &str = include_str!("../../locales/es.json");

/// Global thread-safe i18n manager instance.
static I18N: LazyLock<RwLock<I18nCatalog>> = LazyLock::new(|| {
    let mut catalog = I18nCatalog::new();
    catalog.load_defaults();
    catalog.discover_external_locales();
    RwLock::new(catalog)
});

/// Map of flattened dot-delimited translation keys to localized format strings.
pub type StringTable = HashMap<String, String>;

/// Central catalog containing loaded languages and the active locale.
pub struct I18nCatalog {
    active_language: String,
    languages: HashMap<String, StringTable>,
    language_names: HashMap<String, String>,
}

impl Default for I18nCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl I18nCatalog {
    /// Creates an empty i18n catalog.
    pub fn new() -> Self {
        Self {
            active_language: "en".to_string(),
            languages: HashMap::new(),
            language_names: HashMap::new(),
        }
    }

    /// Loads built-in embedded localization catalogs.
    pub fn load_defaults(&mut self) {
        self.load_from_json_str("en", "English", DEFAULT_EN_LOCALE);
        self.load_from_json_str("es", "Español", BUNDLED_ES_LOCALE);
        self.active_language = "en".to_string();
    }

    /// Discovers user-provided `.json` locale files in local and binary-adjacent `locales/` folders.
    pub fn discover_external_locales(&mut self) {
        let candidate_dirs = [
            std::path::PathBuf::from("locales"),
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("locales")))
                .unwrap_or_else(|| std::path::PathBuf::from("locales")),
        ];

        for dir in candidate_dirs {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                let display_name = match stem {
                                    "en" => "English",
                                    "es" => "Español",
                                    "de" => "Deutsch",
                                    "fr" => "Français",
                                    "it" => "Italiano",
                                    "pt" => "Português",
                                    "ru" => "Русский",
                                    "zh" | "zh-CN" => "简体中文",
                                    "ja" => "日本語",
                                    other => other,
                                };
                                self.load_from_json_str(stem, display_name, &content);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Parses and registers a language string table from raw JSON content.
    pub fn load_from_json_str(&mut self, code: &str, display_name: &str, json_content: &str) {
        if let Ok(parsed) = serde_json::from_str::<Value>(json_content) {
            let mut table = HashMap::new();
            Self::flatten_json(&parsed, String::new(), &mut table);
            self.languages.insert(code.to_string(), table);
            self.language_names
                .insert(code.to_string(), display_name.to_string());
        }
    }

    /// Recursively flattens JSON objects into dot-separated keys: e.g. `toolbar.compile`.
    fn flatten_json(val: &Value, prefix: String, table: &mut StringTable) {
        match val {
            Value::Object(map) => {
                for (k, v) in map {
                    let next_prefix = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{prefix}.{k}")
                    };
                    Self::flatten_json(v, next_prefix, table);
                }
            }
            Value::String(s) => {
                table.insert(prefix, s.clone());
            }
            Value::Number(n) => {
                table.insert(prefix, n.to_string());
            }
            Value::Bool(b) => {
                table.insert(prefix, b.to_string());
            }
            _ => {}
        }
    }

    /// Sets the active interface language code (e.g. `"en"`, `"es"`).
    pub fn set_language(&mut self, lang_code: &str) {
        if self.languages.contains_key(lang_code) {
            self.active_language = lang_code.to_string();
        } else {
            // Check prefix (e.g. "en-US" -> "en")
            let prefix = lang_code.split(['-', '_']).next().unwrap_or("en");
            if self.languages.contains_key(prefix) {
                self.active_language = prefix.to_string();
            }
        }
    }

    /// Returns the currently active language code.
    pub fn current_language(&self) -> &str {
        &self.active_language
    }

    /// Returns all registered language codes and their display names.
    pub fn available_languages(&self) -> Vec<(String, String)> {
        let mut list: Vec<(String, String)> = self
            .language_names
            .iter()
            .map(|(code, name)| (code.clone(), name.clone()))
            .collect();
        list.sort_by(|a, b| a.0.cmp(&b.0));
        list
    }

    /// Translates a key using the active language with English fallback.
    pub fn translate(&self, key: &str) -> String {
        if let Some(table) = self.languages.get(&self.active_language) {
            if let Some(val) = table.get(key) {
                return val.clone();
            }
        }
        // Fallback to English
        if let Some(en_table) = self.languages.get("en") {
            if let Some(val) = en_table.get(key) {
                return val.clone();
            }
        }
        // Key as fallback
        key.to_string()
    }

    /// Translates a key and interpolates `{arg}` placeholders with given replacements.
    pub fn translate_args(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut text = self.translate(key);
        for &(placeholder, val) in args {
            let target = format!("{{{placeholder}}}");
            text = text.replace(&target, val);
        }
        text
    }
}

/// Global convenience function to translate a key.
pub fn tr(key: &str) -> String {
    I18N.read().unwrap().translate(key)
}

/// Global convenience function to translate a key with placeholder arguments.
pub fn tr_args(key: &str, args: &[(&str, &str)]) -> String {
    I18N.read().unwrap().translate_args(key, args)
}

/// Global function to set the active language.
pub fn set_locale(code: &str) {
    I18N.write().unwrap().set_language(code);
}

/// Global function returning the current active locale code.
pub fn current_locale() -> String {
    I18N.read().unwrap().current_language().to_string()
}

/// Global function returning all available languages as `(code, display_name)`.
pub fn available_locales() -> Vec<(String, String)> {
    I18N.read().unwrap().available_languages()
}

/// Convenience macro for translating dot-delimited keys.
///
/// Usage:
/// ```ignore
/// t!("toolbar.compile")
/// t!("editor.words", "count" => 42)
/// t!("viewer.page_indicator", "current" => 1, "total" => 5)
/// ```
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::tr($key)
    };
    ($key:expr, $( $arg:expr => $val:expr ),* $(,)?) => {{
        let args = [
            $( ($arg, &$val.to_string() as &str) ),*
        ];
        $crate::i18n::tr_args($key, &args)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i18n_default_translations() {
        let mut cat = I18nCatalog::new();
        cat.load_defaults();

        assert_eq!(cat.translate("toolbar.compile"), "▶ Compile");
        assert_eq!(cat.translate("editor.find"), "Find");
        assert_eq!(
            cat.translate_args("editor.match_count", &[("current", "1"), ("total", "10")]),
            "1 of 10"
        );
    }

    #[test]
    fn test_i18n_language_switching() {
        let mut cat = I18nCatalog::new();
        cat.load_defaults();

        cat.set_language("es");
        assert_eq!(cat.translate("toolbar.compile"), "▶ Compilar");
        assert_eq!(cat.translate("editor.find"), "Buscar");
        assert_eq!(
            cat.translate_args("editor.match_count", &[("current", "3"), ("total", "8")]),
            "3 de 8"
        );

        // Fallback on missing key
        assert_eq!(cat.translate("nonexistent.key"), "nonexistent.key");
    }

    #[test]
    fn test_t_macro() {
        set_locale("en");
        let s = t!("toolbar.save_file");
        assert_eq!(s, "Save");

        let dynamic = t!("editor.match_count", "current" => 2, "total" => 5);
        assert_eq!(dynamic, "2 of 5");
    }
}
