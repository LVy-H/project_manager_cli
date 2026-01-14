# FolderManager

A powerful CLI tool to keep your workspace organized, built in Rust.

## Features

- **Clean**: Automatically sorts items from Inbox into Projects/Resources based on regex rules
- **CTF**: Manage Capture The Flag events with scaffolding and organization
- **Audit**: Scans workspace for empty folders and suspicious file extensions (magic byte mismatch)
- **Status**: Git repository dashboard showing dirty/clean state and sync status
- **Watch**: File watcher for auto-sorting Inbox items
- **Search**: Flag finder with support for searching inside archives (zip, tar, tar.gz)
- **Undo**: Revert file movement operations

## Installation

```bash
cd rust_src
cargo build --release
```

The binary will be at `target/release/folder_manager`.

## Usage

```bash
# Clean your Inbox (dry run)
folder_manager clean --dry-run

# Clean for real
folder_manager clean

# Initialize a CTF event
folder_manager ctf init "EventName" --date 2026-01-14

# List CTF events
folder_manager ctf list

# Audit workspace health
folder_manager audit

# Show git status dashboard
folder_manager status

# Watch Inbox for changes and auto-sort
folder_manager watch

# Search for CTF flags recursively
folder_manager search ./path --pattern "flag\{.*\}"

# Undo last N operations
folder_manager undo --count 2
```

## Configuration

Create a `config.yaml` in the working directory:

```yaml
paths:
  workspace: /path/to/workspace
  inbox: /path/to/workspace/0_Inbox
  projects: /path/to/workspace/1_Projects

rules:
  clean:
    - pattern: "(?i)ctf.*"
      target: projects/CTFs
    - pattern: ".*\\.pdf$"
      target: resources/Documents

organize:
  ctf_dir: projects/CTFs

ctf:
  default_categories:
    - Web
    - Pwn
    - Crypto
    - Rev
    - Misc
  template_file: null
```

## License

MIT
