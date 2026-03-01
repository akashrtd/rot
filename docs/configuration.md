# Configuration

Global config is stored in `~/.rot/config.json`.

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
  "custom_tools": [],
  "mcp_servers": []
}
```

## Provider Selection

```bash
rot --provider anthropic
rot --provider zai
rot --provider openai
```

## Model Selection

```bash
rot --model claude-sonnet-4-20250514
rot --model glm-5
rot --model gpt-4o
```

If `--model` is not specified, each provider uses its own default.

## Environment Variables

| Variable | Provider | Required |
| --- | --- | --- |
| `ANTHROPIC_API_KEY` | Anthropic | Yes when using Anthropic |
| `ZAI_API_KEY` | z.ai | Yes when using z.ai |
| `OPENAI_API_KEY` | OpenAI-compatible | Yes when using OpenAI-compatible |

## Security Configuration

Persisted security keys:

| Key | Type | Default |
| --- | --- | --- |
| `approval_policy` | `untrusted \| on-request \| never` | `on-request` |
| `sandbox_mode` | `read-only \| workspace-write \| danger-full-access` | `workspace-write` |
| `sandbox_network_access` | `bool` | `false` |

CLI flags override config for the current run.

## Session Storage

Sessions are stored in `~/.local/share/rot/sessions/` on Linux and macOS, organized by working directory hash. Each session is a JSONL file.

## Custom Tools

`custom_tools` lets you define shell-backed tools without recompiling `rot`.

Example:

```json
{
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
  ]
}
```

Environment variables exposed to custom tool commands:
- `ROT_TOOL_NAME`
- `ROT_TOOL_ARGS_FILE`
- `ROT_TOOL_ARGS_JSON`
- `ROT_SESSION_ID`

## MCP Servers

`mcp_servers` loads tools from stdio MCP servers during startup.

Example:

```json
{
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

Behavior:
- exported tool names are namespaced as `mcp__<server>__<tool>`
- MCP servers start under the active sandbox mode
- network access follows `sandbox_network_access`
- under `untrusted` and `on-request`, MCP tools require approval unless approval policy is `never`
- `enabled: false` skips a configured server without removing it from the config file
- relative `cwd` values are resolved from the current workspace directory

Current MCP scope:
- stdio transport only
- protocol version `2025-06-18`
- startup discovery via `tools/list`
- tool invocation via `tools/call`
