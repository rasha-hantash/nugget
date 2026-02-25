# Nugget: Decisions & Open Questions

This document tracks architectural and product decisions for Nugget. Resolved decisions include their rationale. Open questions are tracked for resolution during implementation.

For the complete list of product decisions with full context, see `PRODUCT-DECISIONS.md`.

---

## Resolved Decisions

### Write Path

#### No inbox — GitHub PRs for review

**Status**: RESOLVED

**Context**: The original design had an inbox where all captured knowledge landed for accept/reject review via CLI. Analysis showed this is the #1 retention risk:

- Clipboard captures ~5-15 URLs/day, AI session capture adds ~3-10 learnings/session
- That's 10-35 items/day to review
- Week 1: fun, novel. Week 2: 200 unreviewed items. Week 4: you stop opening it.
- This is the Instapaper/Pocket death spiral — save everything, review nothing.

**Resolution**: The capture agent creates a branch, commits proposed knowledge files, and opens a GitHub PR against the brain repo. The user reviews in GitHub's UI — comments, edits, approves, merges.

**Why PRs work**: Developers use PRs daily. GitHub UI provides richer review than accept/reject. No custom review UI to build. PR abandonment is more visible than inbox abandonment.

#### Session-end hook as capture trigger

**Status**: RESOLVED

**Context**: Two capture trigger options were considered:
1. MCP tool calls during session (Claude calls `capture_learnings` before session ends)
2. Session-end hook (fires automatically after every session)

**Resolution**: Session-end hook. Deterministic, zero friction, no dependency on Claude remembering.

**Why not in-session MCP calls?**
- Claude Code compresses earlier messages during long sessions (compaction loss)
- MCP tools depend on Claude remembering to call them ("Claude forgot" problem)
- Hooks are deterministic — they always fire

#### Post-session transcript analysis

**Status**: RESOLVED

**Resolution**: Read the full JSONL transcript file, not in-session context. Claude Code's compaction means in-session context may be missing earlier parts of the conversation. The transcript has everything.

#### No clipboard capture in v1

**Status**: RESOLVED — deferred to v2

**Context**: Clipboard capture was originally Phase 1A — a background daemon watching the macOS clipboard for URLs.

**Resolution**: Deprioritized. Session capture is the primary mechanism.

**Why**: Clipboard has low signal-to-noise. Copying a URL != understanding it. AI session capture observes real work and produces higher-quality knowledge. The knowledge quality difference is massive:
- Clipboard: zero friction capture, low quality knowledge
- AI session: zero friction capture, high quality knowledge

### Read Path

#### Single MCP tool — Nugget owns retrieval intelligence

**Status**: RESOLVED

**Context**: Three approaches were considered:
- **Option A**: Multiple MCP tools (search, browse, get) — Claude orchestrates
- **Option B**: Single `get_relevant_context(task_description)` tool — Nugget orchestrates
- **Option C**: Hook-based auto-injection — blocked on platform support

**Resolution**: Option B. Claude Code calls one tool, Nugget does all the work.

**Why**: Claude Code shouldn't be responsible for traversing the brain. One tool call is simpler and more reliable than multi-step orchestration. Nugget can optimize retrieval internally without changing the API.

#### Full 3-layer retrieval pipeline from MVP

**Status**: RESOLVED

**Context**: Incremental approach (embeddings-only first, add layers later) was considered.

**Resolution**: Build all three layers from the start:
1. Embedding search — vector similarity
2. Graph expansion — walk relationship edges
3. LLM re-ranking — score for actual relevance

**Why**: The full pipeline works at every brain size. At small sizes, embeddings + re-ranking carry the weight. Graph expansion kicks in as the brain grows. Every new user starts with an empty brain.

### Storage & Organization

#### Agent-managed organization

**Status**: RESOLVED

**Resolution**: The capture agent decides file placement, type, tags, relationships, confidence, filename. The user browses but doesn't organize.

**Why**: Reduces friction to zero on the write path. The agent reads existing brain structure and places files appropriately.

#### Dedicated GitHub repo for brain

**Status**: RESOLVED

**Resolution**: The brain lives in its own repo (e.g., `github.com/you/brain`).

**Why**: PRs need a repo. Separation from project repos keeps the brain portable. Git remotes enable future cross-brain features.

#### SQLite FTS5 instead of tantivy

**Status**: RESOLVED

**Resolution**: Use SQLite FTS5 for full-text search instead of tantivy.

**Why**: One fewer dependency. FTS5 is sufficient for expected brain sizes (hundreds to low thousands of units).

### Platform & Scope

#### Claude Code only

**Status**: RESOLVED

**Resolution**: Build exclusively for Claude Code. Not MCP-generic, not multi-tool.

**Why**: Focus. Claude Code is the target user's primary tool. Designing for generic MCP clients adds complexity without clear benefit in v1.

#### Minimal CLI

**Status**: RESOLVED

**Resolution**: Three primary commands: `nugget init`, `nugget serve`, `nugget ask "..."`. Plus `nugget capture-session` for hook invocation.

**Why**: The CLI is plumbing, not the product. The product is the automatic capture-and-retrieval loop.

---

## Open Questions

### 1. Claude Code hook mechanics

**Priority**: BLOCKER for Phase 3

Does Claude Code support session-end hooks? What event fires? How does the hook access the transcript file path? This is a hard technical dependency — need to verify feasibility before building the write path.

### 2. Brain repo structure

**Priority**: HIGH — needed for Phase 1

Exact directory layout: flat domains at top level? Nested types within domains? File naming conventions (slugified title? UUID prefix?)?

### 3. PR format

**Priority**: MEDIUM — needed for Phase 3

What goes in the PR title ("Knowledge from session 2026-02-24 14:30"?), description (summary of what was learned?), commit message? Should the PR body list each proposed knowledge unit?

### 4. Capture agent packaging

**Priority**: MEDIUM — needed for Phase 3

Part of the `nugget` binary (`nugget capture-session`)? Separate binary? How does the hook invoke it? Background process management (what if it crashes?).

### 5. Relationship detection during capture

**Priority**: MEDIUM — needed for Phase 3

How does the capture agent identify relationships to existing knowledge? It needs to read the brain index during extraction. This creates a dependency: the index must exist before capture works well. First capture with empty index = no relationships detected.

### 6. Conflict resolution

**Priority**: LOW — edge case

What happens if a PR proposes changes to an existing knowledge file updated on main since the branch was created? Auto-rebase? Alert the user?

### 7. Transcript file discovery

**Priority**: HIGH — needed for Phase 3

How does the capture agent find the session's transcript file? Claude Code stores transcripts in `~/.claude/projects/<hash>/`. Need to verify the exact path pattern and JSONL format.

### 8. Multiple sessions in flight

**Priority**: LOW — edge case

If two sessions end close together, how do we avoid race conditions in PR creation? Unique branch names (timestamp + random suffix) should handle this, but needs verification.

---

## Future Considerations

### Knowledge staleness (v2)

No mechanism for knowledge to age out. "Use Redis for sessions" might become harmful if you've since moved to Postgres.

**V1 foundation**: `created` and `last_modified` timestamps in every file.
**V2 approach**: Time-based confidence decay. LLM-powered "is this still true?" detection during retrieval.

### Cold start

An empty brain = zero value. V1 relies on organic growth through session capture. V2: import existing CLAUDE.md files, project docs, notes to seed 20-50 units quickly. Interview mode could seed 20+ units in 30 minutes.

### The `---` problem

Markdown content legitimately contains `---` (horizontal rules). The parser must only treat the first `---` pair as frontmatter. This is a common gotcha — handle in Phase 1.

### Concurrency edge cases

- Two sessions ending simultaneously -> branch naming collisions
- Manual edits to brain files while capture running -> stale index
- Invalid YAML frontmatter from manual edits -> graceful error handling, not panics

### Git edge cases

- Merge conflicts in YAML frontmatter are ugly and confusing
- Brain directory inside cloud-synced folder (iCloud, Dropbox) + Git = corruption
- Large captured content bloats repo over time (consider body size limits)

### Path to profitability

The personal tool is not monetizable on its own. The path: personal tool -> developer adoption -> prove flywheel -> add team features -> sell to engineering orgs as knowledge management. The personal tool is the wedge, not the business.

### Shared review engine (v2+)

The PR-against-a-repo pattern is generic. If building other products with similar review workflows (AI code reviewer), the proposal model, comment threads, and accept/reject actions could be extracted. Build Nugget first, extract later ("extract, don't abstract").
