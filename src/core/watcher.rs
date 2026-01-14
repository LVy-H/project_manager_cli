use crate::config::Config;
use crate::engine::cleaner;
use crate::utils::ui;
use anyhow::{Context, Result};
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;

/// Minimum time a file must be stable (unchanged size) before processing
const FILE_STABILITY_SECONDS: u64 = 2;

/// Debounce timeout for file system events
const DEBOUNCE_SECONDS: u64 = 2;

pub fn watch_inbox(config: &Config) -> Result<()> {
    let inbox_path = config.resolve_path("inbox");

    if !inbox_path.exists() {
        ui::print_error(&format!("Inbox path not found: {:?}", inbox_path));
        return Ok(());
    }

    ui::print_info(&format!("Watching for changes in: {:?}", inbox_path));
    ui::print_info("Press Ctrl+C to stop.");

    // Create a channel to receive the events.
    let (tx, rx) = channel();

    // Create a debouncer with configured timeout
    let mut debouncer = new_debouncer(Duration::from_secs(DEBOUNCE_SECONDS), tx)
        .context("Failed to create file watcher")?;

    // Add a path to be watched
    debouncer
        .watcher()
        .watch(&inbox_path, RecursiveMode::NonRecursive)?;

    // Process events
    for res in rx {
        match res {
            Ok(events) => {
                if events.is_empty() {
                    continue;
                }

                ui::print_dim("Changes detected. Checking file stability...");

                // Wait for files to stabilize before cleaning
                if wait_for_stability(&inbox_path) {
                    ui::print_dim("Files stable. Scanning...");
                    match cleaner::clean_inbox(config, false) {
                        Ok(report) => {
                            if !report.moved.is_empty() {
                                ui::print_success(&format!(
                                    "Auto-cleaned {} items",
                                    report.moved.len()
                                ));
                            }
                            for err in &report.errors {
                                ui::print_error(err);
                            }
                        }
                        Err(e) => ui::print_error(&format!("Auto-clean failed: {}", e)),
                    }
                } else {
                    ui::print_warning("Files still changing, skipping this cycle.");
                }
            }
            Err(e) => ui::print_error(&format!("Watch error: {}", e)),
        }
    }

    Ok(())
}

/// Wait for all files in the inbox to have stable sizes.
/// Returns true if files are stable, false if they're still changing after max attempts.
fn wait_for_stability(inbox_path: &PathBuf) -> bool {
    let max_attempts = 5;
    let check_interval = Duration::from_secs(FILE_STABILITY_SECONDS);

    for attempt in 0..max_attempts {
        // Get current file sizes
        let sizes_before = get_file_sizes(inbox_path);

        if sizes_before.is_empty() {
            return true; // No files to check
        }

        // Wait
        std::thread::sleep(check_interval);

        // Get sizes again
        let sizes_after = get_file_sizes(inbox_path);

        // Check if all sizes are the same
        let mut all_stable = true;
        for (path, size_before) in &sizes_before {
            if let Some(&size_after) = sizes_after.get(path) {
                if size_before != &size_after {
                    all_stable = false;
                    break;
                }
            } else {
                // File was removed, that's fine
            }
        }

        // Also check for new files that appeared
        for path in sizes_after.keys() {
            if !sizes_before.contains_key(path) {
                all_stable = false;
                break;
            }
        }

        if all_stable {
            return true;
        }

        if attempt < max_attempts - 1 {
            ui::print_dim(&format!(
                "Files still changing, waiting... (attempt {}/{})",
                attempt + 1,
                max_attempts
            ));
        }
    }

    false
}

/// Get sizes of all files in a directory
fn get_file_sizes(dir: &PathBuf) -> HashMap<PathBuf, u64> {
    let mut sizes = HashMap::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(metadata) = fs::metadata(&path) {
                    sizes.insert(path, metadata.len());
                }
            }
        }
    }

    sizes
}
