//! Core application state and transitions.
//!
//! This module is UI-independent and is exercised via unit tests to ensure
//! navigation and input logic works before wiring it into the TUI.

use std::{env, io, time::Duration};

#[cfg(not(test))]
use crate::state::WorkspaceState;
use crate::{
    config::Config,
    gh_status::{GhStatusFetcher, GhWorkspaceTarget},
    git_status::{GitStatusFetcher, GitWorkspaceTarget},
    state::AppState,
    workspace::{Workspace, WorkspaceTerminalState},
    worktree::WorktreeManager,
};

const GIT_STATUS_POLL_INTERVAL: Duration = Duration::from_secs(7);
const GH_STATUS_POLL_INTERVAL: Duration = Duration::from_secs(45);

/// Holds global application state.
pub struct App {
    workspaces: Vec<Workspace>,
    selected_index: usize,
    next_workspace_id: u64,
    should_quit: bool,
    focus: FocusArea,
    input_mode: InputMode,
    worktree_manager: Option<WorktreeManager>,
    /// Whether the main panel is fullscreen (sidebar hidden).
    fullscreen: bool,
    /// Persistent user configuration.
    config: Config,
    /// Background poller for git dirty/clean status.
    git_status_fetcher: GitStatusFetcher,
    /// Background poller for PR/CI status from the `gh` CLI.
    gh_status_fetcher: GhStatusFetcher,
}

impl App {
    /// Construct a new `App`, loading persisted state and config.
    pub fn new() -> Self {
        Self::from_state(AppState::load())
    }

    /// Construct an `App` from persisted state with default config.
    pub fn from_state(state: AppState) -> Self {
        Self::from_state_with_manager(state, discover_worktree_manager())
    }

    /// Construct an `App` from persisted state with an explicit manager.
    pub fn from_state_with_manager(
        state: AppState,
        worktree_manager: Option<WorktreeManager>,
    ) -> Self {
        Self::from_state_with_config(state, worktree_manager, Config::default())
    }

    /// Construct an `App` from persisted state, manager, and config.
    pub fn from_state_with_config(
        state: AppState,
        worktree_manager: Option<WorktreeManager>,
        config: Config,
    ) -> Self {
        let workspaces: Vec<Workspace> =
            state.workspaces.into_iter().map(Workspace::from).collect();
        Self {
            next_workspace_id: next_workspace_id(&workspaces),
            workspaces,
            selected_index: state.selected_index,
            should_quit: false,
            focus: FocusArea::Sidebar,
            input_mode: InputMode::Normal,
            worktree_manager,
            fullscreen: false,
            config,
            git_status_fetcher: GitStatusFetcher::new(GIT_STATUS_POLL_INTERVAL),
            gh_status_fetcher: GhStatusFetcher::new(GH_STATUS_POLL_INTERVAL),
        }
    }

    /// Move selection down the workspace list, wrapping at the end.
    pub fn select_next(&mut self) {
        if self.workspaces.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1) % self.workspaces.len();
    }

    /// Move selection up the workspace list, wrapping at the top.
    pub fn select_previous(&mut self) {
        if self.workspaces.is_empty() {
            return;
        }
        if self.selected_index == 0 {
            self.selected_index = self.workspaces.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    /// Move the selected workspace one position earlier in the sidebar.
    ///
    /// Returns `true` if the workspace moved.
    pub fn move_selected_workspace_up(&mut self) -> bool {
        if self.selected_index == 0 || self.workspaces.len() <= 1 {
            return false;
        }

        self.workspaces
            .swap(self.selected_index, self.selected_index - 1);
        self.selected_index -= 1;

        if let Err(err) = self.save_state() {
            self.show_error(format!(
                "workspace reordered but failed to save state: {err}"
            ));
        }

        true
    }

    /// Move the selected workspace one position later in the sidebar.
    ///
    /// Returns `true` if the workspace moved.
    pub fn move_selected_workspace_down(&mut self) -> bool {
        if self.workspaces.len() <= 1 || self.selected_index >= self.workspaces.len() - 1 {
            return false;
        }

        self.workspaces
            .swap(self.selected_index, self.selected_index + 1);
        self.selected_index += 1;

        if let Err(err) = self.save_state() {
            self.show_error(format!(
                "workspace reordered but failed to save state: {err}"
            ));
        }

        true
    }

    /// Start create-workspace input mode.
    pub fn start_create_workspace(&mut self) {
        if self.worktree_manager.is_none() {
            self.show_error("worktree manager unavailable: open the app inside a git repository");
            return;
        }
        self.input_mode = InputMode::CreateWorkspace {
            name: String::new(),
        };
    }

    /// Start delete confirmation mode for the selected workspace.
    pub fn start_delete_workspace(&mut self) {
        let Some(workspace) = self.selected_workspace() else {
            self.show_error("no workspace selected");
            return;
        };
        self.input_mode = InputMode::ConfirmDelete {
            workspace_name: workspace.name().to_string(),
        };
    }

    /// Append an input character for the active input mode.
    pub fn push_input_char(&mut self, ch: char) {
        if let InputMode::CreateWorkspace { name } = &mut self.input_mode {
            name.push(ch);
        }
    }

    /// Delete the last input character for the active input mode.
    pub fn pop_input_char(&mut self) {
        if let InputMode::CreateWorkspace { name } = &mut self.input_mode {
            name.pop();
        }
    }

    /// Confirm the active modal input action.
    pub fn confirm_input(&mut self) {
        match self.input_mode.clone() {
            InputMode::Normal => {}
            InputMode::CreateWorkspace { name } => self.create_workspace(name),
            InputMode::ConfirmDelete { workspace_name } => self.delete_workspace(workspace_name),
            InputMode::ConfirmCloseTab => {
                let _ = self.close_selected_workspace_tab();
                self.input_mode = InputMode::Normal;
            }
            InputMode::Error { .. } => self.input_mode = InputMode::Normal,
        }
    }

    /// Cancel the active modal input action.
    pub fn cancel_input(&mut self) {
        if !matches!(self.input_mode, InputMode::Normal) {
            self.input_mode = InputMode::Normal;
        }
    }

    /// Current input mode.
    pub fn input_mode(&self) -> &InputMode {
        &self.input_mode
    }

    /// Whether a modal UI is active.
    pub fn is_modal_active(&self) -> bool {
        !matches!(self.input_mode, InputMode::Normal)
    }

    /// Signal the application to quit.
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Whether the application should terminate.
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Currently focused area of the UI.
    pub fn focus(&self) -> FocusArea {
        self.focus
    }

    /// Move focus left (to the sidebar).
    pub fn focus_left(&mut self) {
        self.focus = FocusArea::Sidebar;
    }

    /// Move focus right (to the main panel).
    pub fn focus_right(&mut self) {
        self.focus = FocusArea::Main;
    }

    /// Move focus up (to the header).
    pub fn focus_up(&mut self) {
        self.focus = FocusArea::Header;
    }

    /// Move focus down (into the main body; defaults to main panel).
    pub fn focus_down(&mut self) {
        self.focus = FocusArea::Main;
    }

    /// Whether the main panel is in fullscreen mode (sidebar hidden).
    pub fn is_fullscreen(&self) -> bool {
        self.fullscreen
    }

    /// Toggle fullscreen mode on/off.
    pub fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
    }

    /// Exit fullscreen mode (no-op if not fullscreen).
    pub fn exit_fullscreen(&mut self) {
        self.fullscreen = false;
    }

    /// All configured workspaces.
    pub fn workspaces(&self) -> &[Workspace] {
        &self.workspaces
    }

    /// Index of the currently selected workspace.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Currently selected workspace.
    pub fn selected_workspace(&self) -> Option<&Workspace> {
        self.workspaces.get(self.selected_index)
    }

    /// Set the selected workspace index with bounds checking.
    ///
    /// Returns `true` if the selection changed, `false` if out of bounds
    /// or the same as the current index.
    pub fn set_selected_index(&mut self, index: usize) -> bool {
        if self.workspaces.is_empty() || index >= self.workspaces.len() {
            return false;
        }
        let changed = self.selected_index != index;
        self.selected_index = index;
        changed
    }

    /// Current application config.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Replace the current config (e.g. after reloading from disk).
    pub fn reload_config(&mut self) {
        self.config = Config::load();
    }

    /// Ensure selected workspace terminal is running and poll output.
    ///
    /// Passes configured shell and scrollback limit to terminal creation,
    /// then polls all tabs across all workspaces.
    pub fn tick(&mut self, cols: u16, rows: u16) {
        let shell = self.config.default_shell.clone();
        let scrollback_limit = self.config.scrollback_limit();
        let auto_spawn_command = self.config.auto_spawn_command.clone();

        if let Some(workspace) = self.selected_workspace_mut() {
            match workspace.ensure_terminal_started(cols, rows, shell.as_deref(), scrollback_limit)
            {
                Ok(freshly_spawned) => {
                    // Run auto-spawn command on first-ever spawn only.
                    if let Some(cmd) = auto_spawn_command
                        .filter(|_| freshly_spawned && !workspace.has_auto_spawned())
                    {
                        let input = format!("{cmd}\r");
                        if let Err(err) = workspace.write_terminal_input(input.as_bytes()) {
                            workspace
                                .set_terminal_error(format!("failed to auto-run command: {err}"));
                        }
                        workspace.mark_auto_spawned();
                    }
                }
                Err(err) => {
                    workspace.set_terminal_error(format!("failed to start terminal: {err}"));
                }
            }
        }

        for workspace in &mut self.workspaces {
            workspace.poll_tabs();
        }

        let git_targets = self
            .workspaces
            .iter()
            .filter_map(|workspace| {
                workspace.worktree_path().map(|path| {
                    GitWorkspaceTarget::new(workspace.id().to_string(), path.to_path_buf())
                })
            })
            .collect();
        self.git_status_fetcher.set_targets(git_targets);

        let gh_targets = self
            .workspaces
            .iter()
            .filter_map(|workspace| {
                let path = workspace.worktree_path()?;
                let branch_name = workspace.branch_name()?;
                Some(GhWorkspaceTarget::new(
                    workspace.id().to_string(),
                    path.to_path_buf(),
                    branch_name.to_string(),
                ))
            })
            .collect();
        self.gh_status_fetcher.set_targets(gh_targets);

        self.apply_status_updates();
    }

    /// Add a new tab to the selected workspace and persist state.
    pub fn add_tab_to_selected_workspace(&mut self) {
        {
            let Some(workspace) = self.selected_workspace_mut() else {
                return;
            };
            workspace.add_tab();
        }
        if let Err(err) = self.save_state() {
            self.show_error(format!("tab created but failed to save state: {err}"));
        }
    }

    /// Close the active tab in selected workspace, asking for confirmation
    /// if the tab is currently running.
    pub fn start_close_selected_workspace_tab(&mut self) {
        let Some(workspace) = self.selected_workspace() else {
            return;
        };
        if workspace.tab_count() <= 1 {
            return;
        }

        if workspace.terminal_state() == WorkspaceTerminalState::Running {
            self.input_mode = InputMode::ConfirmCloseTab;
        } else {
            let _ = self.close_selected_workspace_tab();
        }
    }

    /// Close the active tab in the selected workspace and persist state.
    pub fn close_selected_workspace_tab(&mut self) -> bool {
        let closed = {
            let Some(workspace) = self.selected_workspace_mut() else {
                return false;
            };
            workspace.close_tab()
        };
        if !closed {
            return false;
        }
        if let Err(err) = self.save_state() {
            self.show_error(format!("tab closed but failed to save state: {err}"));
        }
        true
    }

    /// Select a tab by index in the selected workspace.
    ///
    /// Returns `true` if the selected tab changed.
    pub fn select_selected_workspace_tab(&mut self, index: usize) -> bool {
        let changed = {
            let Some(workspace) = self.selected_workspace_mut() else {
                return false;
            };
            workspace.select_tab(index)
        };
        if changed {
            let _ = self.save_state();
        }
        changed
    }

    /// Send input bytes to the selected workspace terminal.
    pub fn send_selected_terminal_input(&mut self, data: &[u8]) {
        let Some(workspace) = self.selected_workspace_mut() else {
            return;
        };
        if let Err(err) = workspace.write_terminal_input(data) {
            workspace.set_terminal_error(format!("failed to write to terminal: {err}"));
        }
    }

    /// Scroll the selected terminal viewport up by one page.
    pub fn scroll_selected_terminal_up(&mut self) {
        if let Some(workspace) = self.selected_workspace_mut() {
            workspace.scroll_up();
        }
    }

    /// Scroll the selected terminal viewport down by one page.
    pub fn scroll_selected_terminal_down(&mut self) {
        if let Some(workspace) = self.selected_workspace_mut() {
            workspace.scroll_down();
        }
    }

    /// Return the selected terminal viewport to the live bottom.
    pub fn scroll_selected_terminal_to_bottom(&mut self) {
        if let Some(workspace) = self.selected_workspace_mut() {
            workspace.scroll_to_bottom();
        }
    }

    /// Whether the selected terminal is currently showing scrollback.
    pub fn selected_terminal_is_scrolled(&self) -> bool {
        self.selected_workspace()
            .is_some_and(|workspace| workspace.is_scrolled())
    }

    fn apply_status_updates(&mut self) {
        for workspace in &mut self.workspaces {
            if workspace.worktree_path().is_none() {
                workspace.set_git_status(None);
            }
            if workspace.worktree_path().is_none() || workspace.branch_name().is_none() {
                workspace.set_gh_status(None);
            }
        }

        for update in self.git_status_fetcher.drain_updates() {
            if let Some(workspace) = self
                .workspaces
                .iter_mut()
                .find(|workspace| workspace.id() == update.workspace_id)
            {
                workspace.set_git_status(update.status);
            }
        }

        for update in self.gh_status_fetcher.drain_updates() {
            if let Some(workspace) = self
                .workspaces
                .iter_mut()
                .find(|workspace| workspace.id() == update.workspace_id)
            {
                workspace.set_gh_status(update.status);
            }
        }
    }

    /// Persist the current app state to disk.
    pub fn save_state(&self) -> io::Result<()> {
        if std::env::var_os("COMPOSER_TUI_DISABLE_STATE_SAVE").is_some() {
            return Ok(());
        }

        #[cfg(test)]
        {
            // Tests run in parallel and some mutate HOME; avoid cross-test
            // interference from filesystem writes in app-level unit tests.
            Ok(())
        }

        #[cfg(not(test))]
        self.to_state().save()
    }

    #[cfg(not(test))]
    fn to_state(&self) -> AppState {
        let workspaces = self.workspaces.iter().map(WorkspaceState::from).collect();
        AppState::new(workspaces, self.selected_index)
    }

    fn create_workspace(&mut self, raw_name: String) {
        let name = raw_name.trim().to_string();
        if name.is_empty() {
            self.show_error("workspace name cannot be empty");
            return;
        }
        if !is_workspace_name_valid(&name) {
            self.show_error("workspace name must use only [a-zA-Z0-9_-]");
            return;
        }

        let branch = name.clone();
        let Some(manager) = &self.worktree_manager else {
            self.show_error("worktree manager unavailable");
            return;
        };

        let worktree_path = match manager.create_worktree(&name, &branch) {
            Ok(path) => path,
            Err(err) => {
                self.show_error(format!("failed to create workspace '{name}': {err}"));
                return;
            }
        };

        let id = self.allocate_workspace_id();
        self.workspaces
            .push(Workspace::with_worktree(id, &name, worktree_path, &branch));
        self.selected_index = self.workspaces.len().saturating_sub(1);
        self.input_mode = InputMode::Normal;

        if let Err(err) = self.save_state() {
            self.show_error(format!("workspace created but failed to save state: {err}"));
        }
    }

    fn delete_workspace(&mut self, workspace_name: String) {
        let Some(index) = self
            .workspaces
            .iter()
            .position(|ws| ws.name() == workspace_name)
        else {
            self.show_error("selected workspace not found");
            return;
        };

        let has_worktree = self.workspaces[index].worktree_path().is_some();
        if has_worktree {
            let Some(manager) = &self.worktree_manager else {
                self.show_error("worktree manager unavailable");
                return;
            };
            if let Err(err) = manager.delete_worktree(&workspace_name) {
                self.show_error(format!(
                    "failed to delete worktree for '{workspace_name}': {err}"
                ));
                return;
            }
        }

        self.workspaces.remove(index);
        if self.workspaces.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.workspaces.len() {
            self.selected_index = self.workspaces.len() - 1;
        }
        self.input_mode = InputMode::Normal;

        if let Err(err) = self.save_state() {
            self.show_error(format!("workspace deleted but failed to save state: {err}"));
        }
    }

    fn allocate_workspace_id(&mut self) -> String {
        let id = self.next_workspace_id;
        self.next_workspace_id += 1;
        id.to_string()
    }

    fn selected_workspace_mut(&mut self) -> Option<&mut Workspace> {
        self.workspaces.get_mut(self.selected_index)
    }

    fn show_error(&mut self, message: impl Into<String>) {
        self.input_mode = InputMode::Error {
            message: message.into(),
        };
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// Logical UI regions that can receive focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusArea {
    Header,
    Sidebar,
    Main,
}

/// Modal input state for creating/deleting workspaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    CreateWorkspace { name: String },
    ConfirmDelete { workspace_name: String },
    ConfirmCloseTab,
    Error { message: String },
}

fn next_workspace_id(workspaces: &[Workspace]) -> u64 {
    workspaces
        .iter()
        .filter_map(|ws| ws.id().parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn discover_worktree_manager() -> Option<WorktreeManager> {
    env::current_dir()
        .ok()
        .and_then(|cwd| WorktreeManager::new(cwd).ok())
}

fn is_workspace_name_valid(name: &str) -> bool {
    name.chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_correct() {
        let app = App::from_state_with_manager(AppState::default(), None);
        assert_eq!(app.workspaces().len(), 3);
        assert_eq!(app.selected_index(), 0);
        assert!(!app.should_quit());
    }

    #[test]
    fn select_next_advances_selection() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        app.select_next();
        assert_eq!(app.selected_index(), 1);
    }

    #[test]
    fn select_next_wraps_at_end() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        app.selected_index = app.workspaces.len() - 1;
        app.select_next();
        assert_eq!(app.selected_index(), 0);
    }

    #[test]
    fn select_previous_moves_up() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        app.selected_index = 2;
        app.select_previous();
        assert_eq!(app.selected_index(), 1);
    }

    #[test]
    fn select_previous_wraps_at_top() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        app.select_previous();
        assert_eq!(app.selected_index(), app.workspaces().len() - 1);
    }

    #[test]
    fn move_selected_workspace_up_swaps_workspaces_and_selection() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        app.selected_index = 1;

        assert!(app.move_selected_workspace_up());
        assert_eq!(app.selected_index(), 0);
        assert_eq!(app.workspaces()[0].name(), "W2");
        assert_eq!(app.workspaces()[1].name(), "W1");
        assert_eq!(app.workspaces()[2].name(), "W3");
    }

    #[test]
    fn move_selected_workspace_up_is_noop_at_top() {
        let mut app = App::from_state_with_manager(AppState::default(), None);

        assert!(!app.move_selected_workspace_up());
        assert_eq!(app.selected_index(), 0);
        assert_eq!(app.workspaces()[0].name(), "W1");
        assert_eq!(app.workspaces()[1].name(), "W2");
        assert_eq!(app.workspaces()[2].name(), "W3");
    }

    #[test]
    fn move_selected_workspace_down_swaps_workspaces_and_selection() {
        let mut app = App::from_state_with_manager(AppState::default(), None);

        assert!(app.move_selected_workspace_down());
        assert_eq!(app.selected_index(), 1);
        assert_eq!(app.workspaces()[0].name(), "W2");
        assert_eq!(app.workspaces()[1].name(), "W1");
        assert_eq!(app.workspaces()[2].name(), "W3");
    }

    #[test]
    fn move_selected_workspace_down_is_noop_at_bottom() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        app.selected_index = app.workspaces().len() - 1;

        assert!(!app.move_selected_workspace_down());
        assert_eq!(app.selected_index(), 2);
        assert_eq!(app.workspaces()[0].name(), "W1");
        assert_eq!(app.workspaces()[1].name(), "W2");
        assert_eq!(app.workspaces()[2].name(), "W3");
    }

    #[test]
    fn quit_sets_flag() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        app.quit();
        assert!(app.should_quit());
    }

    #[test]
    fn initial_focus_is_sidebar() {
        let app = App::from_state_with_manager(AppState::default(), None);
        assert_eq!(app.focus(), FocusArea::Sidebar);
    }

    #[test]
    fn focus_changes_follow_direction() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        app.focus_right();
        assert_eq!(app.focus(), FocusArea::Main);
        app.focus_up();
        assert_eq!(app.focus(), FocusArea::Header);
        app.focus_left();
        assert_eq!(app.focus(), FocusArea::Sidebar);
        app.focus_down();
        assert_eq!(app.focus(), FocusArea::Main);
    }

    #[test]
    fn create_workspace_requires_worktree_manager() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        app.start_create_workspace();
        assert!(matches!(app.input_mode(), InputMode::Error { .. }));
    }

    #[test]
    fn delete_workspace_opens_confirmation() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        app.start_delete_workspace();
        assert!(matches!(
            app.input_mode(),
            InputMode::ConfirmDelete { workspace_name } if workspace_name == "W1"
        ));
    }

    #[test]
    fn cancel_modal_returns_to_normal_mode() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        app.start_delete_workspace();
        app.cancel_input();
        assert!(matches!(app.input_mode(), InputMode::Normal));
    }

    #[test]
    fn set_selected_index_within_bounds() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        assert!(app.set_selected_index(2));
        assert_eq!(app.selected_index(), 2);
    }

    #[test]
    fn set_selected_index_out_of_bounds_returns_false() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        assert!(!app.set_selected_index(99));
        assert_eq!(app.selected_index(), 0); // unchanged
    }

    #[test]
    fn set_selected_index_empty_workspaces_returns_false() {
        let state = AppState::new(Vec::new(), 0);
        let mut app = App::from_state_with_manager(state, None);
        assert!(!app.set_selected_index(0));
    }

    #[test]
    fn set_selected_index_same_value_returns_false() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        assert!(!app.set_selected_index(0)); // already at 0
    }

    #[test]
    fn toggle_fullscreen_flips_state() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        assert!(!app.is_fullscreen());
        app.toggle_fullscreen();
        assert!(app.is_fullscreen());
        app.toggle_fullscreen();
        assert!(!app.is_fullscreen());
    }

    #[test]
    fn exit_fullscreen_clears_flag() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        app.toggle_fullscreen();
        assert!(app.is_fullscreen());
        app.exit_fullscreen();
        assert!(!app.is_fullscreen());
    }

    #[test]
    fn exit_fullscreen_is_idempotent() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        app.exit_fullscreen(); // already false
        assert!(!app.is_fullscreen());
    }

    #[test]
    fn config_is_accessible() {
        let config = Config {
            sidebar_width: Some(30),
            ..Config::default()
        };
        let app = App::from_state_with_config(AppState::default(), None, config);
        assert_eq!(app.config().sidebar_width(), 30);
    }

    #[test]
    fn reload_config_replaces_stored_config() {
        let mut app = App::from_state_with_config(
            AppState::default(),
            None,
            Config {
                sidebar_width: Some(30),
                ..Config::default()
            },
        );
        assert_eq!(app.config().sidebar_width(), 30);
        // reload_config loads from disk (which may differ), but at minimum
        // it replaces the stored config without panicking.
        app.reload_config();
    }

    #[test]
    fn selected_workspace_tab_crud_updates_selection() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        let workspace = app.selected_workspace().expect("workspace");
        assert_eq!(workspace.tab_count(), 1);
        assert_eq!(workspace.active_tab_index(), 0);

        app.add_tab_to_selected_workspace();
        app.add_tab_to_selected_workspace();

        let workspace = app.selected_workspace().expect("workspace");
        assert_eq!(workspace.tab_count(), 3);
        assert_eq!(workspace.active_tab_index(), 2);

        assert!(app.select_selected_workspace_tab(0));
        let workspace = app.selected_workspace().expect("workspace");
        assert_eq!(workspace.active_tab_index(), 0);

        let _ = app.close_selected_workspace_tab();
        let workspace = app.selected_workspace().expect("workspace");
        assert_eq!(workspace.tab_count(), 2);
        assert_eq!(workspace.active_tab_index(), 0);
    }

    #[test]
    fn select_selected_workspace_tab_bounds_checked() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        assert!(!app.select_selected_workspace_tab(1));

        app.add_tab_to_selected_workspace();
        assert!(!app.select_selected_workspace_tab(99));
        assert!(app.select_selected_workspace_tab(0));
    }

    #[test]
    fn start_close_tab_prompts_confirmation_when_tab_is_running() {
        let mut app = App::from_state_with_manager(AppState::default(), None);
        app.add_tab_to_selected_workspace();
        app.tick(80, 24);

        app.start_close_selected_workspace_tab();
        assert!(matches!(app.input_mode(), InputMode::ConfirmCloseTab));

        app.cancel_input();
        assert!(matches!(app.input_mode(), InputMode::Normal));
    }
}
