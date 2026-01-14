use anyhow::Result;
use clap::{Parser, Subcommand};
use folder_manager::config::Config;
use folder_manager::core::watcher;
use folder_manager::engine::{auditor, cleaner, ctf, search, status, undo};
use folder_manager::utils::ui;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "foldermanager")]
#[command(about = "A powerful CLI tool to keep your workspace organized.", long_about = None)]
struct Cli {
    /// Path to config file (searches ~/.config/foldermanager/config.yaml if not specified)
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Sort items from Inbox into Projects/Resources
    Clean {
        #[arg(long, help = "Simulate moves without executing")]
        dry_run: bool,
    },
    /// Manage CTF events
    Ctf {
        #[command(subcommand)]
        command: CtfCommands,
    },
    /// Audit workspace health (files, empty folders)
    Audit,
    /// Undo last movement operation
    Undo {
        #[arg(short, long, default_value_t = 1)]
        count: usize,
    },
    /// Watch Inbox and auto-sort
    Watch,
    /// Show git status dashboard
    Status,
    /// Search for flags recursively
    Search {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(short, long)]
        pattern: Option<String>,
    },
}

#[derive(Subcommand)]
enum CtfCommands {
    /// Initialize a new CTF event
    Init {
        name: String,
        #[arg(long, help = "YYYY-MM-DD")]
        date: Option<String>,
    },
    /// List CTF events
    List,
}

/// Search for config file in priority order:
/// 1. CLI argument (if provided)
/// 2. $XDG_CONFIG_HOME/foldermanager/config.yaml
/// 3. ~/.config/foldermanager/config.yaml
/// 4. ./config.yaml (current directory)
fn find_config(cli_path: &Option<PathBuf>) -> Result<PathBuf> {
    // 1. CLI argument takes priority
    if let Some(path) = cli_path {
        if path.exists() {
            return Ok(path.clone());
        } else {
            anyhow::bail!("Config file not found: {:?}", path);
        }
    }

    // Build list of candidate paths
    let mut candidates = Vec::new();

    // 2. XDG_CONFIG_HOME / ~/.config
    if let Some(config_dir) = dirs::config_dir() {
        candidates.push(config_dir.join("foldermanager/config.yaml"));
    }

    // 3. Current directory
    candidates.push(PathBuf::from("config.yaml"));

    // Find first existing path
    for path in &candidates {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    // None found, print helpful message
    let searched: Vec<String> = candidates.iter().map(|p| format!("  - {:?}", p)).collect();
    anyhow::bail!(
        "Config file not found. Searched locations:\n{}\n\nUse --config <path> to specify a config file.",
        searched.join("\n")
    );
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Find config file
    let config_path = find_config(&cli.config)?;
    let config = Config::load_from_file(&config_path)?;

    match &cli.command {
        Commands::Clean { dry_run } => {
            let report = cleaner::clean_inbox(&config, *dry_run)?;

            if report.inbox_not_found {
                ui::print_error(&format!(
                    "Inbox path not found: {:?}",
                    config.resolve_path("inbox")
                ));
                return Ok(());
            }

            if report.inbox_empty {
                ui::print_warning("Inbox is empty.");
                return Ok(());
            }

            // Display moved items
            for item in &report.moved {
                if item.dry_run {
                    ui::print_info(&format!(
                        "Would move {:?} -> {:?}",
                        item.source, item.destination
                    ));
                } else {
                    ui::print_success(&format!(
                        "Moved {:?} -> {:?}",
                        item.source.file_name().unwrap_or_default(),
                        item.destination
                    ));
                }
            }

            // Display skipped items
            for item in &report.skipped {
                ui::print_dim(&format!(
                    "Skipped: {:?} ({})",
                    item.path.file_name().unwrap_or_default(),
                    item.reason
                ));
            }

            // Display errors
            for err in &report.errors {
                ui::print_error(err);
            }

            ui::print_info(&format!(
                "\nMoved: {}, Skipped: {}, Errors: {}",
                report.moved.len(),
                report.skipped.len(),
                report.errors.len()
            ));
        }
        Commands::Ctf { command } => match command {
            CtfCommands::Init { name, date } => {
                ctf::create_event(&config, name, date.clone())?;
            }
            CtfCommands::List => {
                ctf::list_events(&config)?;
            }
        },
        Commands::Audit => {
            auditor::audit_workspace(&config)?;
        }
        Commands::Undo { count } => {
            undo::undo_last(&config, *count)?;
        }
        Commands::Watch => {
            watcher::watch_inbox(&config)?;
        }
        Commands::Status => {
            status::show_status(&config)?;
        }
        Commands::Search { path, pattern } => {
            ui::print_info(&format!("Searching for flags in {:?}...", path));
            let report = search::find_flags(path, pattern.clone())?;

            // Display matches
            for m in &report.matches {
                let location = if let Some(ref entry) = m.archive_entry {
                    format!("{} (in {})", entry, m.file_path)
                } else if let Some(line) = m.line_number {
                    format!("{}:{}", m.file_path, line)
                } else {
                    m.file_path.clone()
                };
                ui::print_success(&format!("{}: {}", location, m.matched_text));
            }

            // Summary
            ui::print_info(&format!(
                "\nScanned {} files, found {} matches.",
                report.files_scanned,
                report.matches.len()
            ));

            if !report.errors.is_empty() {
                ui::print_warning(&format!("{} errors occurred:", report.errors.len()));
                for e in report.errors.iter().take(5) {
                    ui::print_dim(&format!("  - {}", e));
                }
            }
        }
    }

    Ok(())
}
