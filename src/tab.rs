//! Tab domain model for per-workspace multi-terminal support.

use std::{io, path::Path};

use portable_pty::ExitStatus;

use crate::terminal::{ScreenBuffer, Terminal};

struct TabTerminal {
    terminal: Terminal,
    screen: ScreenBuffer,
    scroll_offset: usize,
    exit_status: Option<ExitStatus>,
}

/// Runtime terminal state shown in the main panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceTerminalState {
    NotStarted,
    Running,
    Exited,
    Failed,
}

/// A single terminal tab within a workspace.
pub struct Tab {
    terminal: Option<TabTerminal>,
    terminal_error: Option<String>,
    shell_name: String,
    pending_command_input: String,
    last_command_name: Option<String>,
}

impl Tab {
    /// Create a new tab with no terminal started yet.
    pub fn new() -> Self {
        Self {
            terminal: None,
            terminal_error: None,
            shell_name: default_shell_name(),
            pending_command_input: String::new(),
            last_command_name: None,
        }
    }

    /// Ensure this tab has a running terminal session.
    ///
    /// Returns `Ok(true)` when a fresh terminal was spawned, `Ok(false)`
    /// when the existing terminal was kept (possibly resized).
    pub fn ensure_terminal_started(
        &mut self,
        cwd: &Path,
        cols: u16,
        rows: u16,
        shell: Option<&str>,
        scrollback_limit: usize,
    ) -> io::Result<bool> {
        let cols = cols.max(1);
        let rows = rows.max(1);

        let needs_spawn = self
            .terminal
            .as_ref()
            .map(|runtime| runtime.exit_status.is_some())
            .unwrap_or(true);

        if needs_spawn {
            self.terminal.take();
            self.shell_name = shell
                .map(command_name_only)
                .unwrap_or_else(default_shell_name);
            self.pending_command_input.clear();
            let terminal = Terminal::spawn(cwd, shell)?;
            terminal.resize(cols, rows)?;
            let mut screen = ScreenBuffer::new_with_scrollback(
                usize::from(cols),
                usize::from(rows),
                scrollback_limit,
            );
            let initial = terminal.read();
            if !initial.is_empty() {
                screen.write(&initial);
            }
            self.terminal = Some(TabTerminal {
                terminal,
                screen,
                scroll_offset: 0,
                exit_status: None,
            });
            self.terminal_error = None;
            return Ok(true);
        }

        self.resize(cols, rows)?;
        Ok(false)
    }

    /// Resize the tab terminal if present.
    pub fn resize(&mut self, cols: u16, rows: u16) -> io::Result<()> {
        let cols = cols.max(1);
        let rows = rows.max(1);

        if let Some(runtime) = &mut self.terminal {
            runtime.terminal.resize(cols, rows)?;
            runtime.screen.resize(usize::from(cols), usize::from(rows));
            runtime.scroll_offset = runtime
                .scroll_offset
                .min(runtime.screen.max_scroll_offset());
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
            let previous_scrollback_len = runtime.screen.scrollback_len();
            let responses = runtime.screen.write_with_responses(&output);
            if !responses.is_empty() {
                runtime.terminal.write(&responses)?;
            }
            if runtime.scroll_offset > 0 {
                let appended_lines = runtime
                    .screen
                    .scrollback_len()
                    .saturating_sub(previous_scrollback_len);
                runtime.scroll_offset = runtime
                    .scroll_offset
                    .saturating_add(appended_lines)
                    .min(runtime.screen.max_scroll_offset());
            }
        }

        if runtime.exit_status.is_none() {
            runtime.exit_status = runtime.terminal.try_wait()?;
        }

        Ok(())
    }

    /// Send bytes to the terminal, if it's running.
    pub fn write_input(&mut self, data: &[u8]) -> io::Result<()> {
        let should_write = self
            .terminal
            .as_ref()
            .is_some_and(|runtime| runtime.exit_status.is_none());
        if !should_write {
            return Ok(());
        }

        self.track_command_input(data);
        let Some(runtime) = &mut self.terminal else {
            return Ok(());
        };
        runtime.terminal.write(data)
    }

    /// Best-effort short tab label for header rendering.
    pub fn title(&self, index: usize) -> String {
        match self.terminal_state() {
            WorkspaceTerminalState::NotStarted => format!("tab{}", index + 1),
            WorkspaceTerminalState::Running => self
                .last_command_name
                .clone()
                .unwrap_or_else(|| self.shell_name.clone()),
            WorkspaceTerminalState::Exited => "exited".to_string(),
            WorkspaceTerminalState::Failed => "failed".to_string(),
        }
    }

    /// Current terminal screen snapshot.
    pub fn terminal_screen(&self) -> Option<&ScreenBuffer> {
        self.terminal.as_ref().map(|runtime| &runtime.screen)
    }

    /// Scroll the terminal viewport up by roughly one page.
    pub fn scroll_up(&mut self) {
        let Some(runtime) = &mut self.terminal else {
            return;
        };
        let page = runtime.screen.rows().saturating_sub(1).max(1);
        runtime.scroll_offset = runtime
            .scroll_offset
            .saturating_add(page)
            .min(runtime.screen.max_scroll_offset());
    }

    /// Scroll the terminal viewport down by roughly one page.
    pub fn scroll_down(&mut self) {
        let Some(runtime) = &mut self.terminal else {
            return;
        };
        let page = runtime.screen.rows().saturating_sub(1).max(1);
        runtime.scroll_offset = runtime.scroll_offset.saturating_sub(page);
    }

    /// Return to the live bottom of terminal output.
    pub fn scroll_to_bottom(&mut self) {
        let Some(runtime) = &mut self.terminal else {
            return;
        };
        runtime.scroll_offset = 0;
    }

    /// Whether this tab is currently viewing scrollback.
    pub fn is_scrolled(&self) -> bool {
        self.terminal
            .as_ref()
            .is_some_and(|runtime| runtime.scroll_offset > 0)
    }

    /// Current terminal scroll offset (`0` = live bottom).
    pub fn scroll_offset(&self) -> usize {
        self.terminal
            .as_ref()
            .map(|runtime| runtime.scroll_offset)
            .unwrap_or(0)
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

    /// Last terminal error for this tab, if any.
    pub fn terminal_error(&self) -> Option<&str> {
        self.terminal_error.as_deref()
    }

    /// Exit status for the tab terminal, if exited.
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

    fn track_command_input(&mut self, data: &[u8]) {
        for &byte in data {
            match byte {
                b'\r' | b'\n' => {
                    let submitted = self.pending_command_input.trim();
                    if !submitted.is_empty() {
                        self.last_command_name = first_word(submitted);
                    }
                    self.pending_command_input.clear();
                }
                0x7f | 0x08 => {
                    self.pending_command_input.pop();
                }
                0x03 => {
                    self.pending_command_input.clear();
                }
                0x20..=0x7e => {
                    self.pending_command_input.push(byte as char);
                }
                _ => {}
            }
        }
    }
}

impl Default for Tab {
    fn default() -> Self {
        Self::new()
    }
}

fn default_shell_name() -> String {
    #[cfg(unix)]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        command_name_only(&shell)
    }
    #[cfg(windows)]
    {
        let shell = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
        command_name_only(&shell)
    }
}

fn command_name_only(command: &str) -> String {
    command
        .rsplit('/')
        .next()
        .unwrap_or(command)
        .rsplit('\\')
        .next()
        .unwrap_or(command)
        .to_string()
}

fn first_word(input: &str) -> Option<String> {
    input
        .split_whitespace()
        .next()
        .map(command_name_only)
        .filter(|word| !word.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_prefers_last_submitted_command() {
        let mut tab = Tab::new();
        tab.last_command_name = Some("cargo".to_string());
        tab.terminal = Some(TabTerminal {
            terminal: crate::terminal::Terminal::spawn(
                std::env::current_dir().expect("cwd"),
                Some("/bin/sh"),
            )
            .expect("spawn"),
            screen: ScreenBuffer::new(10, 2),
            scroll_offset: 0,
            exit_status: None,
        });

        assert_eq!(tab.title(0), "cargo");
    }

    #[test]
    fn track_command_input_extracts_program_name() {
        let mut tab = Tab::new();
        tab.track_command_input(b"cargo test\r");
        assert_eq!(tab.last_command_name.as_deref(), Some("cargo"));
    }
}
