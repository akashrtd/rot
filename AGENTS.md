# rot - AI Agent Context

> This file provides context for AI coding agents working on the rot project.

---

## Build & Test Commands

```bash
# Build
cargo build                    # Debug build (all crates)
cargo build --release          # Optimized build

# Test
cargo test                     # Run all tests
cargo test --package rot-core  # Test specific crate
cargo test -- --nocapture      # Show test output

# Lint & Format
cargo clippy -- -D warnings    # Lint (warnings as errors)
cargo fmt -- --check           # Check formatting
cargo fmt                      # Auto-format

# Run
cargo run --package rot-cli    # Run CLI
cargo run -- --help            # Show help
```

---

## Code Style

### Errors
- Use `thiserror` for library error types (in `rot-*` crates)
- Use `anyhow` for application code (in `rot-cli`)
- All error types must implement `Debug` and `Display`

### Async
- Use `tokio` runtime for all async code
- Prefer `async fn` over returning `impl Future`
- Use `tokio::fs` not `std::fs` in async context

### Documentation
- All public items must have doc comments (`///`)
- Include examples in doc comments when helpful

### Derives
- Always `#[derive(Debug)]` on structs and enums
- Use `#[derive(Clone)]` when type is `Copy` or needs cloning
- Use `serde::Serialize, serde::Deserialize` for data types

### Naming
- Crate names: `rot-<domain>` (e.g., `rot-core`, `rot-provider`)
- Trait names: nouns (e.g., `Provider`, `Tool`, `Session`)
- Error types: `<Domain>Error` (e.g., `ProviderError`)

---

## Architecture

### Crate Responsibilities

| Crate | Purpose |
|-------|---------|
| `rot-cli` | Binary entry point, CLI parsing |
| `rot-core` | Agent loop, message types, permissions |
| `rot-provider` | LLM provider abstraction |
| `rot-tools` | Built-in tools implementation |
| `rot-session` | JSONL session persistence |
| `rot-rlm` | Recursive Language Model engine |
| `rot-tui` | Terminal UI with ratatui |
| `rot-plugin` | Plugin system (v2.0+, future) |

### Key Traits

```rust
// rot-provider/src/traits.rs
trait Provider {
    async fn stream(&self, request: Request)
        -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>, ProviderError>;
    async fn complete(&self, request: Request) -> Result<Response, ProviderError>;
}

// rot-tools/src/traits.rs
trait Tool {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;
    async fn execute(&self, args: Value, ctx: &ToolContext)
        -> Result<ToolResult, ToolError>;
}
```

### Data Flow

```
User Input → TUI → Agent.process()
                    ↓
              Provider.stream()
                    ↓
              StreamEvent parsing
                    ↓
              Tool execution (if tool call)
                    ↓
              Session persistence
                    ↓
              TUI update
```

---

## Key Patterns

### Provider Pattern
Each provider implements the `Provider` trait and handles:
1. Request transformation to provider format
2. SSE stream parsing
3. Event normalization to `StreamEvent`

### Tool Pattern
Each tool:
1. Defines parameters with `schemars::JsonSchema`
2. Validates input
3. Executes with timeout
4. Returns structured `ToolResult`

### Session Pattern
Sessions use JSONL format:
- Each line is a JSON object with `type` field
- Tree structure via `id` and `parent_id`
- Auto-saved on each message

---

## Common Gotchas

### Tokio Runtime
- Tests need `#[tokio::test]` not `#[test]` for async
- Use `tokio::fs` in async context, never `std::fs`

### Cross-Platform
- Shell commands: `bash -c` on Unix (fallback to `sh -c` if needed), `cmd /C` on Windows
- Path handling: use `std::path::Path` not string concatenation
- PTY: `portable-pty` handles cross-platform terminal

### Serde
- Use `#[serde(tag = "type")]` for discriminated unions
- Use `#[serde(default)]` for optional fields
- JSONL files must end with newline

### Streaming
- SSE events: parse line by line, look for `data: ` prefix
- Handle `data: [DONE]` as stream end
- Buffer partial events until complete

---

## Git Workflow

### Commit Format
```
<type>(<scope>): <description>

Types: feat, fix, refactor, docs, test, chore
Scopes: core, provider, tools, session, rlm, tui, cli
```

### Examples
```
feat(provider): add Anthropic streaming support
fix(tools): handle path traversal in read tool
refactor(core): extract context building to separate module
docs(readme): add installation instructions
test(session): add JSONL roundtrip tests
```

### Branch Naming
- `feature/<description>` - New features
- `fix/<description>` - Bug fixes
- `refactor/<description>` - Code cleanup

---

## Testing Strategy

### Unit Tests
- Place in same file as code: `#[cfg(test)] mod tests { ... }`
- Test one function/behavior per test
- Use descriptive names: `test_read_file_with_offset`

### Integration Tests
- Place in `tests/` directory
- Use `tests/fixtures/` for test data
- Test complete workflows

### Test Coverage
- All public functions
- Error paths
- Edge cases (empty input, max values, unicode)

---

## Documentation Files

| File | Purpose |
|------|---------|
| `brainstorm.md` | Research, decisions, competitive analysis |
| `pdr.md` | Product requirements, personas, metrics |
| `architecture.md` | Technical design with code examples |
| `plan.md` | Task-by-task implementation guide |
| `AGENTS.md` | This file - AI agent context |

---

## Quick Reference

### Add New Provider
1. Create `crates/rot-provider/src/providers/<name>.rs`
2. Implement `Provider` trait
3. Add to `providers/mod.rs`
4. Add tests with mock server

### Add New Tool
1. Create `crates/rot-tools/src/builtin/<name>.rs`
2. Define parameters struct with `JsonSchema`
3. Implement `Tool` trait
4. Register in `builtin/mod.rs`
5. Add tests

### Debug Streaming
```rust
// Add to stream handler
tracing::debug!(event = ?event, "received stream event");
```

---

## When Stuck

1. Read `architecture.md` for design details
2. Check `plan.md` for task-specific guidance
3. Look at similar implementations in codebase
4. Run tests to understand expected behavior
5. Add `tracing` logs to debug

---

## Update This File

When you discover:
- New patterns that work well
- Gotchas not listed here
- Better ways to do things
- Common mistakes to avoid

Add them to this file to help future sessions.
