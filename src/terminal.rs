//! Terminal setup and teardown wrapper using RAII pattern

use crate::error::{Result, TerminalError};
use crossterm::{
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
    event::{EnableMouseCapture, DisableMouseCapture},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};

/// Wrapper around the terminal that handles setup and cleanup via RAII
pub struct TerminalWrapper {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalWrapper {
    /// Create a new terminal wrapper, entering raw mode and alternate screen
    pub fn new() -> Result<Self> {
        enable_raw_mode().map_err(TerminalError::EnableRawMode)?;

        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .map_err(TerminalError::EnterAlternateScreen)?;

        let backend = CrosstermBackend::new(stdout);
        let terminal =
            Terminal::new(backend).map_err(TerminalError::CreateTerminal)?;

        Ok(Self { terminal })
    }

    /// Draw to the terminal
    pub fn draw<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut ratatui::Frame),
    {
        self.terminal
            .draw(f)
            .map_err(TerminalError::DrawError)?;
        Ok(())
    }

    /// Restore terminal to original state
    pub fn restore(&mut self) -> Result<()> {
        disable_raw_mode().map_err(TerminalError::DisableRawMode)?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)
            .map_err(TerminalError::LeaveAlternateScreen)?;
        Ok(())
    }

    /// Get a reference to the underlying terminal
    #[allow(dead_code)]
    pub fn terminal(&self) -> &Terminal<CrosstermBackend<Stdout>> {
        &self.terminal
    }

    /// Get a mutable reference to the underlying terminal
    #[allow(dead_code)]
    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalWrapper {
    fn drop(&mut self) {
        // Ensure cleanup even on panic
        let _ = self.restore();
    }
}
