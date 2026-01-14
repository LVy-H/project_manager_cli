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
