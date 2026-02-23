//! HUD (Head-Up Display) rendering

use crate::app::AppState;
use ratatui::{
    layout::{Alignment, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render the HUD at the top of the screen
pub fn render(frame: &mut Frame, app: &AppState, area: Rect) {
    let block = Block::default()
        .title(" Huginn ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Reset));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split HUD into left (context), center (scroll), and right (status)
    let hud_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            ratatui::layout::Constraint::Percentage(35),
            ratatui::layout::Constraint::Percentage(30),
            ratatui::layout::Constraint::Percentage(35),
        ])
        .split(inner);

    // Left side: View context
    let view_name = app.active_view.name();
    let context = Line::from(vec![
        Span::styled("View: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            view_name,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    let context_para = Paragraph::new(context).alignment(Alignment::Left);
    frame.render_widget(context_para, hud_chunks[0]);

    // Center: Scroll indicator
    if app.is_scrolled {
        let scroll_indicator = Line::from(vec![
            Span::styled("↑ ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("Scrolled: {} lines", app.scroll_offset),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(" ↑", Style::default().fg(Color::Cyan)),
        ]);
        let scroll_para = Paragraph::new(scroll_indicator).alignment(Alignment::Center);
        frame.render_widget(scroll_para, hud_chunks[1]);
    }

    // Right side: Status
    let status_color = if app.command_mode {
        Color::Magenta
    } else if app.is_summarizing {
        Color::Yellow
    } else {
        Color::Green
    };
    let status = Line::from(vec![
        Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            &app.hud_status,
            Style::default().fg(status_color),
        ),
    ]);
    let status_para = Paragraph::new(status).alignment(Alignment::Right);
    frame.render_widget(status_para, hud_chunks[2]);
}
