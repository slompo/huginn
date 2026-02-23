//! Configuration form UI with tui-input

use crate::app::AppState;
use crate::config::ConfigField;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render the configuration form
pub fn render(frame: &mut Frame, app: &mut AppState, area: Rect) {
    let block = Block::default()
        .title(" Configuration ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Create vertical layout for form fields
    let fields = ConfigField::all();
    let constraints: Vec<Constraint> =
        fields.iter().map(|_| Constraint::Length(3)).collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .margin(1)
        .split(inner);

    // Render each field
    for (i, field) in fields.iter().enumerate() {
        let is_focused = app.config_focused_field == *field;

        let border_style = if is_focused {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let field_block = Block::default()
            .title(format!(" {} ", field.label()))
            .borders(Borders::ALL)
            .border_style(border_style);

        let input = app.config_inputs.get(field);
        let value = input.map(|i| i.value()).unwrap_or("");
        let cursor_pos = input.map(|i| i.cursor()).unwrap_or(0);

        // Create the display line with cursor indicator
        let display_text = if is_focused {
            // Show cursor position visually
            let before: String = value.chars().take(cursor_pos).collect();
            let at_cursor: String = value
                .chars()
                .skip(cursor_pos)
                .take(1)
                .collect();
            let after: String = value.chars().skip(cursor_pos + 1).collect();

            if at_cursor.is_empty() {
                // Cursor at end
                Line::styled(
                    format!("{}_", before),
                    Style::default().fg(Color::White),
                )
            } else {
                Line::styled(
                    format!("{}[{}]{}", before, at_cursor, after),
                    Style::default().fg(Color::White),
                )
            }
        } else {
            Line::styled(
                if value.is_empty() { " " } else { value },
                Style::default().fg(Color::Gray),
            )
        };

        let paragraph = Paragraph::new(display_text)
            .block(field_block)
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, chunks[i]);
    }
}
