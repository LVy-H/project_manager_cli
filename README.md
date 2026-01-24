# Wardex

**Ward & index your workspace** - CTF management, project organization, and more.

## Features

- 📥 **Inbox Sorting** - Auto-organize files using regex rules
- 🔍 **Flag Search** - Hunt CTF flags in files and archives
- 🚩 **CTF Management** - Create and manage competition events
- 📊 **Git Dashboard** - Status of all repos at a glance
- ↩️ **Undo Support** - Safely revert file moves
- 👁️ **Watch Mode** - Real-time inbox monitoring

## Installation

### Nix

```bash
nix run github:LVy-H/wardex
# Or add to your flake inputs
```

### Cargo

```bash
cargo install --path .
```

## Usage

```bash
# Sort inbox items
wardex clean

# Config management
wardex config init        # Initialize config with defaults
wardex config show        # View current settings
wardex config edit        # Edit in $EDITOR
wardex config goto inbox  # Print path (for shell integration)

# Watch inbox in real-time
wardex watch

# CTF event management
wardex ctf init Defcon2025 # Defaults to today's date (auto-activates event)
wardex ctf list
wardex ctf use Defcon2025    # Switch active event context manually
wardex ctf info              # Show current event context
wardex ctf import file.zip   # Smart import (moves file, auto-detects category)
wardex ctf add web/chall1    # Manually add challenge (infers category if in subfolder)
wardex ctf path              # Print path to current event (cd $(wardex ctf path))

# Search for flags
wardex search /path/to/ctf

# Workspace health check
wardex status
wardex audit

# Undo last moves
wardex undo -c 3
```

### Context Awareness & Persistence

Wardex knows where you are and what you're working on.

**1. Context Detection**:
- Run commands from **any subdirectory** (e.g., inside `web/chall1`).
- Wardex automatically walks up the tree to find the event root.

**2. Global State**:
- Wardex remembers your active event globally.
- Switch contexts with `wardex ctf use <event>`.
- Run commands from anywhere (e.g., `~/Downloads`), and they will apply to the active event.

**3. Shell Integration**:
Add this function to your `.bashrc` or `.zshrc` for seamless navigation:

```bash
function ctf() {
    if [ "$1" = "goto" ]; then
        # 'wardex ctf path' defaults to current active event
        cd "$(wardex ctf path "$2" "$3")"
    else
        wardex ctf "$@"
    fi
}
```

Usage: `ctf goto` (to active event) or `ctf goto web/chall1`.

## Configuration

Wardex uses a **layered configuration system** with three priority levels (highest to lowest):

1. **Environment Variables** (override everything)
2. **Config Files** (explicit settings)
3. **Defaults** (fallback values)

### Configuration File Locations

Wardex searches for configuration files in this order:

1. `./config.yaml` (current directory)
2. `~/.config/wardex/config.yaml` (XDG config dir)
3. Built-in defaults if neither exists

Create `~/.config/wardex/config.yaml`:

```yaml
paths:
  workspace: ~/workspace
  inbox: ~/workspace/0_Inbox
  projects: ~/workspace/1_Projects
  areas: ~/workspace/2_Areas
  resources: ~/workspace/3_Resources
  archives: ~/workspace/4_Archives

rules:
  clean:
    - pattern: ".*\\.pdf$"
      target: resources/Documents
    - pattern: ".*\\.zip$"
      target: projects

organize:
  ctf_dir: projects/CTFs

ctf:
  default_categories:
    - web
    - pwn
    - crypto
    - rev
    - misc
```

### Environment Variables

**Override any config path** using `WX_` prefix with uppercase paths:

| Environment Variable | Overrides | Example |
|---------------------|-----------|---------|
| `WX_PATHS_WORKSPACE` | `paths.workspace` | `/tmp/workspace` |
| `WX_PATHS_INBOX` | `paths.inbox` | `~/Downloads` |
| `WX_PATHS_PROJECTS` | `paths.projects` | `~/dev` |
| `WX_PATHS_ARCHIVES` | `paths.archives` | `~/archives` |
| `WX_ORGANIZE_CTF_DIR` | `organize.ctf_dir` | `~/ctfs` |

**Examples:**

```bash
# Temporarily use different workspace
WX_PATHS_WORKSPACE=/tmp/test wardex status

# Override inbox location
WX_PATHS_INBOX=~/Downloads wardex clean

# Use custom CTF directory
WX_ORGANIZE_CTF_DIR=~/sec/ctfs wardex ctf list
```

### Default Paths

If no configuration is provided, Wardex uses these defaults:

- **workspace**: `~/workspace`
- **inbox**: `{workspace}/0_Inbox`
- **projects**: `{workspace}/1_Projects`
- **areas**: `{workspace}/2_Areas`
- **resources**: `{workspace}/3_Resources`
- **archives**: `{workspace}/4_Archives`
- **ctf_root**: `{projects}/CTFs`

## License

MIT
