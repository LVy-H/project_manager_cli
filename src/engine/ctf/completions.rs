//! Dynamic completion helpers for shell tab-completion.
//!
//! Completion is instruction-based: it reflects what the user has explicitly
//! configured (via `config.yaml` and `wardex ctf use …`). When we don't have
//! an unambiguous instruction, we return empty rather than guessing — a bad
//! TAB suggestion is worse than no suggestion.
//!
//! Contract: these functions run at TAB-time and must never panic. Any error
//! (missing config, malformed state, IO failure) silently degrades to empty.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use clap_complete::engine::CompletionCandidate;

use crate::config::Config;
use crate::utils::fs::expand_tilde;

/// Path completer for `PathBuf` args (`ctf import`, `--config`, `search`,
/// `info`).
///
/// Replaces clap_complete's built-in path completer so we can handle a
/// bare `~` properly. Upstream (clap_complete 4.6, `custom.rs:298`) only
/// recognises `~` when it's the *parent* of the typed word — so `~/foo<TAB>`
/// works, but `~<TAB>` returns empty and zsh then falls back to POSIX
/// user-home completion, listing every `/etc/passwd` account.
///
/// Behaviour:
/// * `~`           → expand to `$HOME/` (one candidate; user continues typing)
/// * `~/`, `~/foo` → list `$HOME` contents, **expanded to `$HOME/…`** in
///   the candidate string
/// * `/abs/foo`    → list matching entries under the absolute path
/// * `rel/foo`, `foo`, `""` → list matching entries under cwd
///
/// Why we expand `~/` rather than preserving it: clap_complete's generated
/// zsh wrapper feeds candidates through `_describe`, which calls `compadd`
/// without `-Q`. zsh treats `~` as a filename-special char and
/// backslash-quotes it on insertion — so `~/Do<TAB>` would land in the
/// buffer as `\~/Downloads/`, and the literal `~` then survives execve to
/// wardex, breaking `path.exists()`. Emitting the absolute path bypasses
/// zsh's quoting entirely.
///
/// Directories always included (so the user can descend) and get a
/// trailing `/`. Files are filtered by `keep_file` — pass `|_| true` for
/// AnyPath semantics, `|p| p.is_file()` for file-only args.
fn complete_path(
    current: &OsStr,
    keep_file: impl Fn(&std::path::Path) -> bool,
) -> Vec<CompletionCandidate> {
    let raw = current.to_string_lossy();

    // Bare `~` → expand to `$HOME/` so the shell can keep completing without
    // hitting zsh's `~user` fallback (which would list /etc/passwd accounts).
    if raw == "~" {
        if let Some(home) = dirs::home_dir() {
            return vec![CompletionCandidate::new(format!("{}/", home.display()))];
        }
        return Vec::new();
    }

    let (dir_part, file_part) = match raw.rfind('/') {
        Some(idx) => (&raw[..=idx], &raw[idx + 1..]),
        None => ("", raw.as_ref()),
    };

    // Resolve the directory we'll scan, and the prefix we'll emit on
    // candidates. They diverge when `dir_part` starts with `~/`: we scan
    // `$HOME/…` but we *also* emit `$HOME/…` in the candidate so the shell
    // doesn't see a literal `~` to quote.
    let (search_root, emit_prefix): (PathBuf, String) = if dir_part.is_empty() {
        (
            std::env::current_dir()
                .ok()
                .unwrap_or_else(|| PathBuf::from(".")),
            String::new(),
        )
    } else if let Some(rest) = dir_part.strip_prefix("~/") {
        let Some(home) = dirs::home_dir() else {
            return Vec::new();
        };
        let sub = rest.trim_end_matches('/');
        let root = home.join(sub);
        let prefix = if sub.is_empty() {
            format!("{}/", home.display())
        } else {
            format!("{}/{}/", home.display(), sub)
        };
        (root, prefix)
    } else if dir_part.starts_with('/') {
        (PathBuf::from(dir_part), dir_part.to_string())
    } else {
        let Ok(cwd) = std::env::current_dir() else {
            return Vec::new();
        };
        (cwd.join(dir_part), dir_part.to_string())
    };

    let Ok(entries) = std::fs::read_dir(&search_root) else {
        return Vec::new();
    };

    let mut results: Vec<CompletionCandidate> = entries
        .flatten()
        .filter_map(|entry| {
            let name_os = entry.file_name();
            let name = name_os.to_string_lossy();
            if !name.starts_with(file_part) {
                return None;
            }
            let path = entry.path();
            if path.is_dir() {
                Some(CompletionCandidate::new(format!(
                    "{}{}/",
                    emit_prefix, name
                )))
            } else if keep_file(&path) {
                Some(CompletionCandidate::new(format!("{}{}", emit_prefix, name)))
            } else {
                None
            }
        })
        .collect();
    results.sort_by(|a, b| a.get_value().cmp(b.get_value()));
    results
}

/// Complete any path (files or directories). Used for `search` and `info`
/// where either is acceptable.
pub fn any_path_completer(current: &OsStr) -> Vec<CompletionCandidate> {
    complete_path(current, |_| true)
}

/// Complete file paths (directories shown for descent, non-files filtered).
/// Used for `ctf import` and `--config` where the final value must be a file.
pub fn file_path_completer(current: &OsStr) -> Vec<CompletionCandidate> {
    complete_path(current, |p| p.is_file())
}

/// Resolve the CTF root directory from explicit instruction.
///
/// Precedence:
/// 1. `WX_PATHS_CTF_ROOT` env var (explicit override, tilde-expanded).
/// 2. The user's merged `Config` (honors `paths.ctf_root` and
///    `paths.workspace` exactly as the main binary does).
///
/// Returns `None` if no config is loadable or the resolved path does not
/// exist — no hard-coded fallback to `~/workspace/1_Projects/CTFs`.
fn resolve_ctf_root() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("WX_PATHS_CTF_ROOT") {
        let path = expand_tilde(Path::new(&dir));
        if path.exists() {
            return Some(path);
        }
    }

    let root = Config::load().ok()?.ctf_root();
    root.exists().then_some(root)
}

/// Resolve the active event root from global state.
///
/// Reads `~/.local/share/wardex/state.json` and returns the
/// `current_event_path` if it exists on disk.
fn resolve_active_event() -> Option<PathBuf> {
    let state_path = if let Ok(p) = std::env::var("WARDEX_STATE_FILE") {
        PathBuf::from(p)
    } else {
        dirs::data_dir()?.join("wardex").join("state.json")
    };

    let content = std::fs::read_to_string(state_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    let path_str = value.get("current_event_path")?.as_str()?;
    let path = PathBuf::from(path_str);
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// Complete event names for commands like `ctf use <event>`.
///
/// Lists directories inside the CTF root that match the current prefix.
pub fn event_completer(current: &OsStr) -> Vec<CompletionCandidate> {
    let prefix = current.to_string_lossy();

    let Some(ctf_root) = resolve_ctf_root() else {
        return Vec::new();
    };

    let Ok(entries) = std::fs::read_dir(&ctf_root) else {
        return Vec::new();
    };

    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            // Skip hidden directories
            if name.starts_with('.') {
                return None;
            }
            if prefix.is_empty() || name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                Some(CompletionCandidate::new(name))
            } else {
                None
            }
        })
        .collect()
}

/// Complete challenge paths as `category/challenge` within the active event.
/// Used for `ctf path <event> <challenge>` and similar commands.
///
/// Requires an explicit active event (set via `wardex ctf use <event>`).
/// If none is set, returns empty — we never guess a "latest" event because
/// that would silently complete against the wrong one.
pub fn challenge_completer(current: &OsStr) -> Vec<CompletionCandidate> {
    let prefix = current.to_string_lossy();

    let Some(event_root) = resolve_active_event() else {
        return Vec::new();
    };

    let mut challenges = Vec::new();

    let Ok(cats) = std::fs::read_dir(&event_root) else {
        return Vec::new();
    };

    for cat in cats.flatten() {
        if !cat.path().is_dir() {
            continue;
        }
        let cat_name = cat.file_name().to_string_lossy().to_string();
        if cat_name.starts_with('.') {
            continue;
        }

        let Ok(chals) = std::fs::read_dir(cat.path()) else {
            continue;
        };

        for chal in chals.flatten() {
            if !chal.path().is_dir() {
                continue;
            }
            let chal_name = chal.file_name().to_string_lossy().to_string();
            let full_path = format!("{}/{}", cat_name, chal_name);

            if prefix.is_empty() || full_path.to_lowercase().starts_with(&prefix.to_lowercase()) {
                challenges.push(CompletionCandidate::new(full_path));
            }
        }
    }

    challenges
}

/// Complete category names for commands like `ctf add <cat/name>`.
///
/// Priority:
/// 1. Directories inside the active event (reflects what already exists).
/// 2. `config.ctf.default_categories` from the user's config.
///
/// If neither source yields a candidate we return empty — no hard-coded
/// category list. Users get completion from explicit configuration, not
/// from assumptions baked into the binary.
pub fn category_completer(current: &OsStr) -> Vec<CompletionCandidate> {
    let prefix = current.to_string_lossy();

    // If the user has already typed "cat/", don't complete categories
    if prefix.contains('/') {
        return Vec::new();
    }

    let lowered_prefix = prefix.to_lowercase();

    // 1. Try active event root from global state
    if let Some(root) = resolve_active_event() {
        if let Ok(entries) = std::fs::read_dir(&root) {
            let results: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if !name.starts_with('.')
                        && (prefix.is_empty() || name.to_lowercase().starts_with(&lowered_prefix))
                    {
                        Some(CompletionCandidate::new(format!("{}/", name)))
                    } else {
                        None
                    }
                })
                .collect();

            if !results.is_empty() {
                return results;
            }
        }
    }

    // 2. Fall back to user-configured default categories. No hard-coded list.
    let Some(config) = Config::load().ok() else {
        return Vec::new();
    };
    config
        .ctf
        .default_categories
        .iter()
        .filter(|c| prefix.is_empty() || c.to_lowercase().starts_with(&lowered_prefix))
        .map(|c| CompletionCandidate::new(format!("{}/", c)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Redirect XDG_CONFIG_HOME and the WARDEX state file to a temp dir so
    /// each test starts from a known, empty configuration slate.
    ///
    /// Also clears the `WX_*` env vars that Config::load reads via the
    /// `config` crate's environment source — otherwise bleed-over from a
    /// prior test can spoof arbitrary config values.
    fn isolate_env() -> TempDir {
        let td = TempDir::new().expect("tempdir");
        std::env::set_var("XDG_CONFIG_HOME", td.path());
        std::env::set_var("WARDEX_STATE_FILE", td.path().join("state.json"));
        std::env::remove_var("WX_PATHS_WORKSPACE");
        std::env::remove_var("WX_PATHS_CTF_ROOT");
        std::env::remove_var("WX_CTF_DEFAULT_CATEGORIES");
        td
    }

    /// Write a minimal config.yaml into the isolated XDG config dir.
    fn write_config(td: &TempDir, body: &str) {
        let dir = td.path().join("wardex");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.yaml"), body).unwrap();
    }

    #[test]
    #[serial_test::serial]
    fn event_completer_returns_empty_when_no_config() {
        let _td = isolate_env();
        let results = event_completer(OsStr::new(""));
        assert!(
            results.is_empty(),
            "no config means no guess — empty, not a hard-coded root"
        );
    }

    #[test]
    #[serial_test::serial]
    fn category_completer_returns_empty_when_no_config_no_event() {
        let _td = isolate_env();
        let results = category_completer(OsStr::new(""));
        assert!(
            results.is_empty(),
            "without an active event AND without a config, we do not fabricate categories"
        );
    }

    #[test]
    #[serial_test::serial]
    fn category_completer_reads_config_default_categories() {
        let td = isolate_env();
        write_config(
            &td,
            "paths:\n  workspace: /tmp\nctf:\n  default_categories: [foo, bar, baz]\n",
        );
        let results = category_completer(OsStr::new(""));
        let names: Vec<String> = results
            .into_iter()
            .map(|c| c.get_value().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["foo/", "bar/", "baz/"]);
    }

    #[test]
    #[serial_test::serial]
    fn category_completer_filters_config_categories_case_insensitively() {
        let td = isolate_env();
        write_config(
            &td,
            "paths:\n  workspace: /tmp\nctf:\n  default_categories: [Pwn, Web, Crypto]\n",
        );
        let results = category_completer(OsStr::new("pw"));
        let names: Vec<String> = results
            .into_iter()
            .map(|c| c.get_value().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["Pwn/"]);
    }

    #[test]
    fn category_completer_skips_after_slash() {
        let results = category_completer(OsStr::new("pwn/"));
        assert!(results.is_empty(), "should not complete after slash");
    }

    #[test]
    #[serial_test::serial]
    fn challenge_completer_returns_empty_when_no_active_event() {
        let _td = isolate_env();
        let results = challenge_completer(OsStr::new(""));
        assert!(
            results.is_empty(),
            "no active event means no guess — user must `wardex ctf use <event>` first"
        );
    }

    #[test]
    #[serial_test::serial]
    fn path_completer_bare_tilde_expands_to_home_slash() {
        // Regression for two bugs at once:
        //  1. Without a custom handler, zsh's `_files` fallback would list
        //     /etc/passwd user accounts for `~<TAB>`.
        //  2. Returning `~/` literally caused zsh's `_describe` /
        //     compadd to backslash-quote the tilde on insertion, breaking
        //     the path at runtime. Emit the expanded `$HOME/` instead.
        let td = tempfile::tempdir().unwrap();
        let prev_home = std::env::var_os("HOME");
        std::env::set_var("HOME", td.path());

        let results = any_path_completer(OsStr::new("~"));
        let names: Vec<String> = results
            .into_iter()
            .map(|c| c.get_value().to_string_lossy().into_owned())
            .collect();

        match prev_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }

        let expected = format!("{}/", td.path().display());
        assert_eq!(names, vec![expected]);
    }

    #[test]
    #[serial_test::serial]
    fn path_completer_tilde_slash_emits_expanded_home() {
        // `~/<TAB>` should produce candidates with the tilde **expanded**.
        // The previous behavior preserved `~/` literally, which caused
        // zsh's `_describe` to backslash-quote the `~` on insertion (so
        // the user saw `\~/Downloads/` in the buffer and the literal `~`
        // survived to wardex, breaking `path.exists()`).
        let td = tempfile::tempdir().unwrap();
        std::fs::create_dir(td.path().join("Downloads")).unwrap();
        std::fs::write(td.path().join("notes.txt"), b"").unwrap();
        let prev_home = std::env::var_os("HOME");
        std::env::set_var("HOME", td.path());

        let results = any_path_completer(OsStr::new("~/"));
        let names: Vec<String> = results
            .into_iter()
            .map(|c| c.get_value().to_string_lossy().into_owned())
            .collect();

        match prev_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }

        let downloads = format!("{}/Downloads/", td.path().display());
        let notes = format!("{}/notes.txt", td.path().display());

        assert!(
            names.iter().any(|n| n == &downloads),
            "expected expanded {:?} in {:?}",
            downloads,
            names
        );
        assert!(
            names.iter().any(|n| n == &notes),
            "expected expanded {:?} in {:?}",
            notes,
            names
        );
        for name in &names {
            assert!(
                !name.starts_with('~'),
                "candidates must not begin with `~` (zsh would quote it); got {:?}",
                name
            );
        }
    }

    #[test]
    #[serial_test::serial]
    fn path_completer_tilde_subpath_emits_expanded_home() {
        // `~/Do<TAB>` should expand the tilde *and* keep the partial
        // filename for prefix matching, then emit the absolute path.
        let td = tempfile::tempdir().unwrap();
        std::fs::create_dir(td.path().join("Downloads")).unwrap();
        std::fs::create_dir(td.path().join("Documents")).unwrap();
        std::fs::create_dir(td.path().join("Music")).unwrap();
        let prev_home = std::env::var_os("HOME");
        std::env::set_var("HOME", td.path());

        let results = any_path_completer(OsStr::new("~/Do"));
        let names: Vec<String> = results
            .into_iter()
            .map(|c| c.get_value().to_string_lossy().into_owned())
            .collect();

        match prev_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }

        let downloads = format!("{}/Downloads/", td.path().display());
        let documents = format!("{}/Documents/", td.path().display());
        let music = format!("{}/Music/", td.path().display());

        assert!(names.iter().any(|n| n == &downloads));
        assert!(names.iter().any(|n| n == &documents));
        assert!(
            !names.iter().any(|n| n == &music),
            "Music should not match `Do`"
        );
        for name in &names {
            assert!(
                !name.starts_with('~'),
                "candidates must not begin with `~`; got {:?}",
                name
            );
        }
    }

    #[test]
    fn path_completer_file_filter_hides_non_files() {
        // file_path_completer must still include directories (so the user
        // can descend) but filter out non-regular-file leafs.
        let td = tempfile::tempdir().unwrap();
        std::fs::create_dir(td.path().join("sub")).unwrap();
        std::fs::write(td.path().join("plain.txt"), b"").unwrap();

        let prefix = format!("{}/", td.path().display());
        let results = file_path_completer(OsStr::new(&prefix));
        let names: Vec<String> = results
            .into_iter()
            .map(|c| c.get_value().to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().any(|n| n.ends_with("/sub/")));
        assert!(names.iter().any(|n| n.ends_with("/plain.txt")));
    }
}
