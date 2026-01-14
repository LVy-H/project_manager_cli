use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub paths: HashMap<String, String>,
    pub rules: Rules,
    pub organize: Organize,
    pub ctf: CtfConfig,
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

    pub fn resolve_path(&self, key: &str) -> PathBuf {
        // Simple resolution: look up key in paths, else standard path
        if let Some(p) = self.paths.get(key) {
            PathBuf::from(p)
        } else {
            // Fallback if key looks like "projects/CTFs" -> resolve "projects" then join "CTFs"
            if key.contains('/') {
                let parts: Vec<&str> = key.splitn(2, '/').collect();
                if let Some(base) = self.paths.get(parts[0]) {
                    return PathBuf::from(base).join(parts[1]);
                }
            }
            PathBuf::from(key)
        }
    }
}
