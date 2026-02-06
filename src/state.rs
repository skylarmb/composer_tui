//! Persistent application state stored on disk.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::config::config_dir;

const STATE_FILE_NAME: &str = "state.toml";
const STATE_VERSION: u32 = 1;

/// Serialized workspace data for persistence.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceState {
    pub id: String,
    pub name: String,
    #[serde(default, alias = "path")]
    pub worktree_path: Option<PathBuf>,
}

impl WorkspaceState {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            worktree_path: None,
        }
    }
}

/// Serialized app state for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub workspaces: Vec<WorkspaceState>,
    #[serde(default, alias = "selected")]
    pub selected_index: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            workspaces: default_workspaces(),
            selected_index: 0,
        }
    }
}

impl AppState {
    /// Load state from disk, returning defaults on missing/corrupt data.
    pub fn load() -> Self {
        let dir = match config_dir() {
            Ok(dir) => dir,
            Err(err) => {
                eprintln!("Warning: {err}");
                return Self::default();
            }
        };

        if let Err(err) = ensure_state_dir(&dir) {
            eprintln!("Warning: failed to create state dir {dir:?}: {err}");
            return Self::default();
        }

        let path = match state_path() {
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
                eprintln!("Warning: failed to read state {path:?}: {err}");
                return Self::default();
            }
        };

        match toml::from_str::<AppState>(&contents) {
            Ok(state) => state.normalized(),
            Err(err) => {
                eprintln!("Warning: failed to parse state {path:?}: {err}");
                Self::default()
            }
        }
    }

    /// Save state to disk.
    pub fn save(&self) -> io::Result<()> {
        let dir = config_dir()?;
        ensure_state_dir(&dir)?;
        let path = state_path()?;
        let contents = toml::to_string_pretty(self)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        fs::write(path, contents)?;
        Ok(())
    }

    /// Construct state from workspace list and selection.
    pub fn new(workspaces: Vec<WorkspaceState>, selected_index: usize) -> Self {
        Self {
            version: STATE_VERSION,
            workspaces,
            selected_index,
        }
        .normalized()
    }

    fn normalized(mut self) -> Self {
        if self.version == 0 {
            self.version = STATE_VERSION;
        }

        if self.workspaces.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.workspaces.len() {
            self.selected_index = self.workspaces.len() - 1;
        }

        self
    }
}

fn state_path() -> io::Result<PathBuf> {
    Ok(config_dir()?.join(STATE_FILE_NAME))
}

fn ensure_state_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)
}

fn default_workspaces() -> Vec<WorkspaceState> {
    vec![
        WorkspaceState::new("1", "W1"),
        WorkspaceState::new("2", "W2"),
        WorkspaceState::new("3", "W3"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env, fs,
        path::PathBuf,
        process,
        sync::Mutex,
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn with_temp_home<T>(f: impl FnOnce(PathBuf) -> T) -> T {
        let _guard = TEST_LOCK.lock().expect("test lock poisoned");
        let original_home = env::var("HOME").ok();
        let unique = format!(
            "composer_tui_test_{}_{}",
            process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos()
        );
        let temp_home = env::temp_dir().join(unique);
        fs::create_dir_all(&temp_home).expect("failed to create temp home");
        env::set_var("HOME", &temp_home);

        let result = f(temp_home.clone());

        match original_home {
            Some(value) => env::set_var("HOME", value),
            None => env::remove_var("HOME"),
        }
        let _ = fs::remove_dir_all(temp_home);
        result
    }

    #[test]
    fn load_defaults_and_creates_config_dir() {
        with_temp_home(|temp_home| {
            let state = AppState::load();
            assert_eq!(state.workspaces.len(), 3);
            assert_eq!(state.selected_index, 0);

            let dir = config_dir().expect("config dir");
            assert_eq!(dir, temp_home.join(".config").join("composer_tui"));
            assert!(dir.exists(), "config dir should exist");
        });
    }

    #[test]
    fn save_and_load_round_trip() {
        with_temp_home(|_| {
            let workspaces = vec![
                WorkspaceState::new("alpha", "Alpha"),
                WorkspaceState::new("beta", "Beta"),
            ];
            let state = AppState::new(workspaces, 1);
            state.save().expect("save");

            let loaded = AppState::load();
            assert_eq!(loaded.workspaces.len(), 2);
            assert_eq!(loaded.workspaces[0].name, "Alpha");
            assert_eq!(loaded.workspaces[1].name, "Beta");
            assert_eq!(loaded.selected_index, 1);
        });
    }

    #[test]
    fn corrupt_state_falls_back_to_default() {
        with_temp_home(|_| {
            let path = state_path().expect("state path");
            fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
            fs::write(&path, "not = [toml").expect("write");

            let loaded = AppState::load();
            let default = AppState::default();
            assert_eq!(loaded.workspaces.len(), default.workspaces.len());
            assert_eq!(loaded.selected_index, default.selected_index);
            assert_eq!(loaded.workspaces[0].name, default.workspaces[0].name);
        });
    }

    #[test]
    fn normalize_clamps_selected_index() {
        let workspaces = vec![
            WorkspaceState::new("1", "One"),
            WorkspaceState::new("2", "Two"),
        ];
        let state = AppState::new(workspaces, 99);
        assert_eq!(state.selected_index, 1);

        let empty = AppState::new(Vec::new(), 5);
        assert_eq!(empty.selected_index, 0);
    }
}
