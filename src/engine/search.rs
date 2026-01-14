use anyhow::{Context, Result};
use regex::RegexBuilder;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use walkdir::WalkDir;
use zip::ZipArchive;

/// Maximum file size to scan (100MB). Files larger than this are skipped.
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;

/// Maximum size for files inside archives (50MB)
const MAX_ARCHIVE_ENTRY_SIZE: u64 = 50 * 1024 * 1024;

/// Represents a single flag match found during scanning
#[derive(Debug, Clone)]
pub struct FlagMatch {
    /// Path to the file containing the match
    pub file_path: String,
    /// If inside an archive, the entry name within the archive
    pub archive_entry: Option<String>,
    /// The matched flag string
    pub matched_text: String,
    /// Line number (1-indexed) if available
    pub line_number: Option<usize>,
}

/// Result of a flag search operation
#[derive(Debug, Default)]
pub struct SearchReport {
    pub matches: Vec<FlagMatch>,
    pub files_scanned: usize,
    pub files_skipped: usize,
    pub errors: Vec<String>,
}

impl SearchReport {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Search for flags in files under the given path
pub fn find_flags(path: &Path, pattern: Option<String>) -> Result<SearchReport> {
    let pattern_str = pattern.as_deref().unwrap_or(r"(ctf|flag)\{.*?\}");
    let regex = RegexBuilder::new(pattern_str)
        .case_insensitive(true)
        .build()
        .context("Invalid regex pattern")?;

    let mut report = SearchReport::new();

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        let entry_path = entry.path();
        if entry_path.is_file() {
            if let Some(ext) = entry_path.extension().and_then(|s| s.to_str()) {
                let result = match ext {
                    "zip" => scan_zip(entry_path, &regex),
                    "tar" => scan_tar(entry_path, &regex),
                    "gz" | "tgz" => scan_tar_gz(entry_path, &regex),
                    _ => scan_file(entry_path, &regex),
                };
                match result {
                    Ok(matches) => {
                        report.files_scanned += 1;
                        report.matches.extend(matches);
                    }
                    Err(e) => {
                        report
                            .errors
                            .push(format!("{}: {}", entry_path.display(), e));
                    }
                }
            } else {
                match scan_file(entry_path, &regex) {
                    Ok(matches) => {
                        report.files_scanned += 1;
                        report.matches.extend(matches);
                    }
                    Err(e) => {
                        report
                            .errors
                            .push(format!("{}: {}", entry_path.display(), e));
                    }
                }
            }
        }
    }
    Ok(report)
}

/// Scan a single file using streaming BufReader (memory-safe)
fn scan_file(path: &Path, regex: &regex::Regex) -> Result<Vec<FlagMatch>> {
    let mut matches = Vec::new();

    // Check file size first
    let metadata = fs::metadata(path)?;
    if metadata.len() > MAX_FILE_SIZE {
        // Skip large files silently (will be counted in skipped)
        return Ok(matches);
    }

    let file = File::open(path)?;
    let reader = BufReader::new(file);

    // Try to read as text line-by-line
    // For binary files, this will still work but may have long "lines"
    for (line_idx, line_result) in reader.lines().enumerate() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => {
                // If we can't read as UTF-8 lines, fall back to chunked binary scan
                return scan_file_binary(path, regex);
            }
        };

        for mat in regex.find_iter(&line) {
            matches.push(FlagMatch {
                file_path: path.display().to_string(),
                archive_entry: None,
                matched_text: mat.as_str().to_string(),
                line_number: Some(line_idx + 1),
            });
        }
    }

    Ok(matches)
}

/// Scan a binary file using chunked reading with overlap
fn scan_file_binary(path: &Path, regex: &regex::Regex) -> Result<Vec<FlagMatch>> {
    let mut matches = Vec::new();

    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    // Use 64KB chunks with 1KB overlap to catch matches spanning chunk boundaries
    const CHUNK_SIZE: usize = 64 * 1024;
    const OVERLAP: usize = 1024;

    let mut buffer = vec![0u8; CHUNK_SIZE];
    let mut overlap_buffer = Vec::new();

    loop {
        // Prepend overlap from previous chunk
        let mut combined = overlap_buffer.clone();

        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        combined.extend_from_slice(&buffer[..bytes_read]);

        // Convert to lossy string and search
        let text = String::from_utf8_lossy(&combined);
        for mat in regex.find_iter(&text) {
            let matched = mat.as_str().to_string();
            // Avoid duplicates from overlap region
            if !matches
                .iter()
                .any(|m: &FlagMatch| m.matched_text == matched)
            {
                matches.push(FlagMatch {
                    file_path: path.display().to_string(),
                    archive_entry: None,
                    matched_text: matched,
                    line_number: None,
                });
            }
        }

        // Save overlap for next iteration
        if bytes_read >= OVERLAP {
            overlap_buffer = buffer[bytes_read - OVERLAP..bytes_read].to_vec();
        } else {
            overlap_buffer.clear();
        }

        if bytes_read < CHUNK_SIZE {
            break;
        }
    }

    Ok(matches)
}

/// Scan a buffer (used for archive entries)
fn scan_buffer(
    buffer: &[u8],
    file_path: &str,
    archive_entry: Option<String>,
    regex: &regex::Regex,
) -> Vec<FlagMatch> {
    let mut matches = Vec::new();
    let text = String::from_utf8_lossy(buffer);

    for mat in regex.find_iter(&text) {
        matches.push(FlagMatch {
            file_path: file_path.to_string(),
            archive_entry: archive_entry.clone(),
            matched_text: mat.as_str().to_string(),
            line_number: None,
        });
    }

    matches
}

fn scan_zip(path: &Path, regex: &regex::Regex) -> Result<Vec<FlagMatch>> {
    let mut all_matches = Vec::new();
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).context("Failed to open zip")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        if file.is_dir() {
            continue;
        }

        // Skip large entries
        if file.size() > MAX_ARCHIVE_ENTRY_SIZE {
            continue;
        }

        let mut buffer = Vec::new();
        if file.read_to_end(&mut buffer).is_ok() {
            let matches = scan_buffer(&buffer, &path.display().to_string(), Some(name), regex);
            all_matches.extend(matches);
        }
    }
    Ok(all_matches)
}

fn scan_tar(path: &Path, regex: &regex::Regex) -> Result<Vec<FlagMatch>> {
    let mut all_matches = Vec::new();
    let file = File::open(path)?;
    let mut archive = tar::Archive::new(file);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?.to_string_lossy().to_string();

        // Skip large entries
        if entry.size() > MAX_ARCHIVE_ENTRY_SIZE {
            continue;
        }

        let mut buffer = Vec::new();
        if entry.read_to_end(&mut buffer).is_ok() {
            let matches = scan_buffer(
                &buffer,
                &path.display().to_string(),
                Some(entry_path),
                regex,
            );
            all_matches.extend(matches);
        }
    }
    Ok(all_matches)
}

fn scan_tar_gz(path: &Path, regex: &regex::Regex) -> Result<Vec<FlagMatch>> {
    let mut all_matches = Vec::new();
    let file = File::open(path)?;
    let tar = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(tar);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?.to_string_lossy().to_string();

        // Skip large entries
        if entry.size() > MAX_ARCHIVE_ENTRY_SIZE {
            continue;
        }

        let mut buffer = Vec::new();
        if entry.read_to_end(&mut buffer).is_ok() {
            let matches = scan_buffer(
                &buffer,
                &path.display().to_string(),
                Some(entry_path),
                regex,
            );
            all_matches.extend(matches);
        }
    }
    Ok(all_matches)
}
