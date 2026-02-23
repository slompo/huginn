//! Configuration management for Huginn CLI
//!
//! Handles loading, saving, and default values for user configuration.

use crate::error::{ConfigError, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE: &str = "config.json";

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Shell command to run (e.g., "zsh", "bash", "fish")
    pub shell_command: String,

    /// Arguments to pass to the shell
    pub shell_args: Vec<String>,

    /// AI assistant command (e.g., "claude", "aider")
    pub ai_command: String,

    /// Arguments to pass to the AI assistant
    pub ai_args: Vec<String>,

    /// Summarizer command for background HUD updates (e.g., "ollama")
    pub summarizer_command: String,

    /// Arguments to pass to the summarizer
    pub summarizer_args: Vec<String>,

    /// Keyboard shortcuts configuration
    pub shortcuts: Shortcuts,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            shell_command: "zsh".to_string(),
            shell_args: vec!["-l".to_string()],
            ai_command: "claude".to_string(),
            ai_args: vec![],
            summarizer_command: "ollama".to_string(),
            summarizer_args: vec!["run".to_string(), "llama3.2".to_string()],
            shortcuts: Shortcuts::default(),
        }
    }
}

/// Keyboard shortcuts configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shortcuts {
    /// Toggle between Shell and AI views
    pub toggle_view: String,

    /// Force HUD refresh/re-summarize
    pub force_refresh: String,

    /// Open configuration screen
    pub open_config: String,

    /// Quit the application
    pub quit_app: String,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            toggle_view: "ctrl+shift+t".to_string(),
            force_refresh: "ctrl+shift+r".to_string(),
            open_config: "ctrl+shift+c".to_string(),
            quit_app: "ctrl+shift+q".to_string(),
        }
    }
}

/// Holds paths to configuration directories
#[allow(dead_code)]
pub struct ConfigPaths {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
}

impl ConfigPaths {
    /// Determine configuration paths using XDG Base Directory specification
    pub fn new() -> Option<Self> {
        let project_dirs = ProjectDirs::from("com", "huginn", "huginn")?;

        Some(Self {
            config_dir: project_dirs.config_dir().to_path_buf(),
            config_file: project_dirs.config_dir().join(CONFIG_FILE),
        })
    }
}

impl Config {
    /// Load configuration from disk, or create and return defaults if not found
    pub fn load_or_default() -> Result<Self> {
        let paths =
            ConfigPaths::new().ok_or(ConfigError::CannotDeterminePaths)?;

        if !paths.config_file.exists() {
            let config = Config::default();
            config.save_to(&paths.config_file)?;
            return Ok(config);
        }

        Self::load_from(&paths.config_file)
    }

    /// Load configuration from a specific file path
    fn load_from(path: &PathBuf) -> Result<Self> {
        let content =
            fs::read_to_string(path).map_err(ConfigError::ReadError)?;

        serde_json::from_str(&content)
            .map_err(ConfigError::ParseError)
            .map_err(Into::into)
    }

    /// Save configuration to the default location
    pub fn save(&self) -> Result<()> {
        let paths =
            ConfigPaths::new().ok_or(ConfigError::CannotDeterminePaths)?;

        self.save_to(&paths.config_file)
    }

    /// Save configuration to a specific file path
    fn save_to(&self, path: &PathBuf) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(ConfigError::CreateDirError)?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(ConfigError::SerializeError)?;

        // Write to a temp file first, then rename for atomicity
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, content).map_err(ConfigError::WriteError)?;
        fs::rename(&temp_path, path).map_err(ConfigError::WriteError)?;

        Ok(())
    }

    /// Update this config from a map of field values (from the config form)
    pub fn update_from_form(&mut self, values: &HashMap<ConfigField, String>) {
        if let Some(val) = values.get(&ConfigField::ShellCommand) {
            self.shell_command = val.clone();
        }
        if let Some(val) = values.get(&ConfigField::ShellArgs) {
            self.shell_args = parse_args(val);
        }
        if let Some(val) = values.get(&ConfigField::AiCommand) {
            self.ai_command = val.clone();
        }
        if let Some(val) = values.get(&ConfigField::AiArgs) {
            self.ai_args = parse_args(val);
        }
        if let Some(val) = values.get(&ConfigField::SummarizerCommand) {
            self.summarizer_command = val.clone();
        }
        if let Some(val) = values.get(&ConfigField::SummarizerArgs) {
            self.summarizer_args = parse_args(val);
        }
    }
}

/// Configuration form fields
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigField {
    ShellCommand,
    ShellArgs,
    AiCommand,
    AiArgs,
    SummarizerCommand,
    SummarizerArgs,
}

impl ConfigField {
    /// Get all config fields in order
    pub fn all() -> &'static [ConfigField] {
        &[
            ConfigField::ShellCommand,
            ConfigField::ShellArgs,
            ConfigField::AiCommand,
            ConfigField::AiArgs,
            ConfigField::SummarizerCommand,
            ConfigField::SummarizerArgs,
        ]
    }

    /// Get the label for this field
    pub fn label(&self) -> &'static str {
        match self {
            ConfigField::ShellCommand => "Shell Command",
            ConfigField::ShellArgs => "Shell Arguments",
            ConfigField::AiCommand => "AI Command",
            ConfigField::AiArgs => "AI Arguments",
            ConfigField::SummarizerCommand => "Summarizer Command",
            ConfigField::SummarizerArgs => "Summarizer Arguments",
        }
    }

    /// Get the next field in the form
    pub fn next(&self) -> Self {
        let all = Self::all();
        let idx = all.iter().position(|f| f == self).unwrap_or(0);
        all[(idx + 1) % all.len()]
    }

    /// Get the previous field in the form
    pub fn prev(&self) -> Self {
        let all = Self::all();
        let idx = all.iter().position(|f| f == self).unwrap_or(0);
        all[(idx + all.len() - 1) % all.len()]
    }
}

/// Parse a space-separated argument string into a Vec
fn parse_args(s: &str) -> Vec<String> {
    shlex::split(s).unwrap_or_default()
}

/// Convert args Vec to a display string
pub fn args_to_string(args: &[String]) -> String {
    args.iter()
        .map(|s| shlex::try_quote(s).unwrap_or_else(|_| s.clone().into()))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_serialization() {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(config.shell_command, parsed.shell_command);
        assert_eq!(config.ai_command, parsed.ai_command);
    }

    #[test]
    fn test_config_with_custom_shortcuts() {
        let json = r#"{
            "shell_command": "bash",
            "shell_args": [],
            "ai_command": "claude",
            "ai_args": [],
            "summarizer_command": "ollama",
            "summarizer_args": ["run", "llama3.2"],
            "shortcuts": {
                "toggle_view": "ctrl+t",
                "force_refresh": "ctrl+r",
                "open_config": "ctrl+c",
                "quit_app": "ctrl+q"
            }
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.shortcuts.toggle_view, "ctrl+t");
        assert_eq!(config.shell_command, "bash");
    }

    #[test]
    fn test_config_field_navigation() {
        assert_eq!(ConfigField::ShellCommand.next(), ConfigField::ShellArgs);
        assert_eq!(ConfigField::SummarizerArgs.next(), ConfigField::ShellCommand);
        assert_eq!(ConfigField::ShellArgs.prev(), ConfigField::ShellCommand);
    }
}
