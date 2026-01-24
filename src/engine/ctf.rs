use crate::config::Config;
use anyhow::{Context, Result};
use chrono::prelude::*;
use fs_err as fs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// CTF event metadata stored in .ctf_meta.json
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CtfMeta {
    pub name: String,
    pub date: String,
    pub year: i32,
    pub created_at: i64,
    #[serde(default)]
    pub categories: Vec<String>,
}

impl CtfMeta {
    pub fn new(name: &str, date: Option<String>) -> Self {
        let now = Local::now();
        let (year, date_str) = if let Some(d) = date {
            let y = d
                .split('-')
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(now.year());
            (y, d)
        } else {
            (now.year(), now.format("%Y-%m-%d").to_string())
        };

        Self {
            name: name.to_string(),
            date: date_str,
            year,
            created_at: now.timestamp(),
            categories: Vec::new(),
        }
    }

    /// Load metadata from a CTF event directory
    pub fn load(event_dir: &Path) -> Option<Self> {
        let meta_path = event_dir.join(".ctf_meta.json");
        if meta_path.exists() {
            let content = fs::read_to_string(&meta_path).ok()?;
            serde_json::from_str(&content).ok()
        } else {
            None
        }
    }

    /// Save metadata to a CTF event directory
    pub fn save(&self, event_dir: &Path) -> Result<()> {
        let meta_path = event_dir.join(".ctf_meta.json");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(meta_path, content)?;
        Ok(())
    }
}

/// Result of creating a CTF event
#[derive(Debug)]
pub struct CreateEventResult {
    pub event_dir: PathBuf,
    pub categories_created: Vec<String>,
    pub already_exists: bool,
}

/// Result of listing CTF events
#[derive(Debug, Clone)]
pub struct CtfEventInfo {
    pub name: String,
    pub year: i32,
    pub date: Option<String>,
    pub challenge_count: usize,
    pub path: PathBuf,
    pub has_metadata: bool,
}

#[derive(Debug, Default)]
pub struct ListEventsResult {
    pub events: Vec<CtfEventInfo>,
    pub ctf_root_missing: bool,
}

pub fn create_event(
    config: &Config,
    name: &str,
    date: Option<String>,
) -> Result<CreateEventResult> {
    let ctf_root = config.ctf_root();

    if !ctf_root.exists() {
        fs::create_dir_all(&ctf_root).context("Failed to create CTF root directory")?;
    }

    let meta = CtfMeta::new(name, date.clone());
    let folder_name = format!("{}_{}", meta.date.split('-').next().unwrap_or("0000"), name);
    let event_dir = ctf_root.join(&folder_name);

    if event_dir.exists() {
        return Ok(CreateEventResult {
            event_dir,
            categories_created: Vec::new(),
            already_exists: true,
        });
    }

    fs::create_dir(&event_dir).context("Failed to create event directory")?;

    // Create category directories
    let mut categories_created = Vec::new();
    for cat in &config.ctf.default_categories {
        fs::create_dir(event_dir.join(cat)).context("Failed to create category")?;
        categories_created.push(cat.clone());
    }

    // Create notes.md
    fs::File::create(event_dir.join("notes.md")).context("Failed to create notes.md")?;

    // Save metadata
    let mut meta = meta;
    meta.categories = categories_created.clone();
    meta.save(&event_dir)?;

    // Auto-set active event
    if let Err(e) = set_active_event(config, name) {
        println!("Warning: Failed to set active event: {}", e);
    }

    Ok(CreateEventResult {
        event_dir,
        categories_created,
        already_exists: false,
    })
}

pub fn list_events(config: &Config) -> Result<ListEventsResult> {
    let ctf_root = config.ctf_root();

    if !ctf_root.exists() {
        return Ok(ListEventsResult {
            events: Vec::new(),
            ctf_root_missing: true,
        });
    }

    let mut events = Vec::new();
    let entries = fs::read_dir(&ctf_root)?;

    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }

        let path = entry.path();
        let dir_name = entry.file_name().to_string_lossy().to_string();

        // Try to load metadata first
        if let Some(meta) = CtfMeta::load(&path) {
            let challenge_count = count_challenges(&path);
            events.push(CtfEventInfo {
                name: meta.name,
                year: meta.year,
                date: Some(meta.date),
                challenge_count,
                path,
                has_metadata: true,
            });
        } else {
            // Fallback: parse from folder name
            let year = if dir_name.len() >= 4 && dir_name[..4].chars().all(char::is_numeric) {
                dir_name[..4].parse().unwrap_or(0)
            } else {
                0
            };

            // Handle year-only directories (recurse into them)
            if dir_name.len() == 4 && dir_name.chars().all(char::is_numeric) {
                if let Ok(sub_entries) = fs::read_dir(&path) {
                    for sub in sub_entries.flatten() {
                        if !sub.path().is_dir() {
                            continue;
                        }
                        let sub_path = sub.path();
                        let sub_name = sub.file_name().to_string_lossy().to_string();
                        let challenge_count = count_challenges(&sub_path);

                        // Check for metadata in subdirectory
                        let (name, date, has_meta) = if let Some(meta) = CtfMeta::load(&sub_path) {
                            (meta.name, Some(meta.date), true)
                        } else {
                            (sub_name, None, false)
                        };

                        events.push(CtfEventInfo {
                            name,
                            year,
                            date,
                            challenge_count,
                            path: sub_path,
                            has_metadata: has_meta,
                        });
                    }
                }
            } else {
                let challenge_count = count_challenges(&path);
                events.push(CtfEventInfo {
                    name: dir_name,
                    year,
                    date: None,
                    challenge_count,
                    path,
                    has_metadata: false,
                });
            }
        }
    }

    events.sort_by(|a, b| b.year.cmp(&a.year).then_with(|| a.name.cmp(&b.name)));

    Ok(ListEventsResult {
        events,
        ctf_root_missing: false,
    })
}

fn count_challenges(event_dir: &Path) -> usize {
    let mut count = 0;
    if let Ok(cats) = fs::read_dir(event_dir) {
        for cat in cats.flatten() {
            if cat.path().is_dir() && cat.file_name() != ".git" {
                if let Ok(chals) = fs::read_dir(cat.path()) {
                    count += chals.flatten().filter(|c| c.path().is_dir()).count();
                }
            }
        }
    }
    count
}

pub fn import_challenge(
    config: &Config,
    path: &PathBuf,
    category_override: Option<String>,
) -> Result<()> {
    use dialoguer::{theme::ColorfulTheme, Select};

    let event_root = get_active_event_root()?;

    if !path.exists() {
        anyhow::bail!(
            "File not found: {:?}\n\nPlease verify the file path is correct.",
            path
        );
    }

    // Heuristics to guess category
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("challenge")
        .to_lowercase();

    let detected_category = if file_name.contains("web") {
        "web"
    } else if file_name.contains("pwn") || file_name.contains("bof") {
        "pwn"
    } else if file_name.contains("crypto") {
        "crypto"
    } else if file_name.contains("rev") {
        "rev"
    } else if file_name.contains("misc") {
        "misc"
    } else {
        detect_category_from_file(path).unwrap_or("misc")
    };

    // Interactive category selection or override
    let category_string;
    let category = if let Some(cat) = category_override {
        category_string = cat;
        &category_string
    } else {
        let mut categories = config.ctf.default_categories.clone();
        if !categories.contains(&detected_category.to_string()) {
            categories.push(detected_category.to_string());
        }
        // Ensure "misc" is always available
        if !categories.iter().any(|c| c == "misc") {
            categories.push("misc".to_string());
        }

        // Find index of detected category
        let default_idx = categories
            .iter()
            .position(|c| c == detected_category)
            .unwrap_or(0);

        println!("Importing: {:?}", path.file_name().unwrap());

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select Category")
            .default(default_idx)
            .items(&categories)
            .interact()
            .unwrap_or(default_idx);

        category_string = categories[selection].clone();
        &category_string
    };

    // Create category dir if needed
    let category_dir = event_root.join(category);
    if !category_dir.exists() {
        fs::create_dir(&category_dir)?;
    }

    // Determine challenge name from file name (strip extension)
    let challenge_name = Path::new(&file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown_chall");

    let challenge_dir = category_dir.join(challenge_name);
    if challenge_dir.exists() {
        anyhow::bail!("Challenge directory already exists: {:?}", challenge_dir);
    }

    fs::create_dir(&challenge_dir)?;
    println!("Created challenge directory: {:?}", challenge_dir);

    // Move the file
    let dest_file = challenge_dir.join(path.file_name().unwrap());

    match fs::rename(path, &dest_file) {
        Ok(_) => println!("✓ Moved file to {:?}", dest_file),
        Err(_) => {
            // Fallback to copy + delete if rename fails (cross-device link)
            println!("Move failed, attempting copy...");
            fs::copy(path, &dest_file)?;
            fs::remove_file(path)?;
            println!("✓ Copied and removed original file");
        }
    }

    // If it's a zip/tar, offer to extract?
    // For now, just keeping the file there is fine as per "Move not copy" requirement.

    // Add a default solve script
    add_solve_script(&challenge_dir, category)?;

    Ok(())
}

fn detect_category_from_file(path: &Path) -> Option<&'static str> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "zip" => scan_zip_for_category(path),
        "tar" | "gz" | "tgz" => scan_tar_for_category(path),
        "py" | "js" | "html" | "php" => Some("web"),
        "c" | "cpp" | "elf" => Some("pwn"),
        "enc" | "key" | "pem" => Some("crypto"),
        "exe" | "dll" | "asm" => Some("rev"),
        "pcap" | "pcapng" | "mem" => Some("forensics"),
        "jpg" | "png" | "gif" => Some("misc"), // steg?
        _ => None,
    }
}

pub fn add_challenge(_config: &Config, path: &str) -> Result<()> {
    let event_root = get_active_event_root()?;

    let parts: Vec<&str> = path.split('/').collect();

    let (category, name) = if parts.len() == 2 {
        (parts[0].to_string(), parts[1].to_string())
    } else if parts.len() == 1 {
        // Try to infer category from CWD
        let current_dir = std::env::current_dir()?;
        // Check if current dir is a direct child of event_root
        if current_dir.parent() == Some(&event_root) {
            let cat_name = current_dir.file_name().unwrap().to_string_lossy();
            (cat_name.to_string(), parts[0].to_string())
        } else {
            anyhow::bail!(
                "Invalid format. Use <category>/<name> OR run inside a category folder.\n\n\
                Examples:\n  \
                wardex ctf add pwn/buffer-overflow\n  \
                wardex ctf add web/sql-injection"
            );
        }
    } else {
        anyhow::bail!(
            "Invalid format. Use <category>/<name>\n\n\
            Examples:\n  \
            wardex ctf add pwn/buffer-overflow"
        );
    };

    let category_dir = event_root.join(&category);
    if !category_dir.exists() {
        println!("Creating category: {}", category);
        fs::create_dir(&category_dir)?;
    }

    let challenge_dir = category_dir.join(&name);
    if challenge_dir.exists() {
        anyhow::bail!(
            "Challenge already exists: {:?}\n\n\
            Tip: Use a different name or remove the existing directory first.",
            challenge_dir
        );
    }

    fs::create_dir(&challenge_dir)?;
    println!("Created challenge: {}/{}", category, name);

    add_solve_script(&challenge_dir, &category)?;

    Ok(())
}

fn add_solve_script(challenge_dir: &Path, category: &str) -> Result<()> {
    let template = match category {
        "pwn" => {
            r#"from pwn import *

# io = process('./chall')
io = remote('TARGET', PORT)

io.interactive()
"#
        }
        "web" => {
            r#"import requests

URL = "http://TARGET"

r = requests.get(URL)
print(r.text)
"#
        }
        _ => {
            r#"# Solve script for challenge
"#
        }
    };

    fs::write(challenge_dir.join("solve.py"), template)?;
    println!("Created solve.py template");
    Ok(())
}

pub fn generate_writeup(_config: &Config) -> Result<()> {
    let event_root = get_active_event_root()?;

    let meta =
        CtfMeta::load(&event_root).context("Failed to load CTF metadata (.ctf_meta.json)")?;
    let mut writeup_content = format!("# Writeup: {}\n\nDate: {}\n\n", meta.name, meta.date);

    // Walk through categories and challenges
    if let Ok(cats) = fs::read_dir(&event_root) {
        let mut categories: Vec<_> = cats.filter_map(|e| e.ok()).collect();
        categories.sort_by_key(|e| e.file_name());

        for cat in categories {
            if cat.path().is_dir() && !cat.file_name().to_string_lossy().starts_with('.') {
                let cat_name = cat.file_name().to_string_lossy().to_string();

                if let Ok(chals) = fs::read_dir(cat.path()) {
                    let mut challenges: Vec<_> = chals.filter_map(|e| e.ok()).collect();
                    challenges.sort_by_key(|e| e.file_name());

                    for chal in challenges {
                        if chal.path().is_dir() {
                            let chal_name = chal.file_name().to_string_lossy().to_string();

                            // Check for notes
                            let notes_path = chal.path().join("notes.md");
                            let readme_path = chal.path().join("README.md");

                            let content = if notes_path.exists() {
                                fs::read_to_string(notes_path).unwrap_or_default()
                            } else if readme_path.exists() {
                                fs::read_to_string(readme_path).unwrap_or_default()
                            } else {
                                String::new()
                            };

                            if !content.trim().is_empty() {
                                writeup_content
                                    .push_str(&format!("## [{}] {}\n\n", cat_name, chal_name));
                                writeup_content.push_str(&content);
                                writeup_content.push_str("\n\n---\n\n");
                            }
                        }
                    }
                }
            }
        }
    }

    let writeup_path = event_root.join("Writeup.md");
    fs::write(&writeup_path, writeup_content)?;
    println!("Generated writeup at {:?}", writeup_path);

    Ok(())
}

pub fn archive_event(config: &Config, name: &str) -> Result<()> {
    let ctf_root = config.ctf_root();
    // PARA Archives
    let archives_root = config.resolve_path("archives").join("CTFs");

    if !archives_root.exists() {
        fs::create_dir_all(&archives_root)?;
    }

    // Find the event folder
    let mut event_dir = ctf_root.join(name);
    // Try to find it if name is partial specific
    if !event_dir.exists() {
        // search for directory containing name
        if let Ok(entries) = fs::read_dir(&ctf_root) {
            for entry in entries.flatten() {
                let db_name = entry.file_name().to_string_lossy().to_string();
                if db_name.contains(name) {
                    event_dir = entry.path();
                    break;
                }
            }
        }
    }

    if !event_dir.exists() {
        anyhow::bail!("Event directory not found: {}", name);
    }

    // Load meta to get year
    let year = if let Some(meta) = CtfMeta::load(&event_dir) {
        meta.year.to_string()
    } else {
        Local::now().year().to_string()
    };

    let archive_year_dir = archives_root.join(&year);
    if !archive_year_dir.exists() {
        fs::create_dir_all(&archive_year_dir)?;
    }

    let target_dir = archive_year_dir.join(event_dir.file_name().unwrap());

    println!("Archiving {:?} -> {:?}", event_dir, target_dir);
    fs::rename(event_dir, target_dir)?;

    println!("Event archived successfully.");
    Ok(())
}

fn scan_zip_for_category(path: &Path) -> Option<&'static str> {
    use zip::ZipArchive;

    let file = fs::File::open(path).ok()?;
    let mut archive = ZipArchive::new(file).ok()?;

    for i in 0..archive.len().min(50) {
        if let Ok(file) = archive.by_index(i) {
            let name = file.name().to_lowercase();

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
            if idx > 50 {
                break;
            }
            if let Ok(entry) = entry {
                if let Ok(path) = entry.path() {
                    let name = path.to_string_lossy().to_lowercase();

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
                }
            }
        }
    }

    None
}

pub fn get_event_path(
    config: &Config,
    event_name: Option<&str>,
    challenge_name: Option<&str>,
) -> Result<PathBuf> {
    let events = list_events(config)?;

    if events.ctf_root_missing {
        anyhow::bail!("CTF root directory not found");
    }

    if events.events.is_empty() {
        anyhow::bail!("No CTF events found");
    }

    let event_path = if let Some(name) = event_name {
        events
            .events
            .iter()
            .find(|e| e.name.to_lowercase().contains(&name.to_lowercase()))
            .map(|e| e.path.clone())
            .ok_or_else(|| anyhow::anyhow!("Event not found: {}", name))?
    } else {
        // Prefer current event context (local or global) if available
        if let Ok(root) = get_active_event_root() {
            root
        } else {
            // Fallback to latest
            events
                .events
                .iter()
                .max_by_key(|e| e.year)
                .map(|e| e.path.clone())
                .ok_or_else(|| anyhow::anyhow!("No CTF events found"))?
        }
    };

    if let Some(chall) = challenge_name {
        for entry in fs::read_dir(&event_path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                for chall_entry in fs::read_dir(entry.path())? {
                    let chall_entry = chall_entry?;
                    if chall_entry
                        .file_name()
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&chall.to_lowercase())
                    {
                        return Ok(chall_entry.path());
                    }
                }
            }
        }
        anyhow::bail!("Challenge not found: {}", chall);
    }

    Ok(event_path)
}

/// Walk up directory tree to find CTF event root (containing .ctf_meta.json)
pub fn find_event_root() -> Option<PathBuf> {
    let mut current = std::env::current_dir().ok()?;
    loop {
        if current.join(".ctf_meta.json").exists() {
            return Some(current);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

/// Get the active event root from local context or global state
pub fn get_active_event_root() -> Result<PathBuf> {
    // 1. Try local context (walking up)
    if let Some(root) = find_event_root() {
        return Ok(root);
    }
    // 2. Try global state
    let state = crate::core::state::AppState::load();
    if let Some(path) = state.get_event() {
        if path.exists() && path.join(".ctf_meta.json").exists() {
            return Ok(path);
        }
    }
    anyhow::bail!(
        "No active CTF event found.\nRun inside an event dir or use 'wardex ctf use <event>'"
    )
}

pub fn set_active_event(config: &Config, name: &str) -> Result<()> {
    let path = get_event_path(config, Some(name), None)?;
    let mut state = crate::core::state::AppState::load();
    state.set_event(path.clone())?;
    println!("Switched to event: {}", name);
    println!("Context set to: {:?}", path);
    Ok(())
}

/// Get info about current CTF context
pub fn get_context_info(_config: &Config) -> Result<()> {
    let root = match get_active_event_root() {
        Ok(r) => r,
        Err(_) => {
            println!("No active CTF event context detected.");
            println!(
                "Run this command inside a CTF event directory or use 'wardex ctf use <event>'."
            );
            return Ok(());
        }
    };

    let meta = CtfMeta::load(&root).ok_or_else(|| anyhow::anyhow!("Failed to load metadata"))?;

    // Check if we are physically inside the root
    let current = std::env::current_dir()?;
    let is_local = current.starts_with(&root);

    println!("Current Event: {} ({})", meta.name, meta.year);
    println!("Root: {:?}", root);
    println!(
        "Source: {}",
        if is_local {
            "Local Directory"
        } else {
            "Global State"
        }
    );

    if is_local {
        if let Ok(rel) = current.strip_prefix(&root) {
            if rel.components().count() > 0 {
                println!("Location: ./{}", rel.display());
            } else {
                println!("Location: Event Root");
            }
        }
    }
    Ok(())
}
