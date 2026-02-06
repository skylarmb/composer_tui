//! Background GitHub PR/CI status polling for workspaces.

use std::{
    collections::HashMap,
    io,
    path::PathBuf,
    process::Command,
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Aggregated CI result for a PR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GhCiStatus {
    Passing,
    Pending,
    Failing,
}

/// Cached `gh pr view` status for a workspace branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhWorkspaceStatus {
    pub number: u64,
    pub pr_state: String,
    pub title: String,
    pub ci_status: GhCiStatus,
}

/// Input target describing which workspace branch should be polled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhWorkspaceTarget {
    pub workspace_id: String,
    pub cwd: PathBuf,
    pub branch_name: String,
}

impl GhWorkspaceTarget {
    pub fn new(
        workspace_id: impl Into<String>,
        cwd: PathBuf,
        branch_name: impl Into<String>,
    ) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            cwd,
            branch_name: branch_name.into(),
        }
    }
}

/// Status update emitted by the background poller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhStatusUpdate {
    pub workspace_id: String,
    pub status: Option<GhWorkspaceStatus>,
}

enum GhStatusCommand {
    SetTargets(Vec<GhWorkspaceTarget>),
    Stop,
}

/// Polls PR/CI status using the `gh` CLI on a background thread.
pub struct GhStatusFetcher {
    command_tx: Sender<GhStatusCommand>,
    update_rx: Receiver<GhStatusUpdate>,
    worker: Option<JoinHandle<()>>,
}

impl GhStatusFetcher {
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
    pub fn set_targets(&self, targets: Vec<GhWorkspaceTarget>) {
        let _ = self.command_tx.send(GhStatusCommand::SetTargets(targets));
    }

    /// Drain any queued status updates without blocking.
    pub fn drain_updates(&self) -> Vec<GhStatusUpdate> {
        let mut out = Vec::new();
        while let Ok(update) = self.update_rx.try_recv() {
            out.push(update);
        }
        out
    }
}

impl Drop for GhStatusFetcher {
    fn drop(&mut self) {
        let _ = self.command_tx.send(GhStatusCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run_worker(
    poll_interval: Duration,
    command_rx: Receiver<GhStatusCommand>,
    update_tx: Sender<GhStatusUpdate>,
) {
    let mut targets: HashMap<String, GhWorkspaceTarget> = HashMap::new();
    let mut next_poll_at = Instant::now();
    let mut gh_available = true;

    loop {
        let timeout = next_poll_at.saturating_duration_since(Instant::now());
        match command_rx.recv_timeout(timeout) {
            Ok(GhStatusCommand::SetTargets(new_targets)) => {
                targets = new_targets
                    .into_iter()
                    .map(|target| (target.workspace_id.clone(), target))
                    .collect();
                next_poll_at = Instant::now();
            }
            Ok(GhStatusCommand::Stop) => break,
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {
                if !gh_available {
                    for workspace_id in targets.keys() {
                        let _ = update_tx.send(GhStatusUpdate {
                            workspace_id: workspace_id.clone(),
                            status: None,
                        });
                    }
                    next_poll_at = Instant::now() + poll_interval;
                    continue;
                }

                for target in targets.values() {
                    match fetch_gh_status(target) {
                        Ok(status) => {
                            let _ = update_tx.send(GhStatusUpdate {
                                workspace_id: target.workspace_id.clone(),
                                status,
                            });
                        }
                        Err(FetchError::GhUnavailable) => {
                            gh_available = false;
                            for workspace_id in targets.keys() {
                                let _ = update_tx.send(GhStatusUpdate {
                                    workspace_id: workspace_id.clone(),
                                    status: None,
                                });
                            }
                            break;
                        }
                        Err(FetchError::Io) => {
                            let _ = update_tx.send(GhStatusUpdate {
                                workspace_id: target.workspace_id.clone(),
                                status: None,
                            });
                        }
                    }
                }

                next_poll_at = Instant::now() + poll_interval;
            }
        }
    }
}

#[derive(Debug)]
enum FetchError {
    GhUnavailable,
    Io,
}

fn fetch_gh_status(target: &GhWorkspaceTarget) -> Result<Option<GhWorkspaceStatus>, FetchError> {
    let output = Command::new("gh")
        .arg("pr")
        .arg("view")
        .arg(&target.branch_name)
        .arg("--json")
        .arg("number,state,title,statusCheckRollup")
        .arg("--template")
        .arg("{{.number}}\n{{.state}}\n{{.title}}\n{{range .statusCheckRollup}}{{if .conclusion}}{{.conclusion}}{{else}}{{.state}}{{end}}\n{{end}}")
        .current_dir(&target.cwd)
        .output()
        .map_err(|err| match err.kind() {
            io::ErrorKind::NotFound => FetchError::GhUnavailable,
            _ => FetchError::Io,
        })?;

    if !output.status.success() {
        // No PR for branch, auth issue, or transient CLI error.
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_template_output(&stdout))
}

fn parse_template_output(raw: &str) -> Option<GhWorkspaceStatus> {
    let mut lines = raw.lines();
    let number = lines.next()?.trim().parse::<u64>().ok()?;
    let pr_state = lines.next()?.trim().to_ascii_uppercase();
    if pr_state.is_empty() {
        return None;
    }
    let title = lines.next().unwrap_or_default().trim().to_string();
    let rollup_states: Vec<String> = lines
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.to_ascii_uppercase())
        .collect();

    Some(GhWorkspaceStatus {
        number,
        pr_state,
        title,
        ci_status: aggregate_ci_status(&rollup_states),
    })
}

fn aggregate_ci_status(rollup_states: &[String]) -> GhCiStatus {
    if rollup_states.is_empty() {
        return GhCiStatus::Pending;
    }

    if rollup_states.iter().any(|state| is_failing_state(state)) {
        return GhCiStatus::Failing;
    }
    if rollup_states.iter().any(|state| is_pending_state(state)) {
        return GhCiStatus::Pending;
    }
    GhCiStatus::Passing
}

fn is_failing_state(state: &str) -> bool {
    matches!(
        state,
        "FAILURE"
            | "FAILED"
            | "ERROR"
            | "CANCELLED"
            | "TIMED_OUT"
            | "ACTION_REQUIRED"
            | "STARTUP_FAILURE"
    )
}

fn is_pending_state(state: &str) -> bool {
    matches!(
        state,
        "PENDING" | "IN_PROGRESS" | "QUEUED" | "WAITING" | "REQUESTED" | "EXPECTED"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn parse_template_output_reads_pr_state_title_and_checks() {
        let raw = "1234\nOPEN\nPhase 17 PR\nSUCCESS\nPENDING\n";
        let parsed = parse_template_output(raw).expect("parsed status");
        assert_eq!(parsed.number, 1234);
        assert_eq!(parsed.pr_state, "OPEN");
        assert_eq!(parsed.title, "Phase 17 PR");
        assert_eq!(parsed.ci_status, GhCiStatus::Pending);
    }

    #[test]
    fn aggregate_ci_status_failing_wins() {
        let checks = vec!["SUCCESS".to_string(), "FAILURE".to_string()];
        assert_eq!(aggregate_ci_status(&checks), GhCiStatus::Failing);
    }

    #[test]
    fn aggregate_ci_status_defaults_to_pending_without_checks() {
        assert_eq!(aggregate_ci_status(&[]), GhCiStatus::Pending);
    }

    #[test]
    fn fetcher_drains_updates_non_blocking() {
        let fetcher = GhStatusFetcher::new(Duration::from_millis(5));
        fetcher.set_targets(vec![]);
        let updates = fetcher.drain_updates();
        assert!(updates.is_empty());
    }
}
