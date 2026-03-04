<div align="center">

# rot (Recursive Operations Tool)

**A next-generation open-source AI coding agent for your terminal.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

<br/>

<!-- Replace the src below with the actual path or URL to the screenshot you provided -->
<img src="./rot.png" alt="rot TUI interface" width="800" />

</div>

<br/>

**rot** makes AI-assisted coding accessible to every developer, regardless of their preferred LLM provider or environment constraints. It solves the fundamental limitation of AI context windows through recursive intelligence, gracefully handling massive codebases and document-heavy sessions.

---

## Features

- **Interactive TUI**: A fully-featured Terminal User Interface with Vim keybindings, side-by-side execution views, and streaming responses.
- **Multiple AI Providers**: Seamlessly switch between OpenAI, Anthropic, Google Gemini, Ollama (local), and more.
- **The RLM Engine**: Process massive 10M+ token contexts natively. `rot` allows the LLM to recursively access, chunk, and summarize data via an internal REPL environment.
- **Robust Tool Integration**: The agent can inspect files, search codebases, and safely execute bash commands.
- **Strict Security & Sandboxing**: Execute with confidence. `rot` wraps destructive tools in OS-level sandboxing (macOS Seatbelt, Linux bwrap) with discrete permission boundaries.
- **MCP Support Natively**: Connect external tools easily with native Model Context Protocol (MCP) server integration.
- **Session Persistence**: Never lose your train of thought. Sessions are efficiently stored via JSONL and can be resumed at any time.

---

## Quick Start

Fire up the TUI and start pair programming immediately:

```bash
rot
```

Want to use a specific provider or model? It’s as simple as:

```bash
rot --provider openai --model gpt-4o
rot --provider ollama --model llama3.1
rot --provider zai
```

---

## Installation

### One-Line Installer (macOS / Linux)

The fastest way to get started is using our install script:

```bash
curl -fsSL https://raw.githubusercontent.com/akashrtd/rot/main/install.sh | bash
```

_Options:_

- Force reinstall: `ROT_FORCE=1`
- Select a specific release version: `ROT_VERSION=v0.1.0`

### Build From Source

If you prefer to build locally, ensure you have Rust 1.75+ installed:

```bash
git clone https://github.com/akashrtd/rot.git
cd rot
cargo install --path crates/rot-cli
```

---

## Usage Guide

### 1. Interactive TUI (Chat Mode)

This mode provides a responsive, split-pane interface right in your terminal.

**Keybindings:**

- `Enter`: Send message
- `Shift+Enter`: Insert newline
- `Esc`: Switch to Normal mode
- `i`: Switch to Insert mode
- `j` / `Down` & `k` / `Up`: Scroll chat
- `q` / `Ctrl+C`: Quit
- `/`: Open Slash command popup (e.g., `/tools` to inspect capabilities)

### 2. Single-Shot Execution

Execute a quick task and exit automatically. Perfect for shell scripts, CI pipelines, or quick queries.

```bash
rot exec "read main.rs and summarize architecture"
rot exec "find all TODO comments in the workspace and print them"
```

### 3. The RLM Engine for Massive Contexts

For massive repositories, RLM acts as a recursive memory layer, removing standard token limits.

```bash
rot exec --rlm --context ./spec.pdf "extract the core requirements"
rot exec --rlm --rlm-runtime bash --context ./architecture.md "analyze the design"
```

_(Note: PDF processing requires `pdftotext` to be installed on your system)._

### 4. Headless Server Mode

Run `rot` as a local headless service so other applications can integrate with its intelligent API:

```bash
rot serve --host 0.0.0.0 --port 7878
```

---

## Configuration

Your global configuration lives at `~/.rot/config.json`. Here you define API keys, security boundaries, and custom tools.

### Prerequisites & API Keys

Before using cloud models, export at least one provider API key:

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-proj-..."
export GOOGLE_API_KEY="..."
```

### Security Policies

`rot` errs on the side of caution. By default, it requires approval for mutating tasks and disables network access in the sandbox.

You can modify these policies securely per-activation:

```bash
rot --sandbox <read-only|workspace-write|danger-full-access>
rot --ask-for-approval <untrusted|on-request|never>
```

### Model Context Protocol (MCP)

Add MCP servers directly into your `config.json` to extend the AI's capabilities natively:

```json
{
  "mcp_servers": [
    {
      "name": "filesystem",
      "enabled": true,
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "."]
    }
  ]
}
```

---

## Development & Contributing

`rot` is split into several modular crates (`rot-core`, `rot-rlm`, `rot-tui`, etc.).

```bash
# Debug build
cargo build

# Run tests
cargo test
cargo test -p rot-core

# Linting
cargo clippy -- -D warnings
cargo fmt -- --check
```

_For more internal details, see [AGENTS.md](AGENTS.md) and [architecture.md](architecture.md)._

---

### License

`rot` is provided under the MIT License. See the [LICENSE](LICENSE) file for details.
