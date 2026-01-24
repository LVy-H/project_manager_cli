use crate::config::Config;
use anyhow::Result;
use ignore::WalkBuilder;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Default, Clone)]
pub struct WorkspaceStats {
    pub total_projects: usize,
    pub total_repos: usize,
    pub total_files: usize,
    pub total_size_bytes: u64,
    pub file_types: HashMap<String, usize>,
    pub ctf_count: usize,
    pub ctf_solved: usize,
}

pub fn get_stats(config: &Config) -> Result<WorkspaceStats> {
    let workspace = config.resolve_path("workspace");

    // Quick scan using parallel iterator where possible
    // Note: To be fully accurate and fast, we might want to just scan the projects dir
    // but the user asked for "Workspace analytics".

    let mut stats = WorkspaceStats::default();

    if !workspace.exists() {
        return Ok(stats);
    }

    // Count projects (top-level folders in 1_Projects)
    let projects_dir = config.resolve_path("projects");
    if projects_dir.exists() {
        if let Ok(entries) = fs_err::read_dir(&projects_dir) {
            stats.total_projects = entries
                .filter(|e| e.as_ref().map(|x| x.path().is_dir()).unwrap_or(false))
                .count();
        }
    }

    // Count CTFs
    let ctf_root = config.ctf_root();
    if ctf_root.exists() {
        // Using existing ctf module logic would be better but for speed let's just count
        // We might need to handle this smarter to avoid circular deps if we use ctf module
        // But ctf module is engine sibling so it's fine.
        // Actually, let's keep it simple and just count dirs in ctf_root
        if let Ok(entries) = fs_err::read_dir(&ctf_root) {
            stats.ctf_count = entries
                .filter(|e| e.as_ref().map(|x| x.path().is_dir()).unwrap_or(false))
                .count();
        }
    }

    // Deep scan for files and repos
    // This can be slow, so we should limit depth or confirm user intent?
    // "wardex stats" usually implies a comprehensive scan.
    // We can use WalkDir but parallelize accumulation?

    let walker = WalkBuilder::new(&workspace)
        .max_depth(Some(10))
        .follow_links(false)
        .build();

    let stats_mutex = Arc::new(Mutex::new((0u64, 0usize, HashMap::new())));
    let repos_count = Arc::new(Mutex::new(0usize));

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();

        if path.is_file() {
            if let Ok(metadata) = entry.metadata() {
                let size = metadata.len();
                let ext = path.extension().and_then(|s| s.to_str()).map(String::from);

                let mut guard = stats_mutex.lock().unwrap();
                guard.0 += size;
                guard.1 += 1;
                if let Some(ext) = ext {
                    *guard.2.entry(ext).or_insert(0) += 1;
                }
            }
        } else if path.is_dir() {
            if path.join(".git").exists() {
                let mut repos = repos_count.lock().unwrap();
                *repos += 1;
            }
        }
    }

    let (total_size, total_files, file_types) =
        Arc::try_unwrap(stats_mutex).unwrap().into_inner().unwrap();
    let total_repos = Arc::try_unwrap(repos_count).unwrap().into_inner().unwrap();

    stats.total_size_bytes = total_size;
    stats.total_files = total_files;
    stats.file_types = file_types;
    stats.total_repos = total_repos;

    Ok(stats)
}

pub fn print_stats(stats: &WorkspaceStats) {
    println!("📊 Workspace Analytics");
    println!("{}", "-".repeat(40));
    println!("Projects:    {}", stats.total_projects);
    println!("Git Repos:   {}", stats.total_repos);
    println!("CTF Events:  {}", stats.ctf_count);
    println!("Total Files: {}", stats.total_files);
    println!(
        "Total Size:  {:.2} MB",
        stats.total_size_bytes as f64 / 1024.0 / 1024.0
    );

    println!("\n📁 Top File Types");
    let mut sorted_types: Vec<_> = stats.file_types.iter().collect();
    sorted_types.sort_by(|a, b| b.1.cmp(a.1));

    for (ext, count) in sorted_types.iter().take(5) {
        println!("  .{:<4} : {}", ext, count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_workspace_stats_default() {
        let stats = WorkspaceStats::default();
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_size_bytes, 0);
        assert_eq!(stats.total_repos, 0);
        assert!(stats.file_types.is_empty());
    }
}
