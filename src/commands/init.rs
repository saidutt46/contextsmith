use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::{ContextSmithError, Result};

/// Options for the `init` command.
pub struct InitOptions {
    pub root: PathBuf,
    pub config_path: Option<PathBuf>,
    pub force: bool,
    pub no_cache: bool,
}

/// Result of a successful `init` operation.
#[derive(Debug)]
pub struct InitResult {
    pub config_path: PathBuf,
    pub cache_dir: Option<PathBuf>,
    pub created_config: bool,
    pub created_cache: bool,
}

/// Run the init command: create config file and optional cache directory.
pub fn run(options: InitOptions) -> Result<InitResult> {
    // Validate root exists and is a directory
    if !options.root.exists() {
        return Err(ContextSmithError::invalid_path(
            options.root.display().to_string(),
            "directory does not exist",
        ));
    }
    if !options.root.is_dir() {
        return Err(ContextSmithError::invalid_path(
            options.root.display().to_string(),
            "not a directory",
        ));
    }

    // Determine config path
    let config_path = options
        .config_path
        .unwrap_or_else(|| options.root.join("contextsmith.toml"));

    // Check for existing config
    if config_path.exists() && !options.force {
        return Err(ContextSmithError::config(format!(
            "config already exists at '{}' (use --force to overwrite)",
            config_path.display()
        )));
    }

    // Create and save default config
    let config = Config::default();
    config.save(&config_path)?;

    // Create cache directory if requested
    let mut cache_dir = None;
    let mut created_cache = false;
    if !options.no_cache {
        let cache = options.root.join(".contextsmith").join("cache");
        create_dir_if_needed(&cache)?;
        cache_dir = Some(cache);
        created_cache = true;
    }

    Ok(InitResult {
        config_path,
        cache_dir,
        created_config: true,
        created_cache,
    })
}

fn create_dir_if_needed(path: &Path) -> Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path).map_err(|e| {
            ContextSmithError::io(format!("creating directory '{}'", path.display()), e)
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_config_and_cache() {
        let dir = tempfile::tempdir().unwrap();
        let result = run(InitOptions {
            root: dir.path().to_path_buf(),
            config_path: None,
            force: false,
            no_cache: false,
        })
        .unwrap();

        assert!(result.config_path.exists());
        assert!(result.created_config);
        assert!(result.created_cache);
        assert!(result.cache_dir.unwrap().exists());
    }

    #[test]
    fn init_no_cache_skips_cache_dir() {
        let dir = tempfile::tempdir().unwrap();
        let result = run(InitOptions {
            root: dir.path().to_path_buf(),
            config_path: None,
            force: false,
            no_cache: true,
        })
        .unwrap();

        assert!(result.config_path.exists());
        assert!(!result.created_cache);
        assert!(result.cache_dir.is_none());
        assert!(!dir.path().join(".contextsmith").exists());
    }

    #[test]
    fn init_errors_on_existing_config_without_force() {
        let dir = tempfile::tempdir().unwrap();

        // First init succeeds
        run(InitOptions {
            root: dir.path().to_path_buf(),
            config_path: None,
            force: false,
            no_cache: false,
        })
        .unwrap();

        // Second init fails
        let err = run(InitOptions {
            root: dir.path().to_path_buf(),
            config_path: None,
            force: false,
            no_cache: false,
        })
        .unwrap_err();

        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn init_force_overwrites_existing() {
        let dir = tempfile::tempdir().unwrap();

        run(InitOptions {
            root: dir.path().to_path_buf(),
            config_path: None,
            force: false,
            no_cache: false,
        })
        .unwrap();

        let result = run(InitOptions {
            root: dir.path().to_path_buf(),
            config_path: None,
            force: true,
            no_cache: false,
        })
        .unwrap();

        assert!(result.created_config);
    }

    #[test]
    fn init_errors_on_bad_root() {
        let err = run(InitOptions {
            root: PathBuf::from("/nonexistent/path/that/should/not/exist"),
            config_path: None,
            force: false,
            no_cache: false,
        })
        .unwrap_err();

        assert!(err.to_string().contains("does not exist"));
    }
}
