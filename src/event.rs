//! Event handling and keyboard shortcut mapping

use crate::config::Config;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Actions that can be triggered by keyboard shortcuts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Action {
    /// Toggle between Shell and AI views
    ToggleView,

    /// Force HUD refresh
    ForceRefresh,

    /// Open configuration screen
    OpenConfig,

    /// Quit the application
    Quit,

    /// Go back / close current view
    Back,

    /// Move to next field in form
    NextField,

    /// Move to previous field in form
    PrevField,

    /// Submit/save current form
    Submit,

    /// No action
    None,
}

/// Event handler that maps keyboard events to actions
#[allow(dead_code)]
pub struct EventHandler {
    toggle_view: KeyEvent,
    force_refresh: KeyEvent,
    open_config: KeyEvent,
    quit_app: KeyEvent,
}

impl EventHandler {
    /// Create a new event handler from configuration
    pub fn new(config: &Config) -> Self {
        Self {
            toggle_view: parse_key(&config.shortcuts.toggle_view),
            force_refresh: parse_key(&config.shortcuts.force_refresh),
            open_config: parse_key(&config.shortcuts.open_config),
            quit_app: parse_key(&config.shortcuts.quit_app),
        }
    }

    /// Convert a key event to an action (for main views)
    #[allow(dead_code)]
    pub fn handle_key_main(&self, key: KeyEvent) -> Action {
        if key == self.toggle_view {
            Action::ToggleView
        } else if key == self.force_refresh {
            Action::ForceRefresh
        } else if key == self.open_config {
            Action::OpenConfig
        } else if key == self.quit_app {
            Action::Quit
        } else {
            Action::None
        }
    }

    /// Convert a key event to an action (for config view)
    pub fn handle_key_config(&self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Tab | KeyCode::Down => Action::NextField,
            KeyCode::BackTab | KeyCode::Up => Action::PrevField,
            KeyCode::Enter => Action::Submit,
            KeyCode::Esc => Action::Back,
            _ => Action::None,
        }
    }
}

/// Parse a key string like "alt+t" into a KeyEvent
fn parse_key(s: &str) -> KeyEvent {
    let s = s.to_lowercase();
    let parts: Vec<&str> = s.split('+').collect();

    let (code, modifiers) = if parts.len() == 1 {
        parse_key_code(parts[0].trim())
    } else {
        let mut mods = KeyModifiers::empty();
        for part in &parts[..parts.len() - 1] {
            let part = part.trim();
            if part == "alt" {
                mods.insert(KeyModifiers::ALT);
            } else if part == "ctrl" || part == "control" {
                mods.insert(KeyModifiers::CONTROL);
            } else if part == "shift" {
                mods.insert(KeyModifiers::SHIFT);
            } else if part == "super" || part == "meta" || part == "cmd" || part == "command" {
                mods.insert(KeyModifiers::SUPER);
            }
        }
        let (code, _) = parse_key_code(parts[parts.len() - 1].trim());
        (code, mods)
    };

    KeyEvent::new(code, modifiers)
}

/// Parse a single key code string
fn parse_key_code(s: &str) -> (KeyCode, KeyModifiers) {
    let code = match s {
        "enter" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "esc" | "escape" => KeyCode::Esc,
        "backspace" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "page_up" => KeyCode::PageUp,
        "pagedown" | "page_down" => KeyCode::PageDown,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "space" => KeyCode::Char(' '),
        c if c.len() == 1 => KeyCode::Char(c.chars().next().unwrap()),
        c if c.starts_with('f') && c.len() <= 3 => {
            let num: u8 = c[1..].parse().unwrap_or(1);
            KeyCode::F(num.min(12))
        }
        _ => KeyCode::Char('?'),
    };
    (code, KeyModifiers::empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_alt_key() {
        let key = parse_key("alt+t");
        assert_eq!(key.code, KeyCode::Char('t'));
        assert_eq!(key.modifiers, KeyModifiers::ALT);
    }

    #[test]
    fn test_parse_ctrl_key() {
        let key = parse_key("ctrl+c");
        assert_eq!(key.code, KeyCode::Char('c'));
        assert_eq!(key.modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn test_parse_single_char() {
        let key = parse_key("q");
        assert_eq!(key.code, KeyCode::Char('q'));
        assert_eq!(key.modifiers, KeyModifiers::empty());
    }

    #[test]
    fn test_parse_special_key() {
        let key = parse_key("enter");
        assert_eq!(key.code, KeyCode::Enter);
    }
}
