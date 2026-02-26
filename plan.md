# rot - Implementation Plan (AI-Agent Optimized)

> This plan is structured for execution by an AI coding agent following Ralph/SWE-agent best practices.

---

## Plan Philosophy

### Core Principles (Ralph-Inspired)

1. **One Task Per Loop** - Each task is atomic, verifiable, and committable
2. **Fresh Context** - Agent starts each task with clean context, memory in files/git
3. **Backpressure** - Tests, lint, typecheck reject invalid code automatically
4. **Self-Improving** - Agent updates AGENTS.md with learnings
5. **Spec-Driven** - Work from specifications in architecture.md/pdr.md

### Execution Rules

| Rule                 | Description                                         |
| -------------------- | --------------------------------------------------- |
| **READ FIRST**       | Always read referenced files before implementing    |
| **VERIFY ALWAYS**    | Run `cargo test` and `cargo clippy` after each task |
| **COMMIT OFTEN**     | Commit after each completed task                    |
| **UPDATE AGENTS.md** | Add learnings, gotchas, patterns discovered         |
| **ASK IF STUCK**     | After 3 failed attempts, ask for clarification      |

### Task Format

Each task follows this structure:

```markdown
## T[X.X]: Task Title

**Crate**: `rot-xxx`
**Depends on**: T[X.X], T[X.X]
**Est. complexity**: Low/Medium/High

### Goal

[One sentence describing what to build]

### Files to Create/Modify

- `path/to/file.rs` - [what to do]

### Verification

- [ ] `cargo check --package rot-xxx` passes
- [ ] `cargo test --package rot-xxx` passes
- [ ] `cargo clippy --package rot-xxx` passes

### Implementation Notes

[Specific guidance, patterns to follow, things to avoid]
```

---

## Phase 0: Project Setup

### T0.1: Initialize Cargo Workspace

**Crate**: root
**Depends on**: none
**Est. complexity**: Low

#### Goal

Create the Cargo workspace with all 8 crates (7 libraries + 1 binary `rot-cli`).

#### Files to Create

```
rot/
├── Cargo.toml              # Workspace root
├── .gitignore
├── AGENTS.md               # AI agent context
└── crates/
    ├── rot-cli/
    │   ├── Cargo.toml
    │   └── src/main.rs     # Placeholder binary entry
    ├── rot-core/
    │   ├── Cargo.toml
    │   └── src/lib.rs
    ├── rot-rlm/
    │   ├── Cargo.toml
    │   └── src/lib.rs
    ├── rot-provider/
    │   ├── Cargo.toml
    │   └── src/lib.rs
    ├── rot-tools/
    │   ├── Cargo.toml
    │   └── src/lib.rs
    ├── rot-tui/
    │   ├── Cargo.toml
    │   └── src/lib.rs
    ├── rot-plugin/
    │   ├── Cargo.toml
    │   └── src/lib.rs
    └── rot-session/
        ├── Cargo.toml
        └── src/lib.rs
```

#### Root Cargo.toml

```toml
[workspace]
members = [
    "crates/rot-cli",
    "crates/rot-core",
    "crates/rot-rlm",
    "crates/rot-provider",
    "crates/rot-tools",
    "crates/rot-tui",
    "crates/rot-plugin",
    "crates/rot-session",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
license = "MIT"
repository = "https://github.com/your-org/rot"

[workspace.dependencies]
# Async
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"
futures = "0.3"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# TUI
ratatui = "0.29"
crossterm = "0.28"

# HTTP
reqwest = { version = "0.12", features = ["json", "stream", "rustls-tls"] }

# Schema
schemars = "0.8"

# CLI
clap = { version = "4", features = ["derive", "env"] }

# Errors
thiserror = "2"
anyhow = "1"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Utils
ulid = "1"
blake3 = "1"
dirs = "6"
tempfile = "3"
portable-pty = "0.8"
pin-project = "1"
once_cell = "1"
reqwest-eventsource = "0.6"

# Testing
tokio-test = "0.4"

# Internal crates
rot-core = { path = "crates/rot-core" }
rot-rlm = { path = "crates/rot-rlm" }
rot-provider = { path = "crates/rot-provider" }
rot-tools = { path = "crates/rot-tools" }
rot-tui = { path = "crates/rot-tui" }
rot-plugin = { path = "crates/rot-plugin" }
rot-session = { path = "crates/rot-session" }
```

#### Verification

- [ ] `cargo check` passes (workspace compiles with placeholder crates)
- [ ] `tree -L 2` shows correct structure

#### Implementation Notes

- Each crate's Cargo.toml should inherit from workspace: `version.workspace = true`
- Library crates need empty `src/lib.rs` with `//! Crate documentation`
- `rot-cli` starts as minimal `src/main.rs`
- Reference architecture.md section 2 for full structure

---

### T0.2: Create AGENTS.md

**Crate**: root
**Depends on**: T0.1
**Est. complexity**: Low

#### Goal

Create the AI agent context file that will be read at the start of each session.

#### Files to Create

- `AGENTS.md` - Project context for AI agents

#### Content

````markdown
# rot - AI Agent Context

## Build & Test Commands

```bash
cargo build                    # Build all crates
cargo build --release          # Optimized build
cargo test                     # Run all tests
cargo test --package rot-core  # Test specific crate
cargo clippy -- -D warnings    # Lint (treat warnings as errors)
cargo fmt -- --check           # Check formatting
```
````

## Code Style

- Use `thiserror` for library errors, `anyhow` for application
- All public APIs must have doc comments (`///`)
- Prefer `async fn` over returning `impl Future`
- Use `#[derive(Debug)]` on all structs and enums
- Match ergonomics: prefer `match` over `if let` for exhaustiveness

## Architecture

- Provider trait in `rot-provider` - LLM integrations
- Tool trait in `rot-tools` - Capabilities for the agent
- Session in `rot-session` - JSONL persistence
- RLM engine in `rot-rlm` - Recursive context handling

## Key Patterns

- Provider trait: `async fn stream(&self, request) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>>`
- Tool trait: `async fn execute(&self, args, ctx) -> Result<ToolResult, ToolError>`
- All async code uses tokio runtime

## Common Gotchas

- Don't use `std::fs` in async context - use `tokio::fs`
- Portable-pty requires special handling on Windows
- JSONL files must end with newline

## Git Workflow

- Commit after each completed task
- Format: `feat(core): add message types` or `fix(provider): handle stream errors`

````

#### Verification
- [ ] File exists and is readable
- [ ] Commands in file are accurate

---

### T0.3: Setup Error Types

**Crate**: `rot-core`
**Depends on**: T0.1
**Est. complexity**: Low

#### Goal
Create the foundational error types used across all crates.

#### Files to Create
- `crates/rot-core/src/lib.rs` - Re-exports
- `crates/rot-core/src/error.rs` - Error definitions

#### Verification
- [ ] `cargo check --package rot-core` passes
- [ ] `cargo clippy --package rot-core` passes

#### Implementation Notes

> [!IMPORTANT]
> At this stage, only `Io` and `Serialization` errors exist. The `Provider`, `Tool`,
> and `Session` `#[from]` variants must be added later as T1.2, T1.3, and T1.4 are
> completed and those error types become available.

```rust
// Use thiserror for library errors
// Start with only the errors available at this stage.
// Add #[from] variants for ProviderError, ToolError, SessionError
// as those crates are implemented in T1.2, T1.3, T1.4.
#[derive(Debug, thiserror::Error)]
pub enum RotError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}
````

---

## Phase 1: Core Types

### T1.1: Message Types

**Crate**: `rot-core`
**Depends on**: T0.3
**Est. complexity**: Medium

#### Goal

Define the core message types used throughout the system.

#### Files to Create

- `crates/rot-core/src/message.rs`
- Update `crates/rot-core/src/lib.rs`

#### Key Types

```rust
pub struct MessageId(String);           // ULID-based
pub enum Role { User, Assistant, Tool, System }
pub enum ContentBlock {
    Text { text: String },
    Image { data: String, mime_type: String },
    ToolCall { id: String, name: String, arguments: Value },
    ToolResult { tool_call_id: String, content: String, is_error: bool },
    Thinking { thinking: String },
}
pub struct Message {
    pub id: MessageId,
    pub role: Role,
    pub content: Vec<ContentBlock>,
    pub timestamp: u64,
    pub parent_id: Option<MessageId>,
}
```

#### Verification

- [ ] `cargo check --package rot-core` passes
- [ ] `cargo test --package rot-core` passes
- [ ] Add unit tests for `Message::user()` and `Message::assistant()`

#### Implementation Notes

- Use `ulid::Ulid::new().to_string()` for ID generation
- Implement `Serialize/Deserialize` for all types
- Reference architecture.md section 3.1

---

### T1.2: Provider Trait

**Crate**: `rot-provider`
**Depends on**: T0.3, T1.1
**Est. complexity**: Medium

#### Goal

Define the provider abstraction trait and common types.

#### Files to Create

- `crates/rot-provider/src/lib.rs`
- `crates/rot-provider/src/traits.rs`
- `crates/rot-provider/src/types.rs`

#### Key Types

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn models(&self) -> Vec<ModelInfo>;
    fn current_model(&self) -> &str;
    fn set_model(&mut self, model: &str) -> Result<()>;

    async fn stream(&self, request: Request)
        -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>>;

    async fn complete(&self, request: Request) -> Result<Response>;
}

pub struct Request {
    pub messages: Vec<ProviderMessage>,
    pub tools: Vec<ToolDefinition>,
    pub system: Option<String>,
    pub max_tokens: Option<usize>,
    pub thinking: Option<ThinkingConfig>,
}

pub enum StreamEvent {
    TextDelta { delta: String },
    ThinkingDelta { delta: String },
    ToolCallStart { id: String, name: String },
    ToolCallDelta { id: String, delta: String },
    ToolCallEnd { id: String },
    Usage { input: usize, output: usize },
    Done { reason: StopReason },
    Error(String),
}
```

#### Verification

- [ ] `cargo check --package rot-provider` passes
- [ ] `cargo clippy --package rot-provider` passes
- [ ] Add compile-time check that trait is object-safe

#### Implementation Notes

- Use `async_trait` macro for the trait
- `StreamEvent` must be `Clone` for TUI updates
- Reference architecture.md section 5.1

---

### T1.3: Tool Trait

**Crate**: `rot-tools`
**Depends on**: T0.3, T1.1
**Est. complexity**: Low

#### Goal

Define the tool trait and registry.

#### Files to Create

- `crates/rot-tools/src/lib.rs`
- `crates/rot-tools/src/traits.rs`
- `crates/rot-tools/src/registry.rs`

#### Key Types

```rust
pub struct ToolContext {
    pub working_dir: PathBuf,
    pub session_id: String,
    pub timeout: Duration,
}

pub struct ToolResult {
    pub output: String,
    pub metadata: Value,
    pub is_error: bool,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn label(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;

    async fn execute(&self, args: Value, ctx: &ToolContext)
        -> Result<ToolResult, ToolError>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}
```

#### Verification

- [ ] `cargo check --package rot-tools` passes
- [ ] `cargo test --package rot-tools` passes
- [ ] Add test for registry registration

#### Implementation Notes

- Use `schemars` for JSON schema generation
- Registry should support dynamic tool registration
- Reference architecture.md section 6.1

---

### T1.4: Session Types

**Crate**: `rot-session`
**Depends on**: T0.3, T1.1
**Est. complexity**: Medium

#### Goal

Define session entry types for JSONL persistence.

#### Files to Create

- `crates/rot-session/src/lib.rs`
- `crates/rot-session/src/format.rs`

#### Key Types

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SessionEntry {
    #[serde(rename = "session/start")]
    SessionStart {
        id: String,
        timestamp: u64,
        cwd: String,
        model: String,
        provider: String,
    },

    #[serde(rename = "message")]
    Message {
        id: String,
        parent_id: Option<String>,
        timestamp: u64,
        role: String,
        content: Vec<ContentBlock>,
    },

    #[serde(rename = "tool_call")]
    ToolCall { /* ... */ },

    #[serde(rename = "tool_result")]
    ToolResult { /* ... */ },

    #[serde(rename = "compaction")]
    Compaction { /* ... */ },
}
```

#### Verification

- [ ] `cargo check --package rot-session` passes
- [ ] `cargo test --package rot-session` passes
- [ ] Add serde roundtrip tests

#### Implementation Notes

- Use `#[serde(tag = "type")]` for discriminated union
- All timestamps are Unix epoch seconds
- Reference architecture.md section 7.1

---

## Phase 2: Provider Implementation

### T2.1: Anthropic Provider (Streaming)

**Crate**: `rot-provider`
**Depends on**: T1.2
**Est. complexity**: High
**Release target**: v0.1.0 (MVP)

#### Goal

Implement the Anthropic provider with SSE streaming support.

#### Files to Create

- `crates/rot-provider/src/providers/mod.rs`
- `crates/rot-provider/src/providers/anthropic.rs`
- `crates/rot-provider/src/streaming.rs`

#### Key Implementation

```rust
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self { /* ... */ }

    fn to_anthropic_request(&self, request: Request) -> Value { /* ... */ }

    fn parse_sse_event(&self, data: &str) -> Option<StreamEvent> { /* ... */ }
}

#[async_trait]
impl Provider for AnthropicProvider {
    // Implement all trait methods
}
```

#### Verification

- [ ] `cargo check --package rot-provider` passes
- [ ] `cargo test --package rot-provider` passes
- [ ] Add integration test with mock server (use `wiremock` crate)
- [ ] Test streaming parses correctly

#### Implementation Notes

- Use `reqwest::Client` with `rustls-tls` feature
- Parse SSE events line by line
- Handle `content_block_delta` events for text/tool deltas
- Reference architecture.md section 5.2
- API endpoint: `POST https://api.anthropic.com/v1/messages`
- Required headers: `x-api-key`, `anthropic-version: 2023-06-01`

---

### T2.2: OpenAI Provider

**Crate**: `rot-provider`
**Depends on**: T2.1
**Est. complexity**: Medium
**Release target**: v1.0

#### Goal

Implement the OpenAI provider with streaming.

#### Files to Create

- `crates/rot-provider/src/providers/openai.rs`

#### Verification

- [ ] `cargo check --package rot-provider` passes
- [ ] `cargo test --package rot-provider` passes

#### Implementation Notes

- OpenAI uses different SSE format than Anthropic
- Handle `data: [DONE]` as stream end
- Support both `/chat/completions` and `/responses` endpoints

---

### T2.3: Ollama Provider (Local Models)

**Crate**: `rot-provider`
**Depends on**: T2.1
**Est. complexity**: Low
**Release target**: v1.0

#### Goal

Implement provider for local Ollama models.

#### Files to Create

- `crates/rot-provider/src/providers/ollama.rs`

#### Verification

- [ ] `cargo check --package rot-provider` passes

#### Implementation Notes

- Default base URL: `http://localhost:11434`
- Ollama uses OpenAI-compatible API at `/v1/chat/completions`

---

### T2.4: Google Provider

**Crate**: `rot-provider`
**Depends on**: T2.1
**Est. complexity**: Medium
**Release target**: v1.0

#### Goal

Implement the Google Gemini/Vertex AI provider with streaming.

#### Files to Create

- `crates/rot-provider/src/providers/google.rs`

#### Verification

- [ ] `cargo check --package rot-provider` passes
- [ ] `cargo test --package rot-provider` passes

#### Implementation Notes

- Support both Gemini API and Vertex AI endpoints
- Use `generateContent` with SSE streaming
- Handle Google's tool call format (function calling)

---

### T2.5: OpenRouter Provider

**Crate**: `rot-provider`
**Depends on**: T2.1
**Est. complexity**: Low
**Release target**: v1.0

#### Goal

Implement the OpenRouter provider (OpenAI-compatible API).

#### Files to Create

- `crates/rot-provider/src/providers/openrouter.rs`

#### Verification

- [ ] `cargo check --package rot-provider` passes

#### Implementation Notes

- OpenRouter uses OpenAI-compatible API
- Base URL: `https://openrouter.ai/api/v1`
- Additional headers: `HTTP-Referer`, `X-Title`
- Can reuse most of OpenAI provider logic

---

## Phase 3: Tool Implementation

### T3.1: Read Tool

**Crate**: `rot-tools`
**Depends on**: T1.3
**Est. complexity**: Low

#### Goal

Implement the file reading tool.

#### Files to Create

- `crates/rot-tools/src/builtin/mod.rs`
- `crates/rot-tools/src/builtin/read.rs`

#### Parameters Schema

```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ReadParams {
    pub path: String,
    #[serde(default)]
    pub offset: Option<usize>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}
```

#### Verification

- [ ] `cargo test --package rot-tools` passes
- [ ] Test reading files with offset/limit
- [ ] Test path traversal prevention

#### Implementation Notes

- Default limit: 2000 lines
- Truncate output at 50KB
- Security: prevent reading outside working directory

---

### T3.2: Write Tool

**Crate**: `rot-tools`
**Depends on**: T3.1
**Est. complexity**: Low

#### Goal

Implement file creation/overwrite tool.

#### Files to Create

- `crates/rot-tools/src/builtin/write.rs`

#### Verification

- [ ] `cargo test --package rot-tools` passes
- [ ] Test creating new files
- [ ] Test overwriting existing files

---

### T3.3: Edit Tool

**Crate**: `rot-tools`
**Depends on**: T3.1
**Est. complexity**: Medium

#### Goal

Implement surgical file editing with string replacement.

#### Files to Create

- `crates/rot-tools/src/builtin/edit.rs`

#### Parameters Schema

```rust
pub struct EditParams {
    pub path: String,
    pub old_string: String,
    pub new_string: String,
    #[serde(default)]
    pub replace_all: bool,
}
```

#### Verification

- [ ] `cargo test --package rot-tools` passes
- [ ] Test single replacement
- [ ] Test replace_all
- [ ] Test error when old_string not found
- [ ] Test error when old_string appears multiple times without replace_all

---

### T3.4: Bash Tool

**Crate**: `rot-tools`
**Depends on**: T1.3
**Est. complexity**: Medium

#### Goal

Implement shell command execution.

#### Files to Create

- `crates/rot-tools/src/builtin/bash.rs`

#### Verification

- [ ] `cargo test --package rot-tools` passes
- [ ] Test successful command
- [ ] Test failed command (is_error = true)
- [ ] Test timeout handling
- [ ] Test output truncation

#### Implementation Notes

- Default timeout: 120 seconds
- Use `tokio::process::Command`
- Truncate output at 50KB
- Cross-platform: use `bash -c` on Unix (fallback `sh -c`), `cmd /C` on Windows

---

### T3.5: Glob Tool

**Crate**: `rot-tools`
**Depends on**: T1.3
**Est. complexity**: Low

#### Goal

Implement file pattern matching.

#### Files to Create

- `crates/rot-tools/src/builtin/glob.rs`

#### Verification

- [ ] `cargo test --package rot-tools` passes
- [ ] Test `**/*.rs` pattern
- [ ] Test `.gitignore` awareness

#### Implementation Notes

- Use `ignore` crate for `.gitignore` support
- Return file paths relative to working directory

---

### T3.6: Grep Tool

**Crate**: `rot-tools`
**Depends on**: T1.3
**Est. complexity**: Medium

#### Goal

Implement content search with regex.

#### Files to Create

- `crates/rot-tools/src/builtin/grep.rs`

#### Verification

- [ ] `cargo test --package rot-tools` passes
- [ ] Test basic regex search
- [ ] Test file pattern filtering
- [ ] Test context lines (-A, -B, -C)

#### Implementation Notes

- Use `regex` crate
- Support file pattern: `--include "*.rs"`
- Return format: `file.rs:123:matching line`

---

### T3.7: WebFetch Tool

**Crate**: `rot-tools`
**Depends on**: T1.3
**Est. complexity**: Low

#### Goal

Implement URL fetching.

#### Files to Create

- `crates/rot-tools/src/builtin/webfetch.rs`

#### Verification

- [ ] `cargo test --package rot-tools` passes
- [ ] Test fetching HTML
- [ ] Test fetching JSON
- [ ] Test timeout handling

---

## Phase 4: Session Management

### T4.1: Session Store

**Crate**: `rot-session`
**Depends on**: T1.4
**Est. complexity**: Medium

#### Goal

Implement JSONL session persistence.

#### Files to Create

- `crates/rot-session/src/store.rs`

#### Key Implementation

```rust
pub struct SessionStore {
    sessions_dir: PathBuf,
}

impl SessionStore {
    pub async fn create(&self, cwd: &Path, model: &str, provider: &str)
        -> Result<Session>;

    pub async fn load(&self, id: &str) -> Result<Session>;

    pub async fn append(&self, session: &mut Session, entry: SessionEntry)
        -> Result<()>;

    pub async fn list_recent(&self, limit: usize) -> Result<Vec<SessionMeta>>;
}

pub struct Session {
    pub id: String,
    pub file_path: PathBuf,
    pub cwd: PathBuf,
    pub entries: Vec<SessionEntry>,
    pub current_leaf: String,
}
```

#### Verification

- [ ] `cargo test --package rot-session` passes
- [ ] Test session creation
- [ ] Test session loading
- [ ] Test entry appending
- [ ] Test listing sessions

#### Implementation Notes

- Session directory: `~/.local/share/rot/sessions/<cwd-hash>/<id>.jsonl`
- Use `blake3` for cwd hashing
- Each entry is one JSON line

---

## Phase 5: Agent Core

### T5.1: Agent Loop

**Crate**: `rot-core`
**Depends on**: T1.1, T1.2, T1.3, T4.1
**Est. complexity**: High

#### Goal

Implement the main agent processing loop.

#### Files to Create

- `crates/rot-core/src/agent.rs`
- `crates/rot-core/src/context.rs`

#### Key Implementation

```rust
pub struct Agent {
    provider: Box<dyn Provider>,
    tools: ToolRegistry,
    config: AgentConfig,
}

impl Agent {
    pub async fn process(
        &mut self,
        session: &mut Session,
        user_input: String,
    ) -> Result<Message>;
}
```

#### Verification

- [ ] `cargo test --package rot-core` passes
- [ ] Test basic message processing
- [ ] Test tool call execution
- [ ] Test max iterations limit

#### Implementation Notes

- Main loop: send to provider → parse response → execute tools → repeat
- Use `futures_util::StreamExt` for streaming
- Reference architecture.md section 3.2

---

### T5.2: Permission System

**Crate**: `rot-core`
**Depends on**: T1.3
**Est. complexity**: Medium
**Release target**: v1.0

#### Goal

Implement permission checking for tool execution.

#### Files to Create

- `crates/rot-core/src/permission.rs`

#### Key Types

```rust
pub enum Action {
    Allow,
    Deny,
    Ask,
}

pub struct PermissionRule {
    pub tool: String,       // "read", "bash", or "*"
    pub pattern: String,    // glob pattern for paths
    pub action: Action,
}

pub struct PermissionSystem {
    rules: Vec<PermissionRule>,
}

impl PermissionSystem {
    pub fn check(&self, tool: &str, args: &Value) -> Result<(), PermissionError>;
}
```

#### Verification

- [x] `cargo test --package rot-core` passes
- [x] Test allow rules
- [x] Test deny rules
- [x] Test pattern matching

---

## Phase 6: RLM Engine

### T6.1: Context Manager

**Crate**: `rot-rlm`
**Depends on**: T1.1
**Est. complexity**: Medium
**Release target**: v0.1.0 (MVP)

#### Goal

Implement external context storage for RLM.

#### Files to Create

- `crates/rot-rlm/src/lib.rs`
- `crates/rot-rlm/src/context.rs`

#### Key Implementation

```rust
pub struct ContextManager {
    temp_dir: PathBuf,
}

impl ContextManager {
    pub async fn store(&mut self, content: &str) -> Result<String>;
    pub async fn load(&self, var_name: &str) -> Result<String>;
    pub fn build_metadata(&self, content: &str) -> String;
}
```

#### Verification

- [ ] `cargo test --package rot-rlm` passes
- [ ] Test storing large content
- [ ] Test metadata generation

---

### T6.2: REPL Environment

**Crate**: `rot-rlm`
**Depends on**: T6.1
**Est. complexity**: High
**Release target**: v1.0

#### Goal

Implement shell-based REPL environment for RLM.

#### Files to Create

- `crates/rot-rlm/src/repl.rs`

#### Key Implementation

```rust
pub struct ReplEnv {
    shell: String,
    working_dir: PathBuf,
    temp_dir: PathBuf,
}

pub struct ReplResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

impl ReplEnv {
    pub async fn init(&mut self, context_path: &str) -> Result<()>;
    pub async fn execute(&mut self, code: &str) -> Result<ReplResult>;
    pub async fn get_var(&self, name: &str) -> Result<Option<String>>;
}
```

#### Verification

- [ ] `cargo test --package rot-rlm` passes
- [ ] Test code execution
- [ ] Test helper functions (llm_query stubs)

#### Implementation Notes

- Use `portable-pty` or direct shell execution
- Generate setup script with helper functions
- Parse FINAL and FINAL_VAR from output

---

### T6.3: RLM Engine

**Crate**: `rot-rlm`
**Depends on**: T6.1, T6.2, T1.2
**Est. complexity**: High
**Release target**: v1.0

#### Goal

Implement the full RLM processing loop.

#### Files to Create

- `crates/rot-rlm/src/engine.rs`
- `crates/rot-rlm/src/prompts.rs`

#### Key Implementation

```rust
pub struct RlmEngine {
    config: RlmConfig,
    provider: Arc<dyn Provider>,
    repl: ReplEnv,
    context_manager: ContextManager,
}

impl RlmEngine {
    pub async fn process(&mut self, prompt: &str) -> Result<String>;
}
```

#### Verification

- [ ] `cargo test --package rot-rlm` passes
- [ ] Test with mock provider
- [ ] Test iteration limit
- [ ] Test FINAL extraction

#### Implementation Notes

- Reference architecture.md section 4
- System prompt in `prompts.rs`
- Extract code blocks with regex: ` ```repl\n(.*?)\n``` `

---

## Phase 7: TUI

### T7.1: TUI Foundation

**Crate**: `rot-tui`
**Depends on**: T1.1
**Est. complexity**: Medium

#### Goal

Set up ratatui application structure.

#### Files to Create

- `crates/rot-tui/src/lib.rs`
- `crates/rot-tui/src/app.rs`
- `crates/rot-tui/src/event.rs`

#### Key Types

```rust
pub struct App {
    pub state: AppState,
    pub messages: MessagesWidget,
    pub editor: EditorWidget,
    pub status: StatusWidget,
    pub running: bool,
    pub input_mode: InputMode,
}

pub enum AppState {
    Idle,
    Thinking,
    Streaming,
    Error,
}
```

#### Verification

- [ ] `cargo check --package rot-tui` passes
- [ ] Test app creation

---

### T7.2: Message Widget

**Crate**: `rot-tui`
**Depends on**: T7.1
**Est. complexity**: Medium

#### Goal

Implement scrollable message display.

#### Files to Create

- `crates/rot-tui/src/widgets/mod.rs`
- `crates/rot-tui/src/widgets/messages.rs`

#### Verification

- [ ] `cargo check --package rot-tui` passes

---

### T7.3: Editor Widget

**Crate**: `rot-tui`
**Depends on**: T7.1
**Est. complexity**: Medium

#### Goal

Implement multi-line input editor.

#### Files to Create

- `crates/rot-tui/src/widgets/editor.rs`

#### Verification

- [ ] `cargo check --package rot-tui` passes

---

### T7.4: Status Widget

**Crate**: `rot-tui`
**Depends on**: T7.1
**Est. complexity**: Low

#### Goal

Implement status bar with context info.

#### Files to Create

- `crates/rot-tui/src/widgets/status.rs`

#### Verification

- [ ] `cargo check --package rot-tui` passes

---

### T7.5: Event Handling

**Crate**: `rot-tui`
**Depends on**: T7.1
**Est. complexity**: Medium

#### Goal

Implement keyboard event handling.

#### Files to Create

- Update `crates/rot-tui/src/app.rs`
- Update `crates/rot-tui/src/event.rs`

#### Key Bindings

| Key      | Action             |
| -------- | ------------------ |
| `Ctrl+C` | Cancel/Quit        |
| `Enter`  | Submit input       |
| `Esc`    | Normal mode        |
| `i`      | Insert mode        |
| `:`      | Command mode       |
| `j/k`    | Scroll messages    |
| `q`      | Quit (normal mode) |

#### Verification

- [ ] `cargo test --package rot-tui` passes

---

## Phase 8: CLI

### T8.1: CLI Structure

**Crate**: `rot-cli`
**Depends on**: all previous
**Est. complexity**: Medium

#### Goal

Implement CLI argument parsing and commands.

#### Files to Create

- `crates/rot-cli/src/main.rs`
- `crates/rot-cli/src/cli.rs`

#### Key Commands

```bash
rot                     # Interactive chat (default)
rot chat                # Interactive chat
rot exec "prompt"       # Single execution
rot session list        # List sessions
rot session resume ID   # Resume session
rot --provider anthropic --model claude-sonnet-4
```

#### Verification

- [ ] `cargo run --package rot-cli -- --help` works
- [ ] `cargo run --package rot-cli -- chat` starts TUI

---

### T8.2: Chat Command

**Crate**: `rot-cli`
**Depends on**: T8.1
**Est. complexity**: High

#### Goal

Wire together all components for interactive chat.

#### Files to Create

- `crates/rot-cli/src/commands/mod.rs`
- `crates/rot-cli/src/commands/chat.rs`

#### Verification

- [ ] `cargo run --package rot-cli` starts interactive session
- [ ] Can send message and receive response
- [ ] Session persists on exit

---

### T8.3: Exec Command

**Crate**: `rot-cli`
**Depends on**: T8.1
**Est. complexity**: Low

#### Goal

Implement single-shot execution mode.

#### Files to Create

- `crates/rot-cli/src/commands/exec.rs`

#### Verification

- [ ] `cargo run --package rot-cli -- exec "read main.rs"` works

---

## Phase 9: Integration & Polish

### T9.1: End-to-End Test

**Crate**: root
**Depends on**: T8.2
**Est. complexity**: Medium

#### Goal

Create integration tests for full workflow.

#### Files to Create

- `tests/integration_test.rs`
- `tests/fixtures/` - Test project files

#### Verification

- [ ] `cargo test` passes all integration tests

---

### T9.2: Documentation

**Crate**: root
**Depends on**: all
**Est. complexity**: Medium

#### Goal

Create user documentation.

#### Files to Create

- `README.md` - Installation and quick start
- `docs/getting-started.md`
- `docs/configuration.md`
- `docs/tools.md`
- `docs/rlm.md`

#### Verification

- [ ] All commands documented
- [ ] Examples work

---

### T9.3: Release Build

**Crate**: root
**Depends on**: all
**Est. complexity**: Low

#### Goal

Configure release build and test binaries.

#### Files to Modify

- `Cargo.toml` - Add release profile

#### Verification

- [ ] `cargo build --release` produces binary < 20MB
- [ ] Binary starts in < 100ms
- [ ] Works on Linux, macOS, Windows

---

## MVP Scope

> [!NOTE]
> Per pdr.md, the **MVP (v0.1.0)** includes Anthropic-only provider support plus
> RLM foundation. Additional providers, permission rules, and full RLM are **v1.0**.

| Version      | Phases Included                                |
| ------------ | ---------------------------------------------- |
| v0.1.0 (MVP) | Phase 0–5 + T6.1, excluding T2.2–T2.5 and T5.2 |
| v1.0         | T2.2–T2.5, T5.2, T6.2–T6.3, Phase 7–9          |

---

## Task Dependency Graph

```
T0.1 (Workspace) ─┬─► T0.2 (AGENTS.md)
                  ├─► T0.3 (Errors)
                  │
T0.3 ─────────────┼─► T1.1 (Messages) ─┬─► T1.2 (Provider) ─► T2.1 (Anthropic)
                  │                     │                     ├─► T2.2 (OpenAI)
                  │                     │                     ├─► T2.3 (Ollama)
                  │                     │                     ├─► T2.4 (Google)
                  │                     │                     └─► T2.5 (OpenRouter)
                  │                     │
                  ├─► T1.3 (Tool) ──────┼─► T3.1 (Read)
                  │                     ├─► T3.2 (Write)
                  │                     ├─► T3.3 (Edit)
                  │                     ├─► T3.4 (Bash)
                  │                     ├─► T3.5 (Glob)
                  │                     ├─► T3.6 (Grep)
                  │                     └─► T3.7 (WebFetch)
                  │
                  └─► T1.4 (Session) ───► T4.1 (Store)
                                        │
T1.1 + T1.2 + T1.3 + T4.1 ──────────────┼─► T5.1 (Agent)
                                        └─► T5.2 (Permission)

T1.1 ──────────────────────────────────► T6.1 (Context) ─► T6.2 (REPL) ─► T6.3 (RLM)

T1.1 ──────────────────────────────────► T7.1 (TUI) ─► T7.2 (Messages)
                                                       ├─► T7.3 (Editor)
                                                       ├─► T7.4 (Status)
                                                       └─► T7.5 (Events)

All ───────────────────────────────────► T8.1 (CLI) ─► T8.2 (Chat) ─► T8.3 (Exec)
                                                       │
                                                       └─► T9.1 (Tests)
                                                           T9.2 (Docs)
                                                           T9.3 (Release)
```

---

## Execution Checklist

### Before Starting Each Task

- [ ] Read AGENTS.md for project context
- [ ] Read this plan for task details
- [ ] Check dependencies are complete
- [ ] Read referenced architecture.md sections

### After Completing Each Task

- [ ] `cargo check` passes
- [ ] `cargo test` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] Commit with message: `feat(crate): description`
- [ ] Update AGENTS.md with any new patterns/gotchas

### When Stuck

1. After 2 failed attempts: re-read the task and architecture.md
2. After 3 failed attempts: update AGENTS.md with what you learned
3. After 4 failed attempts: ask for clarification

---

## Progress Tracking

| Phase         | Tasks     | Status                        |
| ------------- | --------- | ----------------------------- |
| 0. Setup      | T0.1–T0.3 | ✅ Complete                   |
| 1. Core Types | T1.1–T1.4 | ✅ Complete                   |
| 2. Providers  | T2.1–T2.5 | 🟡 T2.1 done (T2.2–T2.5 v1.0) |
| 3. Tools      | T3.1–T3.7 | ✅ Complete                   |
| 4. Session    | T4.1      | ✅ Complete                   |
| 5. Agent      | T5.1–T5.2 | 🟡 T5.1 done (T5.2 v1.0)      |
| 6. RLM        | T6.1–T6.3 | 🟡 T6.1 done (T6.2–T6.3 v1.0) |
| 7. TUI        | T7.1–T7.5 | ✅ Complete                   |
| 8. CLI        | T8.1–T8.3 | ✅ Complete                   |
| 9. Polish     | T9.1–T9.3 | ✅ Complete                   |

---

## Document History

| Version | Date       | Changes                                                                                                          |
| ------- | ---------- | ---------------------------------------------------------------------------------------------------------------- |
| 1.0     | 2026-02-25 | Initial plan created                                                                                             |
| 1.1     | 2026-02-25 | Fixed: T0.3 error types, T3.6 dependency, missing T2.4/T2.5 providers, workspace deps, MVP scope, layout diagram |
| 1.2     | 2026-02-25 | Normalized roadmap: MVP provider scope, RLM phases, permission timing, and `rot-cli` binary scaffolding          |
