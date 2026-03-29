use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub use sessfind_common::CHUNK_MAX_CHARS;
pub use sessfind_common::CHUNK_MIN_CHARS;
pub use sessfind_common::data_dir;

/// Path to config file: ~/.config/sessfind/config.json
pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
        })
        .join("sessfind")
        .join("config.json")
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Model per LLM backend, keyed by backend name (e.g. "claude", "opencode", "copilot").
    #[serde(default)]
    pub llm_models: HashMap<String, String>,
}

impl Config {
    /// Load config from disk. Returns default if file doesn't exist.
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save config to disk, creating parent dirs if needed.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Get model for a backend, or None if not configured.
    pub fn llm_model(&self, backend_name: &str) -> Option<&str> {
        self.llm_models.get(backend_name).map(|s| s.as_str())
    }
}

pub fn claude_projects_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
        .join("projects")
}

pub fn opencode_db_path() -> PathBuf {
    // OpenCode stores its DB in XDG_DATA_HOME (~/.local/share on Linux/macOS)
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
        .join("share")
        .join("opencode")
        .join("opencode.db")
}

pub fn copilot_session_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".copilot")
        .join("session-state")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_serde_roundtrip() {
        let mut config = Config::default();
        config.llm_models.insert("claude".into(), "sonnet".into());
        config
            .llm_models
            .insert("opencode".into(), "anthropic/claude-sonnet-4-6".into());

        let json = serde_json::to_string(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.llm_model("claude"), Some("sonnet"));
        assert_eq!(
            parsed.llm_model("opencode"),
            Some("anthropic/claude-sonnet-4-6")
        );
        assert_eq!(parsed.llm_model("copilot"), None);
    }

    #[test]
    fn config_default_is_empty() {
        let config = Config::default();
        assert!(config.llm_models.is_empty());
        assert_eq!(config.llm_model("claude"), None);
    }

    #[test]
    fn config_deserialize_missing_fields() {
        let json = "{}";
        let config: Config = serde_json::from_str(json).unwrap();
        assert!(config.llm_models.is_empty());
    }
}
