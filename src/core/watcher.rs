use crate::config::Config;
use crate::core::cleaner;
use crate::utils::ui;
use anyhow::{Context, Result};
use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc::channel;

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

    // Create a watcher object, delivering debounced events.
    // The notification method varies by platform.
    let mut watcher = RecommendedWatcher::new(tx, NotifyConfig::default())
        .context("Failed to create file watcher")?;

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher.watch(&inbox_path, RecursiveMode::NonRecursive)?;

    loop {
        match rx.recv() {
            Ok(Ok(event)) => {
                // We only care about modifications or creations that result in a file being present
                // Notify events can be verbose.
                // Simple strategy: If ANY event happens in inbox, trigger a swift clean scan.
                // To avoid spaming clean on every byte write, we might want to checking event Kind
                // but for now, let's just trigger clean.

                // In V6 notify, Event has .kind
                // Let's filter slightly: ignore Access events?
                if let notify::EventKind::Access(_) = event.kind {
                    continue;
                }

                ui::print_dim("Change detected. Scanning...");
                // Run clean without dry_run
                if let Err(e) = cleaner::clean_inbox(config, false) {
                    ui::print_error(&format!("Auto-clean failed: {}", e));
                }
            }
            Ok(Err(e)) => ui::print_error(&format!("Watch error: {}", e)),
            Err(e) => {
                ui::print_error(&format!("Channel error: {}", e));
                break;
            }
        }
    }

    Ok(())
}
