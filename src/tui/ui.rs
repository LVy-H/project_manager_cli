use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

use crate::tui::app::{App, CurrentScreen};

pub fn render(app: &mut App, f: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3), // Tabs
                Constraint::Min(0),    // Content
                Constraint::Length(1), // Footer
            ]
            .as_ref(),
        )
        .split(f.area());

    // Tabs
    let titles = vec!["Dashboard", "Projects", "CTFs"];
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("Wardex"))
        .select(match app.current_screen {
            CurrentScreen::Dashboard => 0,
            CurrentScreen::Projects => 1,
            CurrentScreen::CTFs => 2,
        })
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, chunks[0]);

    // Content
    match app.current_screen {
        CurrentScreen::Dashboard => render_dashboard(app, f, chunks[1]),
        _ => render_placeholder(f, chunks[1]),
    }

    // Footer
    let footer = Paragraph::new(Line::from(vec![
        Span::raw("Press "),
        Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" to quit"),
    ]));
    f.render_widget(footer, chunks[2]);
}

fn render_dashboard(app: &App, f: &mut Frame, area: ratatui::layout::Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(area);

    // Stats Column
    let stats_text = if let Some(stats) = &app.stats {
        vec![
            Line::from(vec![
                Span::raw("Projects: "),
                Span::styled(
                    stats.total_projects.to_string(),
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::from(vec![
                Span::raw("Git Repos: "),
                Span::styled(
                    stats.total_repos.to_string(),
                    Style::default().fg(Color::Blue),
                ),
            ]),
            Line::from(vec![
                Span::raw("CTF Events: "),
                Span::styled(
                    stats.ctf_count.to_string(),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                Span::raw("Total Files: "),
                Span::raw(stats.total_files.to_string()),
            ]),
            Line::from(vec![
                Span::raw("Total Size: "),
                Span::raw(format!(
                    "{:.2} MB",
                    stats.total_size_bytes as f64 / 1024.0 / 1024.0
                )),
            ]),
        ]
    } else {
        vec![Line::from("Loading stats...")]
    };

    let left_block = Paragraph::new(stats_text).block(
        Block::default()
            .title("Workspace Stats")
            .borders(Borders::ALL),
    );
    f.render_widget(left_block, chunks[0]);

    // Git Status Column
    let repo_text: Vec<Line> = if let Some(repos) = &app.repos {
        if repos.is_empty() {
            vec![Line::from("No repositories found.")]
        } else {
            repos
                .iter()
                .take(20) // Limit display
                .map(|r| {
                    let status_style = if r.is_dirty {
                        Style::default().fg(Color::Red)
                    } else {
                        Style::default().fg(Color::Green)
                    };

                    let sync_icon = match r.sync_status {
                        crate::engine::status::SyncStatus::Synced => "✓",
                        crate::engine::status::SyncStatus::Ahead(_) => "↑",
                        crate::engine::status::SyncStatus::Behind(_) => "↓",
                        crate::engine::status::SyncStatus::Diverged { .. } => "↕",
                        _ => "-",
                    };

                    Line::from(vec![
                        Span::styled(
                            format!("{:<20}", r.name),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" "),
                        Span::styled(if r.is_dirty { "DIRTY" } else { "CLEAN" }, status_style),
                        Span::raw(" "),
                        Span::raw(sync_icon),
                    ])
                })
                .collect()
        }
    } else {
        vec![Line::from("Scanning git status...")]
    };

    let right_block =
        Paragraph::new(repo_text).block(Block::default().title("Git Status").borders(Borders::ALL));
    f.render_widget(right_block, chunks[1]);
}

fn render_placeholder(f: &mut Frame, area: ratatui::layout::Rect) {
    let block = Block::default().title("Coming Soon").borders(Borders::ALL);
    f.render_widget(block, area);
}
