//! Configuration management for composer_tui.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use directories::BaseDirs;
use serde::{Deserialize, Serialize};

/// Name of the configuration directory under ~/.config/.
const CONFIG_DIR_NAME: &str = "composer_tui";
/// Name of the primary configuration file.
const CONFIG_FILE_NAME: &str = "config.toml";

/// Persistent configuration stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub worktree_base_dir: Option<PathBuf>,
}

/// Returns the configuration directory path (`~/.config/composer_tui/`).
pub fn config_dir() -> io::Result<PathBuf> {
    let base_dirs = BaseDirs::new().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "could not determine home directory",
        )
    })?;
    Ok(base_dirs.home_dir().join(".config").join(CONFIG_DIR_NAME))
}

fn config_path() -> io::Result<PathBuf> {
    Ok(config_dir()?.join(CONFIG_FILE_NAME))
}

fn ensure_config_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)
}

impl Config {
    /// Load the configuration from disk.
    ///
    /// Missing or corrupt configs return defaults and never crash.
    pub fn load() -> Self {
        let dir = match config_dir() {
            Ok(dir) => dir,
            Err(err) => {
                eprintln!("Warning: {err}");
                return Self::default();
            }
        };

        if let Err(err) = ensure_config_dir(&dir) {
            eprintln!("Warning: failed to create config dir {dir:?}: {err}");
            return Self::default();
        }

        let path = match config_path() {
            Ok(path) => path,
            Err(err) => {
                eprintln!("Warning: {err}");
                return Self::default();
            }
        };

        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Self::default(),
            Err(err) => {
                eprintln!("Warning: failed to read config {path:?}: {err}");
                return Self::default();
            }
        };

        match toml::from_str::<Config>(&contents) {
            Ok(config) => config,
            Err(err) => {
                eprintln!("Warning: failed to parse config {path:?}: {err}");
                Self::default()
            }
        }
    }

    /// Save the configuration to disk.
    pub fn save(&self) -> io::Result<()> {
        let dir = config_dir()?;
        ensure_config_dir(&dir)?;
        let path = config_path()?;
        let contents = toml::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(path, contents)?;
        Ok(())
    }
}
