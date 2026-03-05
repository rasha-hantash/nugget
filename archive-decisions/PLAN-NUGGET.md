# Nugget: Architecture & Build Plan

## Vision

Nugget is an AI memory layer for Claude Code. It automatically extracts knowledge from Claude Code sessions and makes it available for future sessions. The flywheel: work with Claude Code -> knowledge is captured -> Claude Code gets smarter -> you work better.

**Target user**: Individual developer using Claude Code.
**Platform**: Claude Code only.

---

## Architecture Overview

```
┌──────────────────────────────────────────────────────────────┐
│  WRITE PATH                                                   │
│                                                               │
│  Claude Code session ends                                     │
│         │                                                     │
│         ▼                                                     │
│  Session-end hook fires                                       │
│         │                                                     │
│         ▼                                                     │
│  nugget capture-session (background process)                  │
│         │                                                     │
│         ├── Read session transcript (JSONL)                    │
│         ├── Send to Claude API for extraction                  │
│         ├── Generate knowledge files (markdown + YAML)         │
│         ├── Create branch in brain repo                        │
│         ├── Commit files to appropriate domains                │
│         └── Open GitHub PR                                    │
│                                                               │
│  User reviews PR in GitHub UI → merge → knowledge in brain    │
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│  READ PATH                                                    │
│                                                               │
│  Claude Code working on a task                                │
│         │                                                     │
│         ▼                                                     │
│  Calls get_relevant_context(task_description) via MCP         │
│         │                                                     │
│         ▼                                                     │
│  ┌─────────────────────────────────────────┐                  │
│  │  Nugget Retrieval Engine                │                  │
│  │                                         │                  │
│  │  1. Embedding search (~50 candidates)   │                  │
│  │  2. Graph expansion (~30 unique)        │                  │
│  │  3. LLM re-ranking (top 5-10)          │                  │
│  └───────────────────┬─────────────────────┘                  │
│                      │                                        │
│                      ▼                                        │
│  Returns ranked knowledge to Claude Code                      │
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│  STORAGE                                                      │
│                                                               │
│  Source of truth: Markdown files + YAML frontmatter           │
│  Location: Dedicated GitHub repo (brain/)                     │
│                                                               │
│  Derived index: SQLite                                        │
│  ├── Units table (metadata)                                   │
│  ├── Relationships table (graph edges)                        │
│  ├── FTS5 (full-text search)                                  │
│  └── Embeddings table (vectors)                               │
│                                                               │
│  Rebuilt from files if lost. Nothing is lost.                 │
└──────────────────────────────────────────────────────────────┘
```

---

## Write Path: How Knowledge Gets In

### Trigger

Claude Code **session-end hook**. Automatic, fires every time a session ends. Zero user friction.

### Mechanism

1. Hook fires, invokes `nugget capture-session`
2. Capture agent locates the session's transcript file (JSONL stored by Claude Code)
3. Reads the full transcript — no compaction loss
4. Sends transcript to Claude API with extraction prompt
5. LLM extracts knowledge units: reusable patterns, architectural decisions (not project-specific), domain knowledge, debugging insights, mental models
6. For each unit, the LLM determines: file placement (domain), type, tags, relationships to existing knowledge, confidence, filename
7. Agent creates a branch in the brain repo
8. Commits knowledge files to appropriate domain directories
9. Opens a GitHub PR with all proposed files
10. User notification: "This session will be analyzed for knowledge capture"

### PR format

One PR per session. Title: "Knowledge from session `<date> <time>`". Description: summary of what was learned, list of proposed files. Each file is a knowledge unit organized into the right domain directory.

### Review model

GitHub PRs against the brain repo. The user reviews in the GitHub UI — comments, edits, approves, merges. No custom inbox or review UI.

---

## Read Path: How Knowledge Gets Used

### MCP interface

Single tool: `get_relevant_context(task_description: string) -> ranked knowledge units`

Nugget handles all intelligence internally. Claude Code's only job is to call the tool and use the results.

CLAUDE.md or MCP server instructions tell Claude Code: "Always check the Nugget brain before answering."

### Retrieval pipeline

Search operates on **chunks** (derived from knowledge files), but results are grouped and ranked at the **unit** level.

**Layer 1a — Embedding search.** Embed the task description, cosine similarity against all chunk embeddings. Returns top ~50 chunks.

**Layer 1b — BM25/FTS5 search.** Full-text search against chunks via SQLite FTS5. Returns top ~50 chunks.

**Layer 1c — RRF fusion.** Combine embedding and BM25 results using Reciprocal Rank Fusion: `score = 1/(k + rank)` where k=60. Produces ~30 fused chunk results. Map chunks to parent units.

**Layer 2 — Graph expansion.** For top units from Layer 1c, walk 1-2 hops of relationship edges in the units graph. Pull in chunks from related units. ~40-50 total chunks.

**Layer 3 — LLM re-ranking.** Score remaining chunks (with unit context) for actual relevance to the specific task. Factors in confidence, freshness, source quality. Returns top 5-10 chunks, grouped by unit.

### Entry points

1. **Claude Code** — via MCP tool during sessions
2. **CLI** — `nugget ask "what are route tables?"` (goes through full pipeline + LLM)

---

## Storage & Data Model

### Knowledge file format

```yaml
---
id: pattern/retry-with-idempotency
type: pattern # pattern | concept | decision | bug | belief
domain: coding
tags: [stripe, payments, reliability]
confidence: 0.9 # 0.0-1.0
source: ai-session # ai-session | manual | imported
related:
  - id: concept/idempotency-keys
    relation: uses
  - id: decision/exponential-backoff-config
    relation: implements
created: 2026-02-24
last_modified: 2026-02-24
---
# Retry with Idempotency Keys

When retrying Stripe API calls, ALWAYS use idempotency keys...
```

### Brain directory structure

Agent-managed. The capture agent decides organization. Example:

```
brain/
  brain.yaml                    # Brain metadata: owner, config
  domains/
    coding/
      concepts/
        cache-invalidation.md
      patterns/
        retry-with-idempotency.md
      decisions/
        redis-for-sessions.md
      bugs/
        stripe-client-retry.md
    coding/go/                  # Sub-domains are just nested folders
      patterns/
        error-handling.md
    coding/rust/
      concepts/
        ownership-patterns.md
    management/
      patterns/
        one-on-one-framework.md
  .nugget/                      # Gitignored — derived state
    index.db                    # SQLite: metadata + graph + FTS5
    embeddings/                 # Vector embeddings
```

### Markdown chunking

Knowledge units can be large (10+ pages). A single embedding for a large document captures a blurry average. The index chunks files for search while keeping whole files as source of truth.

**Chunking pipeline:**

1. Parse and strip YAML frontmatter -> attach as metadata to all chunks
2. Parse heading tree (`pulldown-cmark`)
3. Split at heading boundaries (`##`, `###`)
4. Size normalization:
   - Chunks > 512 tokens: sub-split at paragraph boundaries, 10-15% overlap between sub-chunks
   - Chunks < 50 tokens: merge with next sibling section
5. Prepend heading breadcrumb to chunk text before embedding (e.g., "Go Concurrency > Worker Pools > Bounded")

No overlap between heading-level chunks (natural semantic boundaries). Overlap only within sub-splits of oversized sections.

### Derived index (SQLite)

```sql
-- One row per knowledge file
CREATE TABLE units (
    id TEXT PRIMARY KEY,
    path TEXT,
    title TEXT,
    type TEXT,
    domain TEXT,
    tags TEXT,              -- JSON array
    confidence REAL,
    source TEXT,
    created TEXT,
    last_modified TEXT,
    content TEXT             -- full markdown body (for display)
);

-- Chunks derived from units (for search)
CREATE TABLE chunks (
    id TEXT PRIMARY KEY,     -- unit_id + chunk_index
    unit_id TEXT REFERENCES units(id),
    content TEXT,            -- chunk text WITH breadcrumb prepended
    heading_breadcrumb TEXT,
    heading_level INTEGER,
    position INTEGER,        -- sequential order within unit
    embedding BLOB           -- vector
);

-- Graph edges between units
CREATE TABLE relationships (
    source_id TEXT,
    target_id TEXT,
    relation_type TEXT
);

-- FTS5 over chunks (not units)
CREATE VIRTUAL TABLE chunks_fts USING fts5(
    content,
    unit_id UNINDEXED,
    chunk_id UNINDEXED
);
```

Rebuilt from files on startup. If corrupted or lost, rebuild. Nothing is lost.

### Embedding model

Default: **fastembed-rs** (local, CPU, no API key, offline). Users can switch to OpenAI/Voyage in config. Switching triggers a full reindex — all embeddings regenerated from files.

---

## CLI

Minimal. The CLI is plumbing, not the product.

- `nugget init` — one-time brain repo setup (creates directory structure, initializes git, creates brain.yaml)
- `nugget serve` — start the MCP server for Claude Code
- `nugget ask "..."` — direct brain query from the terminal (full retrieval pipeline + LLM formatting)
- `nugget capture-session [--transcript <path>]` — run capture pipeline (invoked by hook or manually)

---

## Workspace Layout

```
nugget/
  Cargo.toml                    # Workspace root
  crates/
    nugget-core/                # Core types: KnowledgeUnit, KnowledgeId, Domain, etc.
    nugget-store/               # File parser/writer, brain directory ops
    nugget-index/               # SQLite (metadata + graph + FTS5) + embeddings
    nugget-retrieve/            # 3-layer retrieval pipeline
    nugget-capture/             # Transcript analysis, LLM extraction, Git/PR operations
    nugget-mcp/                 # MCP server (single tool)
    nugget-cli/                 # CLI entry point
```

---

## Build Phases

### Phase 1: Foundation

**Goal**: Core types exist, can parse/write knowledge files, can initialize a brain.

**nugget-core** (`crates/nugget-core/`)

- `KnowledgeUnit` struct: id, type, domain, tags, confidence, source, related, body, created, last_modified
- `KnowledgeType` enum: pattern, concept, decision, bug, belief
- `RelationType` enum: uses, implements, requires_understanding_of, informed_by, often_combined_with
- `Domain`, `Tag`, `Confidence`, `Relation` types
- Frontmatter serialization/deserialization (serde + serde_yaml)

**nugget-store** (`crates/nugget-store/`)

- Brain directory operations: `init` (create brain/ structure with brain.yaml and domains/)
- Markdown + YAML frontmatter parser/writer
- Read/write knowledge files to disk
- Walk brain directory, collect all knowledge file paths
- List domains

**nugget-cli** (`crates/nugget-cli/`)

- `nugget init [--path <dir>]` — create brain directory

**Key deps**: serde, serde_yaml, chrono, uuid, walkdir, clap

**Deliverable**: `nugget init` creates a correct brain directory. Knowledge files parse and round-trip correctly (parse -> serialize -> parse = identical). Unit tests with insta snapshots.

---

### Phase 2: Read Path (Index + Retrieval + MCP)

**Goal**: Claude Code can query the brain and get relevant knowledge back.

**nugget-index** (`crates/nugget-index/`)

- SQLite schema: units table, chunks table, relationships table, chunks_fts (FTS5). See schema in Storage section above.
- Markdown chunking: parse heading tree, split at heading boundaries, prepend breadcrumbs, size-normalize
- Build index from files: walk brain directory, parse each file, chunk it, insert units + chunks into SQLite
- Generate embeddings per chunk: fastembed-rs (default) or API provider (configurable)
- Incremental update: reindex a single file by path (delete old chunks, re-chunk, re-embed)
- Rebuild: drop and recreate everything from files

**nugget-retrieve** (`crates/nugget-retrieve/`)

- Layer 1a: Embedding search on chunks — cosine similarity, top ~50 chunks
- Layer 1b: BM25/FTS5 search on chunks — full-text match, top ~50 chunks
- Layer 1c: RRF fusion — combine embedding + BM25 results, `score = 1/(k + rank)`, map chunks to parent units
- Layer 2: Graph expansion — for top units, walk 1-2 hops in relationships table, pull chunks from related units
- Layer 3: LLM re-ranking — score chunks with unit context, return top 5-10 grouped by unit
- Single entry point: `retrieve(task_description, index) -> Vec<RankedResult>`

**nugget-mcp** (`crates/nugget-mcp/`)

- MCP server using rmcp (Rust MCP SDK)
- Single tool: `get_relevant_context(task_description: string) -> ranked knowledge units`
- Server instructions: "You have access to the user's personal knowledge brain via Nugget. Always check the brain for relevant context before answering questions."
- Entry point: `nugget serve` (stdio transport for Claude Code)

**nugget-cli** additions:

- `nugget serve` — start MCP server
- `nugget ask "..."` — query the brain from terminal (retrieval pipeline + LLM formats answer)

**Key deps**: rusqlite, fastembed, rmcp, tokio, reqwest (for Claude API)

**Deliverable**: Manually populate a brain with ~20 knowledge files across 2-3 domains. `nugget ask "how to handle retries"` returns relevant results. Configure Claude Code MCP -> ask a domain question -> Claude Code calls `get_relevant_context` -> gets relevant knowledge -> uses it in response.

---

### Phase 3: Write Path (Session Capture + PRs)

**Goal**: Knowledge is automatically captured from Claude Code sessions and proposed via GitHub PRs.

**nugget-capture** (`crates/nugget-capture/`)

_Transcript handling:_

- Discover transcript: locate Claude Code session transcript files (JSONL) in `~/.claude/projects/`
- Parse JSONL: reconstruct conversation (human messages, assistant messages, tool calls)
- Handle large transcripts: chunk if needed for LLM context window

_LLM extraction:_

- Extraction prompt: send transcript to Claude API with structured extraction instructions
- Output: list of knowledge units (type, domain, tags, confidence, title, body, suggested relationships)
- Relationship detection: load existing brain index, pass existing unit summaries to LLM so it can identify links
- **Cold-start note**: First capture with an empty brain index = no relationships detected. Relationships improve as the brain grows.

_File generation:_

- Create markdown files with YAML frontmatter for each extracted unit
- Place in appropriate domain directories based on LLM suggestions

_Git + PR operations:_

- Create branch: `nugget/session-<timestamp>` in brain repo
- Commit knowledge files
- Push branch to remote
- Create GitHub PR (via `gh` CLI or GitHub API)
- PR title: "Knowledge from session `<date> <time>`"
- PR body: summary of extracted knowledge, list of proposed files

_Hook integration:_

- Claude Code session-end hook invokes `nugget capture-session` as background process
- Notification: display "This session will be analyzed for knowledge capture"
- Pass transcript path to capture agent

**nugget-cli** additions:

- `nugget capture-session [--transcript <path>]` — run capture pipeline (invoked by hook or manually)

**Key deps**: reqwest (Claude API), git2 or shell git, serde_json (JSONL parsing)

**Deliverable**: End a Claude Code session -> hook fires -> background capture runs -> GitHub PR appears in brain repo -> review and merge -> next session, Claude Code retrieves the knowledge via MCP. Full flywheel demonstrated.

---

## Open Questions (to resolve during implementation)

1. ~~**Claude Code hook mechanics**~~: RESOLVED. `SessionEnd` hook provides `transcript_path` via stdin JSON. See `DECISIONS.MD`.
2. **Brain repo structure**: Exact directory layout — flat domains at top level? Nested types within domains? File naming conventions?
3. **PR format**: Exact PR title/description/commit message format?
4. **Capture agent packaging**: Part of the `nugget` binary (`nugget capture-session`)? Separate binary?
5. **Relationship detection**: How does the capture agent identify relationships to existing knowledge? Requires index to exist before capture works well.
6. **Conflict resolution**: What happens if a PR modifies an existing knowledge file updated on main since the branch was created?
7. ~~**Transcript file discovery**~~: RESOLVED. Transcripts at `~/.claude/projects/<project-path-with-dashes>/<session-uuid>.jsonl`. Hook provides path directly.
8. **Multiple sessions in flight**: Race conditions if two sessions end simultaneously. Unique branch names help but need verification.

---

## Tech Stack

| Component             | Choice               | Why                                             |
| --------------------- | -------------------- | ----------------------------------------------- |
| Language              | Rust                 | Single binary, performance, ecosystem           |
| CLI                   | clap                 | Standard Rust CLI                               |
| Markdown parsing      | pulldown-cmark       | Rust-native CommonMark, heading tree extraction |
| Markdown chunking     | text-splitter        | Semantic markdown splitting with size control   |
| YAML parsing          | serde_yaml           | Standard Rust YAML                              |
| Database              | SQLite (rusqlite)    | Metadata, graph, FTS5, embedded                 |
| Embeddings (default)  | fastembed-rs         | Local, CPU, offline, no API key                 |
| Embeddings (optional) | OpenAI / Voyage API  | Higher quality, requires API key                |
| LLM (extraction)      | Claude API (reqwest) | Knowledge extraction from transcripts           |
| LLM (re-ranking)      | Claude API (reqwest) | Relevance scoring in retrieval pipeline         |
| MCP server            | rmcp                 | Rust MCP SDK                                    |
| Git operations        | git2 or shell git    | Branch/commit/push for PR creation              |
| GitHub PRs            | gh CLI or GitHub API | PR creation                                     |
| Async runtime         | tokio                | Standard Rust async                             |
| File walking          | walkdir              | Brain directory traversal                       |

---

## Verification

### Phase 1

- `nugget init` creates correct brain directory structure
- Knowledge files round-trip through parse/serialize
- Unit tests with insta snapshots for frontmatter parsing

### Phase 2

- Brain with 20+ manually created files across 2-3 domains
- `nugget ask "cache invalidation"` returns relevant coding knowledge, not fashion knowledge
- Claude Code MCP integration: ask a question -> `get_relevant_context` called -> relevant knowledge returned
- Index rebuilds correctly from files after deletion
- Embedding model switch + full reindex works

### Phase 3

- Session ends -> hook fires -> `nugget capture-session` runs in background
- Transcript parsed correctly (handles long sessions, tool calls)
- LLM extracts meaningful knowledge (not noise, not project-specific details)
- PR created with correct branch, files in right domains, meaningful title/description
- Merge PR -> knowledge in brain -> next session retrieves it
- Two sessions ending simultaneously -> no conflicts

### End-to-end flywheel

1. `nugget init` -> empty brain
2. Configure Claude Code with Nugget MCP
3. Have a Claude Code session about a technical topic
4. Session ends -> PR appears
5. Review and merge PR
6. New session -> ask about same topic -> Claude Code uses knowledge from brain
7. Knowledge compounds with each session
