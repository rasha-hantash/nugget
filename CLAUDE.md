# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is Nugget

Nugget is an AI memory layer for Claude Code. It automatically extracts knowledge from Claude Code sessions and makes it available for future sessions via MCP. Written in Rust as a single binary.

- **Write path**: SessionEnd hook -> transcript analysis -> LLM extraction -> GitHub PR for review
- **Read path**: Single MCP tool `get_relevant_context(task_description)` -> hybrid search + graph expansion + LLM re-ranking
- **Storage**: Markdown files with YAML frontmatter (source of truth) + SQLite derived index

## Build & Test Commands

```bash
cargo build                          # Debug build
cargo build --release                # Release build
cargo test                           # All tests (19 across 3 crates)
cargo test -p nugget-core            # Core types + frontmatter tests (7)
cargo test -p nugget-store           # File I/O + brain directory tests (10)
cargo test -p nugget-cli             # CLI tests (2)
cargo test -p nugget-core -- test_name  # Single test
cargo clippy --all-targets --all-features  # Lint
cargo fmt --check                    # Format check
INSTA_UPDATE=accept cargo test       # Update insta snapshots after intentional changes
```

Run the CLI: `./target/debug/nugget init --path ./brain`

## Crate Architecture

```
crates/
  nugget-core/     # Core types: KnowledgeUnit, KnowledgeType, Relation, frontmatter parsing
  nugget-store/    # Brain directory ops (init, validate, walk) + file I/O (read/write units)
  nugget-cli/      # CLI entry point (clap derive). Currently: `nugget init`
```

Dependency flow: `nugget-cli -> nugget-store -> nugget-core`

### Planned crates (not yet implemented)

- `nugget-index` — SQLite + FTS5 + embeddings (rusqlite, fastembed-rs)
- `nugget-retrieve` — 3-layer retrieval pipeline (embedding + BM25 + graph + LLM re-ranking)
- `nugget-mcp` — MCP server with single tool (rmcp)
- `nugget-capture` — Transcript analysis + GitHub PR creation

## Key Patterns

**Frontmatter format**: Knowledge files are markdown with YAML frontmatter delimited by `---`. Only the first `---` pair is frontmatter; subsequent `---` in body are horizontal rules. The `body` field on `KnowledgeUnit` is `#[serde(skip)]` — it's handled separately by the frontmatter parser, not by serde.

**Brain directory structure** (created by `nugget init`):

```
brain/
  brain.yaml       # { version: 1 }
  domains/         # Subdirectories per knowledge domain
  .gitignore       # Ignores .nugget/
```

**Error handling**: `thiserror` with `NuggetError` enum. All fallible functions return `nugget_core::Result<T>`.

**Testing**: Unit tests inline with `#[cfg(test)]`. Snapshot tests use `insta` crate (snapshots in `src/snapshots/`). File-based tests use `tempfile::TempDir` for isolation.

**KnowledgeUnit.kind vs "type"**: The Rust field is `kind` but serializes as `type` via `#[serde(rename = "type")]` since `type` is a reserved keyword.

## Product Decisions

Key decisions are documented in `PRODUCT-DECISIONS.md` and `DECISIONS.MD`. The implementation roadmap is in `IMPLEMENTATIONS.MD`. These docs are the authoritative source for architectural questions — consult them before proposing alternatives to settled decisions.
