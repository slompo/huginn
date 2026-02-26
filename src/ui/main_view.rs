//! Main view rendering - renders PTY output or welcome screen

use crate::app::{AppState, Selection};
use crate::ui::mascot::{MASCOT, WELCOME_MSG};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::Text,
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use vt100_ctt::Screen;

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

    // Add scroll indicator to title
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
        render_with_scrollback(frame, screen, &app.scrollback_lines, inner, &app.selection);
        return;
    }

    if let Some(screen) = screen {
        render_screen(frame, screen, inner, &app.selection);
    } else {
        // Build welcome message with optional session title
        let mut full_text = format!("{}\n{}", MASCOT, WELCOME_MSG);

        // Add session title if available (for resume context)
        if let Some(ref title) = app.session_title {
            full_text = format!("{}\n\n📋 Retomando: {}", full_text, title);
        }

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
    _selection: &Selection,
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
            let mut col = 0;

            while col < vt_cols.min(display_cols) {
                if let Some(cell) = screen.cell(row as u16, col as u16) {
                    // Skip wide continuation cells
                    if cell.is_wide_continuation() {
                        col += 1;
                        continue;
                    }

                    let ch = cell.contents().to_string();
                    let fg = ansi_to_ratatui_fg_color(cell.fgcolor());
                    let bg = ansi_to_ratatui_bg_color(cell.bgcolor());
                    let mut style = Style::default().fg(fg).bg(bg);

                    if cell.bold() {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    if cell.dim() {
                        style = style.add_modifier(Modifier::DIM);
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

                    let display_ch = if ch.is_empty() { " ".to_string() } else { ch };

                    // For wide characters, add extra space
                    if cell.is_wide() {
                        let span = ratatui::text::Span::styled(
                            format!("{} ", display_ch),
                            style,
                        );
                        spans.push(span);
                        col += 2;
                    } else {
                        let span = ratatui::text::Span::styled(display_ch, style);
                        spans.push(span);
                        col += 1;
                    }
                } else {
                    spans.push(ratatui::text::Span::raw(" "));
                    col += 1;
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
fn render_screen(frame: &mut Frame, screen: &Screen, area: Rect, selection: &Selection) {
    let (screen_rows, screen_cols) = screen.size();
    let screen_rows = screen_rows as usize;
    let screen_cols = screen_cols as usize;

    let display_rows = area.height as usize;
    let display_cols = area.width as usize;

    // Get selection bounds for highlighting
    let selection_bounds = selection.get_selection_bounds();

    let mut lines = Vec::new();

    for row in 0..screen_rows.min(display_rows) {
        let mut spans = Vec::new();
        let mut col = 0;

        while col < screen_cols.min(display_cols) {
            // Check if this cell is in the selection
            let is_selected = is_cell_selected(row, col, &selection_bounds);

            if let Some(cell) = screen.cell(row as u16, col as u16) {
                // Skip wide continuation cells - they're rendered as part of the previous cell
                if cell.is_wide_continuation() {
                    col += 1;
                    continue;
                }

                let ch = cell.contents().to_string();
                let fg = ansi_to_ratatui_fg_color(cell.fgcolor());
                let bg = ansi_to_ratatui_bg_color(cell.bgcolor());
                let mut style = Style::default().fg(fg).bg(bg);

                if cell.bold() {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if cell.dim() {
                    style = style.add_modifier(Modifier::DIM);
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

                // Highlight selected cells
                if is_selected {
                    style = Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD);
                }

                let display_ch = if ch.is_empty() { " ".to_string() } else { ch.clone() };

                // For wide characters, add an extra space to maintain alignment
                if cell.is_wide() {
                    let span = ratatui::text::Span::styled(
                        format!("{} ", display_ch),
                        style,
                    );
                    spans.push(span);
                    col += 2; // Wide chars take 2 columns
                } else {
                    let span = ratatui::text::Span::styled(display_ch, style);
                    spans.push(span);
                    col += 1;
                }
            } else {
                let style = if is_selected {
                    Style::default().fg(Color::Black).bg(Color::Yellow)
                } else {
                    Style::default()
                };
                spans.push(ratatui::text::Span::styled(" ", style));
                col += 1;
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

/// Check if a cell (row, col) is within the selection bounds
fn is_cell_selected(
    row: usize,
    col: usize,
    selection_bounds: &Option<((usize, usize), (usize, usize))>,
) -> bool {
    if let Some((start, end)) = selection_bounds {
        let (start_row, start_col) = *start;
        let (end_row, end_col) = *end;

        if row < start_row || row > end_row {
            return false;
        }

        if row == start_row && row == end_row {
            // Single line selection
            return col >= start_col && col <= end_col;
        }

        if row == start_row {
            // First line of multi-line selection
            return col >= start_col;
        }

        if row == end_row {
            // Last line of multi-line selection
            return col <= end_col;
        }

        // Middle line of multi-line selection
        return true;
    }
    false
}

/// Convert ANSI color to ratatui color (for foreground)
fn ansi_to_ratatui_fg_color(color: vt100_ctt::Color) -> Color {
    match color {
        vt100_ctt::Color::Default => Color::Gray,  // Visible default fg
        vt100_ctt::Color::Idx(i) => match i {
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
        vt100_ctt::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

/// Convert ANSI color to ratatui color (for background)
fn ansi_to_ratatui_bg_color(color: vt100_ctt::Color) -> Color {
    match color {
        vt100_ctt::Color::Default => Color::Reset,  // Transparent bg
        vt100_ctt::Color::Idx(i) => match i {
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
        vt100_ctt::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}
