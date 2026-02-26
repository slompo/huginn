//! LLM Summarizer - Generates context summaries for the HUD
//!
//! Uses a configured LLM (like ollama) to summarize terminal content
//! and provide context-aware status updates.

use crate::error::{Result, TerminalError};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use crossbeam_channel::{Receiver, Sender, bounded};
use vt100_ctt::Screen;

/// Request for summarization
pub struct SummarizeRequest {
    /// Terminal content to summarize
    pub content: String,
    /// Current view context (Shell or AI)
    pub view_context: String,
}

/// Response from summarizer
pub struct SummarizeResponse {
    /// Generated summary
    pub summary: String,
    /// Whether it was successful
    pub success: bool,
}

/// Manages LLM-based summarization
pub struct Summarizer {
    /// Command to run for summarization (e.g., "ollama")
    #[allow(dead_code)]
    command: String,

    /// Arguments for the command (e.g., ["run", "llama3.2"])
    #[allow(dead_code)]
    args: Vec<String>,

    /// Receiver for summarization responses
    response_rx: Receiver<SummarizeResponse>,

    /// Sender for summarization requests
    request_tx: Sender<SummarizeRequest>,

    /// Flag to signal background thread to stop
    running: Arc<AtomicBool>,
}

impl Summarizer {
    /// Create a new summarizer with the given command and args
    pub fn new(command: &str, args: &[String]) -> Result<Self> {
        let (request_tx, request_rx): (Sender<SummarizeRequest>, Receiver<SummarizeRequest>) = bounded(4);
        let (response_tx, response_rx): (Sender<SummarizeResponse>, Receiver<SummarizeResponse>) = bounded(4);

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let command_for_thread = command.to_string();
        let args_for_thread = args.to_vec();

        // Spawn background thread for summarization
        thread::spawn(move || {
            while running_clone.load(Ordering::Relaxed) {
                // Try to receive a request with timeout
                match request_rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(request) => {
                        let result = Self::summarize_sync(
                            &command_for_thread,
                            &args_for_thread,
                            &request.content,
                            &request.view_context,
                        );

                        let response = match result {
                            Ok(summary) => SummarizeResponse {
                                summary,
                                success: true,
                            },
                            Err(e) => SummarizeResponse {
                                summary: format!("Error: {}", e),
                                success: false,
                            },
                        };

                        // Send response (ignore if channel is closed)
                        let _ = response_tx.send(response);
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                        // Timeout, continue loop
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        // Channel closed, exit
                        break;
                    }
                }
            }
        });

        Ok(Self {
            command: command.to_string(),
            args: args.to_vec(),
            response_rx,
            request_tx,
            running,
        })
    }

    /// Request a summarization (non-blocking)
    pub fn request_summary(&self, content: String, view_context: String) -> bool {
        let request = SummarizeRequest {
            content,
            view_context,
        };
        self.request_tx.try_send(request).is_ok()
    }

    /// Try to get the latest response (non-blocking)
    pub fn try_get_response(&self) -> Option<SummarizeResponse> {
        self.response_rx.try_recv().ok()
    }

    /// Check if there's a pending response
    #[allow(dead_code)]
    pub fn has_response(&self) -> bool {
        !self.response_rx.is_empty()
    }

    /// Synchronous summarization (runs in background thread)
    fn summarize_sync(
        command: &str,
        args: &[String],
        content: &str,
        view_context: &str,
    ) -> Result<String> {
        // Limit content length to avoid overwhelming the LLM
        let max_chars = 2000;
        let truncated = if content.len() > max_chars {
            let end = content.char_indices()
                .nth(max_chars)
                .map(|(i, _)| i)
                .unwrap_or(content.len());
            format!("{}...[truncated]", &content[..end])
        } else {
            content.to_string()
        };

        // Create the prompt - different for AI assistant vs shell
        let prompt = if view_context.contains("AI") || view_context.contains("Claude") {
            format!(
                r#"You are analyzing an AI assistant session. Given the terminal output, provide a VERY brief TL;DR (max 8 words) of what the AI is currently doing.

Focus on:
- The main action being performed (editing, reading, running, etc.)
- The target file or component if visible
- The goal of the current operation

Terminal output:
```
{}
```

Provide ONLY the brief TL;DR (e.g., "Fixing HUD status bug in ai_context.rs" or "Implementing copy feature for mouse selection"):"#,
                truncated
            )
        } else {
            format!(
                r#"You are a terminal context analyzer. Given the following terminal output from a {} session, provide a very brief (1-2 sentences max) summary of what's happening. Focus on:
- Current task or command being executed
- Any errors or warnings
- Current directory or project context if visible

Terminal output:
```
{}
```

Provide only the brief summary, no other text:"#,
                view_context,
                truncated
            )
        };

        // Run the LLM command
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                TerminalError::PtyError(format!("Failed to start summarizer: {}", e))
            })?;

        // Send prompt to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes())
                .map_err(|e| {
                    TerminalError::PtyError(format!("Failed to write to summarizer: {}", e))
                })?;
            stdin.flush()
                .map_err(|e| {
                    TerminalError::PtyError(format!("Failed to flush summarizer: {}", e))
                })?;
        }

        // Read response
        let mut output = String::new();
        if let Some(ref mut stdout) = child.stdout {
            stdout.read_to_string(&mut output)
                .map_err(|e| {
                    TerminalError::PtyError(format!("Failed to read from summarizer: {}", e))
                })?;
        }

        // Wait for process to finish
        let _ = child.wait();

        // Clean up the response
        let summary = output.trim()
            .lines()
            .next()
            .unwrap_or("Processing...")
            .to_string();

        // Truncate if too long for HUD
        let max_summary_len = 60;
        if summary.len() > max_summary_len {
            Ok(format!("{}...", &summary[..max_summary_len - 3]))
        } else {
            Ok(summary)
        }
    }

    /// Stop the summarizer
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

impl Drop for Summarizer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Extract visible text content from a VT100 screen
pub fn extract_screen_text(screen: &Screen) -> String {
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
        // Trim trailing spaces and add non-empty lines
        let trimmed = line.trim_end();
        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
    }

    lines.join("\n")
}

/// Check if a summarizer command is available
pub fn is_summarizer_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// Generate a session title from user prompt or screen content
/// Returns None if unable to generate
pub fn generate_session_title(
    command: &str,
    args: &[String],
    user_prompt: Option<&str>,
    screen_content: Option<&str>,
) -> Option<String> {
    // First try with user prompt if available
    if let Some(prompt) = user_prompt {
        if !prompt.trim().is_empty() {
            let title = try_generate_from_prompt(command, args, prompt);
            if let Some(t) = title {
                if t != "NEED_SCREEN" {
                    return Some(t);
                }
            }
        }
    }

    // Fall back to screen content
    if let Some(screen) = screen_content {
        if !screen.trim().is_empty() {
            return try_generate_from_screen(command, args, screen);
        }
    }

    None
}

/// Try to generate title from user prompt
fn try_generate_from_prompt(command: &str, args: &[String], prompt: &str) -> Option<String> {
    let truncated = if prompt.len() > 500 {
        &prompt[..500]
    } else {
        prompt
    };

    let prompt_text = format!(
        r#"Given the user's first prompt below, generate a concise session title (5-8 words) in the format: "VERB + OBJECT + CONTEXT".

Examples of good titles:
- "Fix OAuth2 callback - authentication failing"
- "Implement dark mode toggle - settings UI"
- "Debug vt100 crate - build errors on Linux"
- "Add cargo-dist releases - automated CI/CD"

If you cannot infer a meaningful title from this prompt alone, respond with exactly: NEED_SCREEN

User prompt:
```
{}
```

Session title:"#,
        truncated
    );

    run_llm_command(command, args, &prompt_text)
}

/// Try to generate title from screen content
fn try_generate_from_screen(command: &str, args: &[String], screen: &str) -> Option<String> {
    let truncated = if screen.len() > 2000 {
        &screen[..2000]
    } else {
        screen
    };

    let prompt_text = format!(
        r#"Analyze the terminal screen content below and infer what the user is working on.
Generate a concise session title (5-8 words) in the format: "VERB + OBJECT + CONTEXT".

Examples of good titles:
- "Fixing build errors - Cargo.toml dependencies"
- "Running tests - authentication module"
- "Editing configuration - Huginn settings"

Terminal screen:
```
{}
```

Session title:"#,
        truncated
    );

    run_llm_command(command, args, &prompt_text)
}

/// Run LLM command and get response
fn run_llm_command(command: &str, args: &[String], prompt: &str) -> Option<String> {
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes()).ok()?;
        stdin.flush().ok()?;
    }

    let mut output = String::new();
    if let Some(ref mut stdout) = child.stdout {
        stdout.read_to_string(&mut output).ok()?;
    }

    let _ = child.wait();

    // Clean up response
    let title = output
        .trim()
        .lines()
        .next()
        .unwrap_or("")
        .to_string();

    // Remove quotes if present
    let title = title.trim_matches('"').trim_matches('\'').to_string();

    // Truncate if too long
    if title.len() > 60 {
        Some(format!("{}...", &title[..57]))
    } else if title.is_empty() {
        None
    } else {
        Some(title)
    }
}
