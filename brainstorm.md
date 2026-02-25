# rot - Brainstorming Document

> Recursive Operations Tool - A Rust-based AI coding agent with RLM implementation

## Vision Statement

**rot (Recursive Operations Tool)** is a Rust-based AI coding agent CLI that differentiates through full RLM (Recursive Language Model) implementation, enabling context handling 100x beyond model limits. It ships as a single binary with zero runtime dependencies.

---

## Why Build This?

### Market Gap Analysis

| Gap | Description |
|-----|-------------|
| **No Rust coding agent** | All major coding agents (pi, opencode, Claude Code, Aider) are TypeScript/Node-based |
| **Runtime dependencies** | Existing tools require Node.js/Bun runtime, complicating distribution |
| **Context limitations** | Current agents use compaction/summarization, losing information density |
| **RLM unexplored** | The Recursive Language Model paper presents untapped opportunity |

### User Confirmed Goals (from brainstorming session)

| Goal | Priority | Rationale |
|------|----------|-----------|
| Distribution simplicity | High | Single binary, no runtime dependencies |
| Open source project | High | Community-driven development |
| RLM differentiation | High | Unique recursive context handling |
| Long-term project | Medium | Build sustainably over time |

---

## Reference Projects Analyzed

### pi-mono (TypeScript/Node.js)

**Repository**: https://github.com/badlogic/pi-mono

#### Architecture Overview

```
pi-mono/
├── packages/
│   ├── ai/              # Unified LLM API (@mariozechner/pi-ai)
│   ├── agent/           # Agent runtime (@mariozechner/pi-agent-core)
│   ├── coding-agent/    # CLI + interactive mode
│   ├── tui/             # Terminal UI library
│   ├── web-ui/          # Web chat components
│   ├── mom/             # Slack bot
│   └── pods/            # vLLM deployment CLI
```

#### Key Design Patterns

| Pattern | Implementation | Benefit |
|---------|----------------|---------|
| **TypeBox schemas** | Tool parameters with type safety + serialization | Runtime validation, LLM-compatible |
| **JSONL tree sessions** | In-place branching without file duplication | Full history, git-friendly |
| **Differential rendering** | Only redraw changed lines | Flicker-free TUI |
| **Extension system** | TypeScript modules with event hooks | Maximum flexibility |

#### Philosophy (Explicit Non-Features)

> "Pi is aggressively extensible so it doesn't have to dictate your workflow."

- **No MCP**: Build CLI tools with READMEs (Skills) or extensions
- **No sub-agents**: Spawn via tmux or build with extensions
- **No permission popups**: Use containers or build custom confirmation
- **No plan mode**: Write plans to files or build with extensions
- **No built-in to-dos**: Confuse models; use TODO.md or extensions

#### Provider System

```typescript
// 20+ providers supported
type KnownApi = 
  | "anthropic-messages"
  | "google-generative-ai"
  | "google-gemini-cli"
  | "google-vertex"
  | "openai-completions"
  | "openai-responses"
  | "azure-openai-responses"
  | "bedrock-converse-stream";
```

#### Session Management

- **Storage**: `~/.pi/agent/sessions/--<path-hash>--/<timestamp>_<uuid>.jsonl`
- **Tree structure**: Each entry has `id` and `parentId`
- **Compaction**: Triggered at `contextWindow - reserveTokens` (default 16k)

#### Extension API

```typescript
interface ExtensionAPI {
  on(event, handler): void;
  registerTool(definition): void;
  registerCommand(name, options): void;
  registerShortcut(shortcut, options): void;
  sendMessage(message): void;
  appendEntry(customType, data?): void;
}
```

#### Compaction Strategy

```markdown
## Summary Format
- Goal
- Constraints & Preferences
- Progress (Done / In Progress / Blocked)
- Key Decisions
- Next Steps
- Critical Context
- Modified files list
```

---

### opencode (TypeScript/Bun)

**Repository**: https://github.com/anomalyco/opencode

#### Architecture Overview

```
opencode/
├── packages/
│   ├── opencode/        # Core package - backend logic
│   ├── app/             # Web application (SolidJS)
│   ├── desktop/         # Tauri desktop app
│   ├── console/         # Console/web infrastructure
│   ├── sdk/js/          # JavaScript SDK client
│   ├── plugin/          # Plugin development kit
│   ├── ui/              # Shared UI components
│   ├── util/            # Shared utilities
│   └── containers/      # Docker containers
```

#### Tech Stack

| Component | Technology |
|-----------|------------|
| Runtime | Bun 1.3.9+ |
| Language | TypeScript 5.8 |
| Build | Turborepo 2.5.6 |
| HTTP Server | Hono |
| Database | SQLite (Drizzle ORM) |
| UI | SolidJS |
| AI SDK | Vercel AI SDK v5 |
| Desktop | Tauri |

#### Client/Server Architecture

- **Default port**: 4096
- **Protocol**: HTTP with SSE for events
- **mDNS discovery**: For local clients
- **WebSocket**: Real-time updates

#### Key Abstractions

```typescript
// Instance State - Per-directory lazy singleton
const state = Instance.state(
  async () => initializeState(),
  async (state) => cleanupState()
)

// Bus Events - Type-safe pub/sub with Zod
const Event = BusEvent.define("session.created", z.object({...}))
Bus.publish(Event, { info })

// Database Effects - Deferred side effects
Database.use((db) => {
  db.insert(...).run()
  Database.effect(() => Bus.publish(...))
})
```

#### Tool System

| Tool | Purpose |
|------|---------|
| `bash` | Shell command execution with PTY |
| `read` | File reading with offset/limit |
| `write` | File writing |
| `edit` | File editing with diff |
| `apply_patch` | Apply unified diff patches |
| `glob` | File pattern matching |
| `grep` | Content search |
| `task` | Subagent delegation |
| `webfetch` | URL fetching |
| `websearch` | Web search |
| `lsp` | LSP operations |
| `question` | User questions |

#### Permission System

```typescript
type Action = "allow" | "deny" | "ask"

// Rule-based with pattern matching
const ruleset = PermissionNext.fromConfig({
  "read": "allow",
  "edit": { "*.env": "deny" },
  "bash": "ask"
})
```

#### LSP Integration

- Pre-configured servers for common languages
- Custom servers via `opencode.json`
- Operations: hover, definition, references, symbols, diagnostics

#### MCP Integration

- **StdioClientTransport**: Local MCP servers
- **SSEClientTransport**: Remote via SSE
- **StreamableHTTPClientTransport**: Remote via HTTP streaming

---

### RLM Paper (Recursive Language Models)

**Paper**: "Recursive Language Models" - Alex L. Zhang, Tim Kraka, Omar Khattab (MIT CSAIL)
**Repository**: https://github.com/alexzhang13/rlm

#### Core Innovation

> Treat long prompts as part of an external environment, allowing the LLM to programmatically examine, decompose, and recursively call itself over snippets of the prompt.

#### Algorithm

```
function RLM_COMPLETION(query, context, config):
    environment = create_repl_environment(context)
    message_history = build_system_prompt(query, context_metadata)
    
    for iteration in range(config.max_iterations):
        response = llm.completion(message_history)
        code_blocks = find_code_blocks(response, pattern="```repl")
        
        for code_block in code_blocks:
            result = environment.execute_code(code_block)
        
        final_answer = find_final_answer(response, environment)
        if final_answer:
            return final_answer
    
    return generate_default_answer(message_history)
```

#### Key Design Choices

| Choice | Description | Why It Matters |
|--------|-------------|----------------|
| **Prompt as variable** | Store in REPL, not LLM context | Unbounded input size |
| **Metadata only** | LLM sees length, preview, not content | Forces programmatic access |
| **Symbolic recursion** | Code invokes sub-LLM calls | Complex decomposition |
| **REPL environment** | Python/bash execution | Computation + transformation |

#### REPL Functions

```python
# Available in REPL environment
llm_query(prompt, model=None)        # Single LLM call (fast, one-shot)
llm_query_batched(prompts, model=None)  # Parallel LLM calls
rlm_query(prompt, model=None)        # Recursive RLM sub-call (deep thinking)
rlm_query_batched(prompts, model=None)  # Parallel recursive calls
FINAL(answer)                        # Return final answer
FINAL_VAR(variable_name)             # Return variable as answer
SHOW_VARS()                          # List available variables
```

#### When to Use llm_query vs rlm_query

| Function | Use Case | Depth | Has REPL |
|----------|----------|-------|----------|
| `llm_query` | Simple extraction, summarization, Q&A | 1 (flat) | No |
| `llm_query_batched` | Parallel simple queries | 1 (flat) | No |
| `rlm_query` | Multi-step reasoning, sub-problems | Recursive | Yes |
| `rlm_query_batched` | Parallel complex subtasks | Recursive | Yes |

#### Benchmark Results

| Method | 132k tokens (OOLONG) | 263k tokens | Cost |
|--------|---------------------|-------------|------|
| GPT-5 | 30.2 | 30.8 | Baseline |
| GPT-5-mini | 25.1 | 18.3 | $0.3X |
| **RLM(GPT-5-mini)** | **64.3** | **45.9** | ~Baseline |
| RLM no sub-calls | 54.1 | 40.2 | Lower |

**Key Finding**: RLM(GPT-5-mini) outperforms GPT-5 by 34 points (114% increase) at 132k tokens

#### Emergent Patterns in RLM Trajectories

1. **Chunking + recursive sub-calling**: Defer reasoning to sub-LLM calls
2. **Filtering via regex**: Reduce search space before LLM processing
3. **Passing outputs through variables**: Build long outputs iteratively
4. **Model prior exploitation**: Use regex on known patterns (e.g., "La Union")

#### Training RLM-Qwen3-8B

- Fine-tuned on 1,000 trajectories from Qwen3-Coder-480B
- Outperforms base Qwen3-8B by **28.3%** on average
- Approaches vanilla GPT-5 quality on three long-context tasks

---

## Key Design Decisions

### Confirmed Choices

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **Language** | Rust | Single binary, no runtime, fast startup |
| **Architecture** | Single binary CLI | Simpler than client/server for MVP |
| **LLM SDK** | Custom (like pi-ai) | Full control, no vendor lock-in |
| **Context Strategy** | RLM-first roadmap (foundation in MVP, full in v1.0) | Preserve differentiation with phased delivery |
| **Session Storage** | JSONL files | Human-readable, git-friendly, simple |
| **Extensibility** | Plugin-ready architecture (plugins in v2.0) | Keep MVP focused while preserving extension path |
| **REPL for RLM** | System shell (bash/zsh) | Leverage existing tools, no sandbox complexity |
| **TUI Framework** | ratatui | Modern, active development, immediate mode |
| **Database** | None (JSONL only) | Simplicity for MVP |
| **License** | MIT | Maximum compatibility |

### Tradeoffs Considered

| Option A | Option B | Chosen | Reason |
|----------|----------|--------|--------|
| Client/server | Single binary | Single binary | Simpler distribution |
| SQLite sessions | JSONL files | JSONL | Human-readable, git-friendly |
| WASM plugins | Rust plugins | Rust | Performance, type safety |
| Embedded Python REPL | System shell | System shell | Simpler, leverages existing tools |
| Compaction only | Full RLM | Full RLM | Key differentiator |
| Multi-provider MVP | Single provider MVP | Single provider MVP (Anthropic) + provider-agnostic design | Lower MVP complexity, still future-proof |

---

## User Requirements (Confirmed)

### MVP Scope
- [x] RLM foundation (external context storage, 100k+ token handling)
- [x] All tools: read, write, edit, bash, glob, grep, webfetch
- [x] Anthropic provider (provider-agnostic architecture)
- [x] Basic TUI (messages + input)
- [x] JSONL session storage
- [x] System shell support for RLM context work

### User Preferences from Session

| Question | Answer |
|----------|--------|
| Why Rust? | Distribution simplicity |
| Target user? | OSS project |
| Differentiator? | Long context (RLM) |
| Timeline? | Long-term project |
| RLM approach? | Full implementation via phased delivery |
| MVP tools? | All of them |
| Providers? | Provider-agnostic design |
| TUI? | Full TUI with ratatui |
| Session storage? | JSONL |
| Extensibility? | Plugin-ready architecture (v2 plugins) |
| REPL for RLM? | System shell (bash/zsh) |
| Name? | Keep "rot" (Recursive Operations Tool) |

---

## Competitive Analysis

### Feature Matrix

| Feature | rot | pi | opencode | Claude Code | Aider |
|---------|-----|----|----------:|-------------|-------|
| **Language** | Rust | TypeScript | TypeScript/Bun | TypeScript | Python |
| **Distribution** | Single binary | npm | npm/brew | npm | pip |
| **Runtime deps** | None | Node.js | Bun | Node.js | Python |
| **RLM Support** | Full | None | None | None | None |
| **Providers** | 1 (MVP), 5+ (v1.0) | 20+ | 20+ | 1 (Anthropic) | 10+ |
| **TUI** | ratatui | Custom | Custom | Custom | Custom |
| **Plugins** | Planned (v2, Rust) | TypeScript | TypeScript | MCP | None |
| **Session storage** | JSONL | JSONL | SQLite | Unknown | Git |
| **LSP** | Planned | No | Yes | Yes | No |
| **MCP** | Planned | No | Yes | Yes | No |
| **Open source** | Yes | Yes | Yes | No | Yes |

### Competitive Positioning

| Competitor | rot Advantage | rot Disadvantage |
|------------|---------------|------------------|
| **pi** | Single binary, RLM | Fewer providers, smaller ecosystem |
| **opencode** | Single binary, RLM, simpler | Fewer features, smaller ecosystem |
| **Claude Code** | Multi-provider, open source, RLM | Less polished UX initially |
| **Aider** | Provider agnostic, RLM, any language | Less git-focused |

---

## UX Principles (from Research)

### Core Principles

1. **Transparency**
   - Show what the agent is doing
   - Display diffs before applying
   - Make reasoning visible (toggleable)
   - Clear status indicators

2. **Control**
   - Permission system with granularity
   - Easy undo/rewind
   - Multiple operation modes
   - Interrupt without losing work

3. **Context Awareness**
   - Smart file discovery
   - Repository understanding
   - Project conventions (AGENTS.md)
   - Session continuity

4. **Efficiency**
   - Minimize permission friction
   - Batch similar operations
   - Parallel execution where possible
   - Streaming responses

5. **Safety**
   - Checkpoints before changes
   - Git integration for rollback
   - Sandboxing options
   - Clear destructive action warnings

### UX Anti-Patterns to Avoid

| Anti-Pattern | Why It's Bad | Better Alternative |
|--------------|--------------|-------------------|
| No undo mechanism | Can't recover from mistakes | Git integration + checkpoints |
| Constant permission prompts | Workflow interruption | Session-wide approvals + sandboxing |
| Lost sessions | Can't resume work | Persistent JSONL storage |
| Hidden changes | Trust issues | Always show diffs |
| Context overflow | Degraded performance | RLM + auto-activation |
| No keyboard shortcuts | Slow interaction | Comprehensive shortcut system |
| Single-mode operation | One-size-fits-all fails | Plan mode + auto-accept + normal |
| No error recovery | Dead ends | Rewind + retry patterns |
| Overly long AGENTS.md | Rules get ignored | Keep concise, use skills |
| No parallel execution | Slow for large tasks | Subagents + concurrent tools |

---

## Rust Ecosystem Research

### Recommended Stack

| Category | Choice | Rationale |
|----------|--------|-----------|
| **Async runtime** | Tokio | Most popular, best ecosystem |
| **TUI framework** | ratatui | Modern, active, immediate mode |
| **HTTP client** | reqwest | Streaming support for LLM APIs |
| **JSON** | serde + serde_json | Standard, performant |
| **Schema generation** | schemars | Auto-generate from Rust types |
| **CLI parsing** | clap | Full-featured, derive macros |
| **Error handling** | thiserror + anyhow | Library + app pattern |
| **Logging** | tracing | Structured, async-aware |
| **Shell execution** | portable-pty | Cross-platform PTY |
| **Plugin system** | Trait objects + WASM | Built-in fast + third-party safe |

### TUI Framework Comparison

| Framework | Pros | Cons | Verdict |
|-----------|------|------|---------|
| **ratatui** | Active, immediate mode, modular | More manual work | **Recommended** |
| cursive | Retained mode, built-in views | Less active, harder streaming | Not recommended |
| makepad | Desktop/mobile focus | Not for TUI | Not applicable |

### Plugin System Options

| Approach | Pros | Cons | Use Case |
|----------|------|------|----------|
| **Trait objects** | Simple, fast, type-safe | Compile-time only | Built-in tools |
| **WASM (wasmtime)** | Sandboxed, runtime loading | Complexity, overhead | Third-party plugins |
| **libloading** | Native performance | Unsafe, platform-specific | Not recommended |

---

## Next Steps

1. **Create architecture.md** - Detailed technical architecture
2. **Create pdr.md** - Product design review
3. **Initialize Rust project** - Cargo workspace with crates
4. **Implement Phase 1** - Foundation (core types, Anthropic provider, basic loop)
5. **Iterate based on testing** - Real-world usage feedback

---

## References

- pi-mono: https://github.com/badlogic/pi-mono
- opencode: https://github.com/anomalyco/opencode
- RLM Paper: https://arxiv.org/abs/2512.24601
- RLM Code: https://github.com/alexzhang13/rlm
- ratatui: https://github.com/ratatui/ratatui
- tokio: https://tokio.rs
