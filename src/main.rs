use anyhow::Result;
use clap::{Parser, Subcommand};
use log::{error, info, warn};
use std::path::PathBuf;
use wardex::config::Config;
use wardex::core::watcher;
use wardex::engine::{auditor, cleaner, ctf, scaffold, search, stats, status, undo};

#[derive(Parser)]
#[command(name = "wardex")]
#[command(version)]
#[command(about = "Ward & index your workspace - CTF management, project organization, and more.", long_about = None)]
struct Cli {
    #[arg(long, value_name = "FILE", help = "Path to config file")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Initialize config file with defaults
    Init {
        #[arg(long, help = "Force overwrite if config exists")]
        force: bool,
    },
    /// Show current configuration
    Show,
    /// Edit config in $EDITOR
    Edit,
    /// Navigate to workspace folders (prints path for shell integration)
    Goto {
        #[arg(
            help = "Folder to navigate to: workspace|inbox|projects|areas|resources|archives|ctf"
        )]
        folder: String,
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
    /// smart import a challenge archive
    Import {
        #[arg(help = "Path to challenge zip/tar")]
        file: PathBuf,
        #[arg(
            short,
            long,
            help = "Category (web, pwn, etc.) - skips interactive prompt"
        )]
        category: Option<String>,
    },
    /// Add a new challenge to current event
    Add {
        #[arg(help = "Category/Name (e.g. pwn/stack-buffer)")]
        path: String,
    },
    /// Generate writeup from notes
    Writeup,
    /// Archive an event to 4_Archives
    Archive {
        #[arg(help = "Name of the event to archive")]
        name: String,
    },
    /// Print path to CTF event or challenge (for shell integration)
    Path {
        #[arg(help = "Event name (optional, defaults to current context or latest)")]
        event: Option<String>,
        #[arg(help = "Challenge name (optional)")]
        challenge: Option<String>,
    },
    /// Show current CTF context info
    Info,
    /// Set the current active event context globally
    Use {
        #[arg(help = "Event name/path to activate")]
        event: String,
    },
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new project
    Init {
        #[arg(short, long, help = "Project type (rust, python, node)")]
        type_: String,
        #[arg(short, long, help = "Project name")]
        name: String,
    },
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
    /// Fuzzy find projects
    Find { name: String },
    /// Grep Content in Projects/Resources
    Grep { pattern: String },
    /// Show workspace analytics
    Stats,
    /// Quick file/project info
    Info { path: Option<PathBuf> },
    /// Manage configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
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
        candidates.push(config_dir.join("wardex/config.yaml"));
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
        Commands::Init { type_, name } => {
            scaffold::init_project(&config, name, type_)?;
        }
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
            CtfCommands::Import { file, category } => {
                ctf::import_challenge(&config, file, category.clone())?;
            }
            CtfCommands::Add { path } => {
                ctf::add_challenge(&config, path)?;
            }
            CtfCommands::Writeup => {
                ctf::generate_writeup(&config)?;
            }
            CtfCommands::Archive { name } => {
                ctf::archive_event(&config, name)?;
            }
            CtfCommands::Path { event, challenge } => {
                let path = ctf::get_event_path(&config, event.as_deref(), challenge.as_deref())?;
                println!("{}", path.display());
            }
            CtfCommands::Info => {
                ctf::get_context_info(&config)?;
            }
            CtfCommands::Use { event } => {
                ctf::set_active_event(&config, event)?;
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
                println!("✓ {}: {}", location, m.matched_text);
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
        Commands::Find { name } => {
            let results = search::find_project(&config, name)?;
            if results.is_empty() {
                warn!("No projects found matching '{}'", name);
            } else {
                println!("{:<50} {:<10}", "Project Path", "Score");
                println!("{}", "-".repeat(60));
                for res in results.iter().take(10) {
                    println!("{:<50} {}", res.path.display(), res.score);
                }
            }
        }
        Commands::Grep { pattern } => {
            info!("Grepping in Projects & Resources...");
            let matches = search::content_search(&config, pattern)?;

            for m in &matches {
                println!(
                    "{}:{}: {}",
                    m.file_path,
                    m.line_number.unwrap_or(0),
                    m.matched_text
                );
            }
            info!("Found {} matches.", matches.len());
        }
        Commands::Stats => {
            let stats = stats::get_stats(&config)?;
            stats::print_stats(&stats);
        }
        Commands::Info { path } => {
            let target = path.clone().unwrap_or_else(|| PathBuf::from("."));
            info!("Info for: {:?}", target);
            if target.exists() {
                let meta = fs_err::metadata(&target)?;
                println!(
                    "Type: {:?}",
                    if target.is_dir() { "Directory" } else { "File" }
                );
                println!("Size: {} bytes", meta.len());
                println!("Modified: {:?}", meta.modified()?);
            } else {
                error!("Path not found");
            }
        }
        Commands::Config { command } => {
            handle_config_command(&config, command)?;
        }
    }

    Ok(())
}

fn handle_config_command(config: &Config, command: &ConfigCommands) -> Result<()> {
    match command {
        ConfigCommands::Init { force } => {
            let config_path = dirs::config_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?
                .join("wardex");

            fs_err::create_dir_all(&config_path)?;
            let config_file = config_path.join("config.yaml");

            if config_file.exists() && !force {
                anyhow::bail!(
                    "Config file already exists at {:?}\n\n\
                    Use --force to overwrite",
                    config_file
                );
            }

            let default_config = r#"paths:
  workspace: ~/workspace
  inbox: ~/workspace/0_Inbox
  projects: ~/workspace/1_Projects
  areas: ~/workspace/2_Areas
  resources: ~/workspace/3_Resources
  archives: ~/workspace/4_Archives

organize:
  ctf_dir: projects/CTFs

ctf:
  default_categories:
    - web
    - pwn
    - crypto
    - rev
    - misc
"#;

            fs_err::write(&config_file, default_config)?;
            println!("✓ Config initialized at: {:?}", config_file);
            println!("\nEdit with: wardex config edit");
        }

        ConfigCommands::Show => {
            println!("📋 Current Configuration\n");
            println!("Paths:");
            println!("  workspace:  {:?}", config.resolve_path("workspace"));
            println!("  inbox:      {:?}", config.resolve_path("inbox"));
            println!("  projects:   {:?}", config.resolve_path("projects"));
            println!("  areas:      {:?}", config.resolve_path("areas"));
            println!("  resources:  {:?}", config.resolve_path("resources"));
            println!("  archives:   {:?}", config.resolve_path("archives"));
            println!("  ctf_root:   {:?}", config.ctf_root());
        }

        ConfigCommands::Edit => {
            let config_file = dirs::config_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?
                .join("wardex/config.yaml");

            if !config_file.exists() {
                anyhow::bail!(
                    "Config file not found at {:?}\n\n\
                    Initialize with: wardex config init",
                    config_file
                );
            }

            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

            std::process::Command::new(&editor)
                .arg(&config_file)
                .status()?;

            println!("✓ Config edited. Changes will apply on next wardex command.");
        }

        ConfigCommands::Goto { folder } => {
            let path = match folder.as_str() {
                "workspace" => config.resolve_path("workspace"),
                "inbox" => config.resolve_path("inbox"),
                "projects" => config.resolve_path("projects"),
                "areas" => config.resolve_path("areas"),
                "resources" => config.resolve_path("resources"),
                "archives" => config.resolve_path("archives"),
                "ctf" => config.ctf_root(),
                _ => anyhow::bail!(
                    "Unknown folder: {}\n\n\
                    Available: workspace, inbox, projects, areas, resources, archives, ctf",
                    folder
                ),
            };

            if !path.exists() {
                eprintln!("Warning: Path does not exist: {:?}", path);
            }

            println!("{}", path.display());
        }
    }

    Ok(())
}
