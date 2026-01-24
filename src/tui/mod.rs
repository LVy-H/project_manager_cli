pub mod app;
pub mod event;
pub mod ui;
pub mod update;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

use crate::config::Config;
use crate::tui::app::App;
use crate::tui::event::{DataEvent, Event, EventHandler};
use crate::tui::update::update;
use std::thread;

pub fn run(config: &Config) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and event handler
    let mut app = App::new(config.clone());
    let events = EventHandler::new(250); // 250ms tick rate

    // Spawn background tasks
    let sender = events.sender.clone();
    let config_clone = config.clone();
    thread::spawn(move || {
        if let Ok(stats) = crate::engine::stats::get_stats(&config_clone) {
            sender.send(Event::Data(DataEvent::Stats(stats))).ok();
        }
    });

    let sender = events.sender.clone();
    let config_clone = config.clone();
    thread::spawn(move || {
        if let Ok(report) = crate::engine::status::show_status(&config_clone) {
            sender
                .send(Event::Data(DataEvent::GitStatus(report.repos)))
                .ok();
        }
    });

    // Main loop
    let res = run_app(&mut terminal, &mut app, &events);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    events: &EventHandler,
) -> Result<()> {
    loop {
        terminal.draw(|f| crate::tui::ui::render(app, f))?;

        match events.next()? {
            Event::Key(key) => {
                if let Some(action) = update(app, key) {
                    if action == crate::tui::update::Action::Quit {
                        break;
                    }
                }
            }
            Event::Data(data) => match data {
                DataEvent::Stats(stats) => app.stats = Some(stats),
                DataEvent::GitStatus(repos) => app.repos = Some(repos),
            },
            Event::Tick => {}
            _ => {}
        }
    }
    Ok(())
}
