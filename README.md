# rot

> **Recursive Operations Tool** — an AI-powered coding agent that runs in your terminal.

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

rot is a terminal-native AI coding assistant built in Rust. It connects to LLM providers, executes tools (file I/O, shell, search), and helps you code interactively with a full TUI or single-shot CLI mode.

---

## Features

- **Interactive TUI** — ratatui-based chat interface with vim-style keybindings
- **Built-in tools** — read, write, edit files; run shell commands; glob/grep search; fetch URLs
- **Multi-provider** — Anthropic Claude and z.ai GLM-5 supported (OpenAI-compatible layer for easy additions)
- **Session persistence** — conversations saved as JSONL, resumable across sessions
- **Streaming** — real-time token streaming with thinking indicators
- **Mouse scroll** — scroll through long conversations with your mouse wheel

---

## Quick Start

### Prerequisites

- Rust 1.75+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)

### Install

```bash
git clone https://github.com/your-org/rot.git
cd rot
cargo install --path crates/rot-cli
```

### Set your API key

```bash
# For Anthropic Claude (default)
export ANTHROPIC_API_KEY=your-key-here

# For z.ai GLM-5
export ZAI_API_KEY=your-key-here
```

### Run

```bash
# Interactive chat (default provider: Anthropic)
rot

# Interactive chat with z.ai
rot --provider zai

# Single-shot execution
rot exec "explain this codebase"

# Specify a model
rot --provider zai --model glm-4.7

# List past sessions
rot session list
```

---

## Usage

### Commands

| Command                 | Description                              |
| ----------------------- | ---------------------------------------- |
| `rot` / `rot chat`      | Interactive TUI chat session             |
| `rot exec "prompt"`     | Execute a single prompt and print result |
| `rot session list`      | List recent sessions                     |
| `rot session resume ID` | Resume a previous session                |

### Global Options

| Flag            | Description                       | Default          |
| --------------- | --------------------------------- | ---------------- |
| `--provider`    | LLM provider (`anthropic`, `zai`) | `anthropic`      |
| `--model`       | Model to use                      | Provider default |
| `-v, --verbose` | Enable debug logging              | Off              |

### TUI Keybindings

| Key         | Mode   | Action                                 |
| ----------- | ------ | -------------------------------------- |
| `Enter`     | Insert | Send message                           |
| `Esc`       | Insert | Switch to Normal mode                  |
| `i`         | Normal | Switch to Insert mode                  |
| `j` / `↓`   | Normal | Scroll down                            |
| `k` / `↑`   | Normal | Scroll up                              |
| `G`         | Normal | Jump to bottom (re-enable auto-scroll) |
| `q`         | Normal | Quit                                   |
| `Ctrl+C`    | Any    | Quit                                   |
| Mouse wheel | Any    | Scroll up/down                         |

### Slash Commands

| Command | Action           |
| ------- | ---------------- |
| `/quit` | Exit the session |
| `/exit` | Exit the session |

---

## Built-in Tools

rot has 7 built-in tools the AI can use:

| Tool       | Description                           |
| ---------- | ------------------------------------- |
| `read`     | Read file contents with offset/limit  |
| `write`    | Create or overwrite files             |
| `edit`     | Surgical string replacement in files  |
| `bash`     | Execute shell commands (with timeout) |
| `glob`     | Find files matching glob patterns     |
| `grep`     | Regex search across files             |
| `webfetch` | Fetch URL content                     |

---

## Architecture

rot is a Rust workspace with 8 crates:

```
rot/
├── rot-cli       # CLI entry point (binary: `rot`)
├── rot-core      # Agent loop, message types
├── rot-provider  # LLM provider trait + Anthropic/z.ai/OpenAI-compat
├── rot-tools     # Built-in tool implementations + registry
├── rot-session   # JSONL session persistence
├── rot-tui       # Terminal UI (ratatui + crossterm)
├── rot-rlm       # Recursive Language Model (context manager)
└── rot-plugin    # Plugin system (planned)
```

---

## Providers

### Anthropic (default)

Uses the Messages API with SSE streaming. Set `ANTHROPIC_API_KEY`.

**Models**: `claude-sonnet-4-20250514` (default), plus other Claude models.

### z.ai (Zhipu AI)

Uses the OpenAI-compatible API via the GLM Coding Plan. Set `ZAI_API_KEY`.

**Models**: `glm-5` (default), `glm-4.7`.

### Adding a new provider

Any OpenAI-compatible API can be added by creating a thin wrapper (see `crates/rot-provider/src/providers/zai.rs` — it's ~40 lines).

---

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Lint
cargo clippy -- -D warnings

# Run in dev mode
cargo run --package rot-cli -- --provider zai

# Release build
cargo build --release
```

---

## License

MIT
