//! Configuration management for composer_tui.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use directories::BaseDirs;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};

/// Name of the configuration directory under ~/.config/.
const CONFIG_DIR_NAME: &str = "composer_tui";
/// Name of the primary configuration file.
const CONFIG_FILE_NAME: &str = "config.toml";

/// Default sidebar width in columns.
const DEFAULT_SIDEBAR_WIDTH: u16 = 20;
/// Default scrollback buffer limit in lines.
const DEFAULT_SCROLLBACK_LIMIT: usize = 1000;

/// Commented-out template written when no config file exists yet.
const DEFAULT_CONFIG_TEMPLATE: &str = r##"# composer_tui configuration
# ──────────────────────────
# Uncomment and edit any setting below.
# After saving, press R in the sidebar to reload, or restart the app.

# ── Git worktrees ──────────────────────────────────────────────────
# Base directory for new git worktrees (created when adding workspaces).
# Defaults to <repo>/.worktrees if unset.
# worktree_base_dir = "/path/to/worktrees"

# ── Shell ──────────────────────────────────────────────────────────
# Shell program for workspace terminals.
# Defaults to $SHELL (or /bin/sh on Unix, cmd.exe on Windows).
# default_shell = "/usr/bin/env zsh"

# ── Auto-spawn ─────────────────────────────────────────────────────
# Command to run automatically when a workspace terminal first starts.
# Only runs once per workspace (not on terminal restarts).
# auto_spawn_command = "claude"

# ── Terminal ───────────────────────────────────────────────────────
# Maximum number of scrollback lines per terminal (default: 1000).
# scrollback_limit = 1000

# ── Layout ─────────────────────────────────────────────────────────
# Sidebar width in columns (default: 20).
# sidebar_width = 20

# ── Theme / Colors ─────────────────────────────────────────────────
# [theme]
# Border color when a panel is focused.
# Supports named colors (red, green, yellow, blue, magenta, cyan, white,
# gray, dark_gray, light_red, light_green, light_yellow, light_blue,
# light_magenta, light_cyan) or hex (#rrggbb).
# focused_border_color = "yellow"
#
# Background color for the selected sidebar item (default: reversed text).
# selected_bg_color = "#3a3a3a"
"##;

/// Persistent configuration stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub worktree_base_dir: Option<PathBuf>,

    /// Shell program to use for workspace terminals (e.g. "/usr/bin/env zsh").
    /// Falls back to `$SHELL` / platform default when `None`.
    #[serde(default)]
    pub default_shell: Option<String>,

    /// Command to auto-run in a workspace terminal on its first spawn
    /// (e.g. "claude"). Not re-run on terminal restarts.
    #[serde(default)]
    pub auto_spawn_command: Option<String>,

    /// Maximum number of scrollback lines per terminal. Default 1000.
    #[serde(default)]
    pub scrollback_limit: Option<usize>,

    /// Sidebar width in columns. Default 20.
    #[serde(default)]
    pub sidebar_width: Option<u16>,

    /// Theme / color overrides.
    #[serde(default)]
    pub theme: Option<ThemeConfig>,
}

/// Color overrides for UI chrome.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThemeConfig {
    /// Border color when a panel is focused (e.g. "yellow", "#ff8800", "rgb(255,136,0)").
    pub focused_border_color: Option<String>,
    /// Background color for the selected sidebar item.
    pub selected_bg_color: Option<String>,
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

/// Returns the full path to the config file.
pub fn config_path() -> io::Result<PathBuf> {
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

    /// Write a commented default config template to disk.
    ///
    /// Creates the config file with documentation for every supported
    /// setting, all commented out. Does nothing if the file already exists.
    pub fn write_default_template() -> io::Result<()> {
        let dir = config_dir()?;
        ensure_config_dir(&dir)?;
        let path = config_path()?;
        if path.exists() {
            return Ok(());
        }
        fs::write(path, DEFAULT_CONFIG_TEMPLATE)?;
        Ok(())
    }

    /// Effective sidebar width (default 20).
    pub fn sidebar_width(&self) -> u16 {
        self.sidebar_width.unwrap_or(DEFAULT_SIDEBAR_WIDTH)
    }

    /// Effective scrollback limit (default 1000).
    pub fn scrollback_limit(&self) -> usize {
        self.scrollback_limit.unwrap_or(DEFAULT_SCROLLBACK_LIMIT)
    }

    /// Effective focused border color (default Yellow).
    pub fn focused_border_color(&self) -> Color {
        self.theme
            .as_ref()
            .and_then(|t| t.focused_border_color.as_deref())
            .and_then(parse_color)
            .unwrap_or(Color::Yellow)
    }

    /// Effective selected background color (default None / reversed).
    pub fn selected_bg_color(&self) -> Option<Color> {
        self.theme
            .as_ref()
            .and_then(|t| t.selected_bg_color.as_deref())
            .and_then(parse_color)
    }
}

/// Parse a color string into a ratatui Color.
///
/// Supports named colors ("red", "yellow", "blue", etc.) and hex codes ("#rrggbb").
fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim().to_lowercase();
    match s.as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "darkgrey" | "dark_gray" | "dark_grey" => Some(Color::DarkGray),
        "lightred" | "light_red" => Some(Color::LightRed),
        "lightgreen" | "light_green" => Some(Color::LightGreen),
        "lightyellow" | "light_yellow" => Some(Color::LightYellow),
        "lightblue" | "light_blue" => Some(Color::LightBlue),
        "lightmagenta" | "light_magenta" => Some(Color::LightMagenta),
        "lightcyan" | "light_cyan" => Some(Color::LightCyan),
        "white" => Some(Color::White),
        hex if hex.starts_with('#') && hex.len() == 7 => {
            let r = u8::from_str_radix(&hex[1..3], 16).ok()?;
            let g = u8::from_str_radix(&hex[3..5], 16).ok()?;
            let b = u8::from_str_radix(&hex[5..7], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_are_backward_compatible() {
        // An empty TOML string should parse to valid defaults.
        let config: Config = toml::from_str("").unwrap();
        assert!(config.default_shell.is_none());
        assert!(config.auto_spawn_command.is_none());
        assert!(config.scrollback_limit.is_none());
        assert!(config.sidebar_width.is_none());
        assert!(config.theme.is_none());
        assert_eq!(config.sidebar_width(), DEFAULT_SIDEBAR_WIDTH);
        assert_eq!(config.scrollback_limit(), DEFAULT_SCROLLBACK_LIMIT);
    }

    #[test]
    fn config_round_trip_with_new_fields() {
        let config = Config {
            worktree_base_dir: Some(PathBuf::from("/tmp/test")),
            default_shell: Some("/usr/bin/env zsh".to_string()),
            auto_spawn_command: Some("claude".to_string()),
            scrollback_limit: Some(5000),
            sidebar_width: Some(25),
            theme: Some(ThemeConfig {
                focused_border_color: Some("cyan".to_string()),
                selected_bg_color: Some("#ff8800".to_string()),
            }),
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.default_shell.as_deref(), Some("/usr/bin/env zsh"));
        assert_eq!(parsed.auto_spawn_command.as_deref(), Some("claude"));
        assert_eq!(parsed.scrollback_limit, Some(5000));
        assert_eq!(parsed.sidebar_width, Some(25));
        assert_eq!(parsed.sidebar_width(), 25);
        assert_eq!(parsed.scrollback_limit(), 5000);
    }

    #[test]
    fn parse_color_named() {
        assert_eq!(parse_color("yellow"), Some(Color::Yellow));
        assert_eq!(parse_color("Red"), Some(Color::Red));
        assert_eq!(parse_color("CYAN"), Some(Color::Cyan));
        assert_eq!(parse_color("dark_gray"), Some(Color::DarkGray));
    }

    #[test]
    fn parse_color_hex() {
        assert_eq!(parse_color("#ff8800"), Some(Color::Rgb(255, 136, 0)));
        assert_eq!(parse_color("#000000"), Some(Color::Rgb(0, 0, 0)));
    }

    #[test]
    fn parse_color_invalid() {
        assert_eq!(parse_color("not_a_color"), None);
        assert_eq!(parse_color("#gg0000"), None);
        assert_eq!(parse_color("#fff"), None);
    }

    #[test]
    fn theme_colors_resolve_correctly() {
        let config = Config {
            theme: Some(ThemeConfig {
                focused_border_color: Some("cyan".to_string()),
                selected_bg_color: Some("#112233".to_string()),
            }),
            ..Config::default()
        };
        assert_eq!(config.focused_border_color(), Color::Cyan);
        assert_eq!(config.selected_bg_color(), Some(Color::Rgb(17, 34, 51)));
    }

    #[test]
    fn theme_defaults_when_absent() {
        let config = Config::default();
        assert_eq!(config.focused_border_color(), Color::Yellow);
        assert_eq!(config.selected_bg_color(), None);
    }

    #[test]
    fn old_config_toml_still_parses() {
        // Simulate an old config that only has worktree_base_dir.
        let old_toml = r#"worktree_base_dir = "/tmp/trees""#;
        let config: Config = toml::from_str(old_toml).unwrap();
        assert_eq!(config.worktree_base_dir, Some(PathBuf::from("/tmp/trees")));
        assert!(config.default_shell.is_none());
        assert_eq!(config.sidebar_width(), DEFAULT_SIDEBAR_WIDTH);
    }
}
