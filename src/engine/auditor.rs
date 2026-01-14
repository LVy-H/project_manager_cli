use crate::config::Config;
use anyhow::Result;
use infer;
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use walkdir::WalkDir;

/// Information about a suspicious file extension
#[derive(Debug, Clone)]
pub struct SuspiciousExtension {
    pub path: PathBuf,
    pub declared_ext: String,
    pub actual_ext: String,
}

/// Report from auditing the workspace
#[derive(Debug, Default)]
pub struct AuditReport {
    pub empty_folders: Vec<PathBuf>,
    pub suspicious_extensions: Vec<SuspiciousExtension>,
    pub items_scanned: usize,
    pub workspace_not_found: bool,
}

impl AuditReport {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Audit the workspace for issues like empty folders and mismatched file extensions
pub fn audit_workspace(config: &Config) -> Result<AuditReport> {
    let workspace_root = config.resolve_path("workspace");

    if !workspace_root.exists() {
        return Ok(AuditReport {
            workspace_not_found: true,
            ..Default::default()
        });
    }

    // Collect all entries
    let entries: Vec<_> = WalkDir::new(&workspace_root)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .collect();

    let items_scanned = entries.len();
    let report = Mutex::new(AuditReport::new());

    entries.par_iter().for_each(|entry| {
        let path = entry.path();

        // Check for empty directories
        if path.is_dir() {
            if let Ok(mut read_dir) = fs::read_dir(path) {
                if read_dir.next().is_none() {
                    let mut r = report.lock().unwrap();
                    r.empty_folders.push(path.to_path_buf());
                }
            }
        }

        // Check for files with mismatched extensions
        if path.is_file() {
            if let Ok(Some(kind)) = infer::get_from_path(path) {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let magic_ext = kind.extension();

                if !ext.is_empty() && ext != magic_ext && !is_compatible(ext, magic_ext) {
                    let mut r = report.lock().unwrap();
                    r.suspicious_extensions.push(SuspiciousExtension {
                        path: path.to_path_buf(),
                        declared_ext: ext.to_string(),
                        actual_ext: magic_ext.to_string(),
                    });
                }
            }
        }
    });

    let mut report = report.into_inner().unwrap();
    report.items_scanned = items_scanned;

    Ok(report)
}

fn is_compatible(ext1: &str, ext2: &str) -> bool {
    let pairs = [
        ("jpg", "jpeg"),
        ("jpeg", "jpg"),
        ("yml", "yaml"),
        ("yaml", "yml"),
        ("htm", "html"),
        ("html", "htm"),
        ("cc", "cpp"),
        ("cpp", "cc"),
    ];
    pairs.contains(&(ext1, ext2))
}
