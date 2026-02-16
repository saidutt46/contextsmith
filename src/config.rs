use std::collections::HashMap;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::error::{ContextSmithError, Result};

/// Top-level configuration for ContextSmith.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub ignore: Vec<String>,
    pub generated: Vec<String>,
    pub default_budget: usize,
    pub reserve_tokens: usize,
    pub ranking_weights: RankingWeights,
    pub languages: HashMap<String, LanguageConfig>,
    pub cache: CacheConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RankingWeights {
    pub text: f64,
    pub diff: f64,
    pub recency: f64,
    pub proximity: f64,
    pub test: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LanguageConfig {
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CacheConfig {
    pub enabled: bool,
    pub dir: Option<PathBuf>,
}

// --- Defaults ---

impl Default for Config {
    fn default() -> Self {
        Self {
            ignore: vec![
                "node_modules".into(),
                "target".into(),
                "DerivedData".into(),
                ".next".into(),
                "dist".into(),
                "build".into(),
                ".contextsmith".into(),
                "*.min.js".into(),
                "*.map".into(),
            ],
            generated: vec![
                "*.pb.rs".into(),
                "*.pb.go".into(),
                "*_pb2.py".into(),
                "*.generated.*".into(),
            ],
            default_budget: 12000,
            reserve_tokens: 500,
            ranking_weights: RankingWeights::default(),
            languages: default_languages(),
            cache: CacheConfig::default(),
        }
    }
}

impl Default for RankingWeights {
    fn default() -> Self {
        Self {
            text: 1.0,
            diff: 2.0,
            recency: 0.5,
            proximity: 1.5,
            test: 0.8,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            dir: None,
        }
    }
}

fn default_languages() -> HashMap<String, LanguageConfig> {
    let mut m = HashMap::new();
    m.insert(
        "rust".into(),
        LanguageConfig {
            extensions: vec!["rs".into()],
        },
    );
    m.insert(
        "typescript".into(),
        LanguageConfig {
            extensions: vec!["ts".into(), "tsx".into()],
        },
    );
    m.insert(
        "python".into(),
        LanguageConfig {
            extensions: vec!["py".into()],
        },
    );
    m
}

// --- Config methods ---

impl Config {
    /// Load config from a TOML file.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            ContextSmithError::io(format!("reading config from '{}'", path.display()), e)
        })?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| ContextSmithError::config_with_source("failed to parse config", e))?;
        config.validate()?;
        Ok(config)
    }

    /// Save config to a TOML file.
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let content = toml::to_string_pretty(self)
            .map_err(|e| ContextSmithError::config_with_source("failed to serialize config", e))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ContextSmithError::io(
                    format!("creating config directory '{}'", parent.display()),
                    e,
                )
            })?;
        }
        std::fs::write(path, content).map_err(|e| {
            ContextSmithError::io(format!("writing config to '{}'", path.display()), e)
        })
    }

    /// Validate config values.
    pub fn validate(&self) -> Result<()> {
        if self.default_budget == 0 {
            return Err(ContextSmithError::validation(
                "default_budget",
                "must be greater than 0",
            ));
        }
        if self.reserve_tokens >= self.default_budget {
            return Err(ContextSmithError::validation(
                "reserve_tokens",
                "must be less than default_budget",
            ));
        }
        Ok(())
    }

    /// Merge overrides on top of this config (non-default fields win).
    pub fn merge(&mut self, overrides: Config) {
        if overrides.default_budget != Config::default().default_budget {
            self.default_budget = overrides.default_budget;
        }
        if overrides.reserve_tokens != Config::default().reserve_tokens {
            self.reserve_tokens = overrides.reserve_tokens;
        }
        if overrides.ignore != Config::default().ignore {
            self.ignore = overrides.ignore;
        }
        if overrides.generated != Config::default().generated {
            self.generated = overrides.generated;
        }
        if overrides.cache != Config::default().cache {
            self.cache = overrides.cache;
        }
    }
}

/// Builder for constructing Config with selective overrides.
pub struct ConfigBuilder {
    config: Config,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: Config::default(),
        }
    }

    pub fn with_budget(mut self, budget: usize) -> Self {
        self.config.default_budget = budget;
        self
    }

    pub fn with_reserve(mut self, reserve: usize) -> Self {
        self.config.reserve_tokens = reserve;
        self
    }

    pub fn with_cache_enabled(mut self, enabled: bool) -> Self {
        self.config.cache.enabled = enabled;
        self
    }

    pub fn build(self) -> Result<Config> {
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Discover the config file using standard search order:
/// 1. Explicit path (if provided)
/// 2. ./contextsmith.toml
/// 3. ~/.contextsmith.toml
/// 4. XDG config dir
pub fn find_config_file(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        if p.exists() {
            return Some(p.to_path_buf());
        }
        return None;
    }

    let local = PathBuf::from("contextsmith.toml");
    if local.exists() {
        return Some(local);
    }

    if let Some(home) = dirs_home() {
        let home_config = home.join(".contextsmith.toml");
        if home_config.exists() {
            return Some(home_config);
        }
    }

    if let Some(proj_dirs) = ProjectDirs::from("", "", "contextsmith") {
        let xdg = proj_dirs.config_dir().join("contextsmith.toml");
        if xdg.exists() {
            return Some(xdg);
        }
    }

    None
}

fn dirs_home() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        Config::default().validate().unwrap();
    }

    #[test]
    fn serde_roundtrip() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn validation_rejects_zero_budget() {
        let mut config = Config::default();
        config.default_budget = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validation_rejects_reserve_gte_budget() {
        let mut config = Config::default();
        config.reserve_tokens = config.default_budget;
        assert!(config.validate().is_err());
    }

    #[test]
    fn builder_with_budget() {
        let config = ConfigBuilder::new().with_budget(8000).build().unwrap();
        assert_eq!(config.default_budget, 8000);
    }

    #[test]
    fn merge_overrides_budget() {
        let mut base = Config::default();
        let mut overrides = Config::default();
        overrides.default_budget = 5000;
        base.merge(overrides);
        assert_eq!(base.default_budget, 5000);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("contextsmith.toml");
        let config = Config::default();
        config.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(config, loaded);
    }
}
