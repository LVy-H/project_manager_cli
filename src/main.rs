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
                let result = ctf::create_event(&config, name, date.clone())?;

                if result.already_exists {
                    ui::print_error(&format!(
                        "Event directory already exists: {:?}",
                        result.event_dir
                    ));
                } else {
                    ui::print_success(&format!("Initialized: {:?}", result.event_dir));
                    ui::print_info(&format!(
                        "  + Categories: {}",
                        result.categories_created.join(", ")
                    ));
                    ui::print_info("  + File: notes.md");
                    ui::print_info("  + Metadata: .ctf_meta.json");
                }
            }
            CtfCommands::List => {
                let result = ctf::list_events(&config)?;

                if result.ctf_root_missing {
                    ui::print_warning("No CTF directory found.");
                    return Ok(());
                }

                if result.events.is_empty() {
                    ui::print_warning("No CTF events found.");
                    return Ok(());
                }

                // Print table header
                println!(
                    "{:<30} {:<6} {:<12} {:<10}",
                    "Event", "Year", "Date", "Challenges"
                );
                println!("{}", "-".repeat(60));

                for event in &result.events {
                    let date_str = event.date.as_deref().unwrap_or("-");
                    let meta_indicator = if event.has_metadata { "" } else { "*" };
                    println!(
                        "{:<30} {:<6} {:<12} {:<10}{}",
                        event.name, event.year, date_str, event.challenge_count, meta_indicator
                    );
                }

                if result.events.iter().any(|e| !e.has_metadata) {
                    ui::print_dim("\n* Events without metadata file");
                }
            }
        },
        Commands::Audit => {
            ui::print_info("Auditing workspace...");
            let report = auditor::audit_workspace(&config)?;

            if report.workspace_not_found {
                ui::print_error(&format!(
                    "Workspace not found: {:?}",
                    config.resolve_path("workspace")
                ));
                return Ok(());
            }

            ui::print_info(&format!("Analyzed {} items.", report.items_scanned));

            if !report.empty_folders.is_empty() {
                ui::print_warning(&format!(
                    "\nEmpty Folders Found: {}",
                    report.empty_folders.len()
                ));
                for p in report.empty_folders.iter().take(10) {
                    println!(" - {:?}", p);
                }
                if report.empty_folders.len() > 10 {
                    println!("... and {} more", report.empty_folders.len() - 10);
                }
            }

            if !report.suspicious_extensions.is_empty() {
                ui::print_warning("\nSuspicious Extensions (Magic Byte Mismatch):");
                for item in &report.suspicious_extensions {
                    println!(
                        " - {:?} (Named: .{}, Real: .{})",
                        item.path, item.declared_ext, item.actual_ext
                    );
                }
            }

            ui::print_success("\nAudit Complete.");
        }
        Commands::Undo { count } => {
            let report = undo::undo_last(&config, *count)?;

            if report.no_log_found {
                ui::print_warning("No undo log found.");
                return Ok(());
            }

            if report.log_empty {
                ui::print_warning("Undo log is empty.");
                return Ok(());
            }

            ui::print_info(&format!("Undoing {} operations...", report.undone.len()));

            for item in &report.undone {
                if item.success {
                    ui::print_success(&format!(
                        "Reverted: {:?} -> {:?}",
                        item.source.file_name().unwrap_or_default(),
                        item.destination
                    ));
                } else {
                    ui::print_error(&format!(
                        "Failed: {:?} ({})",
                        item.source.file_name().unwrap_or_default(),
                        item.error.as_deref().unwrap_or("Unknown error")
                    ));
                }
            }

            let success_count = report.undone.iter().filter(|i| i.success).count();
            ui::print_info(&format!(
                "Completed: {}/{} operations",
                success_count,
                report.undone.len()
            ));
        }
        Commands::Watch => {
            watcher::watch_inbox(&config)?;
        }
        Commands::Status => {
            ui::print_info(&format!(
                "Scanning workspace: {:?}",
                config.resolve_path("workspace")
            ));
            let report = status::show_status(&config)?;

            if report.workspace_not_found {
                ui::print_error("Workspace not found.");
                return Ok(());
            }

            if report.repos.is_empty() {
                ui::print_warning("No git repositories found.");
                return Ok(());
            }

            // Print table header
            println!(
                "\n{:<25} {:<12} {:<15} {}",
                "Project", "State", "Sync", "Path"
            );
            println!("{}", "-".repeat(80));

            for repo in &report.repos {
                let state = if repo.is_dirty {
                    format!("⚠ Dirty")
                } else {
                    format!("✓ Clean")
                };
                println!(
                    "{:<25} {:<12} {:<15} {}",
                    repo.name,
                    state,
                    repo.sync_status.display(),
                    repo.path.display()
                );
            }

            let dirty_count = report.repos.iter().filter(|r| r.is_dirty).count();
            ui::print_info(&format!(
                "\nTotal: {} repos ({} dirty)",
                report.repos.len(),
                dirty_count
            ));
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
