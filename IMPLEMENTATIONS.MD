# Nugget: Implementation Plan

## Context

Nugget is an AI memory layer for Claude Code. It automatically extracts knowledge from Claude Code sessions and makes it available for future sessions via MCP.

**Architecture summary:**

- **Write path**: Session-end hook -> transcript analysis -> LLM extraction -> GitHub PR
- **Read path**: Single MCP tool (`get_relevant_context`) -> 3-layer retrieval pipeline
- **Storage**: Markdown files (source of truth) + SQLite derived index (metadata, graph, FTS5, embeddings)
- **CLI**: `nugget init`, `nugget serve`, `nugget ask "..."`, `nugget capture-session`

---

## Crate Structure

```
nugget/
  Cargo.toml                    # Workspace root
  crates/
    nugget-core/                # Core types
    nugget-store/               # File parser/writer, brain directory ops
    nugget-index/               # SQLite + FTS5 + embeddings
    nugget-retrieve/            # 3-layer retrieval pipeline
    nugget-capture/             # Transcript analysis, LLM extraction, Git/PR ops
    nugget-mcp/                 # MCP server (single tool)
    nugget-cli/                 # CLI entry point
```

### What each crate owns

| Crate | Responsibility | Depends on |
| --- | --- | --- |
| nugget-core | Types, serialization | serde, serde_yaml, chrono, uuid |
| nugget-store | File I/O, brain directory ops | nugget-core, walkdir |
| nugget-index | SQLite index, embeddings, rebuild | nugget-core, nugget-store, rusqlite, fastembed |
| nugget-retrieve | 3-layer retrieval pipeline | nugget-index, reqwest (for LLM re-ranking) |
| nugget-capture | Transcript analysis, LLM extraction, Git/PR | nugget-core, nugget-index, reqwest, git2 |
| nugget-mcp | MCP server | nugget-retrieve, rmcp, tokio |
| nugget-cli | CLI entry point | all crates, clap |

---

## Build Order

### Phase 1: Foundation

Build the core types, file I/O, and basic CLI.

**nugget-core** (`crates/nugget-core/`)

- `KnowledgeUnit` struct: id, type, domain, tags, confidence, source, related, body, created, last_modified
- `KnowledgeType` enum: pattern, concept, decision, bug, belief
- `RelationType` enum: uses, implements, requires_understanding_of, informed_by, often_combined_with
- `Domain`, `Tag`, `Confidence`, `Relation` types
- Frontmatter serialization/deserialization (serde + serde_yaml)

**nugget-store** (`crates/nugget-store/`)

- Brain directory operations: `init` (create brain/ structure with brain.yaml and domains/)
- Markdown + YAML frontmatter parser/writer (handle the `---` delimiter correctly — only first pair is frontmatter)
- Read/write knowledge files to disk
- Walk brain directory, collect all knowledge file paths
- List domains

**nugget-cli** (`crates/nugget-cli/`)

- `nugget init [--path <dir>]` — create brain directory

**Key deps**: serde, serde_yaml, chrono, uuid, walkdir, clap

**Deliverable**: `nugget init` creates a correct brain directory. Knowledge files parse and round-trip correctly (parse -> serialize -> parse = identical). Unit tests with insta snapshots.

**Note**: Previous Phase 0 code exists (nugget-core, nugget-store, nugget-inbox, nugget-cli) but the architecture has changed significantly. nugget-inbox is no longer needed (replaced by GitHub PRs). nugget-core and nugget-store may be partially reusable but need review against the new data model.

---

### Phase 2: Read Path (Index + Retrieval + MCP)

Build the entire read path so Claude Code can query the brain.

**nugget-index** (`crates/nugget-index/`)

- SQLite schema:
  - `units` table: id, path, title, type, domain, tags (JSON), confidence, source, created, last_modified, content
  - `relationships` table: source_id, target_id, relation_type
  - FTS5 virtual table over unit content and title
  - `embeddings` table: id, vector (BLOB)
- Build index from files: walk brain directory, parse each file, insert into SQLite
- Generate embeddings: fastembed-rs (default) or API provider (configurable in brain.yaml)
- Incremental update: reindex a single file by path
- Rebuild: drop and recreate from files

**nugget-retrieve** (`crates/nugget-retrieve/`)

- Layer 1: Embedding search — embed query, cosine similarity against all stored vectors, return top ~50
- Layer 2: Graph expansion — for top-20, walk 1-2 hops in relationships table, deduplicate, ~30 candidates
- Layer 3: LLM re-ranking — send candidates + task description to Claude API, score relevance, return top 5-10
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

Build automatic knowledge capture from Claude Code sessions.

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

## Verification

### Phase 1

- `nugget init` creates correct brain directory structure (brain.yaml, domains/)
- Knowledge files round-trip through parse/serialize correctly
- Unit tests with insta snapshots for frontmatter parsing
- The `---` delimiter is handled correctly (only first pair is frontmatter)

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
- Two sessions ending simultaneously -> no conflicts (unique branch names)

### End-to-end flywheel

1. `nugget init` -> empty brain
2. Configure Claude Code with Nugget MCP
3. Have a Claude Code session about a technical topic
4. Session ends -> PR appears
5. Review and merge PR
6. New session -> ask about same topic -> Claude Code uses knowledge from brain
7. Knowledge compounds with each session

---

## Tech Stack

| Component | Choice | Why |
| --- | --- | --- |
| Language | Rust | Single binary, performance, ecosystem |
| CLI | clap | Standard Rust CLI |
| Markdown parsing | pulldown-cmark | Rust-native CommonMark |
| YAML parsing | serde_yaml | Standard Rust YAML |
| Database | SQLite (rusqlite) | Metadata, graph, FTS5, embedded |
| Embeddings (default) | fastembed-rs | Local, CPU, offline, no API key |
| Embeddings (optional) | OpenAI / Voyage API | Higher quality, requires API key |
| LLM (extraction + reranking) | Claude API (reqwest) | Knowledge extraction and relevance scoring |
| MCP server | rmcp | Rust MCP SDK |
| Git operations | git2 or shell git | Branch/commit/push for PR creation |
| GitHub PRs | gh CLI or GitHub API | PR creation |
| Async runtime | tokio | Standard Rust async |
| File walking | walkdir | Brain directory traversal |

---

## Current State

- **Phase 1**: NOT STARTED (previous Phase 0 code may be partially reusable — review needed)
- **Phase 2**: NOT STARTED
- **Phase 3**: NOT STARTED
