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
```

Full TUI with vim keybindings, streaming responses, and tool execution.

### Single-shot Execution

```bash
rot exec "read the README.md and summarize it"
rot --provider zai exec "list all Rust files"
```

Runs one prompt, prints the response, and exits.

### Session Management

```bash
rot session list          # See recent sessions
rot session resume <ID>   # Resume a session (planned)
```

## Next Steps

- Read [Configuration](configuration.md) for provider and model options
- Read [Tools](tools.md) for details on built-in tools
