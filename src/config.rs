use crate::error::KiviError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_path: Option<String>,
    pub show_values: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            address: None,
            mount: None,
            default_path: None,
            show_values: false,
        }
    }
}

impl Config {
    pub fn resolve_path(&self, user_path: Option<&str>) -> Result<String, KiviError> {
        let default = self.default_path.as_deref().filter(|s| !s.is_empty());
        match (user_path.filter(|s| !s.is_empty()), default) {
            (Some(p), Some(d)) => Ok(format!(
                "{}/{}",
                d.trim_end_matches('/'),
                p.trim_start_matches('/')
            )),
            (Some(p), None) => Ok(p.to_string()),
            (None, Some(d)) => Ok(d.to_string()),
            (None, None) => Err(KiviError::Usage(
                "No path provided and no default_path configured.\n  \
                 Pass a path argument or run: kivi config set --default-path <path>"
                    .to_string(),
            )),
        }
    }

    pub fn path() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config"));
        base.join("kivi").join("config.toml")
    }

    pub fn load() -> Result<Self, KiviError> {
        let path = Self::path();

        if !path.exists() {
            let config = Config::default();
            config.save()?;
            eprintln!("Created default config at {}", path.display());
            return Ok(config);
        }

        let contents = std::fs::read_to_string(&path)
            .map_err(|e| KiviError::Config(format!("Failed to read {}: {e}", path.display())))?;

        toml::from_str(&contents)
            .map_err(|e| KiviError::Config(format!("Failed to parse {}: {e}", path.display())))
    }

    pub fn save(&self) -> Result<(), KiviError> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)
            .map_err(|e| KiviError::Config(format!("Failed to serialize config: {e}")))?;
        std::fs::write(&path, contents)?;
        Ok(())
    }
}
