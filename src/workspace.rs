//! Workspace domain model.
//!
//! The MVP keeps only an id and display name; additional fields (paths,
//! status, agent process info) will be added in later phases.

/// Represents a single agent workspace displayed in the sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    id: String,
    name: String,
}

impl Workspace {
    /// Create a new `Workspace` with the given id and display name.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
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
}
