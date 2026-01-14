use anyhow::Result;
use clap::{Parser, Subcommand};
use log::{error, info, warn};
use std::path::PathBuf;
use wardex::config::Config;
use wardex::core::watcher;
use wardex::engine::{auditor, cleaner, ctf, search, status, undo};

#[derive(Parser)]
#[command(name = "wardex")]
#[command(about = "Ward & index your workspace - CTF management, project organization, and more.", long_about = None)]
struct Cli {
    /// Path to config file (searches ~/.config/wardex/config.yaml if not specified)
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

/// Search for config file in priority order
fn find_config(cli_path: &Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = cli_path {
        if path.exists() {
            return Ok(path.clone());
        } else {
            anyhow::bail!("Config file not found: {:?}", path);
        }
    }

    let mut candidates = Vec::new();

    if let Some(config_dir) = dirs::config_dir() {
        candidates.push(config_dir.join("foldermanager/config.yaml"));
    }
    candidates.push(PathBuf::from("config.yaml"));

    for path in &candidates {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    let searched: Vec<String> = candidates.iter().map(|p| format!("  - {:?}", p)).collect();
    anyhow::bail!(
        "Config file not found. Searched locations:\n{}\n\nUse --config <path> to specify a config file.",
        searched.join("\n")
    );
}

fn main() -> Result<()> {
    // Initialize logger with colored output
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .init();

    let cli = Cli::parse();

    let config_path = find_config(&cli.config)?;
    let config = Config::load_from_file(&config_path)?;

    match &cli.command {
        Commands::Clean { dry_run } => {
            let report = cleaner::clean_inbox(&config, *dry_run)?;

            if report.inbox_not_found {
                error!("Inbox path not found: {:?}", config.resolve_path("inbox"));
                return Ok(());
            }

            if report.inbox_empty {
                warn!("Inbox is empty.");
                return Ok(());
            }

            for item in &report.moved {
                if item.dry_run {
                    info!("Would move {:?} -> {:?}", item.source, item.destination);
                } else {
                    info!(
                        "✓ Moved {:?} -> {:?}",
                        item.source.file_name().unwrap_or_default(),
                        item.destination
                    );
                }
            }

            for item in &report.skipped {
                log::debug!(
                    "Skipped: {:?} ({})",
                    item.path.file_name().unwrap_or_default(),
                    item.reason
                );
            }

            for err in &report.errors {
                error!("{}", err);
            }

            info!(
                "Moved: {}, Skipped: {}, Errors: {}",
                report.moved.len(),
                report.skipped.len(),
                report.errors.len()
            );
        }
        Commands::Ctf { command } => match command {
            CtfCommands::Init { name, date } => {
                let result = ctf::create_event(&config, name, date.clone())?;

                if result.already_exists {
                    error!("Event directory already exists: {:?}", result.event_dir);
                } else {
                    info!("✓ Initialized: {:?}", result.event_dir);
                    info!("  + Categories: {}", result.categories_created.join(", "));
                    info!("  + File: notes.md");
                    info!("  + Metadata: .ctf_meta.json");
                }
            }
            CtfCommands::List => {
                let result = ctf::list_events(&config)?;

                if result.ctf_root_missing {
                    warn!("No CTF directory found.");
                    return Ok(());
                }

                if result.events.is_empty() {
                    warn!("No CTF events found.");
                    return Ok(());
                }

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
                    log::debug!("* Events without metadata file");
                }
            }
        },
        Commands::Audit => {
            info!("Auditing workspace...");
            let report = auditor::audit_workspace(&config)?;

            if report.workspace_not_found {
                error!(
                    "Workspace not found: {:?}",
                    config.resolve_path("workspace")
                );
                return Ok(());
            }

            info!("Analyzed {} items.", report.items_scanned);

            if !report.empty_folders.is_empty() {
                warn!("Empty Folders Found: {}", report.empty_folders.len());
                for p in report.empty_folders.iter().take(10) {
                    println!(" - {:?}", p);
                }
                if report.empty_folders.len() > 10 {
                    println!("... and {} more", report.empty_folders.len() - 10);
                }
            }

            if !report.suspicious_extensions.is_empty() {
                warn!("Suspicious Extensions (Magic Byte Mismatch):");
                for item in &report.suspicious_extensions {
                    println!(
                        " - {:?} (Named: .{}, Real: .{})",
                        item.path, item.declared_ext, item.actual_ext
                    );
                }
            }

            info!("✓ Audit Complete.");
        }
        Commands::Undo { count } => {
            let report = undo::undo_last(&config, *count)?;

            if report.no_log_found {
                warn!("No undo log found.");
                return Ok(());
            }

            if report.log_empty {
                warn!("Undo log is empty.");
                return Ok(());
            }

            info!("Undoing {} operations...", report.undone.len());

            for item in &report.undone {
                if item.success {
                    info!(
                        "✓ Reverted: {:?} -> {:?}",
                        item.source.file_name().unwrap_or_default(),
                        item.destination
                    );
                } else {
                    error!(
                        "✗ Failed: {:?} ({})",
                        item.source.file_name().unwrap_or_default(),
                        item.error.as_deref().unwrap_or("Unknown error")
                    );
                }
            }

            let success_count = report.undone.iter().filter(|i| i.success).count();
            info!(
                "Completed: {}/{} operations",
                success_count,
                report.undone.len()
            );
        }
        Commands::Watch => {
            watcher::watch_inbox(&config)?;
        }
        Commands::Status => {
            info!("Scanning workspace: {:?}", config.resolve_path("workspace"));
            let report = status::show_status(&config)?;

            if report.workspace_not_found {
                error!("Workspace not found.");
                return Ok(());
            }

            if report.repos.is_empty() {
                warn!("No git repositories found.");
                return Ok(());
            }

            println!(
                "\n{:<25} {:<12} {:<15} {}",
                "Project", "State", "Sync", "Path"
            );
            println!("{}", "-".repeat(80));

            for repo in &report.repos {
                let state = if repo.is_dirty {
                    "⚠ Dirty"
                } else {
                    "✓ Clean"
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
            info!(
                "Total: {} repos ({} dirty)",
                report.repos.len(),
                dirty_count
            );
        }
        Commands::Search { path, pattern } => {
            info!("Searching for flags in {:?}...", path);
            let report = search::find_flags(path, pattern.clone())?;

            for m in &report.matches {
                let location = if let Some(ref entry) = m.archive_entry {
                    format!("{} (in {})", entry, m.file_path)
                } else if let Some(line) = m.line_number {
                    format!("{}:{}", m.file_path, line)
                } else {
                    m.file_path.clone()
                };
                info!("✓ {}: {}", location, m.matched_text);
            }

            info!(
                "Scanned {} files, found {} matches.",
                report.files_scanned,
                report.matches.len()
            );

            if !report.errors.is_empty() {
                warn!("{} errors occurred:", report.errors.len());
                for e in report.errors.iter().take(5) {
                    log::debug!("  - {}", e);
                }
            }
        }
    }

    Ok(())
}
