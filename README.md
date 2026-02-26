# rot

Recursive Operations Tool, an AI coding agent for the terminal.

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

## Overview

`rot` is a Rust workspace for building and running a terminal-native coding agent.

Main capabilities:
- Interactive TUI chat.
- Single-shot `exec` mode for automation and CI.
- Tool use (`read`, `write`, `edit`, `bash`, `glob`, `grep`, `webfetch`).
- Multi-provider model support (Anthropic, z.ai, OpenAI-compatible).
- Session persistence.
- Sandbox and approval policy controls.

## Installation

### One-line installer

```bash
curl -fsSL https://raw.githubusercontent.com/akashrtd/rot/main/install.sh | bash
```

Optional installer flags:

```bash
# Install a tagged release
ROT_VERSION=v0.1.0 curl -fsSL https://raw.githubusercontent.com/akashrtd/rot/main/install.sh | bash

# Force reinstall
ROT_FORCE=1 curl -fsSL https://raw.githubusercontent.com/akashrtd/rot/main/install.sh | bash
```

### From source

```bash
git clone https://github.com/akashrtd/rot.git
cd rot
cargo install --path crates/rot-cli
```

### Prerequisites

- Rust 1.75+ (`cargo` on PATH)
- At least one provider API key

```bash
export ANTHROPIC_API_KEY=...
# or
export ZAI_API_KEY=...
# or
export OPENAI_API_KEY=...
```

### Verify install

```bash
rot --version
rot --help
```

## Quick Start

```bash
rot
```

Basic alternatives:

```bash
rot --provider zai
rot --provider openai --model gpt-4o
rot -v
```

## Usage

### Interactive chat

```bash
rot
```

### Single-shot execution

```bash
rot exec "read main.rs and summarize architecture"
rot exec "find all TODO comments"
rot exec "write a unit test for parser.rs"
```

### Session commands

```bash
rot session list
rot session resume <ID>
```

## Security and Approval

Global flags:

```bash
rot --sandbox <read-only|workspace-write|danger-full-access>
rot --ask-for-approval <untrusted|on-request|never>
```

Shortcuts:

```bash
rot --full-auto
rot --dangerously-bypass-approvals-and-sandbox
rot --yolo
```

Defaults:
- `sandbox_mode=workspace-write`
- `approval_policy=on-request`
- `sandbox_network_access=false`

In non-interactive `exec`, approval is forced to `never`.

## Exec Automation Output

Machine output modes:

```bash
# JSONL event stream
rot exec "summarize repository status" --json

# Single JSON object
rot exec "summarize repository status" --final-json
```

Output schema validation:

```bash
rot exec "return valid JSON" --final-json --output-schema ./schema.json
```

Exit codes:
- `0` success
- `1` runtime or tool failure
- `2` output schema validation failure

## TUI Keybindings

Insert mode:
- `Enter` send message
- `Shift+Enter` newline
- `Esc` switch to normal mode

Normal mode:
- `i` switch to insert mode
- `j` / `Down` scroll down
- `k` / `Up` scroll up
- `G` jump to bottom
- `q` quit

Any mode:
- `Ctrl+C` quit

Slash command popup:
- Type `/` at the start of the input.
- Use `Up`/`Down` to select.
- Press `Enter` to run selected command.

## Built-in Tools

| Tool | Purpose |
| --- | --- |
| `read` | Read file contents with offset/limit |
| `write` | Create or overwrite files |
| `edit` | Exact string replacement in files |
| `bash` | Execute shell commands |
| `glob` | Find files by pattern |
| `grep` | Regex search across files |
| `webfetch` | Fetch URL content |

## Providers

Configured providers:
- Anthropic
- z.ai
- OpenAI-compatible

Provider selection:

```bash
rot --provider anthropic
rot --provider zai
rot --provider openai
```

## Workspace Layout

```text
crates/
  rot-cli       # binary entrypoint and CLI parsing
  rot-core      # agent loop, security policy, messages
  rot-provider  # provider trait + provider implementations
  rot-tools     # built-in tools
  rot-sandbox   # shell sandbox backends
  rot-session   # session persistence
  rot-tui       # terminal UI
  rot-rlm       # recursive language model engine
  rot-plugin    # plugin crate
```

## Development

Build:

```bash
cargo build
cargo build --release
```

Test:

```bash
cargo test
cargo test -p rot-core
cargo test -p rot-tools
```

Lint and format:

```bash
cargo clippy -- -D warnings
cargo fmt -- --check
```

## License

MIT
