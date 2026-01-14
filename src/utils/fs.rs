use crate::utils::ui;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::config::Config;
use crate::core::undo;

pub fn move_item(config: &Config, src: &Path, dest_dir: &Path, dry_run: bool) -> Result<()> {
    if !dest_dir.exists() {
        if dry_run {
            ui::print_dim(&format!("Would create directory: {:?}", dest_dir));
        } else {
            fs::create_dir_all(dest_dir).context("Failed to create destination directory")?;
        }
    }

    let file_name = src.file_name().context("Invalid source path")?;
    let dest_path = dest_dir.join(file_name);

    if dry_run {
        ui::print_info(&format!("Would move {:?} -> {:?}", src, dest_path));
    } else {
        // Simple rename (mv)
        match fs::rename(src, &dest_path) {
            Ok(_) => {
                ui::print_success(&format!(
                    "Moved {:?} -> {:?}",
                    src.file_name().unwrap(),
                    dest_path
                ));
                // Log operation for undo
                if let Err(e) = undo::log_move(config, src, &dest_path) {
                    ui::print_warning(&format!("Failed to log undo op: {}", e));
                }
            }
            Err(e) => {
                // TODO: Handle cross-device link errors by copy+delete if needed
                ui::print_error(&format!("Failed to move {:?}: {}", src, e));
                return Err(e.into());
            }
        }
    }
    Ok(())
}
