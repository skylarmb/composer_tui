//! Workspace domain model.
//!
//! Workspaces now own one or more tabs, each with an independent terminal.

use std::{
    env, io,
    path::{Path, PathBuf},
};

use portable_pty::ExitStatus;

use crate::{
    state::{TabState, WorkspaceState},
    tab::Tab,
    terminal::ScreenBuffer,
};

pub use crate::tab::WorkspaceTerminalState;

/// Represents a single agent workspace displayed in the sidebar.
pub struct Workspace {
    id: String,
    name: String,
    worktree_path: Option<PathBuf>,
    branch_name: Option<String>,
    tabs: Vec<Tab>,
    active_tab_index: usize,
    /// Whether the auto-spawn command has been sent for this workspace.
    auto_spawned: bool,
}

impl std::fmt::Debug for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workspace")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("worktree_path", &self.worktree_path)
            .field("branch_name", &self.branch_name)
            .field("tab_count", &self.tabs.len())
            .field("active_tab_index", &self.active_tab_index)
            .finish()
    }
}

impl PartialEq for Workspace {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name == other.name
            && self.worktree_path == other.worktree_path
            && self.branch_name == other.branch_name
    }
}

impl Eq for Workspace {}

impl Workspace {
    /// Create a new `Workspace` with the given id and display name.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            worktree_path: None,
            branch_name: None,
            tabs: vec![Tab::new()],
            active_tab_index: 0,
            auto_spawned: false,
        }
    }

    /// Create a workspace backed by a git worktree and branch.
    pub fn with_worktree(
        id: impl Into<String>,
        name: impl Into<String>,
        worktree_path: PathBuf,
        branch_name: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            worktree_path: Some(worktree_path),
            branch_name: Some(branch_name.into()),
            tabs: vec![Tab::new()],
            active_tab_index: 0,
            auto_spawned: false,
        }
    }

    /// Unique identifier for the workspace.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Display name shown in the UI.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Optional worktree path backing this workspace.
    pub fn worktree_path(&self) -> Option<&Path> {
        self.worktree_path.as_deref()
    }

    /// Optional branch name backing this workspace.
    pub fn branch_name(&self) -> Option<&str> {
        self.branch_name.as_deref()
    }

    /// Number of tabs in this workspace.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Index of the active tab.
    pub fn active_tab_index(&self) -> usize {
        self.active_tab_index
    }

    /// Label text for a specific tab index.
    pub fn tab_title(&self, index: usize) -> Option<String> {
        self.tabs.get(index).map(|tab| tab.title(index))
    }

    /// Label text for the active tab.
    pub fn active_tab_title(&self) -> String {
        self.tab_title(self.active_tab_index)
            .unwrap_or_else(|| format!("tab{}", self.active_tab_index + 1))
    }

    /// Add a new tab and select it.
    pub fn add_tab(&mut self) {
        self.tabs.push(Tab::new());
        self.active_tab_index = self.tabs.len() - 1;
    }

    /// Close the currently active tab.
    ///
    /// Returns `true` when a tab was closed, `false` when there is only one
    /// tab left and close is not allowed.
    pub fn close_tab(&mut self) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }

        self.tabs.remove(self.active_tab_index);
        if self.active_tab_index >= self.tabs.len() {
            self.active_tab_index = self.tabs.len() - 1;
        }
        true
    }

    /// Select a tab by index.
    ///
    /// Returns `true` if the selection changed.
    pub fn select_tab(&mut self, index: usize) -> bool {
        if index >= self.tabs.len() || index == self.active_tab_index {
            return false;
        }
        self.active_tab_index = index;
        true
    }

    /// Select the next tab, wrapping at the end.
    pub fn next_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.active_tab_index = (self.active_tab_index + 1) % self.tabs.len();
    }

    /// Select the previous tab, wrapping at the beginning.
    pub fn prev_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        if self.active_tab_index == 0 {
            self.active_tab_index = self.tabs.len() - 1;
        } else {
            self.active_tab_index -= 1;
        }
    }

    /// Ensure the active tab has a running terminal session.
    ///
    /// Returns `Ok(true)` when a fresh terminal was spawned, `Ok(false)`
    /// when the existing terminal was kept (possibly resized).
    pub fn ensure_terminal_started(
        &mut self,
        cols: u16,
        rows: u16,
        shell: Option<&str>,
        scrollback_limit: usize,
    ) -> io::Result<bool> {
        let cwd = self.terminal_cwd()?;
        self.active_tab_mut()
            .expect("workspace should always have at least one tab")
            .ensure_terminal_started(&cwd, cols, rows, shell, scrollback_limit)
    }

    /// Resize the active tab terminal if present.
    pub fn resize_terminal(&mut self, cols: u16, rows: u16) -> io::Result<()> {
        self.active_tab_mut()
            .expect("workspace should always have at least one tab")
            .resize(cols, rows)
    }

    /// Poll terminal output and process lifecycle events for the active tab.
    pub fn poll_terminal(&mut self) -> io::Result<()> {
        self.active_tab_mut()
            .expect("workspace should always have at least one tab")
            .poll_terminal()
    }

    /// Poll all tabs and capture terminal I/O errors on each tab.
    pub fn poll_tabs(&mut self) {
        for tab in &mut self.tabs {
            if let Err(err) = tab.poll_terminal() {
                tab.set_terminal_error(format!("terminal I/O error: {err}"));
            }
        }
    }

    /// Send bytes to the active tab terminal, if it's running.
    pub fn write_terminal_input(&mut self, data: &[u8]) -> io::Result<()> {
        self.active_tab_mut()
            .expect("workspace should always have at least one tab")
            .write_input(data)
    }

    /// Current active tab terminal screen snapshot.
    pub fn terminal_screen(&self) -> Option<&ScreenBuffer> {
        self.active_tab().and_then(Tab::terminal_screen)
    }

    /// Scroll the active tab viewport up by roughly one page.
    pub fn scroll_up(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            tab.scroll_up();
        }
    }

    /// Scroll the active tab viewport down by roughly one page.
    pub fn scroll_down(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            tab.scroll_down();
        }
    }

    /// Return the active tab viewport to the live bottom.
    pub fn scroll_to_bottom(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            tab.scroll_to_bottom();
        }
    }

    /// Whether the active tab is currently viewing scrollback.
    pub fn is_scrolled(&self) -> bool {
        self.active_tab().is_some_and(Tab::is_scrolled)
    }

    /// Current active tab scroll offset (`0` = live bottom).
    pub fn scroll_offset(&self) -> usize {
        self.active_tab().map(Tab::scroll_offset).unwrap_or(0)
    }

    /// Whether the active tab terminal session has exited.
    pub fn terminal_has_exited(&self) -> bool {
        self.active_tab().is_some_and(Tab::terminal_has_exited)
    }

    /// Active tab terminal state for rendering.
    pub fn terminal_state(&self) -> WorkspaceTerminalState {
        self.active_tab()
            .map(Tab::terminal_state)
            .unwrap_or(WorkspaceTerminalState::NotStarted)
    }

    /// Last active-tab terminal error, if any.
    pub fn terminal_error(&self) -> Option<&str> {
        self.active_tab().and_then(Tab::terminal_error)
    }

    /// Exit status for the active tab terminal, if exited.
    pub fn terminal_exit_status(&self) -> Option<&ExitStatus> {
        self.active_tab().and_then(Tab::terminal_exit_status)
    }

    /// Store a terminal error message on the active tab.
    pub fn set_terminal_error(&mut self, message: impl Into<String>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.set_terminal_error(message);
        }
    }

    /// Clear terminal error state on the active tab.
    pub fn clear_terminal_error(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            tab.clear_terminal_error();
        }
    }

    /// Whether the auto-spawn command has already been sent.
    pub fn has_auto_spawned(&self) -> bool {
        self.auto_spawned
    }

    /// Mark that the auto-spawn command has been sent.
    pub fn mark_auto_spawned(&mut self) {
        self.auto_spawned = true;
    }

    fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab_index)
    }

    fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active_tab_index)
    }

    fn terminal_cwd(&self) -> io::Result<PathBuf> {
        if let Some(path) = &self.worktree_path {
            Ok(path.clone())
        } else {
            env::current_dir()
        }
    }
}

impl From<WorkspaceState> for Workspace {
    fn from(state: WorkspaceState) -> Self {
        let tab_count = state
            .tabs
            .unwrap_or_else(|| vec![TabState::default()])
            .len()
            .max(1);
        let tabs = (0..tab_count).map(|_| Tab::new()).collect::<Vec<_>>();
        let active_tab_index = state.active_tab_index.min(tab_count - 1);

        Self {
            id: state.id,
            name: state.name,
            worktree_path: state.worktree_path,
            branch_name: state.branch_name,
            tabs,
            active_tab_index,
            auto_spawned: false,
        }
    }
}

impl From<&Workspace> for WorkspaceState {
    fn from(workspace: &Workspace) -> Self {
        Self {
            id: workspace.id.clone(),
            name: workspace.name.clone(),
            worktree_path: workspace.worktree_path.clone(),
            branch_name: workspace.branch_name.clone(),
            active_tab_index: workspace.active_tab_index,
            tabs: Some(vec![TabState::default(); workspace.tabs.len().max(1)]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};

    fn screen_contains_token(workspace: &Workspace, token: &str) -> bool {
        workspace
            .terminal_screen()
            .map(|screen| {
                (0..screen.rows()).any(|row| {
                    screen
                        .row_text(row)
                        .is_some_and(|line| line.contains(token))
                })
            })
            .unwrap_or(false)
    }

    #[test]
    fn tab_crud_keeps_workspace_non_empty() {
        let mut workspace = Workspace::new("1", "W1");
        assert_eq!(workspace.tab_count(), 1);
        assert_eq!(workspace.active_tab_index(), 0);

        workspace.add_tab();
        workspace.add_tab();
        assert_eq!(workspace.tab_count(), 3);
        assert_eq!(workspace.active_tab_index(), 2);

        assert!(workspace.select_tab(0));
        assert_eq!(workspace.active_tab_index(), 0);
        assert!(!workspace.select_tab(9));

        workspace.next_tab();
        assert_eq!(workspace.active_tab_index(), 1);
        workspace.prev_tab();
        assert_eq!(workspace.active_tab_index(), 0);

        assert!(workspace.close_tab());
        assert_eq!(workspace.tab_count(), 2);
        assert_eq!(workspace.active_tab_index(), 0);

        assert!(workspace.close_tab());
        assert_eq!(workspace.tab_count(), 1);
        assert_eq!(workspace.active_tab_index(), 0);

        assert!(!workspace.close_tab(), "last tab must not close");
    }

    #[test]
    fn tabs_have_independent_terminal_buffers_within_workspace() {
        let mut workspace = Workspace::new("1", "W1");
        workspace
            .ensure_terminal_started(80, 24, None, 1000)
            .expect("start tab 1");

        workspace.add_tab();
        workspace
            .ensure_terminal_started(80, 24, None, 1000)
            .expect("start tab 2");

        // Active tab is tab 2.
        workspace
            .write_terminal_input(b"echo TAB_TWO_ONLY\r")
            .expect("write tab 2");

        let mut found = false;
        for _ in 0..60 {
            workspace.poll_tabs();
            if screen_contains_token(&workspace, "TAB_TWO_ONLY") {
                found = true;
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(found, "second tab should receive its output");

        workspace.select_tab(0);
        for _ in 0..10 {
            workspace.poll_tabs();
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            !screen_contains_token(&workspace, "TAB_TWO_ONLY"),
            "first tab should not show second tab output"
        );

        workspace.select_tab(0);
        let _ = workspace.write_terminal_input(b"exit\r");
        workspace.select_tab(1);
        let _ = workspace.write_terminal_input(b"exit\r");
    }

    #[test]
    fn terminal_round_trip_and_exit_are_handled() {
        let mut workspace = Workspace::new("1", "W1");
        workspace
            .ensure_terminal_started(80, 24, None, 1000)
            .expect("start terminal");

        workspace
            .write_terminal_input(b"echo PHASE12_OK\r")
            .expect("write command");

        let mut found_output = false;
        for _ in 0..60 {
            workspace.poll_terminal().expect("poll terminal");
            let has_token = screen_contains_token(&workspace, "PHASE12_OK");
            if has_token {
                found_output = true;
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }

        assert!(
            found_output,
            "terminal output should contain command result"
        );

        workspace
            .write_terminal_input(b"exit\r")
            .expect("write exit command");
        for _ in 0..60 {
            workspace.poll_terminal().expect("poll terminal");
            if workspace.terminal_has_exited() {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }

        assert!(
            workspace.terminal_has_exited(),
            "terminal should report exited after shell exit"
        );

        workspace
            .ensure_terminal_started(80, 24, None, 1000)
            .expect("restart terminal");
        workspace.poll_terminal().expect("poll restarted terminal");
        assert!(
            !workspace.terminal_has_exited(),
            "terminal should be running again after restart"
        );
    }

    #[test]
    fn workspaces_have_independent_terminal_buffers() {
        let mut ws_one = Workspace::new("1", "W1");
        let mut ws_two = Workspace::new("2", "W2");

        ws_one
            .ensure_terminal_started(80, 24, None, 1000)
            .expect("start ws one");
        ws_two
            .ensure_terminal_started(80, 24, None, 1000)
            .expect("start ws two");

        ws_one
            .write_terminal_input(b"echo WS_ONE_ONLY\r")
            .expect("write ws one");

        let mut found = false;
        for _ in 0..60 {
            ws_one.poll_terminal().expect("poll ws one");
            ws_two.poll_terminal().expect("poll ws two");
            if screen_contains_token(&ws_one, "WS_ONE_ONLY") {
                found = true;
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }

        assert!(found, "first workspace should receive its command output");
        assert!(
            !screen_contains_token(&ws_two, "WS_ONE_ONLY"),
            "second workspace should not receive first workspace output"
        );

        let _ = ws_one.write_terminal_input(b"exit\r");
        let _ = ws_two.write_terminal_input(b"exit\r");
    }

    #[test]
    fn terminal_scrollback_state_is_exposed_and_scrollable() {
        let mut workspace = Workspace::new("1", "W1");
        workspace
            .ensure_terminal_started(80, 10, None, 1000)
            .expect("start terminal");

        workspace
            .write_terminal_input(
                b"i=1; while [ $i -le 80 ]; do echo SCROLL_TEST_$i; i=$((i+1)); done\r",
            )
            .expect("write command");

        let mut saw_tail = false;
        for _ in 0..120 {
            workspace.poll_terminal().expect("poll terminal");
            if screen_contains_token(&workspace, "SCROLL_TEST_80") {
                saw_tail = true;
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(saw_tail, "expected terminal output to fill scrollback");

        workspace.scroll_up();
        assert!(workspace.is_scrolled(), "workspace should report scrolled");
        assert!(
            workspace.scroll_offset() > 0,
            "scroll offset should increase"
        );

        let offset_after_first_page = workspace.scroll_offset();
        workspace.scroll_up();
        assert!(
            workspace.scroll_offset() >= offset_after_first_page,
            "additional scroll up should not reduce offset"
        );

        workspace.scroll_down();
        workspace.scroll_to_bottom();
        assert_eq!(
            workspace.scroll_offset(),
            0,
            "scroll_to_bottom should reset"
        );
        assert!(
            !workspace.is_scrolled(),
            "workspace should report live bottom"
        );

        let _ = workspace.write_terminal_input(b"exit\r");
    }

    #[test]
    fn ensure_terminal_started_returns_true_on_fresh_spawn() {
        let mut workspace = Workspace::new("1", "W1");
        let spawned = workspace
            .ensure_terminal_started(80, 24, None, 1000)
            .expect("start terminal");
        assert!(spawned, "first call should report fresh spawn");

        // Second call should return false (already running, just resized).
        let spawned = workspace
            .ensure_terminal_started(80, 24, None, 1000)
            .expect("resize terminal");
        assert!(!spawned, "second call should not report fresh spawn");

        let _ = workspace.write_terminal_input(b"exit\r");
    }

    #[test]
    fn auto_spawn_tracking_works() {
        let workspace = Workspace::new("1", "W1");
        assert!(!workspace.has_auto_spawned());
    }
}
