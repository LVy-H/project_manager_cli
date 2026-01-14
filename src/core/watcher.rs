use crate::config::Config;
use crate::engine::cleaner;
use anyhow::{Context, Result};
use log::{debug, error, info, warn};
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
        error!("Inbox path not found: {:?}", inbox_path);
        return Ok(());
    }

    info!("Watching for changes in: {:?}", inbox_path);
    info!("Press Ctrl+C to stop.");

    let (tx, rx) = channel();

    let mut debouncer = new_debouncer(Duration::from_secs(DEBOUNCE_SECONDS), tx)
        .context("Failed to create file watcher")?;

    debouncer
        .watcher()
        .watch(&inbox_path, RecursiveMode::NonRecursive)?;

    for res in rx {
        match res {
            Ok(events) => {
                if events.is_empty() {
                    continue;
                }

                debug!("Changes detected. Checking file stability...");

                if wait_for_stability(&inbox_path) {
                    debug!("Files stable. Scanning...");
                    match cleaner::clean_inbox(config, false) {
                        Ok(report) => {
                            if !report.moved.is_empty() {
                                info!("✓ Auto-cleaned {} items", report.moved.len());
                            }
                            for err in &report.errors {
                                error!("{}", err);
                            }
                        }
                        Err(e) => error!("Auto-clean failed: {}", e),
                    }
                } else {
                    warn!("Files still changing, skipping this cycle.");
                }
            }
            Err(e) => error!("Watch error: {}", e),
        }
    }

    Ok(())
}

/// Wait for all files in the inbox to have stable sizes.
fn wait_for_stability(inbox_path: &PathBuf) -> bool {
    let max_attempts = 5;
    let check_interval = Duration::from_secs(FILE_STABILITY_SECONDS);

    for attempt in 0..max_attempts {
        let sizes_before = get_file_sizes(inbox_path);

        if sizes_before.is_empty() {
            return true;
        }

        std::thread::sleep(check_interval);

        let sizes_after = get_file_sizes(inbox_path);

        let mut all_stable = true;
        for (path, size_before) in &sizes_before {
            if let Some(&size_after) = sizes_after.get(path) {
                if size_before != &size_after {
                    all_stable = false;
                    break;
                }
            }
        }

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
            debug!(
                "Files still changing, waiting... (attempt {}/{})",
                attempt + 1,
                max_attempts
            );
        }
    }

    false
}

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
