# Getting Started

## Installation

### From source

```bash
git clone https://github.com/your-org/rot.git
cd rot
cargo install --path crates/rot-cli
```

### Prerequisites

- **Rust 1.75+** — install via [rustup](https://rustup.rs/)
- **API key** — at least one of:
  - `ANTHROPIC_API_KEY` for Anthropic Claude
  - `ZAI_API_KEY` for z.ai GLM-5
  - `OPENAI_API_KEY` for OpenAI models
  - `OPENROUTER_API_KEY` for OpenRouter
  - `GOOGLE_API_KEY` (or `GEMINI_API_KEY`) for Google Gemini
  - Ollama works locally without an API key

## First Run

```bash
# Set your API key
export ANTHROPIC_API_KEY=sk-ant-...

# Launch interactive chat
rot
```

You'll see the TUI with a prompt. Type a message and press Enter.

## Modes

### Interactive Chat (default)

```bash
rot
rot chat
rot --provider zai
rot --provider ollama --model llama3.1
```

Full TUI with vim keybindings, streaming responses, and tool execution.

### Single-shot Execution

```bash
rot exec "read the README.md and summarize it"
rot --provider zai exec "list all Rust files"
rot exec --rlm --context ./large_spec.pdf "extract the core requirements"
rot exec --rlm --rlm-runtime bash --context ./large_spec.pdf "use legacy bash runtime"
```

Runs one prompt, prints the response, and exits.

RLM context preprocessing supports text, JSON, CSV, HTML, and PDF (`pdftotext` required for PDF).

### Session Management

```bash
rot session list          # See recent sessions
rot session resume <ID>   # Load a session by ID
rot session tree <ID>     # View session tree
rot session export <ID> ./session.jsonl
rot session import ./session.jsonl
```

### Tool Inspection

```bash
rot tools
rot tools read
rot providers
rot models
```

### Headless Service

```bash
rot serve
```

## Next Steps

- Read [Configuration](configuration.md) for provider, security, custom tool, and MCP options
- Read [Tools](tools.md) for built-in and external tool details
- Read [Serve Mode](serve.md) for local HTTP automation
