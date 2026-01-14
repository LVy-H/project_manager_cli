use crate::config::Config;
use crate::utils::ui;
use anyhow::{Context, Result};
use chrono::prelude::*;
use std::fs;
use std::path::PathBuf;
use tabled::{Table, Tabled};

pub fn create_event(config: &Config, name: &str, date: Option<String>) -> Result<()> {
    let ctf_root = config.resolve_path(&config.organize.ctf_dir);

    if !ctf_root.exists() {
        fs::create_dir_all(&ctf_root).context("Failed to create CTF root directory")?;
        ui::print_warning(&format!("Created root: {:?}", ctf_root));
    }

    let folder_name = if let Some(d) = date {
        format!("{}_{}", d, name)
    } else {
        let now = Local::now();
        format!("{}_{}", now.year(), name)
    };

    let event_dir = ctf_root.join(&folder_name);

    if event_dir.exists() {
        ui::print_error(&format!("Event directory {:?} already exists.", event_dir));
        return Ok(());
    }

    fs::create_dir(&event_dir).context("Failed to create event directory")?;
    ui::print_success(&format!("Initialized: {:?}", event_dir));

    for cat in &config.ctf.default_categories {
        fs::create_dir(event_dir.join(cat)).context("Failed to create category")?;
    }
    fs::File::create(event_dir.join("notes.md")).context("Failed to create notes.md")?;

    ui::print_info(&format!(
        "  + Categories: {}",
        config.ctf.default_categories.join(", ")
    ));
    ui::print_info("  + File: notes.md");

    Ok(())
}

#[derive(Tabled)]
struct CtfEvent {
    #[tabled(rename = "Event")]
    name: String,
    #[tabled(rename = "Year")]
    year: String,
    #[tabled(rename = "Challenges")]
    count: usize,
}

pub fn list_events(config: &Config) -> Result<()> {
    let ctf_root = config.resolve_path(&config.organize.ctf_dir);
    if !ctf_root.exists() {
        ui::print_warning("No CTF directory found.");
        return Ok(());
    }

    let mut events = Vec::new();
    let entries = fs::read_dir(ctf_root)?;

    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        let year = if name.len() >= 4 && name[..4].chars().all(char::is_numeric) {
            name[..4].to_string()
        } else {
            "????".to_string()
        };

        // Naive challenge counting: count sub-sub-directories
        // But the python script did: check if it's a year container or direct event.
        // We will assume flat for now as "Year_Name" format creates one folder.
        // Python logic: "if name is digit and len 4 -> recurse".
        // Rust: let's match python logic.

        if name.len() == 4 && name.chars().all(char::is_numeric) {
            let sub_entries = fs::read_dir(entry.path())?;
            for sub in sub_entries.flatten() {
                if !sub.path().is_dir() {
                    continue;
                }
                let sub_name = sub.file_name().to_string_lossy().to_string();
                let count = count_challenges(&sub.path());
                events.push(CtfEvent {
                    name: format!("{}/{}", name, sub_name),
                    year: name.clone(),
                    count,
                });
            }
        } else {
            let count = count_challenges(&entry.path());
            events.push(CtfEvent { name, year, count });
        }
    }

    events.sort_by(|a, b| a.name.cmp(&b.name));

    let table = Table::new(events).to_string();
    println!("{}", table);

    Ok(())
}

fn count_challenges(event_dir: &PathBuf) -> usize {
    let mut count = 0;
    if let Ok(cats) = fs::read_dir(event_dir) {
        for cat in cats.flatten() {
            if cat.path().is_dir() {
                if let Ok(chals) = fs::read_dir(cat.path()) {
                    count += chals.flatten().filter(|c| c.path().is_dir()).count();
                }
            }
        }
    }
    count
}
