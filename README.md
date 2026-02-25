# Huginn

A cognitive terminal multiplexer written in Rust. Wraps your shell and AI assistants with a dynamic HUD to prevent context-switching fatigue.

> In Norse mythology, Odin had two ravens: Muninn (Memory) and Huginn (Thought). Huginn flew around the world and returned to whisper everything it had seen into Odin's ear.

## Features

- **Terminal UI with HUD**: Three-panel layout showing context and status at the top
- **Shell Integration**: Wraps your default shell (zsh, bash, fish) in a PTY
- **Session Multiplexing**: Toggle between Shell and AI assistant views with `:t`
- **AI Assistant Support**: Run Claude Code, Aider, or any CLI AI tool in parallel
- **Smart HUD Status**: LLM-powered TL;DR of what the AI is currently doing
- **Mouse Selection**: Click and drag to select text, auto-copies on release
- **Scrollback Navigation**: Shift+Up/Down or mouse scroll to navigate history
- **Full ANSI Support**: Colors, bold, italic, underline, and more
- **Configurable**: JSON-based configuration with native TUI settings screen
- **Cross-platform**: Works on macOS, Linux, and Windows

## Installation

### From Source

```bash
git clone https://github.com/user/huginn.git
cd huginn
cargo build --release
```

The binary will be at `target/release/huginn`.

## Usage

Run Huginn:

```bash
cargo run --release
```

### Keyboard Shortcuts

Press `:` to enter command mode, then:

| Command | Action |
|---------|--------|
| `:t` | Toggle between Shell and AI views |
| `:c` | Open configuration screen |
| `:r` | Force HUD refresh (re-summarize) |
| `:q` | Quit |
| `:?` | Show commands |

### Mouse Navigation

| Action | Result |
|--------|--------|
| Click + Drag | Select text (auto-copies on release) |
| Scroll Up/Down | Navigate scrollback history |
| Shift + Up/Down | Navigate scrollback history |

**Config screen:**

| Shortcut | Action |
|----------|--------|
| `Tab` / `↓` | Next field |
| `Shift+Tab` / `↑` | Previous field |
| `Enter` | Save configuration |
| `Esc` | Back to main view |

## Configuration

Configuration is stored at `~/.config/huginn/config.json`:

```json
{
  "shell_command": "zsh",
  "shell_args": [],
  "ai_command": "claude",
  "ai_args": [],
  "summarizer_command": "ollama",
  "summarizer_args": ["run", "llama3.2"],
  "shortcuts": {
    "toggle_view": "ctrl+shift+t",
    "force_refresh": "ctrl+shift+r",
    "open_config": "ctrl+shift+c",
    "quit_app": "ctrl+shift+q"
  }
}
```

### Summarizer Configuration

The HUD can use an LLM to generate context-aware summaries. Configure your preferred summarizer:

**Using Ollama:**
```json
{
  "summarizer_command": "ollama",
  "summarizer_args": ["run", "llama3.2"]
}
```

**Using Claude CLI:**
```json
{
  "summarizer_command": "claude",
  "summarizer_args": []
}
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  HUD (30% TL;DR | 20% Scroll | 50% AI Status)              │  3 lines
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Main View                                                  │  Flexible
│  (Shell / AI / Config)                                      │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│  Footer (Dynamic shortcuts)                                 │  1 line
└─────────────────────────────────────────────────────────────┘
```

### HUD Layout

- **Left (30%)**: TL;DR of the first AI prompt or current view name
- **Center (20%)**: Scroll indicator when navigating history
- **Right (50%)**: AI status TL;DR (LLM-generated summary of current activity)

## Development Status

### Phase 1: TUI Infrastructure ✅
- [x] Terminal UI with HUD, main view, and footer
- [x] Configuration system with JSON persistence
- [x] Config screen with text input fields
- [x] Event loop with keyboard shortcuts

### Phase 2: PTY Integration ✅
- [x] Real shell integration via portable-pty
- [x] PTY rendering in main view
- [x] Non-blocking PTY reading with background thread

### Phase 3: VT100 Parsing ✅
- [x] ANSI escape sequence parsing (via vt100 crate)
- [x] Proper color and cursor support
- [x] Text attribute rendering (bold, italic, underline, etc.)

### Phase 4: Session Multiplexing ✅
- [x] Dual PTY sessions (Shell + AI)
- [x] Toggle between Shell and AI with `:t`
- [x] State preservation between views
- [x] Both sessions process output simultaneously

### Phase 5: LLM Summarization ✅
- [x] Background HUD updates via LLM (ollama, claude, etc.)
- [x] Context extraction from terminal content
- [x] Periodic summarization (every 30 seconds)
- [x] Manual refresh with `:r` command
- [x] Graceful fallback when LLM unavailable

### Phase 6: Enhanced UX ✅
- [x] Mouse-based text selection with auto-copy
- [x] Scrollback navigation (mouse and keyboard)
- [x] Smart AI status with LLM-generated TL;DR
- [x] ChildKiller for clean process termination

## License

MIT License - see [LICENSE](LICENSE) for details.
