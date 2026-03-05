# Nugget: Updated Plan

## What Nugget Is

Nugget is the **upgrade path** for brain-os knowledge management. It replaces the current Python hooks + keyword search with a Rust binary providing embedding-based retrieval, graph expansion, and structured capture.

### Relationship to Brain-OS Hooks

Brain-OS now has working automation hooks that handle basic capture and retrieval:

| Capability     | Brain-OS Hooks (live)                                               | Nugget (upgrade path)                                                   |
| -------------- | ------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| **Read path**  | Keyword search across ~35 md files, ~35ms, injected on every prompt | Embedding + BM25 hybrid search, Memgraph graph expansion, LLM reranking |
| **Write path** | `claude -p` transcript extraction → markdown files → `gt submit` PR | Structured types w/ frontmatter, relationship detection, deduplication  |
| **Trigger**    | SessionEnd hook + pre-compact ACTION REQUIRED                       | Same SessionEnd hook, but Rust binary instead of Python + `claude -p`   |
| **Scale**      | Works well for <100 files, keyword search degrades after that       | Designed for 100-1000+ knowledge units with indexed retrieval           |

Nugget becomes necessary when brain-os outgrows keyword search (~100+ files). Until then, the hooks are sufficient.

---

## Architecture Overview

```
┌──────────────────────────────────────────────────────────────┐
│  WRITE PATH                                                   │
│  Session ends → nugget capture-session                        │
│    ├── Read transcript (JSONL)                                │
│    ├── Send to Claude API for extraction                      │
│    ├── Generate knowledge files (markdown + YAML frontmatter) │
│    ├── Detect relationships to existing knowledge             │
│    ├── Deduplicate against existing units                     │
│    └── Open GitHub PR                                        │
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│  READ PATH                                                    │
│  Claude Code → get_relevant_context(task) via MCP             │
│    ├── Layer 1: Embedding + BM25 hybrid search (RRF fusion)   │
│    ├── Layer 2: Graph expansion via Memgraph (1-3 hops)       │
│    └── Layer 3: LLM re-ranking → top 5-10 results             │
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│  STORAGE                                                      │
│  Source of truth: Markdown + YAML frontmatter (brain/ repo)   │
│  Derived index: SQLite (metadata, FTS5, embeddings)           │
│  Graph: Memgraph (relationships, domain edges, tag edges)     │
│  Rebuilt from files if lost. Nothing is lost.                 │
└──────────────────────────────────────────────────────────────┘
```

---

## Crate Structure

```
crates/
  nugget-core/      # Core types: KnowledgeUnit, KnowledgeType, Relation, frontmatter
  nugget-store/     # Brain directory ops, file I/O, walk
  nugget-index/     # SQLite + FTS5 + embeddings + Memgraph sync
  nugget-retrieve/  # 3-layer retrieval pipeline
  nugget-capture/   # Transcript analysis, LLM extraction, Git/PR
  nugget-mcp/       # MCP server (single tool: get_relevant_context)
  nugget-cli/       # CLI entry point (clap)
```

Dependency flow: `cli → mcp → retrieve → index → store → core`

---

## Phase 1: Foundation (merge existing branches)

**Status:** Coded on branches, ready to merge.

Existing branches:

- `02-24-feat_add_nugget-core_types_and_frontmatter_parsing`
- `02-24-feat_add_nugget-store_for_brain_directory_and_file_i_o`
- `02-24-feat_add_nugget-cli_with_init_command`

**What's built:**

- `KnowledgeUnit` struct with frontmatter serde
- `KnowledgeType` enum (pattern, concept, decision, bug, belief)
- `Relation` types (uses, implements, requires_understanding_of, informed_by, often_combined_with)
- Brain directory init (`nugget init`)
- File I/O: parse/write markdown + YAML frontmatter
- Directory walking, domain listing
- 19 tests across 3 crates with insta snapshots

**Action:** Merge these branches to main, resolve any conflicts.

**Verification:**

- `cargo test` passes (19 tests)
- `nugget init --path ./brain` creates correct structure
- Knowledge files round-trip through parse/serialize

---

## Phase 2: Read Path — Primary Value-Add

**This is the main reason nugget exists.** The brain-os keyword hook is "good enough" for <100 files. Nugget's read path replaces it with retrieval that actually scales.

### nugget-index

- SQLite schema: units, chunks, relationships tables + FTS5
- Markdown chunking: heading-boundary splitting, breadcrumb prepending, size normalization
- Embedding generation: fastembed-rs (local, CPU, no API key)
- Memgraph sync: unit nodes, domain nodes, tag nodes, typed relationship edges
- Incremental update: reindex single file by path
- Full rebuild from files

### nugget-retrieve

- Layer 1a: Embedding cosine similarity (~50 candidate chunks)
- Layer 1b: BM25/FTS5 full-text search (~50 chunks)
- Layer 1c: RRF fusion — `score = 1/(k + rank)`, k=60, map chunks to parent units
- Layer 2: Memgraph graph expansion — 1-3 hop Cypher traversal for related units
- Layer 3: LLM re-ranking — score for task relevance, return top 5-10 grouped by unit

### nugget-mcp

- MCP server via rmcp (Rust MCP SDK)
- Single tool: `get_relevant_context(task_description: string) -> ranked knowledge units`
- Server instructions tell Claude Code to always check the brain
- Entry point: `nugget serve` (stdio transport)

### nugget-cli additions

- `nugget serve` — start MCP server
- `nugget ask "..."` — terminal query (retrieval + LLM formatting)

### Key deps

rusqlite, fastembed, rmcp, tokio, reqwest, bolt-client or neo4rs (Memgraph)

### Verification

- Brain with 20+ manually created files, `related:` fields linking them
- `nugget ask "cache invalidation"` returns relevant results
- Graph expansion: querying a unit returns related units from 1-3 hops
- MCP integration: Claude Code calls `get_relevant_context` → gets knowledge
- Index rebuilds correctly (SQLite + Memgraph)
- Prerequisite: Memgraph running locally (`docker run -p 7687:7687 memgraph/memgraph`)

---

## Phase 3: Write Path — Structured Capture

**Lower priority** since `capture-learnings.py` (brain-os SessionEnd hook) already handles basic extraction. Nugget's write path adds quality improvements.

### What capture-learnings.py already does

- SessionEnd hook fires → reads transcript → `claude -p` extracts learnings
- Creates markdown files in `brain-os/claude-learnings/`
- Includes confidence scores (rubric in `claude-learnings/README.md`)
- Creates PR via `gt create` + `gt submit`
- Deduplication by session ID, lock file for concurrency

### What nugget-capture adds

- **Structured frontmatter types**: Full `KnowledgeUnit` format (id, type, domain, tags, confidence, source, related) vs. current simple format
- **Relationship detection**: Loads existing brain index, passes unit summaries to LLM so it can link new knowledge to existing units
- **Deduplication against content**: Not just session ID check, but semantic similarity against existing units to avoid near-duplicates
- **Domain routing**: LLM places files in appropriate `domains/` subdirectories
- **Richer PR format**: Summary of extracted knowledge, list of proposed files with types, relationship graph

### nugget-capture implementation

- Transcript handling: parse JSONL, reconstruct conversation, handle large transcripts
- LLM extraction: Claude API with structured extraction prompt
- File generation: markdown + YAML frontmatter per extracted unit
- Git + PR: branch `nugget/session-<timestamp>`, commit, push, create PR
- Hook integration: `nugget capture-session` invoked by SessionEnd hook

### Verification

- Session ends → hook fires → `nugget capture-session` runs
- LLM extracts meaningful knowledge (not noise, not project-specific)
- PR has correct frontmatter, domain placement, relationship links
- Two sessions ending simultaneously → no conflicts

---

## Phase 4: Web UI (deferred)

Design exists in `archive-decisions/Plan-UI.md`. Scope: brain explorer, search interface, knowledge graph visualization. Not needed until retrieval pipeline is solid.

---

## Migration: Replacing Brain-OS Hooks

When nugget's MCP server is ready (end of Phase 2):

1. **Read path migration**: The `brain-os-context.py` UserPromptSubmit hook (keyword search) gets replaced by nugget's MCP `get_relevant_context` tool. Remove the hook from `~/.claude/settings.json`, add nugget MCP server config.

2. **Write path migration** (Phase 3): The `capture-learnings.py` SessionEnd hook gets replaced by `nugget capture-session`. Same trigger, better extraction.

3. **Pre-compact hook**: Stays as-is — it triggers the learnings-capturer agent for in-session capture, which is complementary to end-of-session capture.

4. **Brain directory**: Existing `claude-learnings/` files get migrated into nugget's `domains/` structure during initial `nugget init` from the brain-os repo.

---

## Tech Stack

| Component            | Choice               | Why                                             |
| -------------------- | -------------------- | ----------------------------------------------- |
| Language             | Rust                 | Single binary, performance, ecosystem           |
| CLI                  | clap                 | Standard Rust CLI                               |
| Markdown parsing     | pulldown-cmark       | Rust-native CommonMark, heading tree extraction |
| Markdown chunking    | text-splitter        | Semantic splitting with size control            |
| YAML parsing         | serde_yaml           | Standard Rust YAML                              |
| Database             | SQLite (rusqlite)    | Metadata, FTS5, embeddings, embedded            |
| Graph DB             | Memgraph             | In-memory graph, Cypher, Bolt protocol          |
| Embeddings (default) | fastembed-rs         | Local, CPU, offline, no API key                 |
| LLM                  | Claude API (reqwest) | Extraction + re-ranking                         |
| MCP server           | rmcp                 | Rust MCP SDK                                    |
| Git operations       | git2 or shell git    | Branch/commit/push for PR creation              |
| Async runtime        | tokio                | Standard Rust async                             |

---

## Progress

- [ ] Phase 1: Merge existing branches to main
- [ ] Phase 2: Index + Retrieval + MCP (read path)
- [ ] Phase 3: Structured capture (write path)
- [ ] Phase 4: Web UI
- [ ] Migration: Replace brain-os hooks with nugget
