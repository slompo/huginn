//! Footer with dynamic keyboard shortcuts

use crate::app::{ActiveView, AppState};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Render the footer with context-aware shortcuts
pub fn render(frame: &mut Frame, app: &AppState, area: Rect) {
    let shortcuts = match app.active_view {
        ActiveView::Shell | ActiveView::Ai => {
            if app.command_mode {
                vec![
                    ("t", "Toggle"),
                    ("c", "Config"),
                    ("r", "Refresh"),
                    ("q", "Quit"),
                ]
            } else {
                vec![
                    (":", "Cmd"),
                    ("scroll", "Mouse"),
                ]
            }
        }
        ActiveView::Config => vec![
            ("Tab", "Next"),
            ("Shift+Tab", "Prev"),
            ("Enter", "Save"),
            ("Esc", "Back"),
        ],
    };

    let spans: Vec<Span> = shortcuts
        .iter()
        .flat_map(|(key, action)| {
            vec![
                Span::styled(
                    format!(" [{}]", key),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {} ", action),
                    Style::default().fg(Color::DarkGray),
                ),
            ]
        })
        .collect();

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).alignment(Alignment::Center);

    frame.render_widget(paragraph, area);
}
