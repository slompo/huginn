//! UI rendering module

mod config_ui;
mod footer;
mod hud;
mod main_view;
mod mascot;

use crate::app::{ActiveView, AppState};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};
use vt100_ctt::Screen;

/// Content to render in the main view
#[allow(dead_code)]
pub struct ViewContent {
    /// Current screen content
    pub screen: &'static Screen,
    /// Whether the view is scrolled
    pub is_scrolled: bool,
    /// Scroll offset in lines
    pub scroll_offset: usize,
    /// Scrollback history as text lines (when scrolled)
    pub scrollback_lines: Vec<String>,
}

/// Main render function that composes all UI components
pub fn render(frame: &mut Frame, app: &mut AppState, screen: Option<&Screen>) {
    // Create the main layout: HUD | Main | Footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // HUD (3 lines)
            Constraint::Min(10),    // Main view (flexible)
            Constraint::Length(1),  // Footer (1 line)
        ])
        .split(frame.area());

    // Render HUD at the top
    hud::render(frame, app, chunks[0]);

    // Render main view based on active view
    match app.active_view {
        ActiveView::Shell | ActiveView::Ai => {
            main_view::render(frame, app, chunks[1], screen, app.is_scrolled, app.scroll_offset);
        }
        ActiveView::Config => {
            config_ui::render(frame, app, chunks[1]);
        }
    }

    // Render footer at the bottom
    footer::render(frame, app, chunks[2]);
}
