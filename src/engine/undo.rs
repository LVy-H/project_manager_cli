use crate::config::Config;
use crate::utils::ui;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum OpType {
    Move,
    // Add Copy/Delete later if needed
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Operation {
    pub timestamp: i64,
    pub kind: OpType,
    pub src: PathBuf,
    pub dest: PathBuf,
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

pub fn undo_last(config: &Config, count: usize) -> Result<()> {
    let log_path = get_log_path(config);
    if !log_path.exists() {
        ui::print_warning("No undo log found.");
        return Ok(());
    }

    let file = std::fs::File::open(&log_path)?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();

    if lines.is_empty() {
        ui::print_warning("Undo log is empty.");
        return Ok(());
    }

    let to_undo = lines.len().min(count);
    let (keep, revert) = lines.split_at(lines.len() - to_undo);

    ui::print_info(&format!("Undoing last {} operations...", to_undo));

    for line in revert.iter().rev() {
        let op: Operation = serde_json::from_str(line)?;
        match op.kind {
            OpType::Move => {
                // Reverse move: dest -> src
                ui::print_info(&format!(
                    "Reverting: {:?} -> {:?}",
                    op.dest.file_name().unwrap(),
                    op.src
                ));
                if op.dest.exists() {
                    // We don't log the undo itself to the regular log to avoid loops,
                    // or we could but with a different type. For now, just move back.
                    // We use fs::rename directly to avoid circular logging if we reused move_item
                    if let Some(parent) = op.src.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::rename(&op.dest, &op.src).context("Failed to revert move")?;
                } else {
                    ui::print_warning(&format!("Skipping: {:?} not found", op.dest));
                }
            }
        }
    }

    // Rewrite log without reverted lines
    let mut file = std::fs::File::create(&log_path)?;
    for line in keep {
        writeln!(file, "{}", line)?;
    }

    ui::print_success("Undo complete.");
    Ok(())
}
