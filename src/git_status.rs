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

    Some(GitWorkspaceStatus { dirty })
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

        fs::write(temp.join("README.md"), "changed\n").expect("mutate file");
        let dirty = read_git_status(&temp).expect("dirty status");
        assert!(dirty.dirty, "modified tracked file should be dirty");

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn fetcher_drains_updates_non_blocking() {
        let fetcher = GitStatusFetcher::new(Duration::from_millis(5));
        fetcher.set_targets(vec![]);
        let updates = fetcher.drain_updates();
        assert!(updates.is_empty());
    }
}
