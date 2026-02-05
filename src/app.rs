//! Core application state and transitions.
//!
//! This module is UI-independent and is exercised via unit tests to ensure
//! navigation logic works before wiring it into the TUI.

use std::io;

use crate::{
    state::{AppState, WorkspaceState},
    workspace::Workspace,
};

/// Holds global application state.
#[derive(Debug)]
pub struct App {
    workspaces: Vec<Workspace>,
    selected_index: usize,
    should_quit: bool,
    focus: FocusArea,
}

impl App {
    /// Construct a new `App`, loading persisted state if available.
    pub fn new() -> Self {
        Self::from_state(AppState::load())
    }

    /// Construct an `App` from persisted state.
    pub fn from_state(state: AppState) -> Self {
        let workspaces = state.workspaces.into_iter().map(Workspace::from).collect();
        Self {
            workspaces,
            selected_index: state.selected_index,
            should_quit: false,
            focus: FocusArea::Sidebar,
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

    /// Persist the current app state to disk.
    pub fn save_state(&self) -> io::Result<()> {
        self.to_state().save()
    }

    fn to_state(&self) -> AppState {
        let workspaces = self
            .workspaces
            .iter()
            .map(WorkspaceState::from)
            .collect();
        AppState::new(workspaces, self.selected_index)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_correct() {
        let app = App::from_state(AppState::default());
        assert_eq!(app.workspaces().len(), 3);
        assert_eq!(app.selected_index(), 0);
        assert!(!app.should_quit());
    }

    #[test]
    fn select_next_advances_selection() {
        let mut app = App::from_state(AppState::default());
        app.select_next();
        assert_eq!(app.selected_index(), 1);
    }

    #[test]
    fn select_next_wraps_at_end() {
        let mut app = App::from_state(AppState::default());
        app.selected_index = app.workspaces.len() - 1;
        app.select_next();
        assert_eq!(app.selected_index(), 0);
    }

    #[test]
    fn select_previous_moves_up() {
        let mut app = App::from_state(AppState::default());
        app.selected_index = 2;
        app.select_previous();
        assert_eq!(app.selected_index(), 1);
    }

    #[test]
    fn select_previous_wraps_at_top() {
        let mut app = App::from_state(AppState::default());
        app.select_previous();
        assert_eq!(app.selected_index(), app.workspaces().len() - 1);
    }

    #[test]
    fn quit_sets_flag() {
        let mut app = App::from_state(AppState::default());
        app.quit();
        assert!(app.should_quit());
    }

    #[test]
    fn initial_focus_is_sidebar() {
        let app = App::from_state(AppState::default());
        assert_eq!(app.focus(), FocusArea::Sidebar);
    }

    #[test]
    fn focus_changes_follow_direction() {
        let mut app = App::from_state(AppState::default());
        app.focus_right();
        assert_eq!(app.focus(), FocusArea::Main);
        app.focus_up();
        assert_eq!(app.focus(), FocusArea::Header);
        app.focus_left();
        assert_eq!(app.focus(), FocusArea::Sidebar);
        app.focus_down();
        assert_eq!(app.focus(), FocusArea::Main);
    }
}
