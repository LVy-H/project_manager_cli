use crate::config::Config;
use crate::utils::fs;
use anyhow::Result;
use regex::Regex;
use std::path::PathBuf;

/// Represents a single item that was moved during cleaning
#[derive(Debug, Clone)]
pub struct MovedItem {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub dry_run: bool,
}

/// Represents an item that was skipped (no matching rule)
#[derive(Debug, Clone)]
pub struct SkippedItem {
    pub path: PathBuf,
    pub reason: String,
}

/// Result of a clean operation
#[derive(Debug, Default)]
pub struct CleanReport {
    pub moved: Vec<MovedItem>,
    pub skipped: Vec<SkippedItem>,
    pub errors: Vec<String>,
    pub inbox_empty: bool,
    pub inbox_not_found: bool,
}

impl CleanReport {
    pub fn new() -> Self {
        Self::default()
    }
}

pub fn clean_inbox(config: &Config, dry_run: bool) -> Result<CleanReport> {
    let mut report = CleanReport::new();
    let inbox_path = config.resolve_path("inbox");

    if !inbox_path.exists() {
        report.inbox_not_found = true;
        return Ok(report);
    }

    let items: Vec<_> = std::fs::read_dir(&inbox_path)?
        .filter_map(|e| e.ok())
        .collect();

    if items.is_empty() {
        report.inbox_empty = true;
        return Ok(report);
    }

    // Pre-compile regexes
    let mut rules = Vec::new();
    for rule in &config.rules.clean {
        match Regex::new(&rule.pattern) {
            Ok(re) => rules.push((re, &rule.target)),
            Err(e) => {
                report
                    .errors
                    .push(format!("Invalid regex pattern '{}': {}", rule.pattern, e));
            }
        }
    }

    for entry in items {
        let path = entry.path();
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        let mut matched = false;
        for (re, target_key) in &rules {
            if re.is_match(file_name) {
                let dest = config.resolve_path(target_key);

                match fs::move_item(config, &path, &dest, dry_run) {
                    Ok(_) => {
                        report.moved.push(MovedItem {
                            source: path.clone(),
                            destination: dest.join(file_name),
                            dry_run,
                        });
                    }
                    Err(e) => {
                        report
                            .errors
                            .push(format!("Failed to move {:?}: {}", path, e));
                    }
                }
                matched = true;
                break;
            }
        }

        if !matched {
            report.skipped.push(SkippedItem {
                path: path.clone(),
                reason: "No matching rule".to_string(),
            });
        }
    }

    Ok(report)
}
