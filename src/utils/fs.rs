use anyhow::{Context, Result};
use fs_extra::dir::CopyOptions as DirCopyOptions;
use fs_extra::file::CopyOptions as FileCopyOptions;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::engine::undo;

/// Expand a leading `~` or `~/…` to the user's home directory.
/// Any other value is returned unchanged.
///
/// Shells normally expand `~` before the binary sees it, but completion
/// systems (notably zsh's `_describe`) can backslash-quote the `~` so the
/// literal character survives to runtime. Calling this on `PathBuf` args
/// at the entry point makes wardex robust to that case.
pub fn expand_tilde(path: &Path) -> PathBuf {
    let Some(s) = path.to_str() else {
        return path.to_path_buf();
    };
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    } else if s == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    path.to_path_buf()
}

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
    if !dest_dir.exists() && !dry_run {
        fs_err::create_dir_all(dest_dir).context("Failed to create destination directory")?;
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
        let mut options = DirCopyOptions::new();
        options.copy_inside = true;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_tilde_expands_home_prefix() {
        let home = dirs::home_dir().expect("home dir available for this test");
        assert_eq!(
            expand_tilde(Path::new("~/foo/bar")),
            home.join("foo").join("bar")
        );
        assert_eq!(expand_tilde(Path::new("~")), home);
        assert_eq!(
            expand_tilde(Path::new("/abs/path")),
            PathBuf::from("/abs/path")
        );
        assert_eq!(
            expand_tilde(Path::new("relative")),
            PathBuf::from("relative")
        );
    }

    #[test]
    fn expand_tilde_does_not_touch_embedded_tilde() {
        // Only a *leading* `~/` or bare `~` is expanded; `~` later in the
        // path is treated as a literal character (matches shell behavior).
        assert_eq!(
            expand_tilde(Path::new("/tmp/~foo")),
            PathBuf::from("/tmp/~foo")
        );
        assert_eq!(
            expand_tilde(Path::new("foo/~bar")),
            PathBuf::from("foo/~bar")
        );
    }
}
