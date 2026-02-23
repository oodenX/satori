use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::cli::Position;
use crate::i18n;

#[derive(Debug, Deserialize, Serialize)]
pub struct AppConfig {
    pub settings: Settings,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    pub active_profile: String,
    #[serde(default = "default_target_lang")]
    pub target_lang: String,
    #[serde(default = "default_translation_style")]
    pub translation_style: String,
    #[serde(default = "default_ui_opacity")]
    pub ui_opacity: f64,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_pos: Option<Position>,
    /// Persisted margin offsets from dragging: [top, bottom, left, right]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_margins: Option<[i32; 4]>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_opacity: Option<f64>,
    /// GTK renderer: "auto" (default) or "cairo" (lower memory)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renderer: Option<String>,
    /// Position mode: "dynamic" (remember last position) or "fixed" (reset each time)
    #[serde(default = "default_position_mode")]
    pub position_mode: String,
    /// GUI backend
    #[serde(default)]
    pub gui_backend: GuiBackend,
}

/// GUI framework for the overlay window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GuiBackend {
    #[default]
    Gtk4,
    Slint,
}

fn default_target_lang() -> String {
    i18n::detect_target_lang()
}

fn default_translation_style() -> String {
    "general".to_string()
}

fn default_ui_opacity() -> f64 {
    0.9
}

fn default_position_mode() -> String {
    "dynamic".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Profile {
    pub driver: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    pub base_url: String,
    pub model: String,
    #[serde(default = "default_timeout")]
    pub timeout_sec: u64,
}

fn default_timeout() -> u64 {
    15
}

impl AppConfig {
    #[cfg(test)]
    pub fn active_profile(&self) -> Option<&Profile> {
        self.profiles.get(&self.settings.active_profile)
    }
}

impl Profile {
    pub fn resolve_api_key(&self) -> Result<Option<String>> {
        match &self.api_key_env {
            Some(env_var) => {
                let key = std::env::var(env_var)
                    .with_context(|| format!("Environment variable '{env_var}' not set"))?;
                Ok(Some(key))
            }
            None => Ok(None),
        }
    }
}

fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn config_dir() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME environment variable not set");
            PathBuf::from(home).join(".config")
        });
    base.join("satori")
}

pub fn config_exists() -> bool {
    config_path().exists()
}

pub fn load_config() -> Result<AppConfig> {
    let path = config_path();
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;
    let config: AppConfig =
        toml::from_str(&content).with_context(|| "Failed to parse config file")?;
    Ok(config)
}

/// Save an entire config back to disk.
#[cfg(feature = "gui")]
pub fn save_config(config: &AppConfig) -> Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    let path = config_path();
    let toml_str = toml::to_string_pretty(config).context("Failed to serialize config")?;
    std::fs::write(&path, &toml_str)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// Persist the last overlay position and drag margins to config.
#[cfg(feature = "gui")]
pub fn save_last_pos(pos: Position, margins: Option<[i32; 4]>) -> Result<()> {
    let mut config = load_config().unwrap_or_else(|_| AppConfig {
        settings: Settings {
            active_profile: String::new(),
            target_lang: default_target_lang(),
            translation_style: default_translation_style(),
            ui_opacity: default_ui_opacity(),
            last_pos: None,
            last_margins: None,
            font: None,
            color: None,
            background_color: None,
            background_opacity: None,
            renderer: None,
            position_mode: default_position_mode(),
            gui_backend: GuiBackend::default(),
        },
        profiles: HashMap::new(),
    });
    config.settings.last_pos = Some(pos);
    config.settings.last_margins = margins;
    save_config(&config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_config() {
        let toml_str = r#"
[settings]
active_profile = "openrouter_gemini"
target_lang = "简体中文"
ui_opacity = 0.85
last_pos = "top-right"

[profiles.openrouter_gemini]
driver = "openai_compatible"
api_key_env = "SATORI_OPENROUTER_KEY"
base_url = "https://openrouter.ai/api/v1"
model = "google/gemini-2.5-flash-preview-09-2025"
timeout_sec = 15

[profiles.local_ollama]
driver = "ollama"
base_url = "http://localhost:11434"
model = "llama3-vision"
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.settings.active_profile, "openrouter_gemini");
        assert_eq!(config.settings.target_lang, "简体中文");
        assert!((config.settings.ui_opacity - 0.85).abs() < f64::EPSILON);
        assert_eq!(config.settings.last_pos, Some(Position::TopRight));
        assert_eq!(config.profiles.len(), 2);

        let profile = config.active_profile().unwrap();
        assert_eq!(profile.driver, "openai_compatible");
        assert_eq!(
            profile.api_key_env.as_deref(),
            Some("SATORI_OPENROUTER_KEY")
        );
        assert_eq!(profile.timeout_sec, 15);
    }

    #[test]
    fn parse_minimal_config() {
        let toml_str = r#"
[settings]
active_profile = "test"

[profiles.test]
driver = "ollama"
base_url = "http://localhost:11434"
model = "llava"
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        // target_lang default is locale-dependent
        assert_eq!(config.settings.target_lang, default_target_lang());
        assert!((config.settings.ui_opacity - 0.9).abs() < f64::EPSILON);

        let profile = config.active_profile().unwrap();
        assert!(profile.api_key_env.is_none());
        assert_eq!(profile.timeout_sec, 15);
    }

    #[test]
    fn missing_active_profile() {
        let toml_str = r#"
[settings]
active_profile = "nonexistent"

[profiles.test]
driver = "ollama"
base_url = "http://localhost:11434"
model = "llava"
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert!(config.active_profile().is_none());
    }

    #[test]
    fn resolve_api_key_from_env() {
        let profile = Profile {
            driver: "openai_compatible".to_string(),
            api_key_env: Some("SATORI_TEST_KEY_12345".to_string()),
            base_url: "https://example.com".to_string(),
            model: "test".to_string(),
            timeout_sec: 10,
        };

        // SAFETY: test-only, single-threaded access to environment variable
        unsafe { std::env::set_var("SATORI_TEST_KEY_12345", "sk-secret") };
        let key = profile.resolve_api_key().unwrap();
        assert_eq!(key, Some("sk-secret".to_string()));
        unsafe { std::env::remove_var("SATORI_TEST_KEY_12345") };
    }

    #[test]
    fn resolve_api_key_missing_env() {
        let profile = Profile {
            driver: "openai_compatible".to_string(),
            api_key_env: Some("SATORI_NONEXISTENT_VAR".to_string()),
            base_url: "https://example.com".to_string(),
            model: "test".to_string(),
            timeout_sec: 10,
        };
        assert!(profile.resolve_api_key().is_err());
    }

    #[test]
    fn resolve_api_key_no_env_configured() {
        let profile = Profile {
            driver: "ollama".to_string(),
            api_key_env: None,
            base_url: "http://localhost:11434".to_string(),
            model: "llava".to_string(),
            timeout_sec: 10,
        };
        let key = profile.resolve_api_key().unwrap();
        assert_eq!(key, None);
    }
}
