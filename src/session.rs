//! Session Manager - Handles multiple PTY sessions
//!
//! Manages Shell and AI assistant sessions with the ability to switch between them.

use crate::app::ActiveView;
use crate::config::Config;
use crate::error::{Result, TerminalError};
use crate::pty::PtyManager;
use std::path::PathBuf;
use vt100_ctt::Screen;

/// Manages multiple PTY sessions
pub struct SessionManager {
    /// Shell session PTY
    shell: PtyManager,

    /// AI assistant session PTY
    ai: PtyManager,

    /// Currently active session
    active: ActiveView,
}

impl SessionManager {
    /// Create a new session manager with both Shell and AI PTYs
    /// Both PTYs will start in the given working directory
    pub fn new(config: &Config, cols: u16, rows: u16, cwd: PathBuf) -> Result<Self> {
        // Create shell PTY
        let shell = PtyManager::new(
            &config.shell_command,
            &config.shell_args,
            cols,
            rows,
            Some(cwd.clone()),
        ).map_err(|e| {
            TerminalError::PtyError(format!("Failed to create shell PTY: {}", e))
        })?;

        // Create AI PTY
        let ai = PtyManager::new(
            &config.ai_command,
            &config.ai_args,
            cols,
            rows,
            Some(cwd),
        ).map_err(|e| {
            TerminalError::PtyError(format!("Failed to create AI PTY: {}", e))
        })?;

        Ok(Self {
            shell,
            ai,
            active: ActiveView::Shell,
        })
    }

    /// Set the active session
    pub fn set_active(&mut self, view: ActiveView) {
        self.active = view;
    }

    /// Get the active session
    #[allow(dead_code)]
    pub fn active(&self) -> ActiveView {
        self.active
    }

    /// Get mutable reference to the active PTY
    #[allow(dead_code)]
    pub fn active_pty(&mut self) -> &mut PtyManager {
        match self.active {
            ActiveView::Shell => &mut self.shell,
            ActiveView::Ai => &mut self.ai,
            ActiveView::Config => &mut self.shell, // Default to shell when in config
        }
    }

    /// Get the screen of the active session
    pub fn active_screen(&self) -> &Screen {
        match self.active {
            ActiveView::Shell => self.shell.screen(),
            ActiveView::Ai => self.ai.screen(),
            ActiveView::Config => self.shell.screen(),
        }
    }

    /// Get the screen for a specific view
    #[allow(dead_code)]
    pub fn screen_for(&self, view: ActiveView) -> &Screen {
        match view {
            ActiveView::Shell => self.shell.screen(),
            ActiveView::Ai => self.ai.screen(),
            ActiveView::Config => self.shell.screen(),
        }
    }

    /// Process output from all PTYs
    pub fn process_all(&mut self) {
        self.shell.process_output();
        self.ai.process_output();
    }

    /// Resize all PTYs
    pub fn resize_all(&mut self, cols: u16, rows: u16) {
        self.shell.resize(cols, rows);
        self.ai.resize(cols, rows);
    }

    /// Send input to a specific session
    pub fn send_to(&mut self, view: ActiveView, bytes: &[u8]) -> Result<()> {
        match view {
            ActiveView::Shell => self.shell.send_bytes(bytes),
            ActiveView::Ai => self.ai.send_bytes_tracked(bytes), // Track for first prompt
            ActiveView::Config => Ok(()), // No-op for config view
        }
    }

    /// Get first prompt from AI session
    pub fn get_ai_first_prompt(&self) -> Option<&str> {
        self.ai.get_first_prompt()
    }

    /// Check if AI session has captured first prompt
    #[allow(dead_code)]
    pub fn ai_has_first_prompt(&self) -> bool {
        self.ai.has_first_prompt()
    }

    /// Get AI screen content for progress analysis
    pub fn get_ai_screen_content(&self) -> String {
        crate::summarizer::extract_screen_text(self.ai.screen())
    }

    /// Send pasted text to the active session (with bracketed paste mode)
    pub fn paste_to_active(&mut self, text: &str) -> Result<()> {
        match self.active {
            ActiveView::Shell => self.shell.send_paste(text),
            ActiveView::Ai => self.ai.send_paste(text),
            ActiveView::Config => Ok(()),
        }
    }

    /// Stop all sessions
    pub fn stop_all(&self) {
        self.shell.stop();
        self.ai.stop();
    }

    /// Scroll up in the active session
    pub fn scroll_up(&mut self, n: usize) {
        match self.active {
            ActiveView::Shell => self.shell.scroll_up(n),
            ActiveView::Ai => self.ai.scroll_up(n),
            ActiveView::Config => {}
        }
    }

    /// Scroll down in the active session
    pub fn scroll_down(&mut self, n: usize) {
        match self.active {
            ActiveView::Shell => self.shell.scroll_down(n),
            ActiveView::Ai => self.ai.scroll_down(n),
            ActiveView::Config => {}
        }
    }

    /// Scroll to top in the active session
    pub fn scroll_to_top(&mut self) {
        match self.active {
            ActiveView::Shell => self.shell.scroll_to_top(),
            ActiveView::Ai => self.ai.scroll_to_top(),
            ActiveView::Config => {}
        }
    }

    /// Scroll to bottom in the active session
    pub fn scroll_to_bottom(&mut self) {
        match self.active {
            ActiveView::Shell => self.shell.scroll_to_bottom(),
            ActiveView::Ai => self.ai.scroll_to_bottom(),
            ActiveView::Config => {}
        }
    }

    /// Check if active session is scrolled
    pub fn is_scrolled(&self) -> bool {
        match self.active {
            ActiveView::Shell => self.shell.is_scrolled(),
            ActiveView::Ai => self.ai.is_scrolled(),
            ActiveView::Config => false
        }
    }

    /// Get scroll offset for active session
    pub fn scroll_offset(&self) -> usize {
        match self.active {
            ActiveView::Shell => self.shell.scroll_offset(),
            ActiveView::Ai => self.ai.scroll_offset(),
            ActiveView::Config => 0
        }
    }

    /// Get scrollback text for active session
    pub fn get_scrollback(&self, rows: usize) -> (Vec<String>, usize) {
        match self.active {
            ActiveView::Shell => self.shell.get_scrollback_for_display(rows),
            ActiveView::Ai => self.ai.get_scrollback_for_display(rows),
            ActiveView::Config => (Vec::new(), 0)
        }
    }

    /// Get scrollback line count for active session
    #[allow(dead_code)]
    pub fn scrollback_len(&self) -> usize {
        match self.active {
            ActiveView::Shell => self.shell.scrollback_lines.len(),
            ActiveView::Ai => self.ai.scrollback_lines.len(),
            ActiveView::Config => 0
        }
    }
}
