//! Huginn CLI - A cognitive terminal multiplexer
//!
//! Huginn is your "thought raven" - a micro-terminal multiplexer that wraps
//! your shell and AI assistants, showing a dynamic TL;DR in a HUD at the top.

mod app;
mod config;
mod error;
mod event;
mod pty;
mod session;
mod summarizer;
mod terminal;
mod ui;

use crate::app::AppState;
use crate::config::Config;
use crate::error::Result;
use crate::event::{Action, EventHandler};
use crate::session::SessionManager;
use crate::summarizer::{Summarizer, extract_screen_text, is_summarizer_available};
use crate::terminal::TerminalWrapper;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind};
use futures::StreamExt;
use std::env;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TICK_RATE: Duration = Duration::from_millis(16); // ~60 FPS for smooth terminal
const SUMMARY_INTERVAL: Duration = Duration::from_secs(30); // Summarize every 30 seconds

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = Config::load_or_default().unwrap_or_else(|e| {
        eprintln!("Warning: Could not load config: {}", e);
        Config::default()
    });

    // Initialize terminal
    let mut terminal = TerminalWrapper::new()?;

    // Get initial terminal size (accounting for HUD + Footer)
    let (total_cols, total_rows) =
        crossterm::terminal::size().unwrap_or((80, 24));
    let pty_cols = total_cols;
    let pty_rows = total_rows.saturating_sub(4); // 3 for HUD + 1 for footer

    // Get current working directory to pass to PTY sessions
    let cwd = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    // Initialize session manager with both Shell and AI PTYs
    let mut sessions = SessionManager::new(&config, pty_cols, pty_rows, cwd)?;

    // Initialize summarizer if available
    let summarizer = if is_summarizer_available(&config.summarizer_command) {
        Some(Arc::new(Mutex::new(
            Summarizer::new(
                &config.summarizer_command,
                &config.summarizer_args,
            )?
        )))
    } else {
        None
    };

    // Initialize app state and event handler
    let mut app = AppState::new(config.clone());
    let event_handler = EventHandler::new(&config);

    // Track last summary time
    let mut last_summary_time = Instant::now();

    // Flag to force summary refresh
    let mut force_refresh = false;

    // Create event stream
    let mut events = EventStream::new();
    let mut tick_interval = tokio::time::interval(TICK_RATE);

    // Main event loop
    while !app.should_quit {
        // Process output from all PTYs
        sessions.process_all();

        // Sync active view with session manager
        sessions.set_active(app.active_view);

        // Update scroll state in app for HUD display
        app.scroll_offset = sessions.scroll_offset();
        app.is_scrolled = sessions.is_scrolled();

        // Get scrollback content if scrolled
        if app.is_scrolled {
            if let Ok((_cols, rows)) = crossterm::terminal::size() {
                let main_rows = rows.saturating_sub(4) as usize;
                let (scrollback, _count) = sessions.get_scrollback(main_rows);
                app.scrollback_lines = scrollback;
            }
        } else {
            app.scrollback_lines.clear();
        }

        // Check for summarizer responses
        if let Some(ref summarizer_arc) = summarizer {
            if let Ok(summarizer) = summarizer_arc.try_lock() {
                if let Some(response) = summarizer.try_get_response() {
                    if response.success {
                        app.hud_status = response.summary;
                    } else {
                        // Don't show error, keep previous status
                    }
                    app.is_summarizing = false;
                }
            }
        }

        // Request new summary if interval elapsed or force refresh
        let should_summarize = force_refresh ||
            (Instant::now().duration_since(last_summary_time) >= SUMMARY_INTERVAL);

        if should_summarize {
            if let Some(ref summarizer_arc) = summarizer {
                if let Ok(summarizer) = summarizer_arc.try_lock() {
                    let content = extract_screen_text(sessions.active_screen());
                    if !content.trim().is_empty() {
                        let view_context = app.active_view.name().to_string();
                        if summarizer.request_summary(content, view_context) {
                            app.is_summarizing = true;
                            if force_refresh {
                                app.hud_status = "Summarizing...".to_string();
                            }
                        }
                    }
                }
            }
            last_summary_time = Instant::now();
            force_refresh = false;
        }

        // Draw UI with the active session's screen
        terminal.draw(|frame| {
            ui::render(frame, &mut app, Some(sessions.active_screen()));
        })?;

        // Handle events with tokio::select!
        tokio::select! {
            // Terminal input events
            maybe_event = events.next() => {
                if let Some(Ok(event)) = maybe_event {
                    match event {
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            let result = handle_key_event(
                                &mut app,
                                &event_handler,
                                key,
                                &mut sessions,
                            );
                            if result == KeyEventResult::ForceRefresh {
                                force_refresh = true;
                            }
                        }
                        Event::Mouse(mouse) => {
                            use crossterm::event::MouseEventKind;

                            // In visual mode, handle selection
                            if app.visual_mode {
                                // Calculate row relative to main view area (accounting for HUD)
                                let hud_height = 3u16;
                                let row = mouse.row.saturating_sub(hud_height) as usize;
                                let col = mouse.column as usize;

                                match mouse.kind {
                                    MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                                        app.start_selection(row, col);
                                    }
                                    MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
                                        app.update_selection(row, col);
                                        // Update selection text from screen
                                        let screen = sessions.active_screen();
                                        let text = extract_selection_text(screen, &app.selection);
                                        app.set_selection_text(text);
                                    }
                                    MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
                                        // Selection complete, keep visual mode active
                                    }
                                    MouseEventKind::ScrollUp => {
                                        sessions.scroll_up(1);
                                    }
                                    MouseEventKind::ScrollDown => {
                                        sessions.scroll_down(1);
                                    }
                                    _ => {}
                                }
                            } else {
                                // Normal mode - handle scroll
                                match mouse.kind {
                                    MouseEventKind::ScrollUp => {
                                        sessions.scroll_up(3);
                                    }
                                    MouseEventKind::ScrollDown => {
                                        sessions.scroll_down(3);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Event::Paste(text) => {
                            // Handle paste - send to active PTY
                            if app.active_view == app::ActiveView::Shell
                                || app.active_view == app::ActiveView::Ai {
                                let _ = sessions.send_to_active(&text);
                            }
                        }
                        Event::Resize(cols, rows) => {
                            // Resize all PTYs
                            let pty_rows = rows.saturating_sub(4);
                            sessions.resize_all(cols, pty_rows);
                        }
                        _ => {}
                    }
                }
            }

            // Tick for periodic updates
            _ = tick_interval.tick() => {
                app.on_tick();
            }
        }
    }

    // Cleanup
    if let Some(summarizer_arc) = summarizer {
        if let Ok(summarizer) = summarizer_arc.try_lock() {
            summarizer.stop();
        }
    }
    sessions.stop_all();
    terminal.restore()?;
    Ok(())
}

/// Result of handling a key event
#[derive(PartialEq)]
enum KeyEventResult {
    None,
    ForceRefresh,
}

/// Handle keyboard input events
fn handle_key_event(
    app: &mut AppState,
    handler: &EventHandler,
    key: KeyEvent,
    sessions: &mut SessionManager,
) -> KeyEventResult {
    // Handle visual mode
    if app.visual_mode {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                app.copy_selection();
                app.exit_visual_mode();
                return KeyEventResult::None;
            }
            KeyCode::Char('v') | KeyCode::Char('V') => {
                app.exit_visual_mode();
                return KeyEventResult::None;
            }
            KeyCode::Esc => {
                app.exit_visual_mode();
                return KeyEventResult::None;
            }
            _ => return KeyEventResult::None,
        }
    }

    // Handle command mode
    if app.command_mode {
        if let KeyCode::Char(c) = key.code {
            app.handle_command(c);
            // Check if 'r' was pressed for refresh
            if c == 'r' || c == 'R' {
                return KeyEventResult::ForceRefresh;
            }
        } else if key.code == KeyCode::Esc {
            app.exit_command_mode();
        }
        return KeyEventResult::None;
    }

    // Check for ':' to enter command mode (only in main views)
    if app.active_view != app::ActiveView::Config {
        if let KeyCode::Char(':') = key.code {
            app.enter_command_mode();
            return KeyEventResult::None;
        }

        // 'v' to enter visual mode
        if let KeyCode::Char('v') | KeyCode::Char('V') = key.code {
            if !key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                app.enter_visual_mode();
                return KeyEventResult::None;
            }
        }

        // Handle scrollback navigation in Shell or AI view
        if app.active_view == app::ActiveView::Shell || app.active_view == app::ActiveView::Ai {
            // Shift+Up/Down for scrollback
            if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                match key.code {
                    KeyCode::Up => {
                        sessions.scroll_up(1);
                        return KeyEventResult::None;
                    }
                    KeyCode::Down => {
                        sessions.scroll_down(1);
                        return KeyEventResult::None;
                    }
                    KeyCode::PageUp => {
                        sessions.scroll_up(10);
                        return KeyEventResult::None;
                    }
                    KeyCode::PageDown => {
                        sessions.scroll_down(10);
                        return KeyEventResult::None;
                    }
                    _ => {}
                }
            }

            // Page Up/Down for scrollback (without shift)
            // Ctrl+U/D also works for scroll
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::Char('u') => {
                        sessions.scroll_up(10);
                        return KeyEventResult::None;
                    }
                    KeyCode::Char('d') => {
                        sessions.scroll_down(10);
                        return KeyEventResult::None;
                    }
                    KeyCode::Char('b') => {
                        sessions.scroll_to_top();
                        return KeyEventResult::None;
                    }
                    KeyCode::Char('f') => {
                        sessions.scroll_to_bottom();
                        return KeyEventResult::None;
                    }
                    _ => {}
                }
            }

            // Home/End for scroll to top/bottom when shifted
            if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                match key.code {
                    KeyCode::Home => {
                        sessions.scroll_to_top();
                        return KeyEventResult::None;
                    }
                    KeyCode::End => {
                        sessions.scroll_to_bottom();
                        return KeyEventResult::None;
                    }
                    _ => {}
                }
            }

            // Pass all other keys to the active PTY
            send_key_to_session(key, app.active_view, sessions);
            return KeyEventResult::None;
        }
    }

    // Get action based on current view
    let action = match app.active_view {
        app::ActiveView::Config => handler.handle_key_config(key),
        _ => Action::None,
    };

    // Handle the action
    if action != Action::None {
        let result = handle_action(app, action);
        return result;
    }

    // Handle text input for config view
    if app.handles_text_input() {
        handle_text_input(app, key);
    }

    KeyEventResult::None
}

/// Send a key event to the active session's PTY
fn send_key_to_session(key: KeyEvent, view: app::ActiveView, sessions: &mut SessionManager) {
    let seq = match key.code {
        KeyCode::Char(c) => {
            // Handle modifiers
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                // Ctrl+key sends control character
                let c = c.to_ascii_lowercase();
                if c >= 'a' && c <= 'z' {
                    let ctrl_char = (c as u8) - ('a' as u8) + 1;
                    vec![ctrl_char]
                } else if c == 'i' {
                    vec![9] // Tab
                } else if c == 'm' {
                    vec![13] // Enter
                } else {
                    vec![c as u8]
                }
            } else if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) {
                // Alt+key sends ESC followed by the key
                vec![27, c as u8]
            } else {
                vec![c as u8]
            }
        }
        KeyCode::Enter => vec![13],       // CR
        KeyCode::Backspace => vec![127],  // DEL
        KeyCode::Tab => vec![9],
        KeyCode::Esc => vec![27],
        KeyCode::Up => vec![27, 91, 65],       // ESC [ A
        KeyCode::Down => vec![27, 91, 66],     // ESC [ B
        KeyCode::Left => vec![27, 91, 68],     // ESC [ D
        KeyCode::Right => vec![27, 91, 67],    // ESC [ C
        KeyCode::Home => vec![27, 79, 72],     // ESC O H
        KeyCode::End => vec![27, 79, 70],      // ESC O F
        KeyCode::PageUp => vec![27, 91, 53, 126],   // ESC [ 5 ~
        KeyCode::PageDown => vec![27, 91, 54, 126], // ESC [ 6 ~
        KeyCode::Delete => vec![27, 91, 51, 126],   // ESC [ 3 ~
        KeyCode::F(n) => match n {
            1 => vec![27, 79, 80],        // ESC O P
            2 => vec![27, 79, 81],        // ESC O Q
            3 => vec![27, 79, 82],        // ESC O R
            4 => vec![27, 79, 83],        // ESC O S
            5..=12 => {
                // F5-F12 use ESC [ n ~
                let n = match n {
                    5 => 15,
                    6 => 17,
                    7 => 18,
                    8 => 19,
                    9 => 20,
                    10 => 21,
                    11 => 23,
                    12 => 24,
                    _ => 15,
                };
                vec![27, 91, (n / 10) + 48, (n % 10) + 48, 126]
            }
            _ => return,
        },
        _ => return,
    };

    let _ = sessions.send_to(view, &seq);
}

/// Handle application actions
fn handle_action(app: &mut AppState, action: Action) -> KeyEventResult {
    match action {
        Action::ToggleView => app.toggle_view(),
        Action::OpenConfig => app.open_config(),
        Action::Quit => app.should_quit = true,
        Action::Back => app.go_back(),
        Action::NextField => app.next_config_field(),
        Action::PrevField => app.prev_config_field(),
        Action::Submit => app.save_config(),
        Action::ForceRefresh => {
            app.hud_status = "Refresh requested...".to_string();
            return KeyEventResult::ForceRefresh;
        }
        Action::None => {}
    }
    KeyEventResult::None
}

/// Handle text input for config form
fn handle_text_input(app: &mut AppState, key: KeyEvent) {
    use tui_input::InputRequest;

    if let Some(input) = app.current_input() {
        match key.code {
            KeyCode::Char(c) => {
                input.handle(InputRequest::InsertChar(c));
            }
            KeyCode::Backspace => {
                input.handle(InputRequest::DeletePrevChar);
            }
            KeyCode::Delete => {
                input.handle(InputRequest::DeleteNextChar);
            }
            KeyCode::Left => {
                input.handle(InputRequest::GoToPrevChar);
            }
            KeyCode::Right => {
                input.handle(InputRequest::GoToNextChar);
            }
            KeyCode::Home => {
                input.handle(InputRequest::GoToStart);
            }
            KeyCode::End => {
                input.handle(InputRequest::GoToEnd);
            }
            _ => {}
        }
    }
}

/// Extract selected text from the screen based on selection bounds
fn extract_selection_text(screen: &vt100::Screen, selection: &crate::app::Selection) -> String {
    let bounds = match selection.get_selection_bounds() {
        Some(b) => b,
        None => return String::new(),
    };

    let (start, end) = bounds;
    let (start_row, start_col): (usize, usize) = start;
    let (end_row, end_col): (usize, usize) = end;

    let (screen_rows, screen_cols) = screen.size();
    let screen_rows = screen_rows as usize;
    let screen_cols = screen_cols as usize;

    let mut text = String::new();

    if start_row == end_row {
        // Single line selection
        for col in start_col..=end_col.min(screen_cols - 1) {
            if let Some(cell) = screen.cell(start_row as u16, col as u16) {
                text.push_str(&cell.contents());
            }
        }
    } else {
        // Multi-line selection
        // First line: from start_col to end
        for col in start_col..screen_cols {
            if let Some(cell) = screen.cell(start_row as u16, col as u16) {
                text.push_str(&cell.contents());
            }
        }
        text.push('\n');

        // Middle lines: full lines
        for row in (start_row + 1)..end_row {
            for col in 0..screen_cols {
                if let Some(cell) = screen.cell(row as u16, col as u16) {
                    text.push_str(&cell.contents());
                }
            }
            text.push('\n');
        }

        // Last line: from start to end_col
        for col in 0..=end_col.min(screen_cols - 1) {
            if let Some(cell) = screen.cell(end_row as u16, col as u16) {
                text.push_str(&cell.contents());
            }
        }
    }

    text
}
