# rot - Product Design Review (PDR)

> Recursive Operations Tool - Product Requirements & Design Specification

---

## Executive Summary

**rot** is an open-source Rust-based AI coding agent that uniquely handles contexts 100x beyond LLM limits through full Recursive Language Model (RLM) implementation. It ships as a single binary with zero runtime dependencies, targeting developers who value simplicity, performance, and privacy.

### Key Differentiators

1. **Zero Setup**: Single binary download, no runtime dependencies
2. **Infinite Context**: RLM handles 10M+ token inputs
3. **Provider Freedom**: Works with any LLM provider
4. **Native Performance**: Rust speed and memory safety

---

## Product Vision

### Mission Statement

> Make AI-assisted coding accessible to every developer, regardless of their preferred LLM provider or environment constraints, while solving the fundamental context limitation problem through recursive intelligence.

### Vision Statement

> rot becomes the go-to CLI coding agent for developers who value simplicity, performance, and privacy - distinguished by its ability to handle massive codebases and long-running sessions through RLM technology.

---

## Target Users

### Primary Personas

#### Persona 1: The Infrastructure Developer

| Attribute | Description |
|-----------|-------------|
| **Role** | DevOps/SRE/Platform Engineer |
| **Environment** | Remote servers, containers, restricted networks |
| **Pain Points** | Can't install Node.js, needs portable tools |
| **Goals** | Quick code changes, debugging, automation |
| **Why rot?** | Single binary, SSH-friendly, no dependencies |

**User Story**: "As a DevOps engineer managing hundreds of servers, I need an AI coding assistant that I can scp to any machine and run immediately without asking IT to install Node.js."

#### Persona 2: The Rust Enthusiast

| Attribute | Description |
|-----------|-------------|
| **Role** | Rust Developer / Systems Programmer |
| **Environment** | Linux workstation, values native tooling |
| **Pain Points** | TypeScript tools feel foreign, slow |
| **Goals** | Native performance, Rust ecosystem integration |
| **Why rot?** | Written in Rust, fast, hackable |

**User Story**: "As a Rust developer, I want an AI coding tool that feels native to my ecosystem - fast startup, minimal memory, and I can contribute PRs in my preferred language."

#### Persona 3: The Privacy-Conscious Developer

| Attribute | Description |
|-----------|-------------|
| **Role** | Security Engineer / Privacy Advocate |
| **Environment** | Air-gapped or restricted networks |
| **Pain Points** | Cloud-dependent tools, data concerns |
| **Goals** | Local-first, provider choice, transparency |
| **Why rot?** | Local processing, Ollama support, open source |

**User Story**: "As a security engineer, I need an AI assistant that can work with local models and doesn't require sending my codebase to external services."

### Secondary Personas

#### Persona 4: The AI Researcher

| Attribute | Description |
|-----------|-------------|
| **Role** | ML Researcher / AI Engineer |
| **Environment** | Research lab, experimentation |
| **Pain Points** | Context limitations in experiments |
| **Goals** | Test RLM patterns, long-context research |
| **Why rot?** | First RLM implementation, research platform |

#### Persona 5: The Open Source Contributor

| Attribute | Description |
|-----------|-------------|
| **Role** | OSS Maintainer / Contributor |
| **Environment** | Multiple projects, diverse stacks |
| **Pain Points** | Expensive tools, vendor lock-in |
| **Goals** | Free, open, community-driven |
| **Why rot?** | MIT licensed, open development |

---

## Core Value Propositions

### For All Users

| Value | Description | Evidence |
|-------|-------------|----------|
| **Zero Setup** | Download and run, no installation | Single binary < 20MB |
| **Speed** | Instant startup, responsive TUI | Rust native, < 100ms startup |
| **Freedom** | Use any LLM provider | 5+ providers at launch |
| **Transparency** | See exactly what happens | Open source, verbose mode |

### For Specific Segments

| Segment | Value | Why It Matters |
|---------|-------|----------------|
| Infrastructure | Portable | scp to any server, works everywhere |
| Rust devs | Native | Contribute in Rust, integrate with ecosystem |
| Privacy-focused | Local-first | Ollama support, no forced cloud |
| Researchers | RLM | First production RLM implementation |
| OSS community | Free | MIT license, open governance |

---

## Feature Requirements

### MVP (v0.1.0) - Foundation

| Feature | Priority | Description | Success Criteria |
|---------|----------|-------------|------------------|
| Core agent loop | P0 | Message processing with streaming | Real-time token display |
| Anthropic provider | P0 | Claude API integration | Works with Claude 3.5/4 |
| Basic tools | P0 | read, write, edit, bash | Can modify files |
| Search tools | P0 | glob, grep | Can find files/content |
| Web tool | P0 | webfetch | Can fetch URLs |
| Basic TUI | P0 | Messages + input | Interactive chat works |
| Session persistence | P0 | JSONL save/load | Can resume sessions |
| RLM foundation | P0 | External context storage | Handles 100k+ tokens |

### V1.0 - Full Release

| Feature | Priority | Description | Success Criteria |
|---------|----------|-------------|------------------|
| Full RLM | P0 | Sub-LLM orchestration, recursion | Handles 10M+ tokens |
| Rich TUI | P0 | ratatui widgets, themes, shortcuts | Professional UX |
| Session management | P0 | Resume, branch, compact | Full session control |
| Multiple providers | P0 | Anthropic, OpenAI, Google, Ollama, OpenRouter | 5+ providers |
| Permission system | P1 | Allow/deny/ask rules | Safe by default |
| Configuration | P1 | CLI flags, config files, env vars | Flexible setup |
| AGENTS.md | P1 | Project context files | Project-specific rules |
| Auto RLM activation | P1 | Detect and switch to RLM mode | Seamless experience |

### V2.0 - Extended Features

| Feature | Priority | Description | Success Criteria |
|---------|----------|-------------|------------------|
| Plugin system | P2 | WASM or trait-based plugins | Extensible tools |
| LSP integration | P2 | Code intelligence | Go to definition, etc. |
| Auto-compaction | P2 | Fallback when RLM not needed | Memory efficiency |
| MCP support | P2 | Model Context Protocol | Interoperability |
| Subagents | P2 | Spawn child agents | Parallel work |
| Git integration | P2 | Auto-commit, branches | Safe changes |
| Desktop app | P3 | GUI frontend | Alternative to TUI |

---

## Success Metrics

### Technical Metrics

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| Binary size | < 20MB | Release build size |
| Startup time | < 100ms | Cold start measurement |
| Memory usage | < 50MB idle | Process monitoring |
| Context handling | 10M+ tokens | RLM benchmark |
| Streaming latency | < 50ms first token | Provider response time |

### Product Metrics

| Metric | 6-month Target | 12-month Target |
|--------|----------------|-----------------|
| GitHub stars | 1,000+ | 5,000+ |
| Contributors | 20+ | 50+ |
| Downloads/month | 5,000+ | 20,000+ |
| Provider integrations | 5+ | 10+ |
| Discord members | 500+ | 2,000+ |

### Quality Metrics

| Metric | Target |
|--------|--------|
| Test coverage | > 80% |
| Documentation | All public APIs documented |
| Issue response time | < 48 hours |
| PR review time | < 1 week |

---

## Risk Assessment

### Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| RLM complexity delays MVP | High | High | Phased implementation, compaction fallback |
| Provider API changes | Medium | Medium | Abstraction layer, quick patch releases |
| TUI performance on large outputs | Medium | Medium | Pagination, truncation, virtual scrolling |
| Shell compatibility issues | Medium | Low | Extensive testing, fallback modes |
| Async runtime bugs | Low | High | Tokio is battle-tested, thorough testing |

### Product Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Slow adoption | Medium | High | Good docs, examples, community building |
| Competition from big players | Medium | Medium | Focus on RLM differentiation, Rust niche |
| Feature creep | Medium | Medium | Strict MVP scope, user research |
| Maintainer burnout | Medium | High | Community building, sustainable pace |

### Market Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| LLM providers restrict API access | Low | High | Support local models (Ollama) |
| New context window breakthroughs | Medium | Medium | RLM still valuable for cost/efficiency |
| Economic downturn reduces OSS funding | Medium | Low | Keep lean, sustainable development |

---

## Competitive Analysis

### Competitive Landscape

```
                    High Features
                         │
         opencode ●      │      ● Claude Code
                         │
                         │
    rot ● ───────────────┼───────────────● Cursor
                         │
         pi ●            │      ● Aider
                         │
                    Low Features
              Low Distribution ◄──► High Distribution
```

### Detailed Comparison

| Dimension | rot | pi | opencode | Claude Code | Aider |
|-----------|-----|----|----------:|-------------|-------|
| **Distribution** | Binary | npm | npm/brew | npm | pip |
| **Runtime deps** | None | Node.js | Bun | Node.js | Python |
| **Setup complexity** | Low | Medium | Medium | Medium | Medium |
| **RLM support** | Full | None | None | None | None |
| **Provider choice** | High | High | High | Low | High |
| **Open source** | Yes | Yes | Yes | No | Yes |
| **Maturity** | New | Mature | Mature | Mature | Mature |
| **Performance** | Native | Node | Bun | Node | Python |
| **Plugin ecosystem** | None yet | Rich | Rich | MCP | None |

### Competitive Advantages

| Advantage | Sustainable? | How to Maintain |
|-----------|--------------|-----------------|
| Single binary | Yes | Rust + static linking |
| RLM implementation | Yes (for now) | Continue research, improve algorithm |
| No runtime deps | Yes | Careful dependency selection |
| Rust ecosystem | Yes | Active community engagement |

### Competitive Disadvantages

| Disadvantage | Severity | Mitigation |
|--------------|----------|------------|
| New project, no ecosystem | High | Build community, docs, examples |
| Fewer providers initially | Medium | Add providers systematically |
| Smaller team | Medium | Sustainable pace, community contributions |
| Less polished UX | Medium | Iterate based on feedback |

---

## Go-to-Market Strategy

### Launch Phases

#### Phase 1: Alpha (Weeks 1-8)

| Activity | Description |
|----------|-------------|
| GitHub release | Binary downloads, source code |
| Documentation | README, getting started |
| Community | Discord server, GitHub discussions |
| Target | 100 early adopters |

**Success criteria**: Working MVP, initial feedback collected

#### Phase 2: Beta (Weeks 9-16)

| Activity | Description |
|----------|-------------|
| Package managers | Homebrew, cargo install |
| Documentation site | Full docs, API reference |
| Blog posts | RLM deep dive, Rust benefits |
| Target | 1,000 users |

**Success criteria**: Stable API, positive reviews, contributor growth

#### Phase 3: 1.0 Launch (Weeks 17-24)

| Activity | Description |
|----------|-------------|
| Major release | Full RLM, 5+ providers |
| Press/Tech blogs | Hacker News, Reddit, Twitter |
| Conference talks | RustConf, local meetups |
| Target | 5,000+ users |

**Success criteria**: Product-market fit, sustainable community

### Marketing Channels

| Channel | Priority | Content Type |
|---------|----------|--------------|
| GitHub | High | Code, releases, issues |
| Discord | High | Community support |
| Twitter/X | Medium | Updates, tips, announcements |
| Blog | Medium | Technical deep dives |
| Reddit | Medium | r/rust, r/programming |
| Hacker News | Low | Major releases only |
| Conferences | Low | Talks, demos |

### Community Building

| Initiative | Description |
|------------|-------------|
| Good first issues | Label beginner-friendly issues |
| Contributing guide | Clear CONTRIBUTING.md |
| Code of conduct | Inclusive community |
| Office hours | Weekly Discord sessions |
| Beta tester program | Early access for feedback |

---

## Monetization (Future Consideration)

### Non-Goals for MVP

- No paid tiers
- No enterprise features
- No SaaS offering

### Potential Future Options

| Model | Description | Timeline |
|-------|-------------|----------|
| GitHub Sponsors | Donation-based support | 6+ months |
| Enterprise support | Paid support contracts | 12+ months |
| Hosted version | rot Cloud with managed RLM | 18+ months |
| Enterprise features | SSO, audit logs, etc. | 18+ months |

---

## Success Criteria

### 6-Month Checkpoint

- [ ] v1.0 released with full RLM
- [ ] 5+ LLM providers supported
- [ ] 1,000+ GitHub stars
- [ ] 20+ contributors
- [ ] Active Discord community (500+)
- [ ] Featured in Rust weekly/monthly

### 12-Month Checkpoint

- [ ] v2.0 with plugins, LSP
- [ ] 5,000+ GitHub stars
- [ ] 50+ contributors
- [ ] Used by notable companies/projects
- [ ] Conference talks given
- [ ] Sustainable development pace

### Long-term Vision (2+ years)

- [ ] De-facto CLI coding agent for Rust developers
- [ ] RLM implementation used as reference
- [ ] Thriving plugin ecosystem
- [ ] Optional sustainable business model

---

## Appendix A: User Stories

### Must Have (MVP)

```
As a developer,
I want to start rot and immediately chat with an AI about my code,
So that I can get help without any setup.

As a developer,
I want rot to read and modify files in my project,
So that the AI can actually help me code.

As a developer,
I want rot to handle my large codebase without errors,
So that I can work on real projects, not just toy examples.

As a developer,
I want to resume my previous session,
So that I don't lose context between work sessions.

As a developer,
I want rot to use Anthropic in MVP while keeping a provider-agnostic architecture,
So that I'm not locked into one vendor as support expands in v1.0.
```

### Should Have (v1.0)

```
As a developer,
I want rot to automatically handle very long contexts,
So that I can work on large tasks without manual intervention.

As a developer,
I want a beautiful and responsive TUI,
So that the experience feels professional and enjoyable.

As a developer,
I want to control which actions require permission,
So that I can balance safety with efficiency.

As a developer,
I want to customize rot's behavior with config files,
So that it adapts to my workflow.
```

### Nice to Have (v2.0)

```
As a developer,
I want to extend rot with plugins,
So that I can add custom functionality.

As a developer,
I want LSP integration for code intelligence,
So that the AI has better context about my code.

As a developer,
I want rot to automatically commit changes,
So that I can easily track and revert AI modifications.
```

---

## Appendix B: Non-Goals

### Explicitly Out of Scope (MVP)

| Non-Goal | Reason |
|----------|--------|
| Web UI | Focus on CLI first |
| Desktop app | TUI is sufficient for MVP |
| Mobile app | Not relevant to coding workflow |
| Team collaboration | Focus on individual use |
| Cloud sync | Local-first philosophy |
| Built-in git hosting | Use existing providers |
| Code execution sandbox | Trust the user's environment |
| Voice interface | Not relevant to coding |

### Potentially Future Goals

| Future Goal | When to Consider |
|-------------|------------------|
| Web UI | If users strongly request it |
| Team features | If enterprise demand emerges |
| Cloud sync | If privacy concerns are addressed |
| Mobile companion | If viable use case emerges |

---

## Appendix C: Glossary

| Term | Definition |
|------|------------|
| **RLM** | Recursive Language Model - treats prompts as external environment, enables recursive processing |
| **REPL** | Read-Eval-Print Loop - interactive code execution environment |
| **Sub-LLM** | A child LLM call spawned by the RLM for processing chunks |
| **Context window** | Maximum tokens an LLM can process in one request |
| **JSONL** | JSON Lines format - one JSON object per line |
| **PTY** | Pseudo-terminal - enables interactive shell sessions |
| **Provider** | An LLM API provider (Anthropic, OpenAI, etc.) |
| **Tool** | A capability exposed to the LLM (read file, execute bash, etc.) |
| **Session** | A conversation with the AI, persisted for later resumption |
| **Compaction** | Summarizing old messages to free context space |
| **AGENTS.md** | Project-specific instructions for the AI agent |

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-02-25 | rot team | Initial PDR |
| 1.1 | 2026-02-25 | rot team | Clarified MVP user story for provider scope (Anthropic MVP, multi-provider v1.0) |

---

## Approval

| Role | Name | Status |
|------|------|--------|
| Product Owner | - | Pending |
| Tech Lead | - | Pending |
| Design Lead | - | Pending |
