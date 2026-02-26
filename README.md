<p align="center">
  <h1 align="center">rot</h1>
  <p align="center">
    <strong>Recursive Operations Tool — AI coding agent in your terminal</strong>
  </p>
  <p align="center">
    <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/rust-1.75%2B-orange?style=flat-square&logo=rust" alt="Rust"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue?style=flat-square" alt="License: MIT"></a>
    <img src="https://img.shields.io/badge/tests-94_passing-brightgreen?style=flat-square" alt="Tests">
    <img src="https://img.shields.io/badge/binary-5.1MB-purple?style=flat-square" alt="Binary Size">
  </p>
</p>

---

rot is a **terminal-native AI coding agent** built in Rust. Give it a task, and it reads your code, runs commands, edits files, and builds solutions — all from a single chat interface.

```text
███████████      ███████    ███████████
░░███░░░░░███   ███░░░░░███ ░█░░░███░░░█
 ░███    ░███  ███     ░░███░   ░███  ░
 ░██████████  ░███      ░███    ░███
 ░███░░░░░███ ░███      ░███    ░███
 ░███    ░███ ░░███     ███     ░███
 █████   █████ ░░░███████░      █████
░░░░░   ░░░░░    ░░░░░░░       ░░░░░
```

```
┌ rot ────────────────────────────────────────────────────┐
│you: refactor the authentication module to use JWT       │
│                                                         │
│rot: I'll analyze the current auth module and refactor   │
│     it to use JWT tokens. Let me start by reading the   │
│     existing code...                                    │
│                                                         │
│tool: ↳ read                                             │
│tool: ↳ edit                                             │
│tool: ↳ bash                                             │
│                                                         │
│rot: Done! I've refactored auth.rs to use JWT. Changes:  │
│     • Added jsonwebtoken dependency                     │
│     • Replaced session tokens with JWT claims           │
│     • Updated middleware to verify JWT signatures       │
│     • All 12 tests pass                                 │
└─────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────┐
│›                                                        │
└─────────────────────────────────────────────────────────┘
 ● Ready | anthropic / claude-sonnet-4-20250514 | Ctrl+C: quit
```

## Why rot?

|                       |                                                                       |
| --------------------- | --------------------------------------------------------------------- |
| ⚡ **Fast**           | 5.1MB native binary. Starts instantly. No Python, no Node, no Docker. |
| 🔧 **Agentic**        | Reads, writes, edits files and runs shell commands autonomously.      |
| 🧠 **Multi-provider** | Anthropic Claude, z.ai GLM-5, and any OpenAI-compatible API.          |
| 💬 **Interactive**    | Full TUI with streaming, thinking indicators, and mouse scroll.       |
| 📦 **Portable**       | Single binary. Works on macOS, Linux, and Windows.                    |
| 🔒 **Transparent**    | All tool calls are visible. You see everything the agent does.        |

---

## Table of Contents

- [Quick Start](#quick-start)
- [Installation](#installation)
- [Usage](#usage)
- [Built-in Tools](#built-in-tools)
- [Providers](#providers)
- [Architecture](#architecture)
- [Development](#development)
- [Contributing](#contributing)
- [License](#license)

---

## Quick Start

```bash
# Install
git clone https://github.com/AkashRathod/rot.git && cd rot
cargo install --path crates/rot-cli

# Set API key
export ANTHROPIC_API_KEY=sk-ant-...    # Anthropic Claude
# OR
export ZAI_API_KEY=your-key            # z.ai GLM-5

# Launch
rot
```

That's it. You're pair programming with AI.

---

## Installation

### From source (recommended)

```bash
git clone https://github.com/AkashRathod/rot.git
cd rot
cargo install --path crates/rot-cli
```

### Prerequisites

- **Rust 1.75+** — [install via rustup](https://rustup.rs/)
- **API key** — at least one:

| Provider  | Environment Variable | Get a key                                               |
| --------- | -------------------- | ------------------------------------------------------- |
| Anthropic | `ANTHROPIC_API_KEY`  | [console.anthropic.com](https://console.anthropic.com/) |
| z.ai      | `ZAI_API_KEY`        | [z.ai](https://z.ai)                                    |

### Verify installation

```bash
rot --help
rot --version
```

---

## Usage

### Interactive Chat (default)

```bash
rot                          # Uses default provider (Anthropic)
rot --provider zai           # Use z.ai GLM-5
rot --model glm-4.7          # Specify model
rot -v                       # Verbose/debug logging
```

### Single-shot Execution

```bash
rot exec "read main.rs and explain the architecture"
rot exec "find all TODO comments and create a summary"
rot exec "write a test for the auth module"
```

### Session Management

```bash
rot session list             # List recent sessions
rot session resume <ID>      # Resume a session
```

### TUI Keybindings

| Key       | Mode   | Action                |
| --------- | ------ | --------------------- |
| `Enter`   | Insert | Send message          |
| `Esc`     | Insert | Switch to Normal mode |
| `i`       | Normal | Switch to Insert mode |
| `j` / `↓` | Normal | Scroll down           |
| `k` / `↑` | Normal | Scroll up             |
| `G`       | Normal | Jump to bottom        |
| `q`       | Normal | Quit                  |
| `Ctrl+C`  | Any    | Quit                  |
| 🖱 Scroll | Any    | Scroll up/down        |

### Slash Commands

| Command | Action       |
| ------- | ------------ |
| `/quit` | Exit session |
| `/exit` | Exit session |

---

## Built-in Tools

rot has **7 built-in tools** the AI can use autonomously to complete tasks:

### File Operations

| Tool      | What it does                           | Example                         |
| --------- | -------------------------------------- | ------------------------------- |
| **read**  | Read file contents (with offset/limit) | Read lines 10-50 of `main.rs`   |
| **write** | Create or overwrite files              | Create a new test file          |
| **edit**  | Surgical find-and-replace edits        | Rename a function across a file |

### System

| Tool     | What it does                     | Example                    |
| -------- | -------------------------------- | -------------------------- |
| **bash** | Run shell commands (30s timeout) | `cargo test`, `git status` |
| **glob** | Find files by pattern            | `**/*.rs`, `src/**/test_*` |
| **grep** | Regex search across files        | Find all `TODO` comments   |

### Network

| Tool         | What it does      | Example           |
| ------------ | ----------------- | ----------------- |
| **webfetch** | Fetch URL content | Download API docs |

All tool calls are **visible in the TUI** as they execute — you always know what the agent is doing.

---

## Providers

### Anthropic Claude (default)

```bash
export ANTHROPIC_API_KEY=sk-ant-...
rot
```

**Models:** `claude-sonnet-4-20250514` (default)

### z.ai (Zhipu AI)

```bash
export ZAI_API_KEY=your-key
rot --provider zai
```

**Models:** `glm-5` (default), `glm-4.7`

### Adding a new provider

rot includes a **generic OpenAI-compatible provider layer** — adding any OpenAI-compatible API (OpenAI, Ollama, OpenRouter, etc.) takes ~40 lines:

```rust
// crates/rot-provider/src/providers/your_provider.rs
pub fn new_your_provider(api_key: String) -> OpenAiCompatProvider {
    OpenAiCompatProvider::new(OpenAiCompatConfig {
        name: "your-provider".to_string(),
        base_url: "https://api.your-provider.com/v1".to_string(),
        api_key,
        default_model: "model-name".to_string(),
        models: vec![/* ModelInfo structs */],
    })
}
```

See [`zai.rs`](crates/rot-provider/src/providers/zai.rs) for a working example.

---

## Architecture

rot is a modular Rust workspace with **8 crates**, each with a single responsibility:

```
rot/
├── rot-cli           Binary entry point, CLI arg parsing
├── rot-core          Agent loop, message types, orchestration
├── rot-provider      LLM provider trait + implementations
│   ├── anthropic     Anthropic Claude (Messages API + SSE)
│   ├── openai_compat Generic OpenAI-compatible provider
│   └── zai           z.ai wrapper (GLM-5/GLM-4.7)
├── rot-tools         Tool trait + 7 built-in tools
├── rot-session       JSONL session persistence
├── rot-tui           Terminal UI (ratatui + crossterm)
├── rot-rlm           Recursive Language Model (context manager)
└── rot-plugin        Plugin system (planned)
```

### Data Flow

```
User Input → rot-cli → rot-core (Agent Loop) → rot-provider (LLM API)
                              ↕                         ↓
                        rot-tools (File I/O,      Response/Stream
                         Shell, Search)                 ↓
                              ↕                    rot-tui (Render)
                        rot-session (Persist)           ↓
                                                  Terminal Output
```

### Key Design Decisions

- **Trait-based providers** — `Provider` trait with `stream()` + `complete()` makes adding new LLMs trivial
- **Non-blocking TUI** — agent processing runs in a background `tokio::spawn` task, keeping the UI responsive
- **Tool registry pattern** — tools are registered dynamically, making the system extensible
- **JSONL sessions** — human-readable, git-friendly, easy to debug

---

## Development

```bash
# Build (debug)
cargo build

# Build (release, ~5.1MB binary)
cargo build --release

# Run all tests (94 tests)
cargo test

# Run integration tests only
cargo test -p rot-cli --test integration_test

# Lint (zero warnings policy)
cargo clippy -- -D warnings

# Format
cargo fmt

# Run in dev mode
cargo run --package rot-cli -- --provider zai exec "hello"
```

### Test Coverage

| Crate        | Tests      | Coverage                       |
| ------------ | ---------- | ------------------------------ |
| rot-core     | Unit tests | Agent loop, message conversion |
| rot-provider | 14 tests   | Anthropic, OpenAI-compat, z.ai |
| rot-tools    | Unit tests | All 7 tools                    |
| rot-session  | Unit tests | JSONL persistence              |
| rot-rlm      | 5 tests    | Context storage                |
| rot-tui      | 5 tests    | App state, input handling      |
| Integration  | 8 tests    | Full component wiring          |

---

## Project Status

rot is in **active development**. Current status:

| Phase                           | Status                              |
| ------------------------------- | ----------------------------------- |
| Core types & messages           | ✅ Complete                         |
| Provider system                 | ✅ Anthropic + z.ai + OpenAI-compat |
| Tool system                     | ✅ 7 built-in tools                 |
| Agent loop                      | ✅ Multi-turn with tool use         |
| Session persistence             | ✅ JSONL storage                    |
| Terminal UI                     | ✅ Streaming, scroll, thinking      |
| CLI                             | ✅ chat, exec, session commands     |
| Integration tests & docs        | ✅ Complete                         |
| Permission system               | 🔜 Planned                          |
| Plugin system                   | 🔜 Planned                          |
| More providers (OpenAI, Ollama) | 🔜 Easy to add                      |

---

## Contributing

Contributions are welcome! Here's how to get started:

1. **Fork** the repo and create a branch
2. **Read** `AGENTS.md` for project conventions
3. **Write tests** — we enforce zero clippy warnings
4. **Submit a PR** with a clear description

### Reporting Issues

Found a bug? [Open an issue](https://github.com/AkashRathod/rot/issues) with:

- Your OS and Rust version
- Steps to reproduce
- Expected vs actual behavior

---

## License

MIT — see [LICENSE](LICENSE) for details.

---

<p align="center">
  <sub>Built with 🦀 Rust • Powered by LLMs • Made for developers who live in the terminal</sub>
</p>
