use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub paths: Paths,
    pub rules: Rules,
    pub organize: Organize,
    pub ctf: CtfConfig,
}

/// Explicit path configuration - no magic string parsing
#[derive(Debug, Deserialize)]
pub struct Paths {
    pub workspace: PathBuf,
    pub inbox: PathBuf,
    pub projects: PathBuf,
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
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path).context("Failed to read config file")?;
        let config: Config =
            serde_yml::from_str(&content).context("Failed to parse config YAML")?;
        Ok(config)
    }

    /// Resolve a path key to an absolute path.
    /// Supports:
    /// - Direct keys: "workspace", "inbox", "projects", "ctf_root"
    /// - Custom paths defined in the config
    /// - Relative paths joined to workspace
    pub fn resolve_path(&self, key: &str) -> PathBuf {
        match key {
            "workspace" => self.paths.workspace.clone(),
            "inbox" => self.paths.inbox.clone(),
            "projects" => self.paths.projects.clone(),
            "ctf_root" => self
                .paths
                .ctf_root
                .clone()
                .unwrap_or_else(|| self.paths.projects.join("CTFs")),
            _ => {
                // Check custom paths
                if let Some(path) = self.paths.custom.get(key) {
                    return PathBuf::from(path);
                }
                // Fallback: treat as relative path from projects
                self.paths.projects.join(key)
            }
        }
    }

    /// Get the CTF root directory
    pub fn ctf_root(&self) -> PathBuf {
        self.paths
            .ctf_root
            .clone()
            .unwrap_or_else(|| self.paths.projects.join("CTFs"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        let yaml = r#"
paths:
  workspace: /home/user/workspace
  inbox: /home/user/workspace/0_Inbox
  projects: /home/user/workspace/1_Projects
  ctf_root: /home/user/workspace/1_Projects/CTFs
rules:
  clean: []
organize:
  ctf_dir: projects/CTFs
ctf:
  default_categories: []
  template_file: null
"#;
        serde_yml::from_str(yaml).unwrap()
    }

    #[test]
    fn test_resolve_path_direct_keys() {
        let config = test_config();

        assert_eq!(
            config.resolve_path("workspace"),
            PathBuf::from("/home/user/workspace")
        );
        assert_eq!(
            config.resolve_path("inbox"),
            PathBuf::from("/home/user/workspace/0_Inbox")
        );
        assert_eq!(
            config.resolve_path("projects"),
            PathBuf::from("/home/user/workspace/1_Projects")
        );
    }

    #[test]
    fn test_resolve_path_ctf_root() {
        let config = test_config();

        assert_eq!(
            config.resolve_path("ctf_root"),
            PathBuf::from("/home/user/workspace/1_Projects/CTFs")
        );
    }

    #[test]
    fn test_resolve_path_fallback_to_projects() {
        let config = test_config();

        // Unknown key should be treated as relative to projects
        assert_eq!(
            config.resolve_path("SomeFolder"),
            PathBuf::from("/home/user/workspace/1_Projects/SomeFolder")
        );
    }

    #[test]
    fn test_ctf_root_helper() {
        let config = test_config();
        assert_eq!(
            config.ctf_root(),
            PathBuf::from("/home/user/workspace/1_Projects/CTFs")
        );
    }
}
