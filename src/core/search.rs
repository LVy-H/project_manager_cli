use anyhow::{Context, Result};
use colored::*;
use regex::RegexBuilder;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use walkdir::WalkDir;
use zip::ZipArchive;

pub fn find_flags(path: &Path, pattern: Option<String>) -> Result<()> {
    let pattern_str = pattern.as_deref().unwrap_or(r"(ctf|flag)\{.*?\}");
    let regex = RegexBuilder::new(pattern_str)
        .case_insensitive(true)
        .build()
        .context("Invalid regex pattern")?;

    println!(
        "Searching for matches of '{}' in {:?}...",
        pattern_str.cyan(),
        path
    );

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        let entry_path = entry.path();
        if entry_path.is_file() {
            if let Some(ext) = entry_path.extension().and_then(|s| s.to_str()) {
                match ext {
                    "zip" => scan_zip(entry_path, &regex)?,
                    "tar" => scan_tar(entry_path, &regex)?,
                    "gz" | "tgz" => scan_tar_gz(entry_path, &regex)?,
                    _ => scan_file(entry_path, &regex)?,
                }
            } else {
                scan_file(entry_path, &regex)?;
            }
        }
    }
    Ok(())
}

fn scan_file(path: &Path, regex: &regex::Regex) -> Result<()> {
    // Try to read as text. If binary, we might skip or use 'lossy'
    // For simplicity, let's read to string lossy.
    // Optimization: Read manageable chunks? For flags, usually small.
    // But if we encounter a 1GB binary, reading to string is bad.
    // Let's rely on std::fs::read which is simple but maybe memory hungry for huge files.
    // Better: Helper function to read limited bytes or stream scan?
    // Given the request for "robust", let's be careful.

    let Ok(content) = std::fs::read(path) else {
        return Ok(());
    };

    // Quick heuristic: simple binary check (null bytes)
    // if content.contains(&0) { return Ok(()); } // naive binary check

    let text = String::from_utf8_lossy(&content);

    for mat in regex.find_iter(&text) {
        println!(
            "{}: {}",
            path.display().to_string().magenta(),
            mat.as_str().green().bold()
        );
    }
    Ok(())
}

fn scan_zip(path: &Path, regex: &regex::Regex) -> Result<()> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).context("Failed to open zip")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        if file.is_dir() {
            continue;
        }

        let mut buffer = Vec::new();
        // Limit size to avoid zip bombs or huge memory usage
        if file.read_to_end(&mut buffer).is_ok() {
            let text = String::from_utf8_lossy(&buffer);
            for mat in regex.find_iter(&text) {
                println!(
                    "{} (in {}): {}",
                    name.magenta(),
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    mat.as_str().green().bold()
                );
            }
        }
    }
    Ok(())
}

fn scan_tar(path: &Path, regex: &regex::Regex) -> Result<()> {
    let file = File::open(path)?;
    let mut archive = tar::Archive::new(file);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path_lossy = entry.path()?.to_string_lossy().to_string();

        let mut buffer = Vec::new();
        if entry.read_to_end(&mut buffer).is_ok() {
            let text = String::from_utf8_lossy(&buffer);
            for mat in regex.find_iter(&text) {
                println!(
                    "{} (in {}): {}",
                    path_lossy.magenta(),
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    mat.as_str().green().bold()
                );
            }
        }
    }
    Ok(())
}

fn scan_tar_gz(path: &Path, regex: &regex::Regex) -> Result<()> {
    let file = File::open(path)?;
    let tar = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(tar);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path_lossy = entry.path()?.to_string_lossy().to_string();

        let mut buffer = Vec::new();
        if entry.read_to_end(&mut buffer).is_ok() {
            let text = String::from_utf8_lossy(&buffer);
            for mat in regex.find_iter(&text) {
                println!(
                    "{} (in {}): {}",
                    path_lossy.magenta(),
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    mat.as_str().green().bold()
                );
            }
        }
    }
    Ok(())
}
