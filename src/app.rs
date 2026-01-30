//! Core application state and transitions.
//!
//! This module is UI-independent and is exercised via unit tests to ensure
//! navigation logic works before wiring it into the TUI.

use crate::workspace::Workspace;

/// Holds global application state.
#[derive(Debug)]
pub struct App {
    workspaces: Vec<Workspace>,
    selected_index: usize,
    should_quit: bool,
}

impl App {
    /// Construct a new `App` with the default set of workspaces.
    pub fn new() -> Self {
        let workspaces = default_workspaces();
        Self {
            workspaces,
            selected_index: 0,
            should_quit: false,
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

    /// All configured workspaces.
    pub fn workspaces(&self) -> &[Workspace] {
        &self.workspaces
    }

    /// Index of the currently selected workspace.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn default_workspaces() -> Vec<Workspace> {
    vec![
        Workspace::new("1", "W1"),
        Workspace::new("2", "W2"),
        Workspace::new("3", "W3"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_correct() {
        let app = App::new();
        assert_eq!(app.workspaces().len(), 3);
        assert_eq!(app.selected_index(), 0);
        assert!(!app.should_quit());
    }

    #[test]
    fn select_next_advances_selection() {
        let mut app = App::new();
        app.select_next();
        assert_eq!(app.selected_index(), 1);
    }

    #[test]
    fn select_next_wraps_at_end() {
        let mut app = App::new();
        app.selected_index = app.workspaces.len() - 1;
        app.select_next();
        assert_eq!(app.selected_index(), 0);
    }

    #[test]
    fn select_previous_moves_up() {
        let mut app = App::new();
        app.selected_index = 2;
        app.select_previous();
        assert_eq!(app.selected_index(), 1);
    }

    #[test]
    fn select_previous_wraps_at_top() {
        let mut app = App::new();
        app.select_previous();
        assert_eq!(app.selected_index(), app.workspaces().len() - 1);
    }

    #[test]
    fn quit_sets_flag() {
        let mut app = App::new();
        app.quit();
        assert!(app.should_quit());
    }
}
