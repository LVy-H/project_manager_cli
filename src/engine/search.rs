use anyhow::{Context, Result};
use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use std::fs::File;
use std::io::Read;
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
    let pattern_str = pattern.as_deref().unwrap_or(r"(?i)(ctf|flag)\{.*?\}");
    let matcher = RegexMatcher::new(pattern_str).context("Invalid regex pattern")?;

    let mut report = SearchReport::new();

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        let entry_path = entry.path();
        if entry_path.is_file() {
            // Check file size
            if let Ok(metadata) = std::fs::metadata(entry_path) {
                if metadata.len() > MAX_FILE_SIZE {
                    report.files_skipped += 1;
                    continue;
                }
            }

            if let Some(ext) = entry_path.extension().and_then(|s| s.to_str()) {
                let result = match ext {
                    "zip" => scan_zip(entry_path, &pattern_str),
                    "tar" => scan_tar(entry_path, &pattern_str),
                    "gz" | "tgz" => scan_tar_gz(entry_path, &pattern_str),
                    _ => scan_file(entry_path, &matcher),
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
                match scan_file(entry_path, &matcher) {
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

/// Scan a single file using grep-searcher (ripgrep's library)
fn scan_file(path: &Path, matcher: &RegexMatcher) -> Result<Vec<FlagMatch>> {
    let mut matches = Vec::new();
    let file_path = path.display().to_string();

    let mut searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .build();

    // Use UTF8 sink for line-by-line matching
    let result = searcher.search_path(
        matcher,
        path,
        UTF8(|line_num, line| {
            // The line already matched - extract the match text
            // Use regex to find exact match positions in the line
            if let Ok(regex) = regex::RegexBuilder::new(r"(?i)(ctf|flag)\{.*?\}")
                .case_insensitive(true)
                .build()
            {
                for mat in regex.find_iter(line) {
                    matches.push(FlagMatch {
                        file_path: file_path.clone(),
                        archive_entry: None,
                        matched_text: mat.as_str().to_string(),
                        line_number: Some(line_num as usize),
                    });
                }
            }
            Ok(true)
        }),
    );

    match result {
        Ok(_) => Ok(matches),
        Err(e) => {
            // Binary file or read error - try fallback if needed
            log::debug!("grep-searcher failed for {}: {}", path.display(), e);
            Ok(matches)
        }
    }
}

/// Scan a buffer (used for archive entries)
fn scan_buffer(
    buffer: &[u8],
    file_path: &str,
    archive_entry: Option<String>,
    pattern: &str,
) -> Vec<FlagMatch> {
    let mut matches = Vec::new();
    let text = String::from_utf8_lossy(buffer);

    if let Ok(regex) = regex::RegexBuilder::new(pattern)
        .case_insensitive(true)
        .build()
    {
        for mat in regex.find_iter(&text) {
            matches.push(FlagMatch {
                file_path: file_path.to_string(),
                archive_entry: archive_entry.clone(),
                matched_text: mat.as_str().to_string(),
                line_number: None,
            });
        }
    }

    matches
}

fn scan_zip(path: &Path, pattern: &str) -> Result<Vec<FlagMatch>> {
    let mut all_matches = Vec::new();
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).context("Failed to open zip")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        if file.is_dir() {
            continue;
        }

        if file.size() > MAX_ARCHIVE_ENTRY_SIZE {
            continue;
        }

        let mut buffer = Vec::new();
        if file.read_to_end(&mut buffer).is_ok() {
            let matches = scan_buffer(&buffer, &path.display().to_string(), Some(name), pattern);
            all_matches.extend(matches);
        }
    }
    Ok(all_matches)
}

fn scan_tar(path: &Path, pattern: &str) -> Result<Vec<FlagMatch>> {
    let mut all_matches = Vec::new();
    let file = File::open(path)?;
    let mut archive = tar::Archive::new(file);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?.to_string_lossy().to_string();

        if entry.size() > MAX_ARCHIVE_ENTRY_SIZE {
            continue;
        }

        let mut buffer = Vec::new();
        if entry.read_to_end(&mut buffer).is_ok() {
            let matches = scan_buffer(
                &buffer,
                &path.display().to_string(),
                Some(entry_path),
                pattern,
            );
            all_matches.extend(matches);
        }
    }
    Ok(all_matches)
}

fn scan_tar_gz(path: &Path, pattern: &str) -> Result<Vec<FlagMatch>> {
    let mut all_matches = Vec::new();
    let file = File::open(path)?;
    let tar = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(tar);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?.to_string_lossy().to_string();

        if entry.size() > MAX_ARCHIVE_ENTRY_SIZE {
            continue;
        }

        let mut buffer = Vec::new();
        if entry.read_to_end(&mut buffer).is_ok() {
            let matches = scan_buffer(
                &buffer,
                &path.display().to_string(),
                Some(entry_path),
                pattern,
            );
            all_matches.extend(matches);
        }
    }
    Ok(all_matches)
}
