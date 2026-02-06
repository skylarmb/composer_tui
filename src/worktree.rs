//! Git worktree management helpers.

use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

use git2::{Branch, BranchType, ErrorCode, Repository, WorktreeAddOptions, WorktreePruneOptions};

use crate::config::Config;

/// Metadata about an existing worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeInfo {
    pub name: String,
    pub path: PathBuf,
}

/// Errors that can occur during worktree operations.
#[derive(Debug)]
pub enum WorktreeError {
    NotGitRepo { path: PathBuf },
    BareRepo { path: PathBuf },
    NameExists { name: String, path: PathBuf },
    WorktreeNotFound { name: String },
    InvalidBranchName { branch: String },
    BranchConflict { branch: String },
    Git(git2::Error),
    Io(io::Error),
}

impl fmt::Display for WorktreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorktreeError::NotGitRepo { path } => {
                write!(f, "not a git repository: {}", path.display())
            }
            WorktreeError::BareRepo { path } => {
                write!(
                    f,
                    "bare repository does not support worktrees: {}",
                    path.display()
                )
            }
            WorktreeError::NameExists { name, path } => {
                write!(f, "worktree '{name}' already exists at {}", path.display())
            }
            WorktreeError::WorktreeNotFound { name } => {
                write!(f, "worktree '{name}' not found")
            }
            WorktreeError::InvalidBranchName { branch } => {
                write!(f, "invalid branch name: {branch}")
            }
            WorktreeError::BranchConflict { branch } => {
                write!(
                    f,
                    "branch '{branch}' is already checked out in another worktree"
                )
            }
            WorktreeError::Git(err) => write!(f, "{err}"),
            WorktreeError::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for WorktreeError {}

impl From<git2::Error> for WorktreeError {
    fn from(err: git2::Error) -> Self {
        WorktreeError::Git(err)
    }
}

impl From<io::Error> for WorktreeError {
    fn from(err: io::Error) -> Self {
        WorktreeError::Io(err)
    }
}

/// Core worktree operations backed by libgit2.
pub struct WorktreeManager {
    repo: Repository,
    base_dir: PathBuf,
}

impl WorktreeManager {
    /// Open a git repository and create a worktree manager.
    pub fn new(repo_path: impl AsRef<Path>) -> Result<Self, WorktreeError> {
        let repo_path = repo_path.as_ref();
        let repo = Repository::discover(repo_path).map_err(|_| WorktreeError::NotGitRepo {
            path: repo_path.to_path_buf(),
        })?;
        let workdir = repo.workdir().ok_or_else(|| WorktreeError::BareRepo {
            path: repo_path.to_path_buf(),
        })?;
        let base_dir = resolve_base_dir(workdir, Config::load().worktree_base_dir);
        Ok(Self { repo, base_dir })
    }

    /// Base directory used for new worktrees.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Create a new worktree at `{base_dir}/{name}` on the given branch.
    pub fn create_worktree(&self, name: &str, branch: &str) -> Result<PathBuf, WorktreeError> {
        let target_path = self.worktree_path(name);
        if target_path.exists() || self.worktree_name_exists(name)? {
            return Err(WorktreeError::NameExists {
                name: name.to_string(),
                path: target_path,
            });
        }

        if !Branch::name_is_valid(branch)? {
            return Err(WorktreeError::InvalidBranchName {
                branch: branch.to_string(),
            });
        }

        if self.branch_is_checked_out(branch)? {
            return Err(WorktreeError::BranchConflict {
                branch: branch.to_string(),
            });
        }

        let branch_ref = match self.repo.find_branch(branch, BranchType::Local) {
            Ok(branch_ref) => branch_ref,
            Err(err) if err.code() == ErrorCode::NotFound => {
                let head_commit = self.head_commit()?;
                self.repo.branch(branch, &head_commit, false)?
            }
            Err(err) => return Err(err.into()),
        };

        fs::create_dir_all(&self.base_dir)?;
        let mut opts = WorktreeAddOptions::new();
        opts.reference(Some(branch_ref.get()));
        self.repo.worktree(name, &target_path, Some(&opts))?;
        Ok(target_path)
    }

    /// Delete a worktree and remove it from disk.
    pub fn delete_worktree(&self, name: &str) -> Result<(), WorktreeError> {
        let worktree = self
            .repo
            .find_worktree(name)
            .map_err(|err| match err.code() {
                ErrorCode::NotFound => WorktreeError::WorktreeNotFound {
                    name: name.to_string(),
                },
                _ => WorktreeError::Git(err),
            })?;

        let mut opts = WorktreePruneOptions::new();
        opts.valid(true).working_tree(true);
        worktree.prune(Some(&mut opts))?;
        Ok(())
    }

    /// List existing worktrees for the repository.
    pub fn list_worktrees(&self) -> Result<Vec<WorktreeInfo>, WorktreeError> {
        let mut out = Vec::new();
        let names = self.repo.worktrees()?;
        for name in names.iter().flatten() {
            if let Ok(worktree) = self.repo.find_worktree(name) {
                out.push(WorktreeInfo {
                    name: name.to_string(),
                    path: worktree.path().to_path_buf(),
                });
            }
        }
        Ok(out)
    }

    fn worktree_path(&self, name: &str) -> PathBuf {
        self.base_dir.join(name)
    }

    fn worktree_name_exists(&self, name: &str) -> Result<bool, WorktreeError> {
        let names = self.repo.worktrees()?;
        Ok(names.iter().any(|existing| existing == Some(name)))
    }

    fn head_commit(&self) -> Result<git2::Commit<'_>, WorktreeError> {
        let head = self.repo.head()?;
        Ok(head.peel_to_commit()?)
    }

    fn branch_is_checked_out(&self, branch: &str) -> Result<bool, WorktreeError> {
        if is_branch_head(&self.repo, branch)? {
            return Ok(true);
        }

        let names = self.repo.worktrees()?;
        for name in names.iter().flatten() {
            if let Ok(worktree) = self.repo.find_worktree(name) {
                let worktree_repo = Repository::open(worktree.path())?;
                if is_branch_head(&worktree_repo, branch)? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

fn resolve_base_dir(repo_root: &Path, configured: Option<PathBuf>) -> PathBuf {
    let default = default_base_dir(repo_root);
    let base = configured.unwrap_or(default);
    if base.is_relative() {
        repo_root.join(base)
    } else {
        base
    }
}

fn default_base_dir(repo_root: &Path) -> PathBuf {
    let repo_name = repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo");
    let parent = repo_root.parent().unwrap_or(repo_root);
    parent.join(format!("{repo_name}_worktrees"))
}

fn is_branch_head(repo: &Repository, branch: &str) -> Result<bool, WorktreeError> {
    match repo.head() {
        Ok(head) if head.is_branch() => Ok(head.shorthand() == Some(branch)),
        Ok(_) => Ok(false),
        Err(err) if err.code() == ErrorCode::UnbornBranch => Ok(false),
        Err(err) if err.code() == ErrorCode::NotFound => Ok(false),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Repository, Signature};
    use std::{
        env, fs,
        path::{Path, PathBuf},
        process,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn with_temp_home<T>(f: impl FnOnce(PathBuf) -> T) -> T {
        let _guard = crate::test_support::lock_home_env();
        let original_home = env::var("HOME").ok();
        let unique = format!(
            "composer_tui_worktree_test_{}_{}",
            process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos()
        );
        let temp_home = env::temp_dir().join(format!("{unique}_home"));
        fs::create_dir_all(&temp_home).expect("failed to create temp home");
        env::set_var("HOME", &temp_home);

        let result = f(temp_home.clone());

        match original_home {
            Some(value) => env::set_var("HOME", value),
            None => env::remove_var("HOME"),
        }
        let _ = fs::remove_dir_all(temp_home);
        result
    }

    fn init_repo(repo_path: &Path) -> Repository {
        let repo = Repository::init(repo_path).expect("failed to init repo");
        fs::write(repo_path.join("README.md"), "hello").expect("failed to write file");
        let mut index = repo.index().expect("failed to open index");
        index
            .add_path(Path::new("README.md"))
            .expect("failed to add file");
        let tree_oid = index.write_tree().expect("failed to write tree");
        let tree = repo.find_tree(tree_oid).expect("failed to find tree");
        let sig = Signature::now("composer_tui test", "test@example.com")
            .expect("failed to build signature");
        repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
            .expect("failed to commit");
        drop(tree);
        repo
    }

    fn setup_configured_repo() -> (PathBuf, PathBuf) {
        let unique = format!(
            "composer_tui_worktree_repo_{}_{}",
            process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos()
        );
        let root = env::temp_dir().join(unique);
        let repo_path = root.join("repo");
        let base_dir = root.join("worktrees");
        fs::create_dir_all(&repo_path).expect("failed to create repo dir");
        init_repo(&repo_path);
        Config {
            worktree_base_dir: Some(base_dir.clone()),
            ..Config::default()
        }
        .save()
        .expect("failed to save config");
        (repo_path, base_dir)
    }

    #[test]
    fn new_returns_error_for_non_git_path() {
        with_temp_home(|_| {
            let path = env::temp_dir().join("composer_tui_not_a_repo");
            let _ = fs::create_dir_all(&path);
            let result = WorktreeManager::new(&path);
            assert!(matches!(result, Err(WorktreeError::NotGitRepo { .. })));
            let _ = fs::remove_dir_all(path);
        });
    }

    #[test]
    fn create_list_and_delete_worktree_round_trip() {
        with_temp_home(|_| {
            let (repo_path, base_dir) = setup_configured_repo();
            let manager = WorktreeManager::new(&repo_path).expect("manager");

            let created = manager
                .create_worktree("agent-a", "feature/agent-a")
                .expect("create");
            assert_eq!(created, base_dir.join("agent-a"));
            assert!(created.exists(), "worktree should exist on disk");

            let listed = manager.list_worktrees().expect("list");
            assert!(listed.iter().any(|entry| entry.name == "agent-a"));

            manager.delete_worktree("agent-a").expect("delete");
            assert!(!created.exists(), "worktree should be removed from disk");

            let listed_after_delete = manager.list_worktrees().expect("list");
            assert!(
                !listed_after_delete
                    .iter()
                    .any(|entry| entry.name == "agent-a"),
                "deleted worktree should not remain listed"
            );

            let _ = fs::remove_dir_all(
                repo_path
                    .parent()
                    .expect("repo parent should exist")
                    .to_path_buf(),
            );
        });
    }

    #[test]
    fn create_worktree_on_existing_branch() {
        with_temp_home(|_| {
            let (repo_path, _) = setup_configured_repo();
            let repo = Repository::open(&repo_path).expect("open repo");
            let head_commit = repo
                .head()
                .expect("head")
                .peel_to_commit()
                .expect("head commit");
            repo.branch("feature/existing", &head_commit, false)
                .expect("create branch");

            let manager = WorktreeManager::new(&repo_path).expect("manager");
            let path = manager
                .create_worktree("agent-existing", "feature/existing")
                .expect("create");
            assert!(path.exists(), "worktree should exist for existing branch");

            let _ = fs::remove_dir_all(
                repo_path
                    .parent()
                    .expect("repo parent should exist")
                    .to_path_buf(),
            );
        });
    }

    #[test]
    fn create_worktree_rejects_duplicate_name() {
        with_temp_home(|_| {
            let (repo_path, _) = setup_configured_repo();
            let manager = WorktreeManager::new(&repo_path).expect("manager");
            manager
                .create_worktree("agent-dup", "feature/dup")
                .expect("initial create");
            let second = manager.create_worktree("agent-dup", "feature/dup-2");
            assert!(matches!(second, Err(WorktreeError::NameExists { .. })));

            let _ = fs::remove_dir_all(
                repo_path
                    .parent()
                    .expect("repo parent should exist")
                    .to_path_buf(),
            );
        });
    }

    #[test]
    fn create_worktree_rejects_invalid_branch_name() {
        with_temp_home(|_| {
            let (repo_path, _) = setup_configured_repo();
            let manager = WorktreeManager::new(&repo_path).expect("manager");
            let result = manager.create_worktree("agent-invalid", "bad branch");
            assert!(matches!(
                result,
                Err(WorktreeError::InvalidBranchName { .. })
            ));

            let _ = fs::remove_dir_all(
                repo_path
                    .parent()
                    .expect("repo parent should exist")
                    .to_path_buf(),
            );
        });
    }

    #[test]
    fn create_worktree_rejects_checked_out_branch_conflict() {
        with_temp_home(|_| {
            let (repo_path, _) = setup_configured_repo();
            let repo = Repository::open(&repo_path).expect("open repo");
            let branch = repo
                .head()
                .expect("head")
                .shorthand()
                .expect("branch shorthand")
                .to_string();

            let manager = WorktreeManager::new(&repo_path).expect("manager");
            let result = manager.create_worktree("agent-conflict", &branch);
            assert!(matches!(result, Err(WorktreeError::BranchConflict { .. })));

            let _ = fs::remove_dir_all(
                repo_path
                    .parent()
                    .expect("repo parent should exist")
                    .to_path_buf(),
            );
        });
    }
}
