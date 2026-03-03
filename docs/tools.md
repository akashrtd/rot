# Tools

rot includes 16 built-in tools and can also load external tools from config.

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

## list

List directory contents without shelling out.

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `path` | string | No | Directory path, default `.` |
| `include_hidden` | boolean | No | Include dotfiles |
| `recursive` | boolean | No | Recurse into subdirectories |
| `max_entries` | integer | No | Maximum returned entries |

## codesearch

Search code with ranked file hits and contextual snippets.

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `query` | string | Yes | Code/symbol query |
| `path` | string | No | Search root, default `.` |
| `include` | string | No | Optional file glob filter |
| `max_results` | integer | No | Maximum ranked files |
| `before_context` | integer | No | Context lines before match |
| `after_context` | integer | No | Context lines after match |
| `case_sensitive` | boolean | No | Preserve case in matching |

## lsp

Experimental language-server style lookup with graceful fallback.

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `action` | enum | No | `definition`, `references`, `hover`, `symbols` |
| `query` | string | Yes | Symbol/query text |
| `path` | string | No | Search root, default `.` |
| `language_server` | string | No | Optional server command override |
| `max_results` | integer | No | Fallback result count |

Current behavior:
- explicitly marked `EXPERIMENTAL`
- if no language server is available/configured, it falls back to `codesearch`

## edit

Perform exact string replacement in a file.

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `path` | string | Yes | File path |
| `old_string` | string | Yes | String to find |
| `new_string` | string | Yes | Replacement string |
| `replace_all` | boolean | No | Replace all occurrences, default `false` |

## patch

Apply deterministic multi-hunk exact replacements to a file.

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `path` | string | Yes | File path |
| `hunks` | array | Yes | Ordered replacement hunks |
| `allow_noop` | boolean | No | Allow unmatched hunks |

Each hunk includes:
- `old_string` (required)
- `new_string` (required)
- `replace_all` (optional, default `false`)

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
| `before_context` | integer | No | Context lines before each match |
| `after_context` | integer | No | Context lines after each match |

Results are limited to 200 matches.

## question

Ask a structured clarification question in the tool flow.

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `question` | string | Yes | Clarifying question text |
| `options` | array[string] | No | Suggested options |

## todoread

Read structured task state for the current workspace.

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `json_only` | boolean | No | Return JSON output only |

## todowrite

Mutate structured task state for the current workspace.

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `action` | enum | No | `set`, `add`, `update`, `remove`, `clear` |
| `items` | array | No | Task payloads for set/add/update |
| `ids` | array[string] | No | Task IDs for remove |

## task

Delegate work to a built-in subagent.

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `agent` | string | No | Subagent name for single-task mode |
| `prompt` | string | Yes | Task prompt |
| `swarm` | boolean | No | Enable planner/worker/merge orchestration |
| `workers` | array[string] | No | Worker agents in swarm mode |
| `planner_agent` | string | No | Planner agent override (default `plan`) |
| `merge_agent` | string | No | Merge agent override (default `default`) |

Delegation is bounded by depth, total-task, concurrency, and timeout limits.

## webfetch

Fetch content from a URL.

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `url` | string | Yes | URL to fetch |

Response bodies are truncated to 100 KB.

## websearch

Search the web for concise results (network-gated).

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `query` | string | Yes | Search query |
| `limit` | integer | No | Maximum results, default `5` |

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

## RLM Context Loading

When using `rot exec --rlm --context <file>`, context is preprocessed before entering the RLM runtime.

Supported source types:
- text (UTF-8)
- JSON (pretty-printed)
- CSV (normalized line loading)
- HTML (tag-stripped text extraction)
- PDF (via `pdftotext` when available)

Behavior:
- binary/non-text sources are rejected with clear errors
- extracted text is cached under a managed temporary artifact path
- context metadata includes source path, detected type, and extracted length
