//! Smart challenge import — moves archives into the event, auto-detects category,
//! extracts contents, and creates solve script templates.
//!
//! Category detection uses a confidence system:
//! - **High**: Archive contents match known indicators (e.g. `libc.so` → pwn, `Dockerfile` → web)
//! - **Low**: Filename keywords or file extension heuristics
//! - **None**: Falls back to "misc"

use crate::config::Config;
use crate::utils::fs::{expand_tilde, move_item};
use anyhow::Result;
use fs_err as fs;
use std::path::Path;

use super::{add_solve_script, ChallengeMetadata};

/// Confidence level for category detection
#[derive(Debug, PartialEq)]
enum Confidence {
    /// 100% sure from archive contents (e.g. libc.so → pwn, Dockerfile → web)
    High,
    /// Heuristic guess from filename or file extension
    Low,
    /// No match at all
    None,
}

struct CategoryGuess {
    category: String,
    confidence: Confidence,
}

pub fn import_challenge(
    config: &Config,
    path: &Path,
    category_override: Option<String>,
    name_override: Option<String>,
    auto_mode: bool,
) -> Result<()> {
    use dialoguer::{theme::ColorfulTheme, Input};

    let event_root = super::get_active_event_root()?;

    // Defensive: shells normally expand `~`, but zsh's completion path can
    // backslash-quote it so the literal `~` survives execve. Expanding here
    // makes the runtime forgiving regardless of how the arg got to us.
    let path = expand_tilde(path);
    let path = path.as_path();

    if !path.exists() {
        anyhow::bail!(
            "File not found: {:?}\n\nPlease verify the file path is correct.",
            path
        );
    }

    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("challenge")
        .to_string();

    println!("Importing: {}", file_name);

    // ── Category selection ──────────────────────────────────────────────
    let guess = guess_category(path, &file_name);
    let category = select_category(config, category_override, &guess, auto_mode)?;

    // Create category dir if needed
    let category_dir = event_root.join(&category);
    if !category_dir.exists() {
        fs::create_dir(&category_dir)?;
    }

    // ── Challenge name ─────────────────────────────────────────────────
    // Strip compound extensions like .tar.gz, .tar.bz2, .tar.xz
    let default_name = {
        let stem = Path::new(&file_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown_chall")
            .to_string();
        // If the stem still ends with .tar (from .tar.gz etc.), strip it
        stem.strip_suffix(".tar").unwrap_or(&stem).to_string()
    };

    let challenge_name = if let Some(name) = name_override {
        // Explicit --name flag
        name
    } else if auto_mode {
        // Auto mode: use filename stem directly
        default_name
    } else {
        // Interactive: prompt with editable default
        Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Challenge name")
            .default(default_name)
            .interact_text()?
    };

    let challenge_dir = category_dir.join(&challenge_name);
    if challenge_dir.exists() {
        anyhow::bail!("Challenge directory already exists: {:?}", challenge_dir);
    }

    fs::create_dir(&challenge_dir)?;

    // ── Move file ──────────────────────────────────────────────────────
    match move_item(config, path, &challenge_dir, false) {
        Ok(res) => {
            if res.used_copy_fallback {
                println!("✓ Copied file to {}/{}/", category, challenge_name);
            } else {
                println!("✓ Moved file to {}/{}/", category, challenge_name);
            }
        }
        Err(e) => {
            println!("Error moving file: {}", e);
        }
    }

    // ── Auto-extract archives ──────────────────────────────────────────
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if matches!(ext.as_str(), "zip" | "tar" | "gz" | "tgz") {
        let archive_path = challenge_dir.join(&file_name);
        if archive_path.exists() {
            let should_extract = if auto_mode {
                true
            } else {
                dialoguer::Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt("Extract archive contents?")
                    .default(true)
                    .interact()
                    .unwrap_or(true)
            };

            if should_extract {
                match extract_archive(&archive_path, &challenge_dir) {
                    Ok(count) => println!("✓ Extracted {} items", count),
                    Err(e) => println!("! Extract failed: {}", e),
                }
            }
        }
    }

    // ── Solve script template ──────────────────────────────────────────
    add_solve_script(&challenge_dir, &category)?;

    // ── Challenge metadata ────────────────────────────────────────────
    let mut meta = ChallengeMetadata::new(&challenge_name, &category);
    meta.imported_from = Some(file_name.clone());
    meta.save(&challenge_dir)?;

    println!("✓ Created {}/{}", category, challenge_name);
    Ok(())
}

/// Unified category guessing with confidence levels
fn guess_category(path: &Path, file_name: &str) -> CategoryGuess {
    let lower = file_name.to_lowercase();

    // 1. Try archive content scanning (high confidence)
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if let Some(cat) = match ext.as_str() {
        "zip" => scan_zip_for_category(path),
        "tar" | "gz" | "tgz" => scan_tar_for_category(path),
        _ => None,
    } {
        return CategoryGuess {
            category: cat.to_string(),
            confidence: Confidence::High,
        };
    }

    // 2. Filename keyword matching (low confidence — filenames can be misleading)
    let keyword_match = if lower.contains("web") {
        Some("web")
    } else if lower.contains("pwn") || lower.contains("bof") {
        Some("pwn")
    } else if lower.contains("crypto") {
        Some("crypto")
    } else if lower.contains("rev") && !lower.contains("review") {
        Some("rev")
    } else if lower.contains("misc") {
        Some("misc")
    } else if lower.contains("forensic") {
        Some("forensics")
    } else {
        None
    };

    if let Some(cat) = keyword_match {
        return CategoryGuess {
            category: cat.to_string(),
            confidence: Confidence::Low,
        };
    }

    // 3. File extension heuristics (low confidence)
    let ext_match = match ext.as_str() {
        "py" | "js" | "html" | "php" => Some("web"),
        "c" | "cpp" | "elf" => Some("pwn"),
        "enc" | "key" | "pem" => Some("crypto"),
        "exe" | "dll" | "asm" => Some("rev"),
        "pcap" | "pcapng" | "mem" => Some("forensics"),
        "jpg" | "png" | "gif" => Some("misc"),
        _ => None,
    };

    if let Some(cat) = ext_match {
        return CategoryGuess {
            category: cat.to_string(),
            confidence: Confidence::Low,
        };
    }

    CategoryGuess {
        category: "misc".to_string(),
        confidence: Confidence::None,
    }
}

/// Select category: override > auto > interactive prompt
fn select_category(
    config: &Config,
    category_override: Option<String>,
    guess: &CategoryGuess,
    auto_mode: bool,
) -> Result<String> {
    use dialoguer::{theme::ColorfulTheme, Select};

    // Explicit --category flag
    if let Some(cat) = category_override {
        return Ok(cat);
    }

    // Auto mode: use best guess
    if auto_mode {
        println!(
            "  Category (auto): {} (confidence: {:?})",
            guess.category, guess.confidence
        );
        return Ok(guess.category.clone());
    }

    // Interactive: always show selector
    let mut categories = config.ctf.default_categories.clone();
    if !categories.contains(&guess.category) {
        categories.push(guess.category.clone());
    }
    if !categories.iter().any(|c| c == "misc") {
        categories.push("misc".to_string());
    }

    // Pre-select the guess only if high confidence
    let default_idx = if guess.confidence == Confidence::High {
        categories
            .iter()
            .position(|c| c == &guess.category)
            .unwrap_or(0)
    } else {
        0 // No bias for low/no confidence
    };

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select category")
        .default(default_idx)
        .items(&categories)
        .interact()
        .unwrap_or(default_idx);

    Ok(categories[selection].clone())
}

/// Extract a zip archive into a target directory
fn extract_zip(archive_path: &Path, target_dir: &Path) -> Result<usize> {
    use zip::ZipArchive;

    let file = fs::File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut count = 0;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();

        let out_path = target_dir.join(&name);

        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out_file = fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out_file)?;
            count += 1;
        }
    }

    Ok(count)
}

/// Extract a tar(.gz) archive into a target directory
fn extract_tar(archive_path: &Path, target_dir: &Path) -> Result<usize> {
    use flate2::read::GzDecoder;
    use std::io::{BufReader, Read};
    use tar::Archive;

    let file = fs::File::open(archive_path)?;

    let ext = archive_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let reader: Box<dyn Read> = if ext == "gz" || archive_path.to_string_lossy().ends_with(".tgz") {
        Box::new(GzDecoder::new(BufReader::new(file)))
    } else {
        Box::new(BufReader::new(file))
    };

    let mut archive = Archive::new(reader);
    archive.unpack(target_dir)?;

    // Count extracted files
    let count = walkdir_count(target_dir);
    Ok(count)
}

fn extract_archive(archive_path: &Path, target_dir: &Path) -> Result<usize> {
    let ext = archive_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "zip" => extract_zip(archive_path, target_dir),
        "tar" => extract_tar(archive_path, target_dir),
        "gz" | "tgz" => extract_tar(archive_path, target_dir),
        _ => anyhow::bail!("Unsupported archive format: {}", ext),
    }
}

fn walkdir_count(dir: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.path().is_file() {
                count += 1;
            } else if entry.path().is_dir() {
                count += walkdir_count(&entry.path());
            }
        }
    }
    count
}

/// Classify an archive entry name into a CTF category based on known indicators.
fn classify_entry_name(name: &str) -> Option<&'static str> {
    let name = name.to_lowercase();

    if name.contains("dockerfile")
        || name.contains("package.json")
        || name.contains("app.py")
        || name.contains("server.js")
        || name.contains("index.html")
    {
        return Some("web");
    }

    if name.contains("libc.so")
        || name.ends_with(".elf")
        || name.contains("ld-")
        || name.contains("pwntools")
    {
        return Some("pwn");
    }

    if name.contains("crypto")
        || name.contains("cipher")
        || name.contains("rsa")
        || name.contains("aes")
        || name.contains("key.txt")
    {
        return Some("crypto");
    }

    if name.ends_with(".exe")
        || name.ends_with(".dll")
        || name.contains("ghidra")
        || name.contains("ida")
    {
        return Some("rev");
    }

    None
}

/// Maximum number of archive entries to scan for category detection
const MAX_SCAN_ENTRIES: usize = 50;

fn scan_zip_for_category(path: &Path) -> Option<&'static str> {
    use zip::ZipArchive;

    let file = fs::File::open(path).ok()?;
    let mut archive = ZipArchive::new(file).ok()?;

    for i in 0..archive.len().min(MAX_SCAN_ENTRIES) {
        if let Ok(file) = archive.by_index(i) {
            if let Some(cat) = classify_entry_name(file.name()) {
                return Some(cat);
            }
        }
    }

    None
}

fn scan_tar_for_category(path: &Path) -> Option<&'static str> {
    use flate2::read::GzDecoder;
    use std::io::{BufReader, Read};
    use tar::Archive;

    let file = fs::File::open(path).ok()?;

    let reader: Box<dyn Read> = if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e == "gz")
        .unwrap_or(false)
        || path.to_string_lossy().ends_with(".tgz")
    {
        Box::new(GzDecoder::new(BufReader::new(file)))
    } else {
        Box::new(BufReader::new(file))
    };

    let mut archive = Archive::new(reader);

    if let Ok(entries) = archive.entries() {
        for (idx, entry) in entries.enumerate() {
            if idx > MAX_SCAN_ENTRIES {
                break;
            }
            if let Ok(entry) = entry {
                if let Ok(path) = entry.path() {
                    if let Some(cat) = classify_entry_name(&path.to_string_lossy()) {
                        return Some(cat);
                    }
                }
            }
        }
    }

    None
}
