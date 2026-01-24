use crate::config::Config;
use crate::engine::stats::WorkspaceStats;
use crate::engine::status::RepoStatus;

pub enum CurrentScreen {
    Dashboard,
    Projects,
    CTFs,
}

pub struct App {
    pub config: Config,
    pub current_screen: CurrentScreen,
    pub should_quit: bool,

    // State
    pub stats: Option<WorkspaceStats>,
    pub repos: Option<Vec<RepoStatus>>,
    pub is_loading: bool,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            current_screen: CurrentScreen::Dashboard,
            should_quit: false,
            stats: None,
            repos: None,
            is_loading: false,
        }
    }

    pub fn on_tick(&mut self) {
        // Handle tick events (e.g. spinner animation)
    }
}
