//! Main view rendering - renders PTY output or welcome screen

use crate::app::AppState;
use crate::ui::mascot::{MASCOT, WELCOME_MSG};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::Text,
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use vt100::Screen;

/// Render the main view with PTY screen or welcome message
pub fn render(
    frame: &mut Frame,
    app: &AppState,
    area: Rect,
    screen: Option<&Screen>,
    is_scrolled: bool,
    scroll_offset: usize,
) {
    let view_name = app.active_view.name();

    // Add scroll indicator to title if scrolled
    let title = if is_scrolled {
        format!(" {} [↑ {} lines] ", view_name, scroll_offset)
    } else {
        format!(" {} ", view_name)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if is_scrolled {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // If scrolled, show scrollback at top + current screen at bottom
    if is_scrolled && !app.scrollback_lines.is_empty() {
        render_with_scrollback(frame, screen, &app.scrollback_lines, inner);
        return;
    }

    if let Some(screen) = screen {
        render_screen(frame, screen, inner);
    } else {
        let full_text = format!("{}\n{}", MASCOT, WELCOME_MSG);
        let text = Text::styled(full_text, Style::default().fg(Color::Cyan));
        let paragraph = Paragraph::new(text).alignment(Alignment::Center);
        frame.render_widget(paragraph, inner);
    }
}

/// Render screen with scrollback content at the top
fn render_with_scrollback(
    frame: &mut Frame,
    screen: Option<&Screen>,
    scrollback_lines: &[String],
    area: Rect,
) {
    let display_rows = area.height as usize;
    let display_cols = area.width as usize;

    let scrollback_count = scrollback_lines.len().min(display_rows);
    let screen_rows = display_rows.saturating_sub(scrollback_count);

    let mut all_lines = Vec::new();

    // Add scrollback lines (at top, dimmed)
    for line in scrollback_lines.iter().take(scrollback_count) {
        let truncated: String = line.chars().take(display_cols).collect();
        let padded = format!("{:width$}", truncated, width = display_cols);
        all_lines.push(ratatui::text::Line::styled(
            padded,
            Style::default().fg(Color::DarkGray),
        ));
    }

    // Add current screen content (below scrollback)
    if let Some(screen) = screen {
        let (vt_rows, vt_cols) = screen.size();
        let vt_rows = vt_rows as usize;
        let vt_cols = vt_cols as usize;

        // Start from a position that shows the bottom of the screen
        let start_row = vt_rows.saturating_sub(screen_rows);

        for row in start_row..vt_rows {
            let mut spans = Vec::new();

            for col in 0..vt_cols.min(display_cols) {
                if let Some(cell) = screen.cell(row as u16, col as u16) {
                    let ch = cell.contents().to_string();
                    let fg = ansi_to_ratatui_color(cell.fgcolor());
                    let bg = ansi_to_ratatui_color(cell.bgcolor());
                    let mut style = Style::default().fg(fg).bg(bg);

                    if cell.bold() {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    if cell.italic() {
                        style = style.add_modifier(Modifier::ITALIC);
                    }
                    if cell.underline() {
                        style = style.add_modifier(Modifier::UNDERLINED);
                    }
                    if cell.inverse() {
                        style = style.add_modifier(Modifier::REVERSED);
                    }

                    let span = ratatui::text::Span::styled(
                        if ch.is_empty() { " ".to_string() } else { ch },
                        style,
                    );
                    spans.push(span);
                } else {
                    spans.push(ratatui::text::Span::raw(" "));
                }
            }

            all_lines.push(ratatui::text::Line::from(spans));
        }
    }

    // Fill remaining lines
    while all_lines.len() < display_rows {
        all_lines.push(ratatui::text::Line::default());
    }

    let paragraph = Paragraph::new(all_lines);
    frame.render_widget(paragraph, area);
}

/// Render the VT100 screen content
fn render_screen(frame: &mut Frame, screen: &Screen, area: Rect) {
    let (screen_rows, screen_cols) = screen.size();
    let screen_rows = screen_rows as usize;
    let screen_cols = screen_cols as usize;

    let display_rows = area.height as usize;
    let display_cols = area.width as usize;

    let mut lines = Vec::new();

    for row in 0..screen_rows.min(display_rows) {
        let mut spans = Vec::new();

        for col in 0..screen_cols.min(display_cols) {
            if let Some(cell) = screen.cell(row as u16, col as u16) {
                let ch = cell.contents().to_string();
                let fg = ansi_to_ratatui_color(cell.fgcolor());
                let bg = ansi_to_ratatui_color(cell.bgcolor());
                let mut style = Style::default().fg(fg).bg(bg);

                if cell.bold() {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if cell.italic() {
                    style = style.add_modifier(Modifier::ITALIC);
                }
                if cell.underline() {
                    style = style.add_modifier(Modifier::UNDERLINED);
                }
                if cell.inverse() {
                    style = style.add_modifier(Modifier::REVERSED);
                }

                let span = ratatui::text::Span::styled(
                    if ch.is_empty() { " ".to_string() } else { ch },
                    style,
                );
                spans.push(span);
            } else {
                spans.push(ratatui::text::Span::raw(" "));
            }
        }

        lines.push(ratatui::text::Line::from(spans));
    }

    while lines.len() < display_rows {
        lines.push(ratatui::text::Line::default());
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

/// Convert ANSI color to ratatui color
fn ansi_to_ratatui_color(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(i) => match i {
            0 => Color::Black,
            1 => Color::Red,
            2 => Color::Green,
            3 => Color::Yellow,
            4 => Color::Blue,
            5 => Color::Magenta,
            6 => Color::Cyan,
            7 => Color::Gray,
            8 => Color::DarkGray,
            9 => Color::LightRed,
            10 => Color::LightGreen,
            11 => Color::LightYellow,
            12 => Color::LightBlue,
            13 => Color::LightMagenta,
            14 => Color::LightCyan,
            15 => Color::White,
            _ => Color::Indexed(i),
        },
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}
