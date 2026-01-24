use crate::config::Config;
use anyhow::Result;
use git2::{Repository, StatusOptions};
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::path::{Path, PathBuf};

/// Status of a single git repository
#[derive(Debug, Clone)]
pub struct RepoStatus {
    pub name: String,
    pub path: PathBuf,
    pub is_dirty: bool,
    pub sync_status: SyncStatus,
}

/// Sync status with remote
#[derive(Debug, Clone)]
pub enum SyncStatus {
    Synced,
    Ahead(usize),
    Behind(usize),
    Diverged { ahead: usize, behind: usize },
    Local,    // No remote tracking
    Detached, // Detached HEAD
    NoHead,   // Empty repo
    Unknown,
}

impl SyncStatus {
    pub fn display(&self) -> String {
        match self {
            SyncStatus::Synced => "Synced".to_string(),
            SyncStatus::Ahead(n) => format!("↑ {}", n),
            SyncStatus::Behind(n) => format!("↓ {}", n),
            SyncStatus::Diverged { ahead, behind } => format!("↑ {} ↓ {}", ahead, behind),
            SyncStatus::Local => "Local".to_string(),
            SyncStatus::Detached => "Detached".to_string(),
            SyncStatus::NoHead => "No HEAD".to_string(),
            SyncStatus::Unknown => "-".to_string(),
        }
    }
}

/// Result of status scan
#[derive(Debug, Default)]
pub struct StatusReport {
    pub repos: Vec<RepoStatus>,
    pub workspace_not_found: bool,
}

pub fn show_status(config: &Config) -> Result<StatusReport> {
    let workspace = config.resolve_path("workspace");

    if !workspace.exists() {
        return Ok(StatusReport {
            workspace_not_found: true,
            ..Default::default()
        });
    }

    // Find all .git directories
    let git_dirs: Vec<PathBuf> = WalkBuilder::new(&workspace)
        .max_depth(Some(3))
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false) && e.file_name() == ".git")
        .filter_map(|e| e.path().parent().map(|p| p.to_path_buf()))
        .collect();

    let repos: Vec<RepoStatus> = git_dirs
        .par_iter()
        .filter_map(|path| analyze_repo(path).ok())
        .collect();

    Ok(StatusReport {
        repos,
        workspace_not_found: false,
    })
}

fn analyze_repo(path: &Path) -> Result<RepoStatus> {
    let repo = Repository::open(path)?;
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Check dirty state
    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    let statuses = repo.statuses(Some(&mut opts))?;
    let is_dirty = !statuses.is_empty();

    // Check sync status
    let sync_status = get_sync_status(&repo);

    Ok(RepoStatus {
        name,
        path: path.to_path_buf(),
        is_dirty,
        sync_status,
    })
}

fn get_sync_status(repo: &Repository) -> SyncStatus {
    if repo.head().is_err() {
        return SyncStatus::NoHead;
    }

    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return SyncStatus::Unknown,
    };

    if !head.is_branch() {
        return SyncStatus::Detached;
    }

    let branch = git2::Branch::wrap(head);
    let upstream = match branch.upstream() {
        Ok(u) => u,
        Err(_) => return SyncStatus::Local,
    };

    let local_oid = match branch.get().target() {
        Some(oid) => oid,
        None => return SyncStatus::Unknown,
    };
    let remote_oid = match upstream.get().target() {
        Some(oid) => oid,
        None => return SyncStatus::Unknown,
    };

    let (ahead, behind) = match repo.graph_ahead_behind(local_oid, remote_oid) {
        Ok(ab) => ab,
        Err(_) => return SyncStatus::Unknown,
    };

    match (ahead, behind) {
        (0, 0) => SyncStatus::Synced,
        (a, 0) => SyncStatus::Ahead(a),
        (0, b) => SyncStatus::Behind(b),
        (a, b) => SyncStatus::Diverged {
            ahead: a,
            behind: b,
        },
    }
}
