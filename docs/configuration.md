# Configuration

## Provider Selection

```bash
rot --provider anthropic   # Default
rot --provider zai         # z.ai GLM-5
```

## Model Selection

```bash
rot --model claude-sonnet-4-20250514   # Anthropic (default)
rot --model glm-5                       # z.ai (default)
rot --model glm-4.7                     # z.ai alternate
```

If `--model` is not specified, each provider uses its own default.

## Environment Variables

| Variable            | Provider  | Required                   |
| ------------------- | --------- | -------------------------- |
| `ANTHROPIC_API_KEY` | Anthropic | Yes (when using anthropic) |
| `ZAI_API_KEY`       | z.ai      | Yes (when using zai)       |

## Verbose Logging

```bash
rot -v              # Enable debug logging
rot --verbose       # Same
```

## Session Storage

Sessions are stored in `~/.local/share/rot/sessions/` (Linux/macOS), organized by a hash of the working directory. Each session is a JSONL file containing the full conversation history.

## Agent Configuration

The agent uses these defaults:

| Setting        | Default  | Description                        |
| -------------- | -------- | ---------------------------------- |
| Max iterations | 10       | Maximum tool-use loops per prompt  |
| Max tokens     | 4096     | Maximum output tokens per response |
| System prompt  | Built-in | Describes rot's capabilities       |

These are currently hardcoded. Configuration file support is planned.
