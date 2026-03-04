# rot

> Recursive Operations Tool, a next-generation AI coding agent for the terminal.

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

**rot** makes AI-assisted coding accessible to every developer, regardless of their preferred LLM provider or environment constraints. It solves the fundamental limitation of AI context windows through **recursive intelligence**, handling massive codebases and document-heavy sessions with ease.

---

## ✨ Key Features

- **Zero Setup & No Dependencies:** Shipped as a single, fast Rust binary. No Node.js, Bun, or Python environments to configure. Drop it onto any server or air-gapped machine and go.
- **Infinite Context via RLM:** The Recursive Language Model (RLM) engine natively processes `10M+` token inputs by allowing the LLM to access, chunk, and summarize data recursively via an internal REPL environment.
- **Provider Freedom:** Use the AI models you want. `rot` supports **Anthropic**, **OpenAI**, **Google (Gemini)**, **z.ai**, **OpenRouter**, and **Ollama** (for complete local privacy).
- **Native Performance:** Written entirely in Rust, guaranteeing instant startup times, low idle memory usage, and rock-solid reliability.
- **Rich Native Tools:** Comes with 16+ built-in semantic file tools (`codesearch`, `lsp`, `grep`, `glob`, `patch`), as well as native support for Custom Bash Tools and the **Model Context Protocol (MCP)**.
- **Beautiful TUI:** Fully-featured terminal UI with Vim keybindings, side-by-side execution views, and streaming responses.

---

## 🚀 Installation

### One-line installer (macOS / Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/akashrtd/rot/main/install.sh | bash
```

> **Options:**
> Force reinstall with `ROT_FORCE=1`
> Select a specific release version with `ROT_VERSION=v0.1.0`

### Build from source

Requires Rust `1.75+`.

```bash
git clone https://github.com/akashrtd/rot.git
cd rot
cargo install --path crates/rot-cli
```

### Prerequisites

At least one provider API key exported in your environment (unless exclusively using Ollama locally).

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
# OR
export OPENAI_API_KEY="sk-proj-..."
# OR
export GOOGLE_API_KEY="..."
```

---

## 💻 Usage

### Interactive TUI (Chat Mode)

Fire up the beautiful TUI and start pair programming:

```bash
rot
```

Want to use a specific provider or model?

```bash
rot --provider openai --model gpt-4o
rot --provider ollama --model llama3.1
rot --provider zai
```

**TUI Keybindings:**

- `Enter`: Send message
- `Shift+Enter`: Newline
- `Esc`: Switch to Normal mode
- `i`: Switch to Insert mode
- `j` / `Down` & `k` / `Up`: Scroll up and down
- `q` / `Ctrl+C`: Quit
- `/`: Open Slash command popup (type `/tools` to inspect built-in tools)

### Single-shot Execution (CI / Automation)

Execute a quick task and exit—perfect for scripts or git hooks.

```bash
rot exec "read main.rs and summarize architecture"
rot exec "find all TODO comments in the workspace and print them"
rot --provider anthropic exec "write a unit test for parser.rs"
```

### The RLM Engine (Massive Contexts)

When dealing with giant repositories or massive documents, RLM treats standard LLM limits as a thing of the past.

```bash
rot exec --rlm --context ./spec.pdf "extract the core requirements"
rot exec --rlm --rlm-runtime bash --context ./architecture.md "analyze the design"
```

_(Note: PDF processing requires `pdftotext` to be installed on your system)._

### Session Management

`rot` saves your sessions efficiently via JSONL. You don't lose context between working days.

```bash
rot session list             # View recent sessions
rot session resume <ID>      # Jump back in
rot session tree <ID>        # View context tree
rot session export <ID> ./session.jsonl
```

### Headless Server Mode

`rot` can act as its own headless service for local integrations:

```bash
rot serve --host 0.0.0.0 --port 7878
```

---

## 🛠️ Configuration & MCP

Your global configuration lives at `~/.rot/config.json`. This is where you configure security approvals, custom tools, default providers, and MCP servers.

### Security and Approvals

`rot` takes safety seriously. By default, the sandbox network access is off and approvals are required for destructive actions.

You can modify sandbox and approval policies via config or flags:

```bash
rot --sandbox <read-only|workspace-write|danger-full-access>
rot --ask-for-approval <untrusted|on-request|never>
```

_(Shortcuts for the bold: `rot --yolo` or `rot --full-auto`)_

### MCP (Model Context Protocol)

`rot` ships with native stdio MCP support. Add an MCP server directly in `config.json`:

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

## 🏗️ Development

`rot` is composed of several localized crates (`rot-core`, `rot-rlm`, `rot-tui`, etc.).

```bash
# Debug build
cargo build

# Run tests
cargo test
cargo test -p rot-core

# Lint
cargo clippy -- -D warnings
cargo fmt -- --check
```

See [AGENTS.md](AGENTS.md) and [architecture.md](architecture.md) for deeper technical contexts if you're looking to contribute!

---

## 📜 License

MIT License. See [LICENSE](LICENSE) for details.
