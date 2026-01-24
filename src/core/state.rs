use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct AppState {
    pub current_event_path: Option<PathBuf>,
    pub current_challenge_path: Option<PathBuf>,
}

impl AppState {
    pub fn load() -> Self {
        if let Some(path) = Self::get_state_path() {
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(state) = serde_json::from_str(&content) {
                        return state;
                    }
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(path) = Self::get_state_path() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let content = serde_json::to_string_pretty(self)?;
            fs::write(path, content)?;
        }
        Ok(())
    }

    pub fn set_event(&mut self, path: PathBuf) -> anyhow::Result<()> {
        if !path.exists() {
            anyhow::bail!("Event path does not exist: {:?}", path);
        }
        self.current_event_path = Some(fs::canonicalize(path)?);
        self.save()
    }

    pub fn get_event(&self) -> Option<PathBuf> {
        self.current_event_path.clone().filter(|p| p.exists())
    }

    pub fn clear(&mut self) -> anyhow::Result<()> {
        self.current_event_path = None;
        self.current_challenge_path = None;
        self.save()
    }

    fn get_state_path() -> Option<PathBuf> {
        if let Ok(p) = std::env::var("WARDEX_STATE_FILE") {
            return Some(PathBuf::from(p));
        }
        dirs::data_dir().map(|d| d.join("wardex").join("state.json"))
    }
}
