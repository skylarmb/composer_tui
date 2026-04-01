//! Background git dirty/clean polling for workspaces.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use git2::{Repository, Status, StatusOptions};

/// Cached git status for a workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorkspaceStatus {
    pub dirty: bool,
    pub unstaged_added: usize,
    pub unstaged_deleted: usize,
}

/// Input target describing which workspace path should be polled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorkspaceTarget {
    pub workspace_id: String,
    pub path: PathBuf,
}

impl GitWorkspaceTarget {
    pub fn new(workspace_id: impl Into<String>, path: PathBuf) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            path,
        }
    }
}

/// Status update emitted by the background poller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStatusUpdate {
    pub workspace_id: String,
    pub status: Option<GitWorkspaceStatus>,
}

enum GitStatusCommand {
    SetTargets(Vec<GitWorkspaceTarget>),
    Stop,
}

/// Polls workspace git status on a background thread.
pub struct GitStatusFetcher {
    command_tx: Sender<GitStatusCommand>,
    update_rx: Receiver<GitStatusUpdate>,
    worker: Option<JoinHandle<()>>,
}

impl GitStatusFetcher {
    /// Start a background poller with the provided interval.
    pub fn new(poll_interval: Duration) -> Self {
        let (command_tx, command_rx) = mpsc::channel();
        let (update_tx, update_rx) = mpsc::channel();
        let worker = thread::spawn(move || run_worker(poll_interval, command_rx, update_tx));
        Self {
            command_tx,
            update_rx,
            worker: Some(worker),
        }
    }

    /// Replace the current polling target set.
    pub fn set_targets(&self, targets: Vec<GitWorkspaceTarget>) {
        let _ = self.command_tx.send(GitStatusCommand::SetTargets(targets));
    }

    /// Drain any queued status updates without blocking.
    pub fn drain_updates(&self) -> Vec<GitStatusUpdate> {
        let mut out = Vec::new();
        while let Ok(update) = self.update_rx.try_recv() {
            out.push(update);
        }
        out
    }
}

impl Drop for GitStatusFetcher {
    fn drop(&mut self) {
        let _ = self.command_tx.send(GitStatusCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run_worker(
    poll_interval: Duration,
    command_rx: Receiver<GitStatusCommand>,
    update_tx: Sender<GitStatusUpdate>,
) {
    let mut targets: HashMap<String, PathBuf> = HashMap::new();
    let mut next_poll_at = Instant::now();

    loop {
        let timeout = next_poll_at.saturating_duration_since(Instant::now());
        match command_rx.recv_timeout(timeout) {
            Ok(GitStatusCommand::SetTargets(new_targets)) => {
                targets = new_targets
                    .into_iter()
                    .map(|target| (target.workspace_id, target.path))
                    .collect();
                next_poll_at = Instant::now();
            }
            Ok(GitStatusCommand::Stop) => break,
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {
                for (workspace_id, path) in &targets {
                    let status = read_git_status(path);
                    let _ = update_tx.send(GitStatusUpdate {
                        workspace_id: workspace_id.clone(),
                        status,
                    });
                }
                next_poll_at = Instant::now() + poll_interval;
            }
        }
    }
}

fn read_git_status(path: &Path) -> Option<GitWorkspaceStatus> {
    let repo = Repository::discover(path).ok()?;

    let mut status_options = StatusOptions::new();
    status_options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true);

    let statuses = repo.statuses(Some(&mut status_options)).ok()?;
    let dirty = statuses
        .iter()
        .any(|entry| entry.status() != Status::CURRENT);

    let (unstaged_added, unstaged_deleted) = unstaged_line_counts(&repo).unwrap_or((0, 0));

    Some(GitWorkspaceStatus {
        dirty,
        unstaged_added,
        unstaged_deleted,
    })
}

/// Build a human-readable list of lines describing the working-tree changes
/// for the given path.  Used to populate the changes panel modal.
///
/// Returns `None` only when the path is not inside a git repository.
pub fn read_changes_panel_lines(path: &Path) -> Option<Vec<String>> {
    let repo = Repository::discover(path).ok()?;

    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true);
    let statuses = repo.statuses(Some(&mut opts)).ok()?;

    let mut staged: Vec<String> = Vec::new();
    let mut unstaged: Vec<String> = Vec::new();
    let mut untracked: Vec<String> = Vec::new();
    let mut conflicts: Vec<String> = Vec::new();

    for entry in statuses.iter() {
        let path_str = entry.path().unwrap_or("?").to_string();
        let s = entry.status();

        if s.contains(Status::CONFLICTED) {
            conflicts.push(format!("  !!  {path_str}"));
            continue;
        }

        // Staged (index) changes
        if s.intersects(
            Status::INDEX_NEW
                | Status::INDEX_MODIFIED
                | Status::INDEX_DELETED
                | Status::INDEX_RENAMED
                | Status::INDEX_TYPECHANGE,
        ) {
            let code = if s.contains(Status::INDEX_NEW) {
                "A"
            } else if s.contains(Status::INDEX_DELETED) {
                "D"
            } else if s.contains(Status::INDEX_RENAMED) {
                "R"
            } else {
                "M"
            };
            staged.push(format!("  {code}   {path_str}"));
        }

        // Unstaged (workdir) changes
        if s.intersects(
            Status::WT_MODIFIED | Status::WT_DELETED | Status::WT_RENAMED | Status::WT_TYPECHANGE,
        ) {
            let code = if s.contains(Status::WT_DELETED) {
                "D"
            } else if s.contains(Status::WT_RENAMED) {
                "R"
            } else {
                "M"
            };
            unstaged.push(format!("  {code}   {path_str}"));
        }

        if s.contains(Status::WT_NEW) {
            untracked.push(format!("  ?   {path_str}"));
        }
    }

    let mut lines: Vec<String> = Vec::new();

    if !conflicts.is_empty() {
        lines.push("Conflicts:".to_string());
        lines.extend(conflicts);
        lines.push(String::new());
    }
    if !staged.is_empty() {
        lines.push("Staged:".to_string());
        lines.extend(staged);
        lines.push(String::new());
    }
    if !unstaged.is_empty() {
        lines.push("Unstaged:".to_string());
        lines.extend(unstaged);
        lines.push(String::new());
    }
    if !untracked.is_empty() {
        lines.push("Untracked:".to_string());
        lines.extend(untracked);
        lines.push(String::new());
    }

    if lines.is_empty() {
        lines.push("Nothing to show — working tree is clean.".to_string());
    }

    Some(lines)
}

/// Build a list of diff lines for the given workspace path.
///
/// When `show_branch_diff` is `true` the diff is computed between HEAD and the
/// working-tree (equivalent to `git diff HEAD`).  When `false` only unstaged
/// changes are shown (equivalent to `git diff`).
///
/// Each returned string is a single patch line — file headers, hunk headers,
/// context lines, and `+`/`-` lines — ready for display.
///
/// Returns `None` only when the path is not inside a git repository.
pub fn read_diff_lines(path: &Path, show_branch_diff: bool) -> Option<Vec<String>> {
    let repo = Repository::discover(path).ok()?;

    let diff = if show_branch_diff {
        let head = repo.head().ok()?;
        let commit = head.peel_to_commit().ok()?;
        let tree = commit.tree().ok()?;
        repo.diff_tree_to_workdir_with_index(Some(&tree), None)
            .ok()?
    } else {
        repo.diff_index_to_workdir(None, None).ok()?
    };

    let mut lines: Vec<String> = Vec::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let origin = line.origin();
        let content = std::str::from_utf8(line.content()).unwrap_or("");
        // File-header, hunk-header, and binary lines already contain their
        // full text; context/add/delete lines need the origin char prepended.
        let text = match origin {
            'F' | 'H' | 'B' => content
                .trim_end_matches('\n')
                .trim_end_matches('\r')
                .to_string(),
            _ => {
                let trimmed = content.trim_end_matches('\n').trim_end_matches('\r');
                format!("{origin}{trimmed}")
            }
        };
        if !text.is_empty() {
            lines.push(text);
        }
        true
    })
    .ok()?;

    if lines.is_empty() {
        lines.push("Nothing to show — working tree is clean.".to_string());
    }

    Some(lines)
}

fn unstaged_line_counts(repo: &Repository) -> Option<(usize, usize)> {
    let mut diff_options = git2::DiffOptions::new();
    diff_options
        .include_untracked(true)
        .recurse_untracked_dirs(true);
    let diff = repo
        .diff_index_to_workdir(None, Some(&mut diff_options))
        .ok()?;
    let stats = diff.stats().ok()?;
    Some((stats.insertions(), stats.deletions()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Repository, Signature};
    use std::{
        fs,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    fn init_repo(path: &Path) -> Repository {
        let repo = Repository::init(path).expect("init repo");
        fs::write(path.join("README.md"), "hello\n").expect("write file");
        let mut index = repo.index().expect("open index");
        index
            .add_path(Path::new("README.md"))
            .expect("index add path");
        index.write().expect("persist index");
        let tree_oid = index.write_tree().expect("write tree");
        let tree = repo.find_tree(tree_oid).expect("find tree");
        let sig = Signature::now("composer_tui test", "test@example.com").expect("build signature");
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .expect("commit");
        drop(tree);
        repo
    }

    #[test]
    fn read_git_status_reports_clean_and_dirty() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let temp = std::env::temp_dir().join(format!(
            "composer_tui_git_status_{}_{}",
            std::process::id(),
            unique
        ));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).expect("create temp dir");
        init_repo(&temp);

        let clean = read_git_status(&temp).expect("clean status");
        assert!(!clean.dirty, "fresh repo should be clean");
        assert_eq!(clean.unstaged_added, 0);
        assert_eq!(clean.unstaged_deleted, 0);

        fs::write(temp.join("README.md"), "changed\n").expect("mutate file");
        let dirty = read_git_status(&temp).expect("dirty status");
        assert!(dirty.dirty, "modified tracked file should be dirty");
        assert!(
            dirty.unstaged_added > 0 || dirty.unstaged_deleted > 0,
            "dirty file should produce non-zero line stats"
        );

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn fetcher_drains_updates_non_blocking() {
        let fetcher = GitStatusFetcher::new(Duration::from_millis(5));
        fetcher.set_targets(vec![]);
        let updates = fetcher.drain_updates();
        assert!(updates.is_empty());
    }

    #[test]
    fn read_diff_lines_returns_empty_message_on_clean_repo() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let temp = std::env::temp_dir().join(format!(
            "composer_tui_diff_clean_{}_{}",
            std::process::id(),
            unique
        ));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).expect("create temp dir");
        init_repo(&temp);

        // Both diff types should report a clean working tree.
        let unstaged = read_diff_lines(&temp, false).expect("unstaged diff lines");
        assert_eq!(unstaged, vec!["Nothing to show — working tree is clean."]);

        let branch = read_diff_lines(&temp, true).expect("branch diff lines");
        assert_eq!(branch, vec!["Nothing to show — working tree is clean."]);

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn read_diff_lines_unstaged_shows_modified_content() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let temp = std::env::temp_dir().join(format!(
            "composer_tui_diff_unstaged_{}_{}",
            std::process::id(),
            unique
        ));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).expect("create temp dir");
        init_repo(&temp);

        // Unstaged modification.
        fs::write(temp.join("README.md"), "changed\n").expect("mutate file");

        let lines = read_diff_lines(&temp, false).expect("diff lines");
        let joined = lines.join("\n");
        assert!(
            joined.contains('+') || joined.contains('-'),
            "unstaged diff should contain diff lines: {joined}"
        );
        assert!(
            joined.contains("README.md"),
            "diff should mention the changed file: {joined}"
        );

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn read_diff_lines_branch_diff_includes_staged_changes() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let temp = std::env::temp_dir().join(format!(
            "composer_tui_diff_branch_{}_{}",
            std::process::id(),
            unique
        ));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).expect("create temp dir");
        let repo = init_repo(&temp);

        // Stage a new file — this would be invisible to `git diff` (unstaged)
        // but visible to `git diff HEAD` (branch diff).
        fs::write(temp.join("NEW.md"), "new file\n").expect("write new file");
        let mut index = repo.index().expect("open index");
        index.add_path(Path::new("NEW.md")).expect("stage new file");
        index.write().expect("persist index");

        let branch = read_diff_lines(&temp, true).expect("branch diff lines");
        let joined = branch.join("\n");
        assert!(
            joined.contains("NEW.md"),
            "branch diff should contain the staged new file: {joined}"
        );

        // Unstaged diff should NOT include the staged-only file.
        let unstaged = read_diff_lines(&temp, false).expect("unstaged diff lines");
        let unstaged_joined = unstaged.join("\n");
        assert!(
            !unstaged_joined.contains("NEW.md"),
            "unstaged diff should not include purely staged file: {unstaged_joined}"
        );

        let _ = fs::remove_dir_all(&temp);
    }
}
