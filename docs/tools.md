# Tools

rot includes 8 built-in tools and can also load external tools from config.

## read

Read file contents with optional offset and limit.

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `path` | string | Yes | File path relative to the working directory |
| `offset` | integer | No | Start line, zero-indexed |
| `limit` | integer | No | Maximum lines to read |

## write

Create or overwrite a file. Parent directories are created automatically.

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `path` | string | Yes | File path |
| `content` | string | Yes | File contents to write |

## edit

Perform exact string replacement in a file.

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `path` | string | Yes | File path |
| `old` | string | Yes | String to find |
| `new` | string | Yes | Replacement string |
| `replace_all` | boolean | No | Replace all occurrences, default `false` |

## bash

Execute a shell command.

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `command` | string | Yes | Shell command to run |
| `timeout` | integer | No | Timeout in seconds, default `30` |

Output is truncated to 50 KB.

## glob

Find files matching a glob pattern. Respects `.gitignore`.

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `pattern` | string | Yes | Glob pattern such as `**/*.rs` |

Results are limited to 1000 paths.

## grep

Search file contents with a regex pattern. Respects `.gitignore`.

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `pattern` | string | Yes | Regex pattern |
| `include` | string | No | File glob filter such as `*.rs` |
| `context` | integer | No | Context lines around each match |

Results are limited to 200 matches.

## task

Delegate work to a built-in subagent.

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `agent` | string | Yes | Built-in subagent name |
| `prompt` | string | Yes | Task prompt for the subagent |

Delegation is bounded by depth, total-task, concurrency, and timeout limits.

## webfetch

Fetch content from a URL.

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `url` | string | Yes | URL to fetch |

Response bodies are truncated to 100 KB.

## External Tools

rot can also load:
- `custom_tools`: config-defined shell commands
- `mcp_servers`: tools discovered from stdio MCP servers

External tools appear in the same tool transcript flow as built-ins.

Naming:
- custom tools use the configured tool name directly
- MCP tools are exported as `mcp__<server>__<tool>`

Inspection:
- `rot tools` lists all loaded tools
- `rot tools <name>` shows one tool schema
- `/tools` lists loaded tools in the TUI
- `/tool <name>` shows one tool schema in the TUI
