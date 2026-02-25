//! AI Context Analysis
//!
//! Detects AI assistant state from screen content and generates TL;DR summaries.

/// AI execution states detected from screen content
#[derive(Debug, Clone, PartialEq)]
pub enum AiState {
    Idle,
    Thinking,
    Running,
    WaitingForInput,
    Complete,
    Error,
}

/// Analyze screen content for AI progress indicators with context
pub fn detect_ai_progress(screen_content: &str) -> (AiState, String) {
    let content_lower = screen_content.to_lowercase();

    // Pattern matching for common AI states (Claude Code specific)
    // Check for active work patterns first (higher priority)

    // Claude Code specific patterns
    if content_lower.contains("thinking") || content_lower.contains("analyzing")
        || content_lower.contains("let me") || content_lower.contains("i'll")
        || content_lower.contains("i will") || content_lower.contains("i'm going to")
    {
        let context = extract_context(screen_content);
        let status = if !context.is_empty() {
            format!("Working: {}", context)
        } else {
            "Thinking...".to_string()
        };
        (AiState::Thinking, status)
    } else if content_lower.contains("running") || content_lower.contains("executing")
        || content_lower.contains("bash") || content_lower.contains("command")
    {
        let context = extract_context(screen_content);
        let status = if !context.is_empty() {
            format!("Running: {}", context)
        } else {
            "Running command...".to_string()
        };
        (AiState::Running, status)
    } else if content_lower.contains("reading") || content_lower.contains("searching")
        || content_lower.contains("finding") || content_lower.contains("looking")
        || content_lower.contains("checking") || content_lower.contains("examining")
    {
        let context = extract_context(screen_content);
        let status = if !context.is_empty() {
            format!("Reading: {}", context)
        } else {
            "Reading files...".to_string()
        };
        (AiState::Running, status)
    } else if content_lower.contains("editing") || content_lower.contains("writing")
        || content_lower.contains("modifying") || content_lower.contains("updating")
        || content_lower.contains("adding") || content_lower.contains("creating")
        || content_lower.contains("implementing") || content_lower.contains("fixing")
    {
        let context = extract_context(screen_content);
        let status = if !context.is_empty() {
            format!("Editing: {}", context)
        } else {
            "Editing files...".to_string()
        };
        (AiState::Running, status)
    } else if content_lower.contains("building") || content_lower.contains("compiling")
        || content_lower.contains("cargo") || content_lower.contains("npm")
        || content_lower.contains("make")
    {
        (AiState::Running, "Building...".to_string())
    } else if content_lower.contains("testing") || content_lower.contains("tests")
        || content_lower.contains("pytest") || content_lower.contains("jest")
    {
        (AiState::Running, "Running tests...".to_string())
    } else if content_lower.contains("waiting") || (content_lower.contains("enter") && content_lower.contains("for"))
        || content_lower.contains("prompt") || content_lower.contains("ask")
    {
        (AiState::WaitingForInput, "Awaiting input".to_string())
    } else if content_lower.contains("idle") || screen_content.trim().is_empty() {
        (AiState::Idle, "Idle".to_string())
    } else if content_lower.contains("complete") || content_lower.contains("done")
        || content_lower.contains("finished") || content_lower.contains("success")
    {
        (AiState::Complete, "Done".to_string())
    } else if content_lower.contains("error:") || content_lower.contains("failed:")
        || content_lower.contains("panic") || content_lower.contains("exception")
    {
        let context = extract_context(screen_content);
        let status = if !context.is_empty() {
            format!("Error: {}", context)
        } else {
            "Error".to_string()
        };
        (AiState::Error, status)
    } else {
        // Default to ready state
        (AiState::Complete, "Ready".to_string())
    }
}

/// Extract contextual information from screen content (file names, tasks, etc.)
fn extract_context(screen_content: &str) -> String {
    let lines: Vec<&str> = screen_content.lines().collect();
    let mut context_parts: Vec<String> = Vec::new();

    for line in lines.iter().rev().take(20) {
        let line_lower = line.to_lowercase();

        // Look for file references
        if line_lower.contains(".rs") || line_lower.contains(".js") || line_lower.contains(".ts")
            || line_lower.contains(".py") || line_lower.contains(".go") || line_lower.contains(".md")
            || line_lower.contains("file:") || line_lower.contains("src/")
        {
            // Extract file name
            if let Some(file) = extract_filename(line) {
                if !context_parts.contains(&file) {
                    context_parts.push(file);
                }
            }
        }

        // Look for task references
        if line_lower.contains("task") || line_lower.contains("todo") || line_lower.contains("step") {
            if let Some(task) = extract_task(line) {
                context_parts.push(task);
            }
        }

        // Limit context length
        if context_parts.len() >= 2 {
            break;
        }
    }

    // Truncate to fit HUD
    let result = context_parts.join(", ");
    if result.len() > 40 {
        format!("{}...", &result[..37])
    } else {
        result
    }
}

/// Extract a file name from a line
fn extract_filename(line: &str) -> Option<String> {
    // Look for common file patterns
    let words: Vec<&str> = line.split_whitespace().collect();
    for word in words {
        if word.contains(".rs") || word.contains(".js") || word.contains(".ts")
            || word.contains(".py") || word.contains(".go") || word.contains(".md")
        {
            // Clean up the filename
            let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '/' && c != '_');
            if clean.len() > 2 && clean.len() < 40 {
                return Some(clean.to_string());
            }
        }
    }
    None
}

/// Extract a task description from a line
fn extract_task(line: &str) -> Option<String> {
    let line_lower = line.to_lowercase();

    // Look for task patterns
    if let Some(idx) = line_lower.find("task") {
        let remainder = &line[idx..];
        if remainder.len() > 5 && remainder.len() < 50 {
            return Some(remainder.trim().to_string());
        }
    }

    if let Some(idx) = line_lower.find("step") {
        let remainder = &line[idx..];
        if remainder.len() > 5 && remainder.len() < 50 {
            return Some(remainder.trim().to_string());
        }
    }

    None
}

/// Generate a simple TL;DR of a prompt (no LLM needed)
pub fn generate_simple_tldr(prompt: &str) -> String {
    // Remove common filler words
    let cleaned = prompt
        .replace("please ", "")
        .replace("Please ", "")
        .replace("can you ", "")
        .replace("Can you ", "")
        .replace("could you ", "")
        .replace("Could you ", "")
        .replace("i want to ", "")
        .replace("I want to ", "")
        .replace("i need to ", "")
        .replace("I need to ", "")
        .replace("help me ", "")
        .replace("Help me ", "")
        .trim()
        .to_string();

    // Truncate to fit HUD (max ~35 chars for left side)
    let max_len = 35;
    if cleaned.len() > max_len {
        format!("{}...", &cleaned[..max_len - 3])
    } else if cleaned.is_empty() {
        "New session".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_thinking() {
        let content = "Claude is thinking about your request...";
        let (state, msg) = detect_ai_progress(content);
        assert_eq!(state, AiState::Thinking);
        assert_eq!(msg, "Thinking...");
    }

    #[test]
    fn test_detect_running() {
        let content = "Running tests...";
        let (state, msg) = detect_ai_progress(content);
        assert_eq!(state, AiState::Running);
    }

    #[test]
    fn test_generate_tldr() {
        assert_eq!(generate_simple_tldr("Please help me fix this bug"), "fix this bug");
        assert_eq!(generate_simple_tldr("Can you explain how this works?"), "explain how this works?");
    }

    #[test]
    fn test_truncate_tldr() {
        let long = "This is a very long prompt that should be truncated because it exceeds the limit";
        let result = generate_simple_tldr(long);
        assert!(result.len() <= 35);
        assert!(result.ends_with("..."));
    }
}
