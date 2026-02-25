# Nugget: Product Decisions

A clean record of product decisions made during the strategy session (February 2026). These decisions supersede any conflicting content in earlier planning docs.

---

## Vision & Scope

### Target user

**Individual developer using Claude Code.** Not designed for other AI tools, teams, or non-developers in v1.

### Platform

**Claude Code only.** Not MCP-generic. If MCP adoption grows, other tools may benefit later, but we're not designing for that.

### Core thesis

Nugget is an AI memory layer for Claude Code. Work with Claude Code -> knowledge is captured -> Claude Code gets smarter -> you work better. The flywheel compounds over time.

---

## Write Path (How Knowledge Gets In)

### Decision: Session-end hook as capture trigger

**Automatic, fires every time a session ends.** Zero user friction. No dependency on Claude remembering to call an MCP tool.

**Why not MCP tool calls during session?**

- Claude Code compresses earlier messages during long sessions (compaction loss)
- MCP tools depend on Claude remembering to call them ("Claude forgot" problem)
- Session-end hooks are deterministic

**Open question**: The exact hook mechanics are unresolved — does Claude Code support session-end hooks today? What event fires? How does the hook access the transcript file path? This is a hard blocker for the write path. See `DECISIONS.MD` for full details.

### Decision: Post-session transcript analysis

A background process reads the full session transcript (JSONL files stored by Claude Code) and sends it to an LLM for knowledge extraction. This ensures nothing is lost to compaction.

**Open question**: The exact transcript file location and JSONL format need verification. Claude Code stores transcripts in `~/.claude/projects/` — path pattern and format are assumed, not confirmed. See `DECISIONS.MD` for full details.

### Decision: Claude API for extraction

Configurable to other providers if straightforward, otherwise Claude-only.

### Decision: Broad capture net

The capture agent casts a broad net: reusable patterns, architectural decisions (not project-specific), domain knowledge, debugging insights, mental models. The user is the final filter via PR review. Better to over-propose and let the user reject than to miss valuable knowledge.

### Decision: GitHub PRs for review (no inbox)

The capture agent creates a branch, commits proposed knowledge files, and opens a PR against the brain repo. The user reviews via the GitHub UI — comments, edits, approves, merges.

**Why not an inbox?**

- Inbox abandonment is the #1 retention risk ("Instapaper death spiral")
- PRs are a review model developers use daily
- GitHub UI provides comments, edits, approval — richer than accept/reject
- No custom review UI to build

### Decision: One PR per session

A single session might produce knowledge across multiple domains. The PR contains all proposed files, organized into the appropriate domain directories.

### Decision: Transparency notification

When a session ends: "This session will be analyzed for knowledge capture." Transparency builds trust.

---

## Read Path (How Knowledge Gets Used)

### Decision: Single MCP tool — Nugget owns retrieval intelligence

Claude Code calls one tool: `get_relevant_context(task_description)`. Nugget handles everything internally — keyword extraction, embedding search, graph traversal, relevance ranking. Claude Code's only job is to call the tool and use the results.

**Why not multiple tools (search, browse, get)?**

- Claude Code shouldn't be responsible for traversing the brain
- One tool call is simpler and more reliable than multi-step tool orchestration
- Nugget can optimize retrieval internally without changing the API

### Decision: Full 3-layer retrieval pipeline from MVP

1. **Embedding search** — vector similarity
2. **Graph expansion** — walk relationship edges
3. **LLM re-ranking** — score for actual relevance

**Why build all three from the start?**

- At small brain sizes, embeddings + LLM re-ranking carry the weight
- Graph expansion kicks in progressively as the brain grows
- Building incrementally was considered but rejected — the full pipeline works at every brain size

### Decision: Two entry points, same engine

1. **Claude Code** — via MCP tool during sessions
2. **CLI** — `nugget ask "what are route tables?"` for direct terminal queries

### Deferred: Hook-based auto-injection (v2+)

Nugget intercepts the prompt, searches the brain, and injects relevant context before Claude Code responds. Currently blocked on Claude Code platform support (hooks can't inject content into conversations).

---

## Storage & Data Model

### Decision: Markdown files with YAML frontmatter (source of truth)

Human-readable, Git-versionable, editable in any editor. Non-negotiable — the PR workflow requires actual files.

### Decision: SQLite derived index

- Units table (id, path, title, type, domain, tags, confidence, source, created, last_modified)
- Relationships table (source_id, target_id, relation_type)
- FTS5 virtual table for full-text search
- Embeddings table (id, vector)

Rebuilt from files if corrupted or lost. Nothing is lost.

### Decision: Agent-managed organization

The capture agent decides file placement, type, tags, relationships, confidence, filename. The user can browse the brain repo but never needs to manually organize.

### Decision: Dedicated GitHub repo for brain

e.g., github.com/you/brain. Clone locally for MCP server access. PRs are against this repo.

### Decision: Configurable embedding model

Default: fastembed-rs (local, CPU, no API key, offline). Switchable to OpenAI/Voyage via config. Switching triggers full reindex.

### Decision: Knowledge file metadata

Every knowledge unit includes: id, type (pattern | concept | decision | bug | belief), domain, tags, confidence (0.0-1.0), source (ai-session | manual | ...), related (id + relation type), created, last_modified.

---

## CLI

### Decision: Minimal CLI

- `nugget init` — one-time brain repo setup
- `nugget serve` — start the MCP server
- `nugget ask "..."` — direct brain query (full retrieval pipeline + LLM)

The CLI is plumbing, not the product.

### Decision: Rust

Single binary, performance, ecosystem alignment.

---

## Cold Start

### Decision: Organic growth for MVP

Brain populates as the user works with Claude Code. No seeding required.

### Deferred: Import existing docs (v2)

Point Nugget at CLAUDE.md files, project docs, notes. Could seed 20-50 units quickly.

---

## Privacy

### Decision: Acceptable for v1

Session transcripts sent to Claude API for extraction. Same trust boundary as using Claude Code itself.

---

## V2 / Future Items

1. **Knowledge staleness** — time-based confidence decay, LLM "is this still true?" detection during retrieval. Foundation: `created` and `last_modified` timestamps are in v1.
2. **Import/seed** — process existing docs, CLAUDE.md files, project notes to seed the brain. Solves cold start faster than organic growth.
3. **Interview mode** — Nugget asks questions to extract tacit knowledge. "When you get paged, what's the first thing you check?" A 30-minute session could seed 20+ units.
4. **Clipboard capture** — deprioritized. Low signal-to-noise ratio compared to session capture. Copying a URL != understanding it.
5. **Hook-based auto-injection** — zero-friction read path. Blocked on Claude Code platform support (hooks can't inject content into conversations).
6. **Cross-brain sharing** — cherry-pick knowledge from colleagues' brains via Git remotes.
7. **Shared review engine** — extract generic proposal/review components (directory-per-proposal, comment threads, accept/reject/modify) for reuse in other products.
8. **Team features** — knowledge sharing, onboarding acceleration. Path to profitability — personal tool is the wedge, team features are the business.
9. **Direct query UI** — beyond CLI, potentially a web UI or chat interface for querying the brain.
10. **Configurable LLM providers** — full provider abstraction for both extraction and re-ranking (Ollama, local models, etc.).
