//! PTY (Pseudoterminal) Manager
//!
//! Handles spawning and communicating with shell processes.

use crate::error::{Result, TerminalError};
use portable_pty::{CommandBuilder, PtyPair, PtySize};
use std::env;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use crossbeam_channel::{Receiver, Sender, bounded};
use vt100::Parser;

/// Maximum scrollback lines to keep
const MAX_SCROLLBACK_LINES: usize = 10000;

/// PTY Manager that handles shell process and terminal emulation
pub struct PtyManager {
    /// The PTY pair (master + slave)
    #[allow(dead_code)]
    pair: PtyPair,

    /// Writer to send input to the shell
    writer: Box<dyn Write + Send>,

    /// VT100 parser for interpreting terminal output
    parser: Parser,

    /// Receiver for PTY output from background thread
    output_rx: Receiver<Vec<u8>>,

    /// Flag to signal reader thread to stop
    running: Arc<AtomicBool>,

    /// Scrollback buffer - stores rendered lines that scrolled off
    pub scrollback_lines: Vec<String>,

    /// Current scroll position (0 = at bottom, >0 = scrolled up by N lines)
    scroll_offset: usize,

    /// Terminal dimensions
    terminal_height: u16,
    terminal_width: u16,

    /// Previous screen content for comparison
    prev_screen_content: Vec<String>,
}

impl PtyManager {
    /// Create a new PTY manager with the given shell command and size
    pub fn new(
        shell_cmd: &str,
        shell_args: &[String],
        cols: u16,
        rows: u16,
        cwd: Option<PathBuf>,
    ) -> Result<Self> {
        let pty_system = portable_pty::native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| {
                TerminalError::PtyError(format!("Failed to open PTY: {}", e))
            })?;

        let mut cmd = CommandBuilder::new(shell_cmd);
        for arg in shell_args {
            cmd.arg(arg);
        }

        let working_dir = cwd.unwrap_or_else(|| {
            env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        });
        cmd.cwd(working_dir);

        let _child = pair.slave.spawn_command(cmd).map_err(|e| {
            TerminalError::PtyError(format!("Failed to spawn shell: {}", e))
        })?;

        let writer = pair.master.take_writer().map_err(|e| {
            TerminalError::PtyError(format!("Failed to get PTY writer: {}", e))
        })?;

        let mut reader = pair.master.try_clone_reader().map_err(|e| {
            TerminalError::PtyError(format!("Failed to get PTY reader: {}", e))
        })?;

        let parser = Parser::new(rows, cols, 0);
        let running = Arc::new(AtomicBool::new(true));
        let (output_tx, output_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = bounded(64);

        let running_clone = running.clone();
        thread::spawn(move || {
            let mut buf = [0u8; 8192];
            while running_clone.load(Ordering::Relaxed) {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        if output_tx.send(data).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        if e.kind() != std::io::ErrorKind::WouldBlock {
                            thread::sleep(Duration::from_millis(10));
                        }
                    }
                }
            }
        });

        Ok(Self {
            pair,
            writer,
            parser,
            output_rx,
            running,
            scrollback_lines: Vec::new(),
            scroll_offset: 0,
            terminal_height: rows,
            terminal_width: cols,
            prev_screen_content: Vec::new(),
        })
    }

    /// Process available output from the PTY
    pub fn process_output(&mut self) -> bool {
        let mut processed = false;

        while let Ok(data) = self.output_rx.try_recv() {
            // Before processing, save current screen state
            let old_screen = self.extract_screen_content();

            self.parser.process(&data);
            processed = true;

            // After processing, compare and save scrolled lines
            let new_screen = self.extract_screen_content();
            self.detect_and_save_scrolled_content(&old_screen, &new_screen);
        }

        processed
    }

    /// Extract current screen content as a vector of lines
    fn extract_screen_content(&self) -> Vec<String> {
        let screen = self.parser.screen();
        let (rows, cols) = screen.size();

        let mut lines = Vec::new();
        for row in 0..rows {
            let mut line = String::new();
            for col in 0..cols {
                if let Some(cell) = screen.cell(row, col) {
                    let ch = cell.contents();
                    if !ch.is_empty() {
                        line.push_str(&ch);
                    } else {
                        line.push(' ');
                    }
                } else {
                    line.push(' ');
                }
            }
            lines.push(line.trim_end().to_string());
        }
        lines
    }

    /// Detect content that scrolled off and save it
    fn detect_and_save_scrolled_content(&mut self, old_screen: &[String], new_screen: &[String]) {
        if old_screen.is_empty() || new_screen.is_empty() {
            return;
        }

        // Check if the screen scrolled (top line changed, content moved up)
        // Look for old content in new screen to detect scroll amount
        let old_top = old_screen.first().map(|s| s.as_str()).unwrap_or("");
        let new_second = new_screen.get(1).map(|s| s.as_str()).unwrap_or("");

        // If old top line appears at position 1 in new screen, we scrolled by 1
        if !old_top.is_empty() && old_top == new_second {
            // Save the line that scrolled off
            if let Some(line) = old_screen.first() {
                if !line.is_empty() {
                    self.scrollback_lines.push(line.clone());

                    // Limit scrollback size
                    if self.scrollback_lines.len() > MAX_SCROLLBACK_LINES {
                        self.scrollback_lines.remove(0);
                    }
                }
            }
        }

        // Also check for multiple line scroll by looking for pattern matches
        for (i, old_line) in old_screen.iter().enumerate() {
            if i > 0 && i < new_screen.len() {
                if old_line == &new_screen[i] && !old_line.is_empty() {
                    // Lines above this in old_screen may have scrolled off
                    for j in 0..i {
                        if !old_screen[j].is_empty() {
                            // Check if not already added
                            if self.scrollback_lines.last() != Some(&old_screen[j]) {
                                self.scrollback_lines.push(old_screen[j].clone());

                                if self.scrollback_lines.len() > MAX_SCROLLBACK_LINES {
                                    self.scrollback_lines.remove(0);
                                }
                            }
                        }
                    }
                    break;
                }
            }
        }

        self.prev_screen_content = new_screen.to_vec();
    }

    /// Scroll up by n lines
    pub fn scroll_up(&mut self, n: usize) {
        let max_scroll = self.max_scroll_offset();
        self.scroll_offset = (self.scroll_offset + n).min(max_scroll);
    }

    /// Scroll down by n lines
    pub fn scroll_down(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    /// Scroll to top
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = self.max_scroll_offset();
    }

    /// Scroll to bottom
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    /// Get maximum scroll offset
    fn max_scroll_offset(&self) -> usize {
        // Can scroll up to the number of scrollback lines
        self.scrollback_lines.len()
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn is_scrolled(&self) -> bool {
        self.scroll_offset > 0
    }

    /// Send input to the shell
    #[allow(dead_code)]
    pub fn send_input(&mut self, input: &str) -> Result<()> {
        write!(self.writer, "{}", input)
            .map_err(|e| TerminalError::PtyError(format!("Write error: {}", e)))?;
        self.writer
            .flush()
            .map_err(|e| TerminalError::PtyError(format!("Flush error: {}", e)))?;
        Ok(())
    }

    /// Send a raw byte sequence to the shell
    pub fn send_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer.write_all(bytes).map_err(|e| {
            TerminalError::PtyError(format!("Write error: {}", e))
        })?;
        self.writer
            .flush()
            .map_err(|e| TerminalError::PtyError(format!("Flush error: {}", e)))?;
        // Reset scroll to bottom when user types
        self.scroll_offset = 0;
        Ok(())
    }

    /// Resize the terminal
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.parser.set_size(rows, cols);
        self.terminal_height = rows;
        self.terminal_width = cols;

        let _ = self.pair.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    }

    pub fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }

    /// Get scrollback content for display
    /// Returns (scrollback_lines, current_screen_lines) based on scroll position
    pub fn get_scrollback_for_display(&self, screen_rows: usize) -> (Vec<String>, usize) {
        if self.scroll_offset == 0 || self.scrollback_lines.is_empty() {
            return (Vec::new(), 0);
        }

        // How many scrollback lines to show
        let scrollback_count = self.scroll_offset.min(screen_rows).min(self.scrollback_lines.len());

        // Get lines from the end of scrollback
        let start = self.scrollback_lines.len().saturating_sub(scrollback_count);
        let lines: Vec<String> = self.scrollback_lines[start..].to_vec();

        (lines, scrollback_count)
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

impl Drop for PtyManager {
    fn drop(&mut self) {
        self.stop();
    }
}
