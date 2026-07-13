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

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(default_path: Option<&str>) -> Config {
        Config {
            default_path: default_path.map(|s| s.to_string()),
            ..Config::default()
        }
    }

    #[test]
    fn resolve_path_user_only() {
        let c = cfg(None);
        assert_eq!(c.resolve_path(Some("app/db")).unwrap(), "app/db");
    }

    #[test]
    fn resolve_path_default_only() {
        let c = cfg(Some("team/prod"));
        assert_eq!(c.resolve_path(None).unwrap(), "team/prod");
    }

    #[test]
    fn resolve_path_both_joins() {
        let c = cfg(Some("team/prod"));
        assert_eq!(c.resolve_path(Some("db")).unwrap(), "team/prod/db");
    }

    #[test]
    fn resolve_path_both_trims_slashes() {
        let c = cfg(Some("team/prod/"));
        assert_eq!(c.resolve_path(Some("/db")).unwrap(), "team/prod/db");
    }

    #[test]
    fn resolve_path_neither_errors() {
        let c = cfg(None);
        assert!(c.resolve_path(None).is_err());
    }

    #[test]
    fn resolve_path_empty_strings_treated_as_none() {
        let c = cfg(Some(""));
        assert!(c.resolve_path(Some("")).is_err());
    }

    #[test]
    fn resolve_path_empty_user_uses_default() {
        let c = cfg(Some("team/prod"));
        assert_eq!(c.resolve_path(Some("")).unwrap(), "team/prod");
    }

    #[test]
    fn resolve_path_empty_default_uses_user() {
        let c = cfg(Some(""));
        assert_eq!(c.resolve_path(Some("app/db")).unwrap(), "app/db");
    }

    #[test]
    fn default_config_show_values_is_false() {
        let c = Config::default();
        assert!(!c.show_values);
    }

    #[test]
    fn config_roundtrip_toml() {
        let c = Config {
            address: Some("https://vault.example.com".into()),
            mount: Some("secret".into()),
            default_path: Some("team/prod".into()),
            show_values: true,
        };
        let serialized = toml::to_string_pretty(&c).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.address.as_deref(), Some("https://vault.example.com"));
        assert_eq!(deserialized.mount.as_deref(), Some("secret"));
        assert_eq!(deserialized.default_path.as_deref(), Some("team/prod"));
        assert!(deserialized.show_values);
    }

    #[test]
    fn config_toml_omits_none_fields() {
        let c = Config::default();
        let serialized = toml::to_string_pretty(&c).unwrap();
        assert!(!serialized.contains("address"));
        assert!(!serialized.contains("mount"));
        assert!(!serialized.contains("default_path"));
    }
}
