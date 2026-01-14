use crate::config::Config;
use crate::utils::ui;
use anyhow::Result;
use infer;
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

pub struct AuditReport {
    pub empty_folders: Vec<PathBuf>,
    pub suspicious_extensions: Vec<(PathBuf, String, String)>, // Path, Expected, Actual
}

impl AuditReport {
    pub fn new() -> Self {
        Self {
            empty_folders: Vec::new(),
            suspicious_extensions: Vec::new(),
        }
    }
}

pub fn audit_workspace(config: &Config) -> Result<()> {
    // Audit whole workspace or specific strict areas?
    // Let's audit the root workspace logic as per the Python script:
    // "Scans your workspace for messiness (loose files, empty folders)"
    let workspace_root = config.resolve_path("workspace");

    if !workspace_root.exists() {
        ui::print_error(&format!("Workspace root not found: {:?}", workspace_root));
        return Ok(());
    }

    ui::print_info(&format!("Broadcasting audit on: {:?}", workspace_root));

    // 1. Collect all directories and files first (WalkDir is serial but fast enough for structure)
    // For massive workspaces, we might want to parallelize the check, but walking needs to be sequential or use specialized crate.
    // We will use WalkDir to collect paths, then Rayon to process them.
    let entries: Vec<_> = WalkDir::new(&workspace_root)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .collect();

    ui::print_info(&format!("Analyzing {} items...", entries.len()));

    let report = std::sync::Mutex::new(AuditReport::new());

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

        // Check for files (loose files in root areas or strict checks)
        // Here we just check for suspicious extensions for now as "loose files" definition is vague without area context
        if path.is_file() {
            // "Magic Byte" Check
            if let Ok(Some(kind)) = infer::get_from_path(path) {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let magic_ext = kind.extension();

                // Simple mismatch check (not perfect, e.g. jpeg vs jpg)
                // Only flag if completely different and not related (like txt vs unknown)
                // This is a "Creative" feature: detect masked files
                if !ext.is_empty() && ext != magic_ext && !is_compatible(ext, magic_ext) {
                    let mut r = report.lock().unwrap();
                    r.suspicious_extensions.push((
                        path.to_path_buf(),
                        ext.to_string(),
                        magic_ext.to_string(),
                    ));
                }
            }
        }
    });

    let report = report.into_inner().unwrap();

    // Print Report
    if !report.empty_folders.is_empty() {
        ui::print_warning("\nEmpty Folders Found:");
        for p in report.empty_folders.iter().take(10) {
            println!(" - {:?}", p);
        }
        if report.empty_folders.len() > 10 {
            println!("... and {} more", report.empty_folders.len() - 10);
        }
    }

    if !report.suspicious_extensions.is_empty() {
        ui::print_warning("\nSuspicious Extensions (Magic Byte Mismatch):");
        for (p, ext, magic) in report.suspicious_extensions {
            println!(" - {:?} (Named: .{}, Real: .{})", p, ext, magic);
        }
    }

    ui::print_success("\nAudit Complete.");

    Ok(())
}

fn is_compatible(ext1: &str, ext2: &str) -> bool {
    // Common aliases
    let pairs = [
        ("jpg", "jpeg"),
        ("jpeg", "jpg"),
        ("yml", "yaml"),
        ("yaml", "yml"),
        ("htm", "html"),
        ("html", "htm"),
        ("cc", "cpp"),
        ("cpp", "cc"),
        ("txt", "text"), // infer doesn't detect txt usually but good strictness
    ];
    pairs.contains(&(ext1, ext2))
}
