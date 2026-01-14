use crate::config::Config;
use crate::utils::ui;
use anyhow::Result;
use git2::{Repository, StatusOptions};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use tabled::settings::Style;
use tabled::{Table, Tabled};
use walkdir::WalkDir;

#[derive(Tabled)]
struct RepoStatus {
    #[tabled(rename = "Project")]
    name: String,
    #[tabled(rename = "State")]
    state: String,
    #[tabled(rename = "Sync")]
    sync: String,
    #[tabled(rename = "Path")]
    path: String,
}

pub fn show_status(config: &Config) -> Result<()> {
    let workspace = config.resolve_path("workspace");

    ui::print_info(&format!(
        "Scanning workspace for git repositories: {:?}",
        workspace
    ));

    // Find all .git directories
    // We use WalkDir but filter efficiently
    let git_dirs: Vec<PathBuf> = WalkDir::new(&workspace)
        .min_depth(1)
        .max_depth(3) // Optimization: Assume projects aren't deeper than 3 levels from workspace root usually
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir() && e.file_name() == ".git")
        .map(|e| e.path().parent().unwrap().to_path_buf())
        .collect();

    if git_dirs.is_empty() {
        ui::print_warning("No git repositories found.");
        return Ok(());
    }

    let statuses: Vec<RepoStatus> = git_dirs
        .par_iter()
        .filter_map(|path| analyze_repo(path).ok())
        .collect();

    if statuses.is_empty() {
        ui::print_warning("Could not analyze any found repositories.");
        return Ok(());
    }

    let mut table = Table::new(statuses);
    table.with(Style::rounded());
    println!("{}", table);

    Ok(())
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

    let state = if is_dirty {
        format!("{} Dirty", ui::colorize("⚠", "yellow"))
    } else {
        format!("{} Clean", ui::colorize("✓", "green"))
    };

    // Check sync status (simple HEAD vs origin/HEAD)
    let sync = match get_sync_status(&repo) {
        Ok(s) => s,
        Err(_) => "-".to_string(),
    };

    Ok(RepoStatus {
        name,
        state,
        sync,
        path: path.display().to_string(),
    })
}

fn get_sync_status(repo: &Repository) -> Result<String> {
    if repo.head().is_err() {
        return Ok("No HEAD".to_string());
    }
    let head = repo.head()?;

    // Check if branch is tracking a remote
    // if not, return Local
    if !head.is_branch() {
        return Ok("Detached".to_string());
    }

    let branch = git2::Branch::wrap(head);
    let upstream = match branch.upstream() {
        Ok(u) => u,
        Err(_) => return Ok("Local".to_string()),
    };

    let local_oid = branch.get().target().unwrap();
    let remote_oid = upstream.get().target().unwrap();

    let (ahead, behind) = repo.graph_ahead_behind(local_oid, remote_oid)?;

    if ahead == 0 && behind == 0 {
        Ok("Synced".to_string())
    } else if ahead > 0 && behind == 0 {
        Ok(format!("↑ {}", ahead))
    } else if behind > 0 && ahead == 0 {
        Ok(format!("↓ {}", behind))
    } else {
        Ok(format!("↑ {} ↓ {}", ahead, behind))
    }
}
