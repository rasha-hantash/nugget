# Nugget: Implementation Plan

## Context

Nugget is a personal knowledge brain — a Git-versioned collection of markdown files that AI agents can read and write to. The core problem: making it **incredibly easy** for people to add their knowledge without manual effort.

Two capture mechanisms emerged as priorities:

1. **Clipboard capture** — a background listener that detects URLs (and optionally prose) copied to the clipboard and proposes them to the inbox. No code blocks — those aren't knowledge; the _workflow_ of solving a problem is.
2. **AI session capture** — Claude Code (and eventually Claude co-work) pushes learnings to your inbox after helping you. Zero friction — the AI was already there, it saw what happened.

Both are opt-in. Both produce inbox items in the same format. Both can be built in parallel after a thin shared foundation.

---

## Build Order

### Phase 0: Shared Foundation (sequential, do first) -- DONE

Build the minimal shared types and inbox infrastructure that both workstreams depend on.

**nugget-core** (`crates/nugget-core/`)

- `KnowledgeUnit` struct: id, type, domain, tags, confidence, source, related, body
- `InboxItem` struct: extends KnowledgeUnit with `suggested_domain`, `suggested_path`, `captured_at`, `capture_method`, `capture_context`
- `KnowledgeType` enum: concept, pattern, decision, bug, belief
- `CaptureMethod` enum: clipboard-url, clipboard-text, ai-session, web-capture, manual
- `Domain`, `Tag`, `Confidence`, `Relation` types
- Frontmatter serialization/deserialization (serde + serde_yaml)

**nugget-store** (`crates/nugget-store/`)

- Brain directory operations: `init` (create brain/ structure), `domain add`
- Markdown + YAML frontmatter parser/writer (serde_yaml)
- Read/write knowledge files to disk

**nugget-inbox** (`crates/nugget-inbox/`)

- Write inbox items to `brain/inbox/` with timestamped filenames
- List inbox items (parse all files in inbox/)
- Accept: move from inbox/ to domain path, strip inbox-specific fields
- Reject: delete from inbox
- Batch operations: accept/reject multiple by index

**nugget-cli** (`crates/nugget-cli/`) — minimal

- `nugget init` — create brain directory
- `nugget domain add <name>` — create domain folder + domain.yaml
- `nugget domain list` — list all domains
- `nugget inbox` — list pending items
- `nugget accept <indices>` / `nugget reject <indices>`
- `nugget review` — interactive one-by-one review

**Status**: All 4 crates built, compiling, 7 tests passing, clippy clean.

---

### Phase 1A: Clipboard Capture (parallel workstream)

A background daemon that watches the macOS clipboard and proposes URLs to the inbox.

**nugget-clipboard** (`crates/nugget-clipboard/`)

- macOS pasteboard polling (using `arboard` crate)
- Poll interval: ~500ms–1s
- Change detection: only act when clipboard content changes

**Heuristic filter pipeline:**

1. **Type detection**: is this a URL, prose, code, or junk?
   - URL regex match → proceed
   - Code detection (indentation patterns, syntax keywords, brackets density) → **drop** (per design decision: code blocks aren't knowledge)
   - Short strings (< 20 chars) → drop
   - Looks like a password/token (high entropy, no spaces) → drop
2. **URL filtering:**
   - Ignore common non-knowledge URLs: localhost, google.com, github.com/settings, mail.google.com, etc.
   - Allow: blog posts, documentation, Stack Overflow, articles, tech sites
   - Special case: `claude.ai/chat/*` URLs → flag for richer extraction (future: Claude co-work capture)
3. **Dedup**: skip if this exact URL was captured in the last 24h
4. **Propose**: write an InboxItem to `brain/inbox/` with `capture_method: clipboard-url`

**For the MVP, URL-only capture.** Prose detection is a future enhancement — the SNR is too low without more context about what app the user copied from.

**Daemon lifecycle:**

- `start()` — start monitoring in background, write PID to `brain/.nugget/clipboard.pid`
- `stop()` — read PID file, send SIGTERM
- `status()` — check if PID is alive
- `run()` — run in foreground (for development/debugging)

**Config** in `brain.yaml`:

```yaml
clipboard:
  enabled: true
  capture_urls: true
  capture_text: false # opt-in later
  poll_interval_ms: 500
  ignore_domains:
    - localhost
    - mail.google.com
    - accounts.google.com
```

**CLI integration:**

- `nugget daemon start` — start the clipboard listener in background
- `nugget daemon stop` — stop it
- `nugget daemon status` — is it running?
- `nugget daemon run` — run in foreground

**Key dependencies:** nugget-core, nugget-inbox, nugget-store, arboard, regex

---

### Phase 1B: AI Session Capture (parallel workstream)

An MCP server that Claude Code (and other AI tools) can call to push learnings to your inbox.

**nugget-capture** (`crates/nugget-capture/`)

- `capture_from_conversation(summary, learnings, decisions, context)`:
  - Each learning → separate InboxItem of type `pattern` or `concept`
  - Each decision → separate InboxItem of type `decision`
  - `capture_method: ai-session`
  - `confidence: 0.7` (AI-observed, not yet user-validated)
  - Suggested domain based on context (e.g., if working in a Rust project → `coding/rust`)
- `capture_from_url(url, title, summary, tags)`:
  - Create an InboxItem with the URL as source, title as first line of body, summary as body
  - `capture_method: web-capture`
  - For MVP, accept pre-processed data (the MCP tool caller does the extraction)
- `capture_from_text(text, source, suggested_domain)`:
  - Create an InboxItem from raw text
  - `capture_method: manual`
  - For MVP, treat the text as a single knowledge unit (skip LLM splitting)

**nugget-mcp** (`crates/nugget-mcp/`)

- MCP server using `rmcp` crate (Rust MCP SDK)
- Expose tools:
  - `capture_learnings` — post-session knowledge push (summary, learnings[], decisions[], context?)
  - `capture_url` — URL-based capture (url, title, summary, tags?, domain?)
  - `capture_text` — raw text capture (text, source?, domain?)
  - `inbox_status` — how many items pending, recent items preview
- Entry point: `nugget mcp --brain ~/brain`
- Uses stdio transport (stdin/stdout for Claude Code communication)
- Claude Code config:
  ```json
  {
    "mcpServers": {
      "nugget": {
        "command": "nugget",
        "args": ["mcp", "--brain", "~/brain"]
      }
    }
  }
  ```

**Key dependencies:** nugget-core, nugget-inbox, nugget-store, rmcp, tokio, serde_json

---

### Phase 2: Search & Retrieval (after both workstreams converge)

Make the captured knowledge actually useful.

- **nugget-index**: SQLite metadata store + tantivy full-text search
- **nugget-retrieve**: basic 2-layer retrieval (embedding search + full-text, skip LLM reranking for MVP)
- **nugget-mcp**: add `search_knowledge` and `get_knowledge` tools
- **nugget-cli**: `nugget search "query" --domain coding`

This is when the full loop closes: Claude Code captures knowledge → user reviews inbox → knowledge enters brain → Claude Code retrieves it next session.

---

## What each parallel agent needs to know

### Agent A: Clipboard Capture

- Owns: `crates/nugget-clipboard/`
- Depends on: nugget-core (types), nugget-inbox (write items), nugget-store (brain path)
- Focus: macOS clipboard polling, URL detection, heuristic filtering, daemon lifecycle
- Does NOT need: LLM calls, MCP, network fetching

### Agent B: AI Session Capture

- Owns: `crates/nugget-capture/`, `crates/nugget-mcp/`
- Depends on: nugget-core (types), nugget-inbox (write items), nugget-store (brain path)
- Focus: MCP server, capture functions, conversation/URL/text capture
- Does NOT need: clipboard APIs, daemon management

---

## Verification

1. **Phase 0**: `nugget init && nugget domain add coding && nugget inbox` → empty inbox, correct directory structure
2. **Phase 1A**: Start daemon → copy a blog URL → `nugget inbox` shows it → copy `localhost:3000` → inbox unchanged → copy a code snippet → inbox unchanged
3. **Phase 1B**: Configure Claude Code with Nugget MCP → have Claude Code call `capture_learnings` → `nugget inbox` shows the captured learnings → `nugget accept 1` → file appears in correct domain folder
4. **End-to-end**: Copy a URL while browsing → it appears in inbox. Ask Claude Code for help debugging → it captures learnings at session end → they appear in inbox. Review and accept both. Run `nugget search` → both are findable.

---

## Tech stack summary

| Component        | Crate            | Key deps                         |
| ---------------- | ---------------- | -------------------------------- |
| Core types       | nugget-core      | serde, serde_yaml, chrono, uuid  |
| File store       | nugget-store     | nugget-core, serde_yaml, walkdir |
| Inbox            | nugget-inbox     | nugget-core, nugget-store        |
| Clipboard daemon | nugget-clipboard | arboard, regex, nugget-inbox     |
| Capture engine   | nugget-capture   | nugget-inbox                     |
| MCP server       | nugget-mcp       | rmcp, tokio, nugget-capture      |
| CLI              | nugget-cli       | clap, nugget-\*                  |
| Index (Phase 2)  | nugget-index     | rusqlite, tantivy, fastembed     |

---

## Current state

- **Phase 0**: DONE — all 4 foundation crates built and tested
- **Phase 1A**: NOT STARTED
- **Phase 1B**: NOT STARTED
- **Phase 2**: NOT STARTED
