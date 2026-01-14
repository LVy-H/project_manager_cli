use anyhow::Result;
use clap::{Parser, Subcommand};
use folder_manager::config::Config;
use folder_manager::core::{auditor, cleaner, ctf, status, undo, watcher};
use folder_manager::utils::ui;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "foldermanager")]
#[command(about = "A powerful CLI tool to keep your workspace organized.", long_about = None)]
struct Cli {
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load config (naive assumption about location for now, e.g., current dir or ~/.config)
    // We'll look in current dir for now as per Python behavior usually, or specific path
    // The Python one seemed to assume config.yaml is in the package or current dir?
    // Let's assume it's in the parent directory for development (project root)
    // or we search for it.

    // For this migration, we hardcode looking at ../config.yaml relative to run location
    // or just expects config.yaml in CWD.
    let config_path = PathBuf::from("config.yaml");
    if !config_path.exists() {
        ui::print_error("config.yaml not found in current directory.");
        return Ok(());
    }

    let config = Config::load_from_file(&config_path)?;

    match &cli.command {
        Commands::Clean { dry_run } => {
            cleaner::clean_inbox(&config, *dry_run)?;
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
    }

    Ok(())
}
