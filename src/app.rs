//! Core application state and transitions.
//!
//! This module is UI-independent and is exercised via unit tests to ensure
//! navigation and input logic works before wiring it into the TUI.

use std::{env, io};

use crate::{
    state::{AppState, WorkspaceState},
    workspace::Workspace,
    worktree::WorktreeManager,
};

/// Holds global application state.
pub struct App {
    workspaces: Vec<Workspace>,
    selected_index: usize,
    next_workspace_id: u64,
    should_quit: bool,
    focus: FocusArea,
    input_mode: InputMode,
    worktree_manager: Option<WorktreeManager>,
}

impl App {
    /// Construct a new `App`, loading persisted state if available.
    pub fn new() -> Self {
        Self::from_state(AppState::load())
    }

    /// Construct an `App` from persisted state.
    pub fn from_state(state: AppState) -> Self {
        Self::from_state_with_manager(state, discover_worktree_manager())
    }

    /// Construct an `App` from persisted state with an explicit manager.
    pub fn from_state_with_manager(
        state: AppState,
        worktree_manager: Option<WorktreeManager>,
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

    /// Ensure selected workspace terminal is running and poll output.
    pub fn tick_terminals(&mut self, cols: u16, rows: u16) {
        if let Some(workspace) = self.selected_workspace_mut() {
            if let Err(err) = workspace.ensure_terminal_started(cols, rows, None) {
                workspace.set_terminal_error(format!("failed to start terminal: {err}"));
            }
        }

        for workspace in &mut self.workspaces {
            if let Err(err) = workspace.poll_terminal() {
                workspace.set_terminal_error(format!("terminal I/O error: {err}"));
            }
        }
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

    /// Persist the current app state to disk.
    pub fn save_state(&self) -> io::Result<()> {
        self.to_state().save()
    }

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
}
