use anyhow::{Context, Result};
use std::fs;
use std::io::ErrorKind;
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
/// Handles cross-device moves by falling back to copy + delete.
pub fn move_item(
    config: &Config,
    src: &Path,
    dest_dir: &Path,
    dry_run: bool,
) -> Result<MoveResult> {
    if !dest_dir.exists() {
        if !dry_run {
            fs::create_dir_all(dest_dir).context("Failed to create destination directory")?;
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

    // Try rename first (fast, same-device)
    match fs::rename(src, &dest_path) {
        Ok(_) => {
            // Log operation for undo
            if let Err(e) = undo::log_move(config, src, &dest_path) {
                eprintln!("Warning: Failed to log undo op: {}", e);
            }
            Ok(MoveResult {
                success: true,
                used_copy_fallback: false,
            })
        }
        Err(e) if e.kind() == ErrorKind::CrossesDevices || e.raw_os_error() == Some(18) => {
            // EXDEV (18) = cross-device link error
            // Fall back to copy + delete
            copy_and_delete(config, src, &dest_path)?;
            Ok(MoveResult {
                success: true,
                used_copy_fallback: true,
            })
        }
        Err(e) => Err(e).context(format!("Failed to move {:?}", src)),
    }
}

/// Copy source to destination, then delete source.
/// Works across filesystem boundaries.
fn copy_and_delete(config: &Config, src: &Path, dest: &Path) -> Result<()> {
    if src.is_dir() {
        // Recursively copy directory
        copy_dir_recursive(src, dest)?;
        fs::remove_dir_all(src).context("Failed to remove source directory after copy")?;
    } else {
        // Copy file
        fs::copy(src, dest).context("Failed to copy file")?;
        fs::remove_file(src).context("Failed to remove source file after copy")?;
    }

    // Log for undo
    if let Err(e) = undo::log_move(config, src, dest) {
        eprintln!("Warning: Failed to log undo op: {}", e);
    }

    Ok(())
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    // Note: These tests require a mock Config, which we'll skip for now
    // as they would require more setup. The functions themselves are tested
    // via integration tests.

    #[test]
    fn test_copy_dir_recursive() {
        let src_dir = TempDir::new().unwrap();
        let dest_dir = TempDir::new().unwrap();

        // Create some files in src
        let file1 = src_dir.path().join("file1.txt");
        let mut f = File::create(&file1).unwrap();
        writeln!(f, "content1").unwrap();

        let subdir = src_dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        let file2 = subdir.join("file2.txt");
        let mut f = File::create(&file2).unwrap();
        writeln!(f, "content2").unwrap();

        // Copy
        let dest_path = dest_dir.path().join("copied");
        copy_dir_recursive(src_dir.path(), &dest_path).unwrap();

        // Verify
        assert!(dest_path.join("file1.txt").exists());
        assert!(dest_path.join("subdir").join("file2.txt").exists());
    }
}
