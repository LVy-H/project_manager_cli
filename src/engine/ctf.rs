use crate::config::Config;
use anyhow::{Context, Result};
use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
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

pub fn import_challenge(_config: &Config, path: &PathBuf) -> Result<()> {
    // 1. Check if we are in a CTF event directory
    let current_dir = std::env::current_dir()?;
    // simple check: look for .ctf_meta.json
    if !current_dir.join(".ctf_meta.json").exists() {
        anyhow::bail!("Not inside a CTF event directory (missing .ctf_meta.json)");
    }

    if !path.exists() {
        anyhow::bail!("Challenge file not found: {:?}", path);
    }

    println!("Analyzing challenge archive: {:?}", path);

    // Heuristics to guess category
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("challenge")
        .to_lowercase();

    // Heuristic 1: File name contains category
    let category = if file_name.contains("web") {
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
        // scan content for cues
        "misc" // Default to misc if unknown
    };

    println!("Detected category: {}", category);

    // Create category dir if needed
    let category_dir = current_dir.join(category);
    if !category_dir.exists() {
        fs::create_dir(&category_dir)?;
    }

    // Determine challenge name from archive name (strip extension)
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

    // Extract archive
    // Note: In a real implementation we would iterate and exact files here.
    // For now we just copy the archive there for manual extraction or use standard tools
    // but the plan says "Smart Import", so let's try to extract if we can.

    // For this MVP, let's just copy the file into the folder
    let dest_file = challenge_dir.join(path.file_name().unwrap());
    fs::copy(path, &dest_file)?;
    println!("Imported archive to {:?}", dest_file);

    // Add a default solve script
    add_solve_script(&challenge_dir, category)?;

    Ok(())
}

pub fn add_challenge(_config: &Config, path: &str) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    if !current_dir.join(".ctf_meta.json").exists() {
        anyhow::bail!("Not inside a CTF event directory.");
    }

    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid format. Use <category>/<name> (e.g. pwn/overflow)");
    }
    let category = parts[0];
    let name = parts[1];

    let category_dir = current_dir.join(category);
    if !category_dir.exists() {
        println!("Creating category: {}", category);
        fs::create_dir(&category_dir)?;
    }

    let challenge_dir = category_dir.join(name);
    if challenge_dir.exists() {
        anyhow::bail!("Challenge already exists: {:?}", challenge_dir);
    }

    fs::create_dir(&challenge_dir)?;
    println!("Created challenge: {}/{}", category, name);

    add_solve_script(&challenge_dir, category)?;

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
    let current_dir = std::env::current_dir()?;
    if !current_dir.join(".ctf_meta.json").exists() {
        anyhow::bail!("Not inside a CTF event directory.");
    }

    let meta = CtfMeta::load(&current_dir).context("Failed to load metadata")?;
    let mut writeup_content = format!("# Writeup: {}\n\nDate: {}\n\n", meta.name, meta.date);

    // Walk through categories and challenges
    if let Ok(cats) = fs::read_dir(&current_dir) {
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

    let writeup_path = current_dir.join("Writeup.md");
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
