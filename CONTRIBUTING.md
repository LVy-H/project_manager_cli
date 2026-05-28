# Contributing to Wardex

Thank you for your interest in contributing to Wardex! This guide will help you understand the project structure and development workflow.

## Project Structure

```
wardex/
├── src/
│   ├── main.rs           # CLI entry point and command routing (clap)
│   ├── lib.rs            # Module exports
│   ├── config.rs         # Layered config: files → env vars → defaults
│   ├── output.rs         # Display formatting for command reports
│   ├── core/
│   │   ├── state.rs      # Global CTF context (~/.local/share/wardex/)
│   │   ├── templates.rs  # Solve script templates (pwn, web, generic)
│   │   └── watcher.rs    # Real-time inbox file monitoring
│   ├── engine/
│   │   ├── cleaner.rs    # Inbox sorting via regex rules
│   │   ├── scaffold.rs   # Project scaffolding (rust, python, node)
│   │   ├── auditor.rs    # Workspace health checks
│   │   ├── status.rs     # Git dashboard (parallel repo scanning)
│   │   ├── search.rs     # Flag search, fuzzy find, content grep
│   │   ├── stats.rs      # Workspace analytics
│   │   ├── undo.rs       # Transaction log for reversible file moves
│   │   └── ctf/          # CTF-specific modules
│   │       ├── mod.rs        # CtfMeta struct, re-exports
│   │       ├── event.rs      # Event lifecycle (create, list, schedule, finish)
│   │       ├── challenge.rs  # Challenge add/solve/status/writeup
│   │       ├── import.rs     # Smart archive import with category detection
│   │       ├── archive.rs    # Event archival and zip creation
│   │       ├── resolve.rs    # Fuzzy path resolution
│   │       └── shelve.rs     # Interactive shelve workflow
│   ├── tui/              # Optional TUI dashboard (feature-gated)
│   └── utils/
│       └── fs.rs         # Cross-device file moves via fs_extra
├── tests/
│   └── cli_integration.rs  # Integration tests (assert_cmd + tempfile)
├── docs/                 # Architecture, design, RFCs, and planning docs
├── README.md             # User documentation
├── CHANGELOG.md          # Version history and migration guides
└── CONTRIBUTING.md       # You are here!
```

Planning docs for upcoming work live in [`docs/plan/README.md`](docs/plan/README.md).

## Engine Modules

Each module in `src/engine/` implements a specific feature domain:

| Module | Purpose | Key Types/Functions |
|--------|---------|---------------------|
| `cleaner.rs` | Inbox automation | `clean_inbox()` |
| `ctf/event.rs` | Event lifecycle | `create_event()`, `list_events()`, `finish_event()` |
| `ctf/challenge.rs` | Challenge management | `add_challenge()`, `solve_challenge()`, `challenge_status()` |
| `ctf/shelve.rs` | Interactive shelve flow | `shelve_challenge()` |
| `ctf/import.rs` | Smart archive import | `import_challenge()` |
| `ctf/archive.rs` | Event archival | `archive_event()` |
| `ctf/resolve.rs` | Fuzzy path resolution | `resolve_event()`, `resolve_challenge()` |
| `search.rs` | Flag detection | `search_flags()`, `scan_archives()` |
| `status.rs` | Git dashboard | `git_status_all()` |
| `auditor.rs` | Workspace audit | `audit_workspace()` |
| `stats.rs` | Workspace analytics | `workspace_stats()` |
| `scaffold.rs` | Project scaffolding | `scaffold_project()` |
| `undo.rs` | Safety net | `track_move()`, `revert_operations()` |

## Architecture

### Configuration System

Wardex uses a **three-tier configuration system**:

1. **Environment Variables** (`WX_*`) - Runtime overrides
2. **Config Files** (`config.yaml`) - User preferences
3. **Defaults** - Sensible fallbacks

Implementation: `src/config.rs`
- `Config::load()` - Merges all layers
- `resolve_path()` - Path resolution with variable substitution

### Command Flow

```
User Command → main.rs → Engine Module → Config → Filesystem
                 ↓
              Clap CLI parsing
                 ↓
            Match subcommand
                 ↓
          Call engine function
```

Example: `wardex ctf import challenge.zip`

1. `main.rs` parses command with Clap
2. Routes to `engine::ctf::import_challenge()`
3. Function reads config, validates paths
4. Performs file operations
5. Updates metadata (`.ctf_meta.json`)

## Design Docs And RFCs

Wardex uses lightweight design docs for CLI decisions:

- [`docs/CLI_DESIGN.md`](docs/CLI_DESIGN.md) defines the command design rules
- [`docs/rfcs/README.md`](docs/rfcs/README.md) explains when to write an RFC

Please use the RFC process for major CLI, shell integration, workflow, or command-semantics changes.

## Development Workflow

### Setting Up

```bash
# Clone and build
git clone <repo-url>
cd wardex
cargo build

# Run tests
cargo test

# Run linter
cargo clippy

# Install locally
cargo install --path .
```

### Adding a New Feature

**Example: Adding a new CTF command**

1. **Add to CLI** (`src/main.rs`):
   ```rust
   #[derive(Subcommand)]
   enum CtfCommand {
       // existing commands...
       Stats,  // new command
   }
   
   // In match arm:
   CtfCommand::Stats => engine::ctf::show_stats(&config)?,
   ```

2. **Implement in Engine** (e.g., `src/engine/ctf/event.rs` or a new file in `src/engine/ctf/`):
   ```rust
   pub fn show_stats(config: &Config) -> Result<()> {
       // Implementation
       Ok(())
   }
   ```

3. **Add Tests**:
   ```rust
   #[cfg(test)]
   mod tests {
       #[test]
       fn test_show_stats() {
           // Test implementation
       }
   }
   ```

4. **Update Documentation**: Add usage example to README.md

### Code Style

- Use `cargo fmt` before committing
- Follow Rust naming conventions (snake_case for functions, CamelCase for types)
- Add error context with `.context()` from anyhow
- Prefer descriptive error messages with actionable tips

**Example:**
```rust
// ❌ Bad
if !path.exists() {
    bail!("File not found");
}

// ✅ Good
if !path.exists() {
    bail!(
        "Challenge file not found: {:?}\n\n\
        Please verify the file path is correct.",
        path
    );
}
```

### Error Handling

All public functions return `Result<T>` (using `anyhow::Result`):

```rust
use anyhow::{Result, Context};

pub fn my_function(path: &Path) -> Result<()> {
    let content = fs::read_to_string(path)
        .context("Failed to read file")?;
    
    // ... process content
    
    Ok(())
}
```

### Testing

Run tests before submitting a PR:

```bash
# All tests
cargo test

# Specific module
cargo test ctf

# With output
cargo test -- --nocapture
```

## Cachix Binary Cache (maintainers)

CI builds the flake on every push and pushes the resulting `cargoArtifacts`
+ `wardex` derivations to the `wardex` cache on Cachix. Cold-cache `nix
build` for users (and for the maintainer on a fresh machine) then drops
from ~5 min to a few seconds of fetch.

### One-time setup (when the cache doesn't exist yet)

1. Sign in at [cachix.org](https://app.cachix.org) (free for OSS).
2. Create a cache named `wardex`. Cachix will display a *public signing
   key* (e.g. `wardex.cachix.org-1:…`).
3. Generate a *write auth token* under cache settings → "Auth tokens".
4. In the GitHub repo: **Settings → Secrets → Actions**, add
   `CACHIX_AUTH_TOKEN` with the value from step 3.
5. (Optional, recommended) advertise the cache to end users by adding
   `nixConfig` to `flake.nix`:

   ```nix
   nixConfig = {
     extra-substituters = [ "https://wardex.cachix.org" ];
     extra-trusted-public-keys = [ "wardex.cachix.org-1:..." ];  # public key from step 2
   };
   ```

   With this in place, `nix run github:LVy-H/wardex` fetches the prebuilt
   binary on cold cache (after the user accepts the flake-config trust
   prompt — that's normal Nix behavior).

### Using the cache locally without `nixConfig`

Until step 5 is done (or for users who don't want to trust the per-flake
config), they can wire the cache up globally with one command:

```bash
cachix use wardex
```

This appends the substituter + public key to their user-level
`nix.conf`. Subsequent `nix build` commands fetch from the cache.

## Pull Request Guidelines

1. **One feature per PR** - Keep changes focused
2. **Descriptive commits** - Explain the "why", not just the "what"
3. **Update documentation** - README, PREVIEW, or this file if applicable
4. **Pass CI checks** - Tests, clippy, and formatting
5. **Provide context** - Explain your motivation and approach

## Questions?

Feel free to open an issue for:
- Feature proposals
- Bug reports  
- Architecture questions
- Documentation improvements

Happy contributing! 🚀
