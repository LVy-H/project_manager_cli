use crate::config::Config;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum OpType {
    Move,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Operation {
    pub timestamp: i64,
    pub kind: OpType,
    pub src: PathBuf,
    pub dest: PathBuf,
}

/// Result of an undo operation
#[derive(Debug, Clone)]
pub struct UndoItem {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub success: bool,
    pub error: Option<String>,
}

/// Result of undo operations
#[derive(Debug, Default)]
pub struct UndoReport {
    pub undone: Vec<UndoItem>,
    pub no_log_found: bool,
    pub log_empty: bool,
}

fn get_log_path(config: &Config) -> PathBuf {
    let workspace = config.resolve_path("workspace");
    workspace.join(".undo_log.jsonl")
}

pub fn log_move(config: &Config, src: &Path, dest: &Path) -> Result<()> {
    let op = Operation {
        timestamp: chrono::Utc::now().timestamp(),
        kind: OpType::Move,
        src: src.to_path_buf(),
        dest: dest.to_path_buf(),
    };

    let log_path = get_log_path(config);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .context("Failed to open undo log")?;

    let json = serde_json::to_string(&op)?;
    writeln!(file, "{}", json)?;
    Ok(())
}

pub fn undo_last(config: &Config, count: usize) -> Result<UndoReport> {
    let log_path = get_log_path(config);

    if !log_path.exists() {
        return Ok(UndoReport {
            no_log_found: true,
            ..Default::default()
        });
    }

    let file = std::fs::File::open(&log_path)?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();

    if lines.is_empty() {
        return Ok(UndoReport {
            log_empty: true,
            ..Default::default()
        });
    }

    let to_undo = lines.len().min(count);
    let (keep, revert) = lines.split_at(lines.len() - to_undo);

    let mut undone = Vec::new();

    for line in revert.iter().rev() {
        let op: Operation = serde_json::from_str(line)?;
        match op.kind {
            OpType::Move => {
                if op.dest.exists() {
                    // Create parent directory if needed
                    if let Some(parent) = op.src.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }

                    match std::fs::rename(&op.dest, &op.src) {
                        Ok(_) => {
                            undone.push(UndoItem {
                                source: op.dest.clone(),
                                destination: op.src.clone(),
                                success: true,
                                error: None,
                            });
                        }
                        Err(e) => {
                            undone.push(UndoItem {
                                source: op.dest.clone(),
                                destination: op.src.clone(),
                                success: false,
                                error: Some(e.to_string()),
                            });
                        }
                    }
                } else {
                    undone.push(UndoItem {
                        source: op.dest.clone(),
                        destination: op.src.clone(),
                        success: false,
                        error: Some("Source file not found".to_string()),
                    });
                }
            }
        }
    }

    // Rewrite log without reverted lines
    let mut file = std::fs::File::create(&log_path)?;
    for line in keep {
        writeln!(file, "{}", line)?;
    }

    Ok(UndoReport {
        undone,
        no_log_found: false,
        log_empty: false,
    })
}
