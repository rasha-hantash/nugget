# Nugget

**An AI memory layer for Claude Code.**

---

## What is it?

Nugget automatically extracts knowledge from your Claude Code sessions and makes it available for future sessions. It's a collection of markdown files — patterns you use, decisions you've made, bugs you've fixed, concepts you understand — organized by domain, versioned with Git, and queryable by AI.

When Nugget is connected to Claude Code, the AI stops giving you generic answers and starts giving you answers informed by _your_ knowledge, _your_ conventions, and _your_ past decisions.

---

## Who is it for?

**Individual developers using Claude Code** who want their AI assistant to remember what they've learned across sessions.

V1 is laser-focused on this user. Team features, other AI tools, and non-developers are future expansion.

---

## The Core Problem

Every Claude Code session starts from zero. You teach Claude how you think, what conventions you follow, what you've tried before — and then the session ends and all of that knowledge vanishes. The next session, you start over.

Nugget makes that knowledge persist.

---

## How It Works

### The flywheel

```
You work with Claude Code → session ends
    ↓
Nugget reads the session transcript → extracts knowledge
    ↓
Opens a PR in your brain repo → you review and merge
    ↓
Next session, Claude Code queries your brain → gives better answers
    ↓
You work better → more knowledge captured → ...
```

The more you use Claude Code with Nugget, the smarter it gets. The knowledge compounds.

### Your brain is a Git repo

A dedicated GitHub repo (e.g., `github.com/you/brain`) containing markdown files organized by domain:

```
brain/
  brain.yaml                          # Brain metadata and config
  domains/
    coding/
      concepts/cache-invalidation.md
      patterns/retry-with-idempotency.md
      decisions/redis-for-sessions.md
    coding/go/
      patterns/error-handling.md
    coding/rust/
      concepts/ownership-patterns.md
    management/
      patterns/one-on-one-framework.md
  .nugget/                            # Gitignored — derived state
    index.db                          # SQLite: metadata + graph + FTS5
    embeddings/                       # Vector embeddings for search
```

Each file is a knowledge unit with structured metadata:

```markdown
---
type: pattern
domain: coding
tags: [stripe, payments, reliability]
confidence: 0.9
source: ai-session
related:
  - id: concept/idempotency-keys
    relation: uses
created: 2026-02-24
last_modified: 2026-02-24
---

# Retry with Idempotency Keys

When retrying Stripe API calls, ALWAYS use idempotency keys...
```

Files encode a knowledge graph through their relationships. Nugget indexes this into a searchable, traversable structure. But the files are always the source of truth — human-readable, Git-versioned, editable in any tool.

### Organization is agent-managed

You never manually organize your brain. The capture agent decides:

- Which domain folder to place each file in
- What type it is (pattern, concept, decision, bug, belief)
- What tags to apply
- What relationships to create (links to existing knowledge)
- Confidence level and filename

You can always browse the brain repo — it's just folders and markdown files — but you don't need to manage it.

---

## How Knowledge Gets In

### Primary: Automatic session capture

Every Claude Code session is automatically captured. When a session ends:

1. A session-end hook fires
2. A background process reads the full session transcript (JSONL)
3. Sends it to an LLM for knowledge extraction
4. Extracts reusable patterns, architectural decisions, domain knowledge, debugging insights
5. Creates a branch in your brain repo
6. Commits proposed knowledge files to appropriate domain directories
7. Opens a GitHub PR

You review the PR in GitHub — the UI you already use. Comment, edit, approve, merge. Merged knowledge is in your brain for the next session.

**Why PRs instead of an inbox?** PRs are a review model developers use daily. No new workflow to learn, no custom UI to build, and PR abandonment is more visible than inbox abandonment.

**Why post-session transcript analysis?** Claude Code compresses earlier messages during long sessions. By reading the full transcript file, nothing is lost.

### Future capture mechanisms (v2+)

- **Import existing docs** — seed the brain from CLAUDE.md files, project docs, notes
- **Interview mode** — Nugget asks you questions to extract tacit knowledge
- **Clipboard capture** — background URL monitoring
- **Cross-brain cherry-pick** — pull knowledge from colleagues' brains

---

## How Claude Code Uses Your Knowledge

### Single MCP tool

Claude Code calls one tool: `get_relevant_context(task_description)`. Nugget handles all the intelligence internally — keyword extraction, embedding search, graph traversal, relevance ranking. Claude Code's only job is to call the tool and use the results.

### Three-layer retrieval pipeline

**Layer 1 — Similarity search.** Find the ~50 knowledge units most semantically similar to the current task. Fast but imprecise.

**Layer 2 — Graph expansion.** For the top results, walk their relationship links. Your `cache-invalidation` concept links to `ttl-strategy` pattern and `redis-choice` decision. Those wouldn't have matched on text alone but are exactly what you need.

**Layer 3 — Relevance ranking.** An LLM scores the remaining candidates for actual relevance to the specific task. Filters out false positives. Factors in confidence, freshness, and source quality.

The result: the 5-10 most relevant pieces of YOUR knowledge, ranked from most to least useful.

Domain context helps too. When you're working in a coding context, coding knowledge gets boosted — but not exclusively. A management insight about "technical debt prioritization" might still surface if it's genuinely relevant.

---

## How You Interact With It

### CLI (minimal)

- `nugget init` — one-time brain repo setup
- `nugget serve` — start the MCP server for Claude Code
- `nugget ask "cache invalidation strategies"` — direct brain query from the terminal

### Claude Code MCP

Add Nugget to your Claude Code config:

```json
{
  "mcpServers": {
    "nugget": {
      "command": "nugget",
      "args": ["serve", "--brain", "~/brain"]
    }
  }
}
```

Claude Code automatically queries your brain for relevant context before answering.

### Your editor

Knowledge files are just markdown. Open them in VS Code, Obsidian, or any text editor.

### GitHub

PRs for review. Browse your brain repo on GitHub. Standard Git workflows.

---

## What Makes It Different

| Other tools                                           | Nugget                                                      |
| ----------------------------------------------------- | ----------------------------------------------------------- |
| Claude Code starts every session from zero            | Nugget carries knowledge across sessions                    |
| CLAUDE.md is manual and project-scoped                | Nugget captures automatically and works across all projects |
| Note-taking apps require you to organize as you write | AI extracts and organizes for you; you just review PRs      |
| Knowledge bases are write-once, read-never            | Three-layer retrieval makes knowledge actually get used     |
| AI tools use generic training data                    | AI tools connected to Nugget use YOUR knowledge             |

---

## Privacy

Session transcripts are sent to the Claude API for knowledge extraction. This is the same trust boundary as using Claude Code itself — your code is already going through Claude.

---

## The Vision

Today: Claude Code is smarter because it knows what you know.

Tomorrow: import your existing docs, interview mode to extract tacit knowledge, share brains with colleagues, team knowledge that compounds.

The personal tool is the foundation. Everything else builds on top.
