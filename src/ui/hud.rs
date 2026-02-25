//! HUD (Head-Up Display) rendering

use crate::app::{ActiveView, AppState};
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

    // Split HUD into left (session name), center (scroll), and right (status)
    // Give more space to right side for AI context status
    let hud_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            ratatui::layout::Constraint::Percentage(30),
            ratatui::layout::Constraint::Percentage(20),
            ratatui::layout::Constraint::Percentage(50),
        ])
        .split(inner);

    // Left side: Session name (AI first prompt TL;DR or View name)
    // Show TL;DR in BOTH views if AI session has started
    let left_content = if app.ai_session_started && !app.ai_first_prompt_tldr.is_empty() {
        // Show TL;DR of first prompt (no label)
        Line::from(vec![
            Span::styled(
                &app.ai_first_prompt_tldr,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        // Default: show view name
        let view_name = app.active_view.name();
        Line::from(vec![
            Span::styled("View: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                view_name,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    };
    let left_para = Paragraph::new(left_content).alignment(Alignment::Left);
    frame.render_widget(left_para, hud_chunks[0]);

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

    // Right side: Status (AI progress shown in BOTH views when AI session active)
    let right_content = if app.ai_session_started && !app.ai_progress.is_empty() {
        // Show AI progress with state-based color in both views
        let progress_color = match app.ai_progress.as_str() {
            "Thinking..." => Color::Magenta,
            "Working..." => Color::Cyan,
            "Error" => Color::Red,
            "Awaiting input" => Color::Yellow,
            _ => Color::Green,
        };
        Line::from(vec![
            Span::styled(
                &app.ai_progress,
                Style::default().fg(progress_color),
            ),
        ])
    } else {
        // Regular status display (when no AI session)
        let status_color = if app.command_mode {
            Color::Magenta
        } else if app.is_summarizing {
            Color::Yellow
        } else {
            Color::Green
        };
        Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &app.hud_status,
                Style::default().fg(status_color),
            ),
        ])
    };
    let right_para = Paragraph::new(right_content).alignment(Alignment::Right);
    frame.render_widget(right_para, hud_chunks[2]);
}
