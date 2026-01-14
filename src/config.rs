use anyhow::{Context, Result};
use config::{Config as ConfigBuilder, Environment, File, FileFormat};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub paths: Paths,
    pub rules: Rules,
    pub organize: Organize,
    pub ctf: CtfConfig,
}

/// Explicit path configuration
#[derive(Debug, Deserialize)]
pub struct Paths {
    pub workspace: PathBuf,
    pub inbox: Option<PathBuf>,
    pub projects: Option<PathBuf>,
    pub areas: Option<PathBuf>,
    pub resources: Option<PathBuf>,
    pub archives: Option<PathBuf>,
    /// Explicit CTF root path (optional, defaults to projects/CTFs)
    pub ctf_root: Option<PathBuf>,
    /// Additional custom paths for rules
    #[serde(flatten)]
    pub custom: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct Rules {
    pub clean: Vec<CleanRule>,
}

#[derive(Debug, Deserialize)]
pub struct CleanRule {
    pub pattern: String,
    pub target: String,
}

#[derive(Debug, Deserialize)]
pub struct Organize {
    pub ctf_dir: String,
}

#[derive(Debug, Deserialize)]
pub struct CtfConfig {
    pub default_categories: Vec<String>,
    pub template_file: Option<String>,
}

impl Config {
    /// Load configuration from multiple sources (layered):
    /// 1. Default config file in current directory
    /// 2. User config in ~/.config/wardex/
    /// 3. Environment variables with WX_ prefix
    pub fn load() -> Result<Self> {
        let mut builder = ConfigBuilder::builder();

        // 1. Current directory config.yaml
        builder = builder.add_source(File::new("config", FileFormat::Yaml).required(false));

        // 2. XDG config directory
        if let Some(config_dir) = dirs::config_dir() {
            let path = config_dir.join("wardex/config.yaml");
            if path.exists() {
                builder =
                    builder.add_source(File::from(path).format(FileFormat::Yaml).required(false));
            }
        }

        // 3. Environment variables with WX_ prefix
        // e.g., WX_PATHS_WORKSPACE=/tmp/workspace
        builder = builder.add_source(
            Environment::with_prefix("WX")
                .separator("_")
                .try_parsing(true),
        );

        let config = builder.build().context("Failed to build config")?;
        config
            .try_deserialize()
            .context("Failed to deserialize config")
    }

    /// Load from a specific file path
    pub fn load_from_file(path: &std::path::Path) -> Result<Self> {
        let builder = ConfigBuilder::builder()
            .add_source(File::from(path.to_path_buf()).format(FileFormat::Yaml))
            .add_source(
                Environment::with_prefix("WX")
                    .separator("_")
                    .try_parsing(true),
            );

        let config = builder.build().context("Failed to build config")?;
        config
            .try_deserialize()
            .context("Failed to deserialize config")
    }

    /// Resolve a path key to an absolute path.
    pub fn resolve_path(&self, key: &str) -> PathBuf {
        match key {
            "workspace" => self.paths.workspace.clone(),
            "inbox" => self
                .paths
                .inbox
                .clone()
                .unwrap_or_else(|| self.paths.workspace.join("0_Inbox")),
            "projects" => self
                .paths
                .projects
                .clone()
                .unwrap_or_else(|| self.paths.workspace.join("1_Projects")),
            "areas" => self
                .paths
                .areas
                .clone()
                .unwrap_or_else(|| self.paths.workspace.join("2_Areas")),
            "resources" => self
                .paths
                .resources
                .clone()
                .unwrap_or_else(|| self.paths.workspace.join("3_Resources")),
            "archives" => self
                .paths
                .archives
                .clone()
                .unwrap_or_else(|| self.paths.workspace.join("4_Archives")),
            "ctf_root" => self.ctf_root(),
            _ => {
                // Check custom paths
                if let Some(path) = self.paths.custom.get(key) {
                    return PathBuf::from(path);
                }
                // Fallback: treat as relative path from projects
                self.resolve_path("projects").join(key)
            }
        }
    }

    /// Get the CTF root directory
    pub fn ctf_root(&self) -> PathBuf {
        self.paths
            .ctf_root
            .clone()
            .unwrap_or_else(|| self.resolve_path("projects").join("CTFs"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_config() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
paths:
  workspace: /home/user/workspace
rules:
  clean: []
organize:
  ctf_dir: projects/CTFs
ctf:
  default_categories: []
  template_file: null
"#
        )
        .unwrap();
        file
    }

    #[test]
    fn test_load_from_file() {
        let file = create_test_config();
        let config = Config::load_from_file(file.path()).unwrap();

        assert_eq!(
            config.paths.workspace,
            PathBuf::from("/home/user/workspace")
        );
    }

    #[test]
    fn test_resolve_path_direct_keys() {
        let file = create_test_config();
        let config = Config::load_from_file(file.path()).unwrap();

        assert_eq!(
            config.resolve_path("workspace"),
            PathBuf::from("/home/user/workspace")
        );
        assert_eq!(
            config.resolve_path("inbox"),
            PathBuf::from("/home/user/workspace/0_Inbox")
        );
    }

    #[test]
    fn test_resolve_path_ctf_root() {
        let file = create_test_config();
        let config = Config::load_from_file(file.path()).unwrap();

        assert_eq!(
            config.resolve_path("ctf_root"),
            PathBuf::from("/home/user/workspace/1_Projects/CTFs")
        );
    }

    #[test]
    fn test_ctf_root_helper() {
        let file = create_test_config();
        let config = Config::load_from_file(file.path()).unwrap();

        assert_eq!(
            config.ctf_root(),
            PathBuf::from("/home/user/workspace/1_Projects/CTFs")
        );
    }
}
