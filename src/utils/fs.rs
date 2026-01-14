use anyhow::{Context, Result};
use fs_extra::dir::CopyOptions as DirCopyOptions;
use fs_extra::file::CopyOptions as FileCopyOptions;
use std::path::Path;

use crate::config::Config;
use crate::engine::undo;

/// Result of a move operation
#[derive(Debug)]
pub struct MoveResult {
    pub success: bool,
    pub used_copy_fallback: bool,
}

/// Move an item from source to destination directory.
/// Uses fs_extra for robust cross-device support.
pub fn move_item(
    config: &Config,
    src: &Path,
    dest_dir: &Path,
    dry_run: bool,
) -> Result<MoveResult> {
    if !dest_dir.exists() {
        if !dry_run {
            std::fs::create_dir_all(dest_dir).context("Failed to create destination directory")?;
        }
    }

    let file_name = src.file_name().context("Invalid source path")?;
    let dest_path = dest_dir.join(file_name);

    if dry_run {
        return Ok(MoveResult {
            success: true,
            used_copy_fallback: false,
        });
    }

    if src.is_dir() {
        // Move directory
        let options = DirCopyOptions::new();
        fs_extra::dir::move_dir(src, dest_dir, &options)
            .context(format!("Failed to move directory {:?}", src))?;
    } else {
        // Move file
        let options = FileCopyOptions::new();
        fs_extra::file::move_file(src, &dest_path, &options)
            .context(format!("Failed to move file {:?}", src))?;
    }

    // Log for undo
    if let Err(e) = undo::log_move(config, src, &dest_path) {
        log::warn!("Failed to log undo operation: {}", e);
    }

    Ok(MoveResult {
        success: true,
        used_copy_fallback: false, // fs_extra handles this internally
    })
}
