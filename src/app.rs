//! Application state and state machine logic

use crate::config::{args_to_string, Config, ConfigField};
use std::collections::HashMap;
use tui_input::Input;

/// Current active view in the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveView {
    /// Shell view (default terminal)
    #[default]
    Shell,
    /// AI assistant view
    Ai,
    /// Configuration screen
    Config,
}

impl ActiveView {
    /// Get a human-readable name for the view
    pub fn name(&self) -> &'static str {
        match self {
            ActiveView::Shell => "Shell",
            ActiveView::Ai => "AI Assistant",
            ActiveView::Config => "Configuration",
        }
    }
}

/// Selection state for visual mode
#[derive(Debug, Clone, Default)]
pub struct Selection {
    /// Start position (row, col)
    pub start: Option<(usize, usize)>,
    /// End position (row, col)
    pub end: Option<(usize, usize)>,
    /// Selected text
    pub text: String,
}

impl Selection {
    pub fn is_active(&self) -> bool {
        self.start.is_some()
    }

    pub fn clear(&mut self) {
        self.start = None;
        self.end = None;
        self.text.clear();
    }

    pub fn set_start(&mut self, row: usize, col: usize) {
        self.start = Some((row, col));
        self.end = Some((row, col));
    }

    pub fn set_end(&mut self, row: usize, col: usize) {
        self.end = Some((row, col));
    }

    /// Get selection bounds (normalized: start <= end)
    pub fn get_selection_bounds(&self) -> Option<((usize, usize), (usize, usize))> {
        let start = self.start?;
        let end = self.end?;

        // Normalize so start <= end
        if start.0 < end.0 || (start.0 == end.0 && start.1 <= end.1) {
            Some((start, end))
        } else {
            Some((end, start))
        }
    }
}

/// Main application state
pub struct AppState {
    /// Loaded configuration
    pub config: Config,

    /// Currently active view
    pub active_view: ActiveView,

    /// Flag to signal the app should quit
    pub should_quit: bool,

    /// Currently focused field in config form
    pub config_focused_field: ConfigField,

    /// Input buffers for config form fields
    pub config_inputs: HashMap<ConfigField, Input>,

    /// HUD context message
    pub hud_context: String,

    /// HUD status message
    pub hud_status: String,

    /// Flag indicating a summary is being processed
    pub is_summarizing: bool,

    /// Command mode - waiting for command after ':'
    pub command_mode: bool,

    /// Scroll position for current view (0 = bottom)
    pub scroll_offset: usize,

    /// Whether the view is scrolled (not at bottom)
    pub is_scrolled: bool,

    /// Scrollback content for rendering when scrolled
    pub scrollback_lines: Vec<String>,

    /// Current text selection
    pub selection: Selection,

    /// First prompt sent to AI assistant (for HUD session name)
    pub ai_first_prompt: Option<String>,

    /// TL;DR of first AI prompt (truncated for display)
    pub ai_first_prompt_tldr: String,

    /// Current AI execution progress (for HUD status)
    pub ai_progress: String,

    /// Flag indicating AI session has received first input
    pub ai_session_started: bool,
}

impl AppState {
    /// Create a new app state with the given configuration
    pub fn new(config: Config) -> Self {
        let mut config_inputs = HashMap::new();

        // Initialize input fields from config values
        config_inputs.insert(
            ConfigField::ShellCommand,
            Input::new(config.shell_command.clone()),
        );
        config_inputs.insert(
            ConfigField::ShellArgs,
            Input::new(args_to_string(&config.shell_args)),
        );
        config_inputs.insert(
            ConfigField::AiCommand,
            Input::new(config.ai_command.clone()),
        );
        config_inputs.insert(
            ConfigField::AiArgs,
            Input::new(args_to_string(&config.ai_args)),
        );
        config_inputs.insert(
            ConfigField::SummarizerCommand,
            Input::new(config.summarizer_command.clone()),
        );
        config_inputs.insert(
            ConfigField::SummarizerArgs,
            Input::new(args_to_string(&config.summarizer_args)),
        );

        Self {
            config,
            active_view: ActiveView::default(),
            should_quit: false,
            config_focused_field: ConfigField::ShellCommand,
            config_inputs,
            hud_context: "Welcome to Huginn".to_string(),
            hud_status: "Ready".to_string(),
            is_summarizing: false,
            command_mode: false,
            scroll_offset: 0,
            is_scrolled: false,
            scrollback_lines: Vec::new(),
            selection: Selection::default(),
            ai_first_prompt: None,
            ai_first_prompt_tldr: String::new(),
            ai_progress: "Idle".to_string(),
            ai_session_started: false,
        }
    }

    /// Toggle between Shell and AI views
    pub fn toggle_view(&mut self) {
        self.active_view = match self.active_view {
            ActiveView::Shell => ActiveView::Ai,
            ActiveView::Ai => ActiveView::Shell,
            ActiveView::Config => ActiveView::Shell,
        };
        self.update_hud_for_view();
    }

    /// Open the configuration screen
    pub fn open_config(&mut self) {
        self.active_view = ActiveView::Config;
        self.hud_context = "Configuration".to_string();
        self.hud_status = "Edit settings".to_string();
    }

    /// Go back from current view
    pub fn go_back(&mut self) {
        if self.active_view == ActiveView::Config {
            self.active_view = ActiveView::Shell;
            self.update_hud_for_view();
        }
    }

    /// Move to next config field
    pub fn next_config_field(&mut self) {
        self.config_focused_field = self.config_focused_field.next();
    }

    /// Move to previous config field
    pub fn prev_config_field(&mut self) {
        self.config_focused_field = self.config_focused_field.prev();
    }

    /// Get the currently focused input
    pub fn current_input(&mut self) -> Option<&mut Input> {
        self.config_inputs.get_mut(&self.config_focused_field)
    }

    /// Save config from form values
    pub fn save_config(&mut self) {
        // Extract values from inputs
        let values: HashMap<ConfigField, String> = self
            .config_inputs
            .iter()
            .map(|(field, input)| (*field, input.value().to_string()))
            .collect();

        // Update config
        self.config.update_from_form(&values);

        // Persist to disk
        if let Err(e) = self.config.save() {
            self.hud_status = format!("Error saving config: {}", e);
        } else {
            self.hud_status = "Config saved".to_string();
        }

        // Return to shell view
        self.active_view = ActiveView::Shell;
        self.update_hud_for_view();
    }

    /// Update HUD based on current view
    fn update_hud_for_view(&mut self) {
        self.hud_context = match self.active_view {
            ActiveView::Shell => "Shell Mode".to_string(),
            ActiveView::Ai => "AI Assistant".to_string(),
            ActiveView::Config => "Configuration".to_string(),
        };
        self.hud_status = "Ready".to_string();
    }

    /// Handle tick event (for periodic updates)
    pub fn on_tick(&mut self) {
        // Placeholder for future periodic updates
    }

    /// Check if we're in a form view that handles text input
    pub fn handles_text_input(&self) -> bool {
        self.active_view == ActiveView::Config
    }

    /// Enter command mode (after pressing ':')
    pub fn enter_command_mode(&mut self) {
        self.command_mode = true;
        self.hud_status = ":".to_string();
    }

    /// Exit command mode
    pub fn exit_command_mode(&mut self) {
        self.command_mode = false;
        self.hud_status = "Ready".to_string();
    }

    /// Handle a command letter in command mode
    /// Returns true if the command was handled
    pub fn handle_command(&mut self, c: char) -> bool {
        self.command_mode = false;

        match c {
            't' | 'T' => {
                self.toggle_view();
                self.hud_status = ":t - Toggle view".to_string();
                true
            }
            'c' | 'C' => {
                self.open_config();
                self.hud_status = ":c - Config opened".to_string();
                true
            }
            'r' | 'R' => {
                self.hud_status = ":r - Refresh requested".to_string();
                true
            }
            'q' | 'Q' => {
                self.should_quit = true;
                true
            }
            '?' => {
                self.hud_status = ":t :c :r :q".to_string();
                true
            }
            _ => {
                self.hud_status = format!(": Unknown command '{}'", c);
                false
            }
        }
    }

    /// Start selection at position
    pub fn start_selection(&mut self, row: usize, col: usize) {
        self.selection.set_start(row, col);
    }

    /// Update selection end position
    pub fn update_selection(&mut self, row: usize, col: usize) {
        self.selection.set_end(row, col);
    }

    /// Copy selection to clipboard
    pub fn copy_selection(&mut self) -> bool {
        if self.selection.text.is_empty() {
            self.hud_status = "No selection to copy".to_string();
            return false;
        }

        match arboard::Clipboard::new() {
            Ok(mut clipboard) => {
                match clipboard.set_text(&self.selection.text) {
                    Ok(_) => {
                        let len = self.selection.text.len();
                        self.hud_status = format!("Copied {} chars", len);
                        true
                    }
                    Err(e) => {
                        self.hud_status = format!("Copy failed: {}", e);
                        false
                    }
                }
            }
            Err(e) => {
                self.hud_status = format!("Clipboard error: {}", e);
                false
            }
        }
    }

    /// Set the selected text
    pub fn set_selection_text(&mut self, text: String) {
        self.selection.text = text;
    }

    /// Set the first AI prompt and generate its TL;DR
    pub fn set_first_ai_prompt(&mut self, prompt: &str) {
        if self.ai_first_prompt.is_none() {
            self.ai_first_prompt = Some(prompt.to_string());
            self.ai_first_prompt_tldr = crate::ai_context::generate_simple_tldr(prompt);
            self.ai_session_started = true;
        }
    }

    /// Update AI progress from screen analysis
    pub fn update_ai_progress(&mut self, progress: &str) {
        self.ai_progress = progress.to_string();
    }
}
