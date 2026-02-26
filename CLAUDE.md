# Huginn - Build Instructions

## Requirements

- **Rust**: 1.75 or later
- **Cargo**: Included with Rust installation

## Quick Start

```bash
# Clone and build
git clone https://github.com/user/huginn.git
cd huginn
cargo build --release
```

The binary will be at `target/release/huginn`.

## Dependencies

All dependencies are managed by Cargo and listed in `Cargo.toml`:

| Crate | Version | Purpose |
|-------|---------|---------|
| tokio | 1.x | Async runtime |
| ratatui | 0.29 | TUI framework |
| crossterm | 0.28 | Terminal backend |
| vt100-ctt | 0.17 | VT100/ANSI parsing |
| portable-pty | 0.8 | PTY support |
| tui-input | 0.14 | Text input widget |
| crossbeam-channel | 0.5 | Channel communication |
| arboard | 3.4 | Clipboard support |

## Troubleshooting

### Build fails with vt100-ctt error

1. Check Rust version:
   ```bash
   rustc --version  # Must be >= 1.75
   ```

2. Update Rust if needed:
   ```bash
   rustup update stable
   ```

3. Clean and rebuild:
   ```bash
   cargo clean
   cargo build --release
   ```

### Network issues downloading crates

If crates.io is unreachable, try a mirror:

```bash
# In ~/.cargo/config.toml
[source.crates-io]
replace-with = "tuna"

[source.tuna]
registry = "https://mirrors.tuna.tsinghua.edu.cn/git/crates.io-index.git"
```

## Project Structure

```
huginn/
├── src/
│   ├── main.rs       # Entry point, event loop
│   ├── app.rs        # Application state machine
│   ├── pty.rs        # PTY management with vt100 parser
│   ├── session.rs    # Dual PTY session manager
│   ├── config.rs     # Configuration loading/saving
│   ├── summarizer.rs # LLM-powered HUD summarization
│   ├── terminal.rs   # Terminal RAII wrapper
│   ├── event.rs      # Keyboard/mouse event handling
│   ├── ai_context.rs # AI progress detection
│   ├── error.rs      # Error types
│   └── ui/
│       ├── mod.rs       # UI coordination
│       ├── main_view.rs # VT100 screen rendering
│       ├── hud.rs       # HUD panel
│       ├── footer.rs    # Footer with shortcuts
│       └── config_ui.rs # Configuration form
├── Cargo.toml        # Dependencies
├── Cargo.lock        # Pinned versions (committed)
└── README.md         # User documentation
```

## Development

```bash
# Run in debug mode
cargo run

# Run with release optimizations
cargo run --release

# Check for errors without building
cargo check

# Run clippy linter
cargo clippy
```
