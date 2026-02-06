//! Workspace domain model.
//!
//! The MVP keeps only an id and display name; additional fields (paths,
//! status, agent process info) will be added in later phases.

use std::path::{Path, PathBuf};

use crate::state::WorkspaceState;

/// Represents a single agent workspace displayed in the sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    id: String,
    name: String,
    worktree_path: Option<PathBuf>,
    branch_name: Option<String>,
}

impl Workspace {
    /// Create a new `Workspace` with the given id and display name.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            worktree_path: None,
            branch_name: None,
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
}

impl From<WorkspaceState> for Workspace {
    fn from(state: WorkspaceState) -> Self {
        Self {
            id: state.id,
            name: state.name,
            worktree_path: state.worktree_path,
            branch_name: state.branch_name,
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
