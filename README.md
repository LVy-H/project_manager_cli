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

# Watch inbox in real-time
wardex watch

# CTF event management
wardex ctf init Defcon2025
wardex ctf list

# Search for flags
wardex search /path/to/ctf

# Workspace health check
wardex status
wardex audit

# Undo last moves
wardex undo -c 3
```

## Configuration

Create `~/.config/wardex/config.yaml`:

```yaml
paths:
  workspace: ~/workspace
  inbox: ~/workspace/0_Inbox
  projects: ~/workspace/1_Projects

rules:
  clean:
    - pattern: ".*\\.pdf$"
      target: projects/Documents

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

Override config with `WX_` prefix:

```bash
WX_PATHS_WORKSPACE=/tmp/test wardex status
```

## License

MIT
