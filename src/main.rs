//! Huginn CLI - A cognitive terminal multiplexer
//!
//! Huginn is your "thought raven" - a micro-terminal multiplexer that wraps
//! your shell and AI assistants, showing a dynamic TL;DR in a HUD at the top.

mod ai_context;
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
use crate::summarizer::{Summarizer, extract_screen_text, is_summarizer_available, generate_session_title};
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
    let pty_rows = total_rows.saturating_sub(6); // 3 HUD + 1 footer + 2 main view borders

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
                let main_rows = rows.saturating_sub(6) as usize;
                let (scrollback, _count) = sessions.get_scrollback(main_rows);
                app.scrollback_lines = scrollback;
            }
        } else {
            app.scrollback_lines.clear();
        }

        // AI Context tracking
        // 1. Check for first prompt capture in AI session
        if !app.ai_session_started {
            if let Some(prompt) = sessions.get_ai_first_prompt() {
                app.set_first_ai_prompt(prompt);
            }
        }

        // 2. Update AI progress based on screen content
        let screen_content = sessions.get_ai_screen_content();
        // Use simple detection as fallback
        let (_, simple_progress) = ai_context::detect_ai_progress(&screen_content);

        // 3. Generate session title on first prompt (after short delay for screen update)
        if app.generating_title {
            // Small delay to let the terminal update after Enter
            app.generating_title = false;

            // Get screen content from shell
            let shell_screen = sessions.screen_for(app::ActiveView::Shell);
            let screen_text = extract_screen_text(shell_screen);

            // Try to generate title
            let title = generate_session_title(
                &config.summarizer_command,
                &config.summarizer_args,
                None, // No user prompt captured yet
                Some(&screen_text),
            );

            if let Some(t) = title {
                app.session_title = Some(t);
            }
        }

        // Check for summarizer responses
        if let Some(ref summarizer_arc) = summarizer {
            if let Ok(summarizer) = summarizer_arc.try_lock() {
                if let Some(response) = summarizer.try_get_response() {
                    if response.success {
                        // Update AI progress if session is active, otherwise hud_status
                        if app.ai_session_started {
                            app.update_ai_progress(&response.summary);
                        } else {
                            app.hud_status = response.summary;
                        }
                    } else {
                        // On error, use simple detection
                        app.update_ai_progress(&simple_progress);
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
                        // Use "AI" context when AI session is active for better TL;DR
                        let view_context = if app.ai_session_started {
                            "AI".to_string()
                        } else {
                            app.active_view.name().to_string()
                        };
                        if summarizer.request_summary(content, view_context) {
                            app.is_summarizing = true;
                            if force_refresh {
                                if app.ai_session_started {
                                    app.update_ai_progress("Summarizing...");
                                } else {
                                    app.hud_status = "Summarizing...".to_string();
                                }
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

                            // Calculate row relative to main view area (accounting for HUD + border)
                            // HUD: 3 lines, Main view border: 1 line (top border)
                            let offset_y = 4u16; // 3 (HUD) + 1 (main view top border)
                            let row = mouse.row.saturating_sub(offset_y) as usize;
                            // Column offset: 1 for the left border of main view
                            let col = mouse.column.saturating_sub(1) as usize;

                            // Handle mouse selection (works in any mode)
                            match mouse.kind {
                                MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                                    // Start selection
                                    app.start_selection(row, col);
                                    // Set initial selection text (single cell)
                                    let screen = sessions.active_screen();
                                    let text = extract_selection_text(screen, &app.selection);
                                    app.set_selection_text(text);
                                }
                                MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
                                    app.update_selection(row, col);
                                    // Update selection text from screen
                                    let screen = sessions.active_screen();
                                    let text = extract_selection_text(screen, &app.selection);
                                    app.set_selection_text(text);
                                }
                                MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
                                    // Selection complete - copy automatically
                                    let screen = sessions.active_screen();
                                    let text = extract_selection_text(screen, &app.selection);
                                    let len = text.len();
                                    app.set_selection_text(text);
                                    // Auto-copy on mouse release
                                    if len > 0 {
                                        app.copy_selection();
                                        app.selection.clear();
                                    }
                                }
                                MouseEventKind::ScrollUp => {
                                    sessions.scroll_up(3);
                                }
                                MouseEventKind::ScrollDown => {
                                    sessions.scroll_down(3);
                                }
                                _ => {}
                            }
                        }
                        Event::Paste(text) => {
                            // Handle paste - send to active PTY with bracketed paste mode
                            if app.active_view == app::ActiveView::Shell
                                || app.active_view == app::ActiveView::Ai {
                                let _ = sessions.paste_to_active(&text);
                            }
                        }
                        Event::Resize(cols, rows) => {
                            // Resize all PTYs
                            let pty_rows = rows.saturating_sub(6);
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
    // Ctrl+C sends SIGINT to terminal (pass through)
    // Note: Cmd+C is intercepted by Terminal.app and never reaches huginn
    // Copy is done automatically on mouse release instead
    if let KeyCode::Char('c') | KeyCode::Char('C') = key.code {
        let has_ctrl = key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL);
        let has_shift = key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT);
        let has_super = key.modifiers.contains(crossterm::event::KeyModifiers::SUPER);

        // Ctrl+C without Shift/Super passes through to terminal (SIGINT)
        if has_ctrl && !has_shift && !has_super {
            // Let Ctrl+C pass through to send_key_to_session
        }
        // All other 'c' keys pass through normally
    }

    // Handle command mode
    if app.command_mode {
        if let KeyCode::Char(c) = key.code {
            // If ":" pressed again, send ":" to terminal and exit command mode
            if c == ':' {
                app.exit_command_mode();
                let _ = sessions.send_to(app.active_view, b":");
                return KeyEventResult::None;
            }
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

            // Detect first Enter in shell to generate session title
            if key.code == KeyCode::Enter && app.active_view == app::ActiveView::Shell {
                if !app.first_prompt_processed {
                    app.generating_title = true;
                    app.first_prompt_processed = true;
                }
            }

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
                // Ctrl+key sends control character (only for ASCII a-z)
                let c = c.to_ascii_lowercase();
                if c >= 'a' && c <= 'z' {
                    let ctrl_char = (c as u8) - ('a' as u8) + 1;
                    vec![ctrl_char]
                } else if c == 'i' {
                    vec![9] // Tab
                } else if c == 'm' {
                    vec![13] // Enter
                } else {
                    // For non-ASCII chars with Ctrl, just encode as UTF-8
                    let mut buf = [0u8; 4];
                    let s = c.encode_utf8(&mut buf);
                    s.as_bytes().to_vec()
                }
            } else if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) {
                // Alt+key sends ESC followed by the UTF-8 encoded key
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                let mut result = vec![27]; // ESC
                result.extend_from_slice(s.as_bytes());
                result
            } else {
                // Regular character - encode as UTF-8
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                s.as_bytes().to_vec()
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
fn extract_selection_text(screen: &vt100_ctt::Screen, selection: &crate::app::Selection) -> String {
    let bounds = match selection.get_selection_bounds() {
        Some(b) => b,
        None => return String::new(),
    };

    let (start, end) = bounds;
    let (start_row, start_col): (usize, usize) = start;
    let (end_row, end_col): (usize, usize) = end;

    let (_, screen_cols) = screen.size();
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
