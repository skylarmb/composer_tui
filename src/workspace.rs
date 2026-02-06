//! Workspace domain model.
//!
//! The MVP keeps only an id and display name; additional fields (paths,
//! status, agent process info) will be added in later phases.

use std::{
    env, io,
    path::{Path, PathBuf},
};

use portable_pty::ExitStatus;

use crate::{
    state::WorkspaceState,
    terminal::{ScreenBuffer, Terminal},
};

struct WorkspaceTerminal {
    terminal: Terminal,
    screen: ScreenBuffer,
    exit_status: Option<ExitStatus>,
}

/// Represents a single agent workspace displayed in the sidebar.
pub struct Workspace {
    id: String,
    name: String,
    worktree_path: Option<PathBuf>,
    branch_name: Option<String>,
    terminal: Option<WorkspaceTerminal>,
    terminal_error: Option<String>,
}

impl std::fmt::Debug for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workspace")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("worktree_path", &self.worktree_path)
            .field("branch_name", &self.branch_name)
            .field("has_terminal", &self.terminal.is_some())
            .field("terminal_error", &self.terminal_error)
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

/// Runtime terminal state shown in the main panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceTerminalState {
    NotStarted,
    Running,
    Exited,
    Failed,
}

impl Workspace {
    /// Create a new `Workspace` with the given id and display name.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            worktree_path: None,
            branch_name: None,
            terminal: None,
            terminal_error: None,
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
            terminal: None,
            terminal_error: None,
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

    /// Ensure this workspace has a running terminal session.
    pub fn ensure_terminal_started(
        &mut self,
        cols: u16,
        rows: u16,
        shell: Option<&str>,
    ) -> io::Result<()> {
        let cols = cols.max(1);
        let rows = rows.max(1);

        let needs_spawn = self
            .terminal
            .as_ref()
            .map(|runtime| runtime.exit_status.is_some())
            .unwrap_or(true);

        if needs_spawn {
            self.terminal.take();
            let cwd = self.terminal_cwd()?;
            let terminal = Terminal::spawn(cwd, shell)?;
            terminal.resize(cols, rows)?;
            let mut screen = ScreenBuffer::new(usize::from(cols), usize::from(rows));
            let initial = terminal.read();
            if !initial.is_empty() {
                screen.write(&initial);
            }
            self.terminal = Some(WorkspaceTerminal {
                terminal,
                screen,
                exit_status: None,
            });
            self.terminal_error = None;
            return Ok(());
        }

        self.resize_terminal(cols, rows)
    }

    /// Resize the workspace terminal if present.
    pub fn resize_terminal(&mut self, cols: u16, rows: u16) -> io::Result<()> {
        let cols = cols.max(1);
        let rows = rows.max(1);

        if let Some(runtime) = &mut self.terminal {
            runtime.terminal.resize(cols, rows)?;
            runtime.screen.resize(usize::from(cols), usize::from(rows));
        }
        Ok(())
    }

    /// Poll terminal output and process lifecycle events.
    pub fn poll_terminal(&mut self) -> io::Result<()> {
        let Some(runtime) = &mut self.terminal else {
            return Ok(());
        };

        let output = runtime.terminal.read();
        if !output.is_empty() {
            runtime.screen.write(&output);
        }

        if runtime.exit_status.is_none() {
            runtime.exit_status = runtime.terminal.try_wait()?;
        }

        Ok(())
    }

    /// Send bytes to the terminal, if it's running.
    pub fn write_terminal_input(&mut self, data: &[u8]) -> io::Result<()> {
        let Some(runtime) = &mut self.terminal else {
            return Ok(());
        };
        if runtime.exit_status.is_some() {
            return Ok(());
        }
        runtime.terminal.write(data)
    }

    /// Current terminal screen snapshot.
    pub fn terminal_screen(&self) -> Option<&ScreenBuffer> {
        self.terminal.as_ref().map(|runtime| &runtime.screen)
    }

    /// Whether terminal session has exited.
    pub fn terminal_has_exited(&self) -> bool {
        self.terminal
            .as_ref()
            .is_some_and(|runtime| runtime.exit_status.is_some())
    }

    /// Terminal state for rendering.
    pub fn terminal_state(&self) -> WorkspaceTerminalState {
        if self.terminal_error.is_some() {
            WorkspaceTerminalState::Failed
        } else if let Some(runtime) = &self.terminal {
            if runtime.exit_status.is_some() {
                WorkspaceTerminalState::Exited
            } else {
                WorkspaceTerminalState::Running
            }
        } else {
            WorkspaceTerminalState::NotStarted
        }
    }

    /// Last terminal error for this workspace, if any.
    pub fn terminal_error(&self) -> Option<&str> {
        self.terminal_error.as_deref()
    }

    /// Exit status for the workspace terminal, if exited.
    pub fn terminal_exit_status(&self) -> Option<&ExitStatus> {
        self.terminal
            .as_ref()
            .and_then(|runtime| runtime.exit_status.as_ref())
    }

    /// Store a terminal error message and keep existing screen data.
    pub fn set_terminal_error(&mut self, message: impl Into<String>) {
        self.terminal_error = Some(message.into());
    }

    /// Clear terminal error state.
    pub fn clear_terminal_error(&mut self) {
        self.terminal_error = None;
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
        Self {
            id: state.id,
            name: state.name,
            worktree_path: state.worktree_path,
            branch_name: state.branch_name,
            terminal: None,
            terminal_error: None,
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
    fn terminal_round_trip_and_exit_are_handled() {
        let mut workspace = Workspace::new("1", "W1");
        workspace
            .ensure_terminal_started(80, 24, None)
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
            .ensure_terminal_started(80, 24, None)
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
            .ensure_terminal_started(80, 24, None)
            .expect("start ws one");
        ws_two
            .ensure_terminal_started(80, 24, None)
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
}
