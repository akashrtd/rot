# Built-in Tools

rot includes 7 built-in tools that the AI can invoke during conversations.

## read

Read file contents with optional offset and limit.

**Parameters:**
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `path` | string | ✅ | File path (relative to working dir) |
| `offset` | integer | | Start line (0-indexed) |
| `limit` | integer | | Max lines to read |

## write

Create or overwrite a file. Creates parent directories automatically.

**Parameters:**
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `path` | string | ✅ | File path |
| `content` | string | ✅ | File contents to write |

## edit

Perform surgical string replacement in a file.

**Parameters:**
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `path` | string | ✅ | File path |
| `old` | string | ✅ | String to find |
| `new` | string | ✅ | Replacement string |
| `replace_all` | boolean | | Replace all occurrences (default: false) |

## bash

Execute a shell command.

**Parameters:**
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `command` | string | ✅ | Shell command to run |
| `timeout` | integer | | Timeout in seconds (default: 30) |

**Limits:** Output truncated to 50KB. Commands run in the working directory.

## glob

Find files matching a glob pattern. Respects `.gitignore`.

**Parameters:**
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `pattern` | string | ✅ | Glob pattern (e.g., `**/*.rs`) |

**Limits:** Max 1000 results.

## grep

Search file contents with regex patterns. Respects `.gitignore`.

**Parameters:**
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `pattern` | string | ✅ | Regex pattern |
| `include` | string | | Glob filter for files (e.g., `*.rs`) |
| `context` | integer | | Context lines around matches (default: 0) |

**Limits:** Max 200 matches.

## webfetch

Fetch content from a URL.

**Parameters:**
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `url` | string | ✅ | URL to fetch |

**Limits:** Response body truncated to 100KB.
