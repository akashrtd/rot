# Serve Mode

`rot serve` runs a local HTTP API for non-interactive automation.

## Start

```bash
rot serve
rot serve --host 0.0.0.0 --port 7878
```

By default it binds to `127.0.0.1:7878`.

## Endpoints

### `GET /health`

Response:

```json
{"status":"ok"}
```

### `POST /exec`

Request:

```json
{
  "prompt": "Summarize src/main.rs",
  "provider": "openai",
  "model": "gpt-4o-mini",
  "agent": "default"
}
```

Response (final-json style):

```json
{
  "status": "ok|error",
  "final_text": "string",
  "tool_calls": [{"name":"string","arguments":{}}],
  "usage": {"input_tokens":0,"output_tokens":0},
  "elapsed_ms": 0,
  "error": null
}
```

## Security

- Serve mode is non-interactive.
- Effective approval policy must be `never`.
- Sandbox and network behavior follows the same runtime security config as `rot exec`.
