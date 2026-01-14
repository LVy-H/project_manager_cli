use crate::config::Config;
use crate::utils::{fs, ui};
use anyhow::Result;
use regex::Regex;

pub fn clean_inbox(config: &Config, dry_run: bool) -> Result<()> {
    let inbox_path = config.resolve_path("inbox");

    if !inbox_path.exists() {
        ui::print_error(&format!("Inbox path not found: {:?}", inbox_path));
        return Ok(());
    }

    let items: Vec<_> = std::fs::read_dir(&inbox_path)?
        .filter_map(|e| e.ok())
        .collect();

    if items.is_empty() {
        ui::print_warning("Inbox is empty.");
        return Ok(());
    }

    ui::print_info(&format!(
        "Scanning {} items in {:?}...",
        items.len(),
        inbox_path
    ));

    // Pre-compile regexes
    let mut rules = Vec::new();
    for rule in &config.rules.clean {
        match Regex::new(&rule.pattern) {
            Ok(re) => rules.push((re, &rule.target)),
            Err(e) => ui::print_error(&format!("Invalid regex pattern '{}': {}", rule.pattern, e)),
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
                // Resolution logic: target_key might be "projects/CTFs"
                // Split by first slash to find root key in config.paths
                // let parts: Vec<&str> = target_key.splitn(2, '/').collect();
                // let dest_root_key = parts[0];

                // Use config.resolve_path logic or custom logic here
                // We'll reuse resolve_path logic partially via config.resolve_path which handles keys
                // effectively we treat the target_key as a key for resolve_path if it matches a path key,
                // or we rely on resolve_path's fallback.
                // Actually 'target: projects/CTFs' -> resolve_path("projects/CTFs")

                let dest = config.resolve_path(target_key);

                if let Err(e) = fs::move_item(config, &path, &dest, dry_run) {
                    ui::print_error(&format!("Failed to move {:?}: {}", path, e));
                }
                matched = true;
                break;
            }
        }

        if !matched {
            ui::print_dim(&format!("Skipped: {} (No matching rule)", file_name));
        }
    }

    Ok(())
}
