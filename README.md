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
- Tool use (`read`, `write`, `edit`, `bash`, `glob`, `grep`, `task`, `webfetch`).
- Config-driven custom tools and MCP stdio servers.
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
rot --provider ollama --model llama3.1
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
rot exec "continue from previous state" --session <ID>
rot exec "try an alternative approach" --session <ID> --fork
```

### Session commands

```bash
rot session list
rot session resume <ID>
rot session tree <ID>
rot session export <ID> ./session.jsonl
rot session import ./session.jsonl
```

### Tool inspection

```bash
rot tools
rot tools read
rot tools mcp__filesystem__read_file
```

### Provider and model discovery

```bash
rot providers
rot models
rot --provider openrouter models
```

### Headless service mode

```bash
rot serve
rot serve --host 0.0.0.0 --port 7878
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

External tool behavior:
- Custom command tools run under the active sandbox at call time.
- MCP stdio servers start under the active sandbox at startup.
- MCP tools are exported as `mcp__<server>__<tool>`.
- Under `untrusted` and `on-request`, MCP tools require approval by default.

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

## RLM Mode

Use Recursive Language Model mode for large context analysis:

```bash
rot exec --rlm --context ./spec.pdf "extract requirements"
rot exec --rlm --rlm-runtime bash --context ./spec.pdf "use bash runtime"
rot exec --rlm --rlm-isolation docker --rlm-docker-image python:3.11-slim --context ./spec.pdf "run in docker"
```

RLM safety:
- RLM inherits sandbox/network policy from the parent command.
- In danger mode (`--yolo`), add `--allow-unsafe-rlm` to run RLM explicitly.

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

Tool inspection in the TUI:
- `/tools` lists loaded tools
- `/tool <name>` shows one tool schema

## Built-in Tools

| Tool | Purpose |
| --- | --- |
| `read` | Read file contents with offset/limit |
| `list` | List files and directories |
| `codesearch` | Ranked code-aware workspace search |
| `lsp` | Experimental LSP-style lookup with fallback |
| `write` | Create or overwrite files |
| `edit` | Exact string replacement in files |
| `patch` | Apply deterministic multi-hunk edits |
| `question` | Ask structured clarification questions |
| `todoread` | Read structured task state |
| `todowrite` | Update structured task state |
| `bash` | Execute shell commands |
| `glob` | Find files by pattern |
| `grep` | Regex search across files |
| `task` | Delegate work to a built-in subagent |
| `webfetch` | Fetch URL content |
| `websearch` | Search the web for concise results |

## Configuration

Global config lives at `~/.rot/config.json`.

Example:

```json
{
  "provider": "anthropic",
  "model": "claude-3-5-sonnet-latest",
  "approval_policy": "on-request",
  "sandbox_mode": "workspace-write",
  "sandbox_network_access": false,
  "api_keys": {
    "anthropic": "sk-ant-..."
  },
  "custom_tools": [
    {
      "name": "echo_args",
      "description": "Echo the raw JSON arguments",
      "command": "cat \"$ROT_TOOL_ARGS_FILE\"",
      "parameters_schema": {
        "type": "object",
        "properties": {
          "text": { "type": "string" }
        }
      },
      "timeout_secs": 30
    }
  ],
  "mcp_servers": [
    {
      "name": "filesystem",
      "enabled": true,
      "command": "npx",
      "args": [
        "-y",
        "@modelcontextprotocol/server-filesystem",
        "."
      ],
      "cwd": ".",
      "env": {},
      "startup_timeout_secs": 20,
      "tool_timeout_secs": 60
    }
  ]
}
```

Custom tool command contract:
- `ROT_TOOL_NAME`
- `ROT_TOOL_ARGS_FILE`
- `ROT_TOOL_ARGS_JSON`
- `ROT_SESSION_ID`

MCP scope in this release:
- stdio transport only
- protocol version `2025-06-18`
- startup discovery through `tools/list`
- tool invocation through `tools/call`
- per-server `enabled` flag
- optional per-server `cwd`

## Providers

Configured providers:
- Anthropic
- z.ai
- OpenAI
- Ollama
- OpenRouter
- Google

Provider selection:

```bash
rot --provider anthropic
rot --provider zai
rot --provider openai
rot --provider ollama
rot --provider openrouter
rot --provider google
```

Serve mode API is documented in [`docs/serve.md`](docs/serve.md).

## Workspace Layout

```text
crates/
  rot-cli       # binary entrypoint and CLI parsing
  rot-core      # agent loop, security policy, messages
  rot-mcp       # MCP stdio client
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
