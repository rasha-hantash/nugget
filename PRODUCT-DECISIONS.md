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

**Verified**: Claude Code has a `SessionEnd` hook event. It receives `transcript_path`, `session_id`, `cwd`, and `reason` via stdin JSON. Supports `"async": true` for fire-and-forget. Configuration in `~/.claude/settings.json`.

### Decision: Post-session transcript analysis

A background process reads the full session transcript (JSONL files stored by Claude Code) and sends it to an LLM for knowledge extraction. This ensures nothing is lost to compaction.

**Verified**: Transcripts at `~/.claude/projects/<project-path-with-dashes>/<session-uuid>.jsonl`. JSONL format with `user`, `assistant`, and tool-use message types. The `SessionEnd` hook provides `transcript_path` directly.

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

### Decision: Hybrid search + graph expansion + LLM re-ranking

The retrieval pipeline searches against **chunks** (derived from files), grouped/ranked at the **unit** level:

1. **Embedding search** on chunks — vector similarity (~50 chunks)
2. **BM25/FTS5 search** on chunks — full-text match (~50 chunks)
3. **RRF fusion** — combine embedding + BM25 via Reciprocal Rank Fusion, map to parent units
4. **Graph expansion** via SQLite — walk relationship edges in the `relationships` table, pull in chunks from related units
5. **LLM re-ranking** — score chunks with unit context for actual relevance (top 5-10)

**Why hybrid search (embeddings + BM25)?** BM25 catches exact terminology that embeddings miss. Proven in production RAG systems.

**Why SQLite for graph expansion instead of a graph database?** See [Graph storage decision](#decision-sqlite-for-graph-relationships-no-graph-database) below. At Nugget's expected scale (hundreds to low thousands of units), SQLite recursive CTEs handle 1-3 hop traversals efficiently. The relationship data from ~30 candidate units easily fits in an LLM context window, making the model itself the most capable "graph engine" for relevance reasoning.

**Why build all layers from the start?** The full pipeline works at every brain size. At small sizes, embeddings + re-ranking carry the weight. Graph expansion kicks in as the brain grows.

### Decision: Markdown-aware chunking

Knowledge units can be large (10+ pages). Files are chunked in the derived index, not on disk. Heading-based structural chunking with breadcrumb prepending — split at `##`/`###` boundaries, prepend heading path to each chunk before embedding (e.g., "Go Concurrency > Worker Pools"). Sub-split oversized sections at paragraph boundaries; merge tiny sections with siblings.

### Decision: Two entry points, same engine

1. **Claude Code** — via MCP tool during sessions
2. **CLI** — `nugget ask "what are route tables?"` for direct terminal queries

### Deferred: Hook-based auto-injection (v2+)

Nugget intercepts the prompt, searches the brain, and injects relevant context before Claude Code responds. Currently blocked on Claude Code platform support (hooks can't inject content into conversations).

---

## Storage & Data Model

### Decision: Markdown files with YAML frontmatter (source of truth)

Human-readable, Git-versionable, editable in any editor. Non-negotiable — the PR workflow requires actual files.

### Decision: SQLite derived index (text search + embeddings + graph)

- **Units table**: id, path, title, type, domain, tags, confidence, source, created, last_modified, content
- **Chunks table**: id, unit_id, content (with breadcrumb), heading_breadcrumb, heading_level, position, embedding
- **Relationships table**: source_id, target_id, relation_type — graph edges between units
- **FTS5 virtual table**: full-text search over chunk content

Rebuilt from files if corrupted or lost. Nothing is lost.

### Decision: SQLite for graph relationships (no graph database)

Relationships between knowledge units are stored in a SQLite `relationships` table alongside all other derived data. No external graph database (Memgraph, Neo4j, etc.) is used.

**What lives in SQLite (all in one database):**

- Knowledge unit metadata (units table)
- Chunks with embeddings (chunks table)
- Relationship edges from frontmatter `related:` fields (relationships table)
- FTS5 full-text search (chunks_fts virtual table)

**Why SQLite is sufficient — no graph database needed:**

At Nugget's expected scale (hundreds to low thousands of knowledge units), a dedicated graph database is infrastructure complexity that doesn't earn its keep. This decision was informed by [Hamel Husain & Jo Kristian Bergum's analysis on graph databases in RAG systems](https://hamel.dev/notes/llm/rag/p7-graph-db.html), which argues that teams should exhaust simpler approaches before reaching for specialized graph infrastructure.

The core arguments:

1. **Scale doesn't justify it.** Graph databases earn their keep at millions of nodes with billions of edges (social networks, fraud detection, supply chains). Nugget's brain will have hundreds to low thousands of units — orders of magnitude below the threshold where SQLite recursive CTEs become a bottleneck. Early Facebook ran their social graph on MySQL.

2. **The model is the best graph engine at this scale.** After Layer 1c (RRF fusion), we have ~30 candidate units. Each has frontmatter with explicit `related: [{id: ..., relation: uses}]` fields. We can hand the model these units plus their relationship metadata and ask "which are relevant?" The model can reason over 30 units with relationship data trivially — this is exactly the kind of work LLMs excel at. Infrastructure complexity for graph traversal is a bet against rapidly improving model capabilities.

3. **Defining the graph is the hard part, not traversing it.** The challenging work is extracting meaningful relationships during capture (the LLM extraction in the write path). Once relationships are in frontmatter `related:` fields, traversing them in SQLite is straightforward — a simple recursive CTE handles 1-3 hops efficiently at our scale.

4. **One fewer runtime dependency.** No Docker container, no Bolt protocol, no sync strategy between two databases, no failure mode to handle when the graph DB is unavailable. SQLite is embedded — the entire derived index is a single file that rebuilds from markdown source of truth.

5. **Measure before adding complexity.** Per the article's guidance: teams should have evals proving that graph expansion improves retrieval quality before adopting graph infrastructure. Build the simpler version first, measure whether graph expansion via SQLite is a bottleneck, and only then consider whether a graph database would help.

**When would a graph database make sense?** If the brain grows to a scale where:
- SQLite recursive CTEs become measurably slow (unlikely below 100K units)
- Real-time multi-hop traversal latency matters (Nugget's retrieval is not latency-critical — it's a background MCP call)
- Graph algorithms (PageRank, community detection) demonstrably improve retrieval quality (measure first)

**Graph expansion implementation:** SQLite recursive CTEs for 1-3 hop traversal:

```sql
WITH RECURSIVE related_units(id, depth) AS (
    SELECT target_id, 1 FROM relationships WHERE source_id = ?
    UNION
    SELECT r.target_id, ru.depth + 1
    FROM relationships r JOIN related_units ru ON r.source_id = ru.id
    WHERE ru.depth < 3
)
SELECT DISTINCT id FROM related_units;
```

This is simple, fast at our scale, and doesn't require a separate database process.

**Why not Mem0 (the product):**

- Mem0 bundles graph construction + storage + retrieval with LLM extraction on every read/write — adds 3 runtime dependencies (LLM API, vector DB, graph DB) and API costs
- Nugget's 3-layer pipeline (BM25 + embeddings + LLM re-ranking) is more sophisticated than Mem0's retrieval
- For other users, "install Rust binary" is simpler than "set up Neo4j + Qdrant + OpenAI API key"
- The graph enrichment idea from Mem0 is valuable — we adopt it at write time (LLM extracts relationships during capture, writes them to frontmatter `related:` fields) without the runtime dependency

**Rebuild**: The SQLite database (including relationships) is derived from markdown files and can be rebuilt from scratch. `nugget rebuild` re-derives everything from the markdown source of truth.

**References:**

- [You Don't Need a Graph DB (Probably) — Hamel Husain / Jo Kristian Bergum](https://hamel.dev/notes/llm/rag/p7-graph-db.html) — primary inspiration for this decision
- [Mem0 Graph Memory architecture](https://docs.mem0.ai/open-source/features/graph-memory) — informed the graph enrichment approach (adopted at write time only)
- [Mem0 paper (arxiv)](https://arxiv.org/html/2504.19413v1) — dual retrieval (entity-centric + semantic triplet) informed Layer 2 design
- [Graph-based Agent Memory taxonomy](https://arxiv.org/html/2602.05665) — survey of graph memory approaches

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
