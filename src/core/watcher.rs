use crate::config::Config;
use crate::core::cleaner;
use crate::utils::ui;
use anyhow::{Context, Result};
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode};
use std::sync::mpsc::channel;
use std::time::Duration;

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

    // Create a debouncer with 2 seconds timeout
    let mut debouncer =
        new_debouncer(Duration::from_secs(2), tx).context("Failed to create file watcher")?;

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
                ui::print_dim("Debounced changes detected. Scanning...");
                if let Err(e) = cleaner::clean_inbox(config, false) {
                    ui::print_error(&format!("Auto-clean failed: {}", e));
                }
            }
            Err(e) => ui::print_error(&format!("Watch error: {}", e)),
        }
    }

    Ok(())
}
