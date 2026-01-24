use crate::tui::app::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(PartialEq)]
pub enum Action {
    None,
    Quit,
}

pub fn update(_app: &mut App, key_event: KeyEvent) -> Option<Action> {
    match key_event.code {
        KeyCode::Char('q') | KeyCode::Esc => Some(Action::Quit),
        KeyCode::Char('c') => {
            if key_event.modifiers.contains(KeyModifiers::CONTROL) {
                Some(Action::Quit)
            } else {
                None
            }
        }
        _ => None,
    }
}
