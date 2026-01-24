# Wardex Architecture

## Overview

Wardex is a CLI tool designed to help developers and security researchers organize their workspace, manage CTF events, and maintain a disciplined file structure. It follows a modular architecture separating the core CLI interface from the underlying logic engines.

## Core Structure

The application is structured into three main layers:

1.  **CLI Interface (`src/main.rs`)**: Handles command-line argument parsing (using `clap`), logging initialization, and dispatches commands to the appropriate engines.
2.  **Configuration (`src/config.rs`)**: Manages the layered configuration system (Env vars > Config file > Defaults).
3.  **Engines (`src/engine/`)**: Contains the business logic for specific features.
4.  **Core/Utils**: Shared utilities and state management.

## Module Breakdown

### 1. Engine Modules (`src/engine/`)

-   **`auditor.rs`**: Scans the workspace for health issues like empty folders or file extension mismatches (magic byte verification via `infer`).
-   **`cleaner.rs`**: Implements the inbox sorting logic. It uses regex rules defined in `config.yaml` to move files from the Inbox to Projects or Resources.
-   **`ctf.rs`**: Manages Capture The Flag events. It handles creating event directories, importing challenges, and generating writeup templates.
-   **`scaffold.rs`**: Generates boilerplate for new projects (Rust, Python, Node.js).
-   **`search.rs`**: Powered by `ripgrep` (grep-searcher) and `skim` (fuzzy finder). It performs:
    -   **Flag Search**: Recursive search for `flag{...}` patterns in files and archives (zip, tar, gz).
    -   **Project Find**: Fuzzy search for project directories.
    -   **Content Grep**: Fast text search within projects.
-   **`stats.rs`**: Aggregates workspace analytics (file counts, types, size) using parallel iteration (`rayon`).
-   **`status.rs`**: Provides a git dashboard by scanning all repositories in the workspace and reporting their status (dirty, ahead/behind).
-   **`undo.rs`**: Maintains a transaction log of file movements to allow safe reversion of `clean` operations.

### 2. Core Modules (`src/core/`)

-   **`state.rs`**: Manages persistent global state (like the currently active CTF event) stored in `~/.local/share/wardex/state.json` (or similar).
-   **`watcher.rs`**: Implements the real-time file watcher using `notify-debouncer-mini` to trigger inbox cleaning automatically.

### 3. Utilities (`src/utils/`)

-   **`fs.rs`**: A wrapper around file system operations using `fs_extra` and `fs_err` to provide robust error messages and cross-device move support.

## Design Decisions

-   **Parallelism**: `rayon` is used extensively for heavy I/O tasks like workspace auditing and git status checks to ensure high performance.
-   **Error Handling**: `anyhow` is used for application-level error handling, providing context-rich error messages. `fs_err` replaces `std::fs` to give file-path aware errors.
-   **Gitignore Respect**: The `ignore` crate is used for file walking to automatically respect `.gitignore` rules, preventing searches from looking into `target/`, `node_modules/`, etc.
-   **Configuration**: The `config` crate allows flexible configuration via environment variables (e.g., `WX_PATHS_WORKSPACE`), making it easy to adapt to different environments without changing the config file.

## Data Flow

1.  **Command Invocation**: User runs `wardex <command>`.
2.  **Config Load**: `Config::load()` builds the configuration object.
3.  **Dispatch**: `main.rs` calls the relevant function in `src/engine/<module>.rs`.
4.  **Execution**: The engine performs the task, often using `src/utils/fs.rs` for file ops.
5.  **Output**: Results are printed to stdout (for data) or via `log` (for info/errors).

## Future Improvements

-   **TUI Dashboard**: Interactive terminal interface for monitoring workspace status.
-   **Plugin System**: Allow custom scripts for challenge processing.
-   **Container Integration**: Docker management for CTF challenges.
