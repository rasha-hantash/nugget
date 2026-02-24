# Nugget: Decisions & Open Questions

## Holes in the Personal Use Case

### 1. The Read Path Doesn't Exist Yet — And It's Where All the Value Is

Right now the plan is: capture knowledge (Phase 0-1) → search/retrieve (Phase 2). But "making Claude Code smarter" requires Claude Code to actually _read_ your brain. Until Phase 2 ships, Nugget is a write-only system — a fancy note-taker with no payoff.

Worse, even with Phase 2's MCP search tools, there's a **discovery problem**: Claude Code doesn't proactively search external knowledge sources before answering. You'd need to either:

- Add "always check my Nugget brain first" to your CLAUDE.md (fragile, manual)
- Manually ask Claude "check my brain for X" every time (defeats the purpose)
- Build a Claude Code hook that auto-queries Nugget before every response (invasive, slow)

**This is the single biggest risk.** If the read path is clunky, the flywheel never spins. Consider pulling a minimal read path into Phase 1B — even just exposing the inbox + accepted files as MCP resources so Claude Code can see them without a full search pipeline.

### 2. The Inbox Will Kill You

The inbox-review model is your quality control, but it's also the most likely point of abandonment. Math:

- Clipboard captures ~5-15 URLs/day if you're a heavy browser
- AI session capture adds ~3-10 learnings/session, maybe 2 sessions/day
- That's 10-35 items/day to review

Week 1: fun, novel. Week 2: 200 unreviewed items. Week 4: you stop opening it. This is the Instapaper/Pocket death spiral — save everything, review nothing.

Mitigations worth considering:

- **Auto-expire**: inbox items older than 7 days get archived, not deleted
- **Confidence-based auto-accept**: AI session learnings with high confidence from your own session could skip the inbox entirely
- **Batch-smart review**: instead of one-by-one, show a daily digest grouped by domain — "accept all 7 coding items? [y/n]"

### 3. Knowledge Staleness

There's no mechanism for knowledge to age out or get revalidated. "Use Redis for sessions" was good in January 2024. If you've since moved to Postgres sessions, that knowledge is now _actively harmful_ — Claude Code will confidently suggest the wrong thing because your brain told it to.

You need at minimum:

- A `last_validated` timestamp
- A decay function on confidence over time
- Or a periodic "is this still true?" review prompt

### 4. Cold Start: Weeks of Effort Before Any Payoff

An empty brain = zero value. A brain with 5 items = near-zero value. You probably need ~50-100 well-curated knowledge units before retrieval starts being meaningfully better than Claude's base knowledge. That's weeks of daily inbox review for a developer who's busy _actually developing_.

The interview mode described in the product doc is the right answer here, but it's not in the implementation plan. Consider prioritizing it — a 30-minute interview session could seed 20-30 high-quality units instantly.

### 5. Clipboard Capture SNR Is Genuinely Low

Copying a URL doesn't mean you read it, understood it, or endorse it. The clipboard capture creates a brain full of "articles I meant to read" rather than "things I actually know." This is fundamentally different from AI session capture, where the AI participated in real work and observed real outcomes.

For personal use, consider deprioritizing clipboard capture (Phase 1A) and focusing entirely on AI session capture (Phase 1B). The knowledge quality difference is massive, and the friction-to-value ratio is inverted:

- Clipboard: zero friction capture, low quality knowledge
- AI session: zero friction capture, high quality knowledge

### 6. Concurrency Edge Cases

These are real implementation bugs waiting to happen:

- User runs `nugget accept 3` while the clipboard daemon is writing a new item — index shift mid-operation
- Two AI sessions capture simultaneously — sub-second timestamp collisions in filenames (UUIDs help, but the index-based UI becomes confusing)
- `nugget accept` crashes mid-move — file is deleted from inbox but never written to the domain folder. You need atomic move or a write-then-delete pattern.
- Manual edits to knowledge files introduce invalid YAML frontmatter — every read operation needs graceful error handling, not panics

### 7. The `---` Problem

Your body is separated from frontmatter by `---`. But markdown content legitimately contains `---` (horizontal rules, nested frontmatter in pasted content). Your parser needs to handle this robustly — only the _first_ `---` pair is frontmatter. This is a common gotcha.

---

## On Git as Foundation

For the **personal developer** use case, Git is the right call:

- Developers already know it
- Version history is free
- Branching/merging enables the "cherry-pick from Sarah's brain" workflow
- Offline-first, no server dependency
- GitHub/GitLab give you free hosting and a web UI

The concern about non-technical users is real but it's a **different product**. Don't let it pollute the MVP. If Nugget succeeds for developers first, the hosted/UI version is a natural expansion — you wrap Git in a web service and users never see it.

Real Git edge cases for personal use:

- Merge conflicts in YAML frontmatter are ugly and confusing
- If the brain directory is inside a cloud-synced folder (iCloud, Dropbox), Git + sync = corruption
- Large captured articles bloat the repo over time (consider a body size limit or separate content-addressed storage)

---

## Path to Profitability

**The personal use case is not monetizable on its own.** A solo developer getting slightly better Claude Code answers is a nice-to-have. The effort to maintain a brain doesn't justify paying for it when CLAUDE.md files and project-level context already exist.

Where the money is:

1. **Team knowledge sharing (the real product)** — "Your new hire queries the team brain and gets up to speed in days, not months." Measurable ROI: reduced onboarding time, preserved institutional knowledge when people leave, fewer repeated mistakes. Requires permissions, access control, curation workflows, and probably a hosted service.

2. **AI extraction as the premium layer** — Free: manual capture + CLI. Paid: AI-powered capture (session learnings, URL extraction, interview mode) + hosted retrieval API. The LLM calls cost money anyway, so charging for them is natural.

3. **Curated expert brains (speculative but interesting)** — "Subscribe to a senior Rust engineer's public brain" or "Download the security best practices brain." Marketplace play — high upside, high risk, needs critical mass.

4. **Most realistic path** — Build the personal tool → get developer adoption → prove the flywheel works → add team features → sell to engineering orgs as a knowledge management tool. The personal tool is the wedge, not the business.

---

## Revised MVP Priorities

1. **Skip Phase 1A (clipboard)** for now. AI session capture is higher quality, lower noise, and directly demonstrates the flywheel. Clipboard is a "nice to have" that adds complexity without proving the core thesis.

2. **Pull a minimal read path into Phase 1B.** Even if it's just exposing files as MCP resources (no search index), Claude Code needs to be able to _use_ the knowledge immediately. Otherwise you're asking users to invest weeks before seeing any return.

3. **Add a `nugget seed` or `nugget interview` command early.** Solve the cold start problem. Let users dump their existing CLAUDE.md rules, coding preferences, and mental models into the brain in one session.

4. **Make inbox review embarrassingly fast.** The accept/reject UX is the make-or-break for retention. Consider: `nugget review` shows items one at a time with a single keypress (y/n/s for accept/reject/skip), auto-categorized, with sensible defaults that make "accept all" safe for high-confidence items.

The core thesis is strong — knowledge compounds, AI should use YOUR knowledge, and capture should be frictionless. The risk isn't the idea, it's the activation energy: how long before a user feels the payoff. Shrink that gap and the rest follows.

---

## How Claude Code Accesses Brain Knowledge

The read path is where Nugget delivers value. Capturing knowledge is necessary but not sufficient — Claude Code needs to actually _use_ what's in the brain. Three approaches, in order of implementation complexity:

### Option A: MCP tools + smart instructions (implementing now)

The MCP server exposes browsing and search tools: `list_domains`, `list_knowledge`, `read_knowledge`, `search_brain`. The `ServerInfo.instructions` field tells Claude: "You have access to the user's personal knowledge brain. Before starting tasks, check the brain for relevant knowledge." Claude proactively calls tools based on the task at hand. Nugget provides the tools, Claude decides when to use them.

- **Pros**: works today, no external dependencies, Claude already knows how to use MCP tools
- **Cons**: Claude may not always remember to check the brain; relies on instruction-following

### Option B: Single "give me what I need" tool (future)

Instead of multiple browse/search tools, expose one tool: `get_relevant_context(task_description)`. Claude passes in the full task description, nugget figures out what's relevant and returns it. Nugget is the smart part, not Claude — it does the relevance matching internally.

Requires: keyword extraction, tag matching, possibly embeddings/semantic search.

- **Pros**: simpler for Claude (one tool call), nugget controls relevance quality
- **Cons**: harder to build well, needs good relevance matching to avoid returning noise

### Option C: Hook-based auto-injection (future, currently blocked)

A hook fires on prompt submission, searches the brain based on the prompt text, and injects relevant knowledge into Claude's context automatically. The user never asks Claude to "check my brain" — it happens transparently.

Currently **not feasible**: Claude Code hooks can run shell commands but cannot inject content into the conversation. Would require changes to Claude Code itself (e.g., a pre-prompt context injection mechanism).

- **Pros**: zero-friction, fully automatic, highest-quality UX
- **Cons**: blocked on Claude Code platform changes, risk of injecting irrelevant context, latency on every prompt

### Decision

Start with A, evolve toward B as we learn what "relevant" means in practice. C is the north star but blocked on platform support.

---

## Revised Inbox Model: PRs Against Your Brain

### The Problem with the Current Inbox

The current inbox is a flat list of files with binary accept/reject. That's too thin. When you're reviewing a proposed knowledge unit, you need:

- To see _where_ it will live in the brain structure — not just the file, but the tree around it
- To leave feedback, not just accept/reject — "good, but reclassify this" or "merge with my existing concept"
- The AI that proposed it to be able to _read your feedback_ and respond or update
- To modify the content before accepting, not just take it as-is

### The Mental Model: Every Capture is a PR

Every capture — whether from clipboard, AI session, or manual — creates a **proposal** against your brain. You review it with full context, leave comments, and merge/reject/request changes. The AI can see your feedback and iterate.

This is not a GitHub PR clone. It's a lighter-weight review flow tuned for knowledge:

```
nugget review 3

  Proposed by: Claude Code (ai-session)
  Captured: 2 minutes ago
  Target: brain/domains/coding/rust/patterns/error-handling-with-thiserror.md

  brain/
    domains/
      coding/
        rust/
          concepts/
            ownership-patterns.md
            lifetime-elision.md
          patterns/
            retry-with-backoff.md
          ► error-handling-with-thiserror.md  [NEW]  ← proposed
      management/
    inbox/
      ...3 more items

  ---
  type: pattern
  domain: coding/rust
  tags: [error-handling, thiserror, anyhow]
  confidence: 0.7
  ---

  # Error Handling with thiserror

  When building library crates, use `thiserror` for defining error types...

  [a]ccept  [r]eject  [m]odify  [c]omment  [s]kip
```

### Review Actions

- **Accept** — merge into brain at the target path
- **Reject** — discard (or archive to `.nugget/rejected/`)
- **Modify** — edit the content, metadata, or target path before accepting (opens in `$EDITOR` or inline edit)
- **Comment** — leave a note for the AI that proposed it ("this should also mention anyhow vs thiserror", "confidence should be higher — I've validated this")
- **Skip** — come back to it later

### Comment Threads: The AI Feedback Loop

Comments are stored alongside the proposal. When Claude Code (or any AI tool) connects via MCP, it can call `get_review_feedback` to see your comments on its proposals and either update the item or respond.

This closes a loop that doesn't exist today: AI proposes → you give nuanced feedback → AI learns from your feedback → next proposal is better. Without comments, the AI only knows "accepted" or "rejected" — no signal about _why_.

### What This Means for the Data Model

The current `InboxItem` is a single markdown file with frontmatter. The revised model needs:

- **The proposed file** — the knowledge unit content (what exists today)
- **Proposal metadata** — who proposed it, when, why, target path, current status
- **Comment thread** — a list of comments with author + timestamp (yours and the AI's)
- **Brain context** — the target location and what already exists nearby

Implementation options:

1. **Directory per proposal**: `brain/inbox/<id>/proposal.md` + `brain/inbox/<id>/comments.yaml` + `brain/inbox/<id>/meta.yaml`
2. **Sidecar files**: `brain/inbox/<id>.md` + `brain/inbox/<id>.comments.yaml`
3. **Embedded in frontmatter**: extend the frontmatter with a `comments` array (simplest but gets unwieldy)

Option 1 (directory per proposal) is cleanest — each proposal is self-contained and easy to enumerate.

### MCP Tools for the Review Loop

The MCP server needs new tools to support this:

- `get_review_feedback(proposal_id?)` — get comments on a specific proposal or all proposals with unread comments
- `respond_to_comment(proposal_id, comment)` — AI responds to a reviewer comment
- `update_proposal(proposal_id, updated_content)` — AI revises a proposal based on feedback

### Phasing

This doesn't require stopping the current Phase 1A/1B work. The capture logic (clipboard polling, MCP server, writing files) stays the same. What changes:

1. **Phase 1.5**: Restructure inbox from flat files to directory-per-proposal format, add comment support, update `nugget review` CLI with tree view and comment flow
2. **Phase 1B addition**: Add `get_review_feedback` and `respond_to_comment` MCP tools so the AI side of the loop works

The current flat-file inbox is a fine stepping stone — it proves capture works. The review UX is a layer on top, not a rewrite.

---

## Architecture: Shared Review Engine for Multiple Products

### The Three-Layer Architecture

Nugget and a future AI code reviewer are not parent-child — they're siblings built on a shared foundation.

```
Layer 3: Products (what users see)
  ├── Nugget (knowledge review)
  └── AI Code Reviewer (code review)

Layer 2: Review Engine (shared)
  ├── Proposal model (directory-per-proposal)
  ├── Comment threads
  ├── Accept/reject/modify/comment actions
  ├── Tree context view
  └── MCP tools for AI feedback loop

Layer 1: Storage (shared)
  ├── Git-versioned files
  ├── YAML frontmatter parsing
  └── File I/O
```

### Layer 1: Storage (File I/O)

File I/O means reading and writing files to disk — the low-level operations everything depends on. In Nugget, this is what `nugget-store` handles:

- **Reading**: parsing a markdown file from `brain/domains/coding/patterns/retry.md` into a `KnowledgeUnit` struct — split the YAML frontmatter from the markdown body, deserialize both
- **Writing**: taking a `KnowledgeUnit` struct, serializing the frontmatter back to YAML, combining it with the body, and writing it to disk as a `.md` file
- **Moving**: when you accept an inbox item, moving the file from `brain/inbox/` to `brain/domains/coding/patterns/`
- **Listing**: walking a directory to find all `.md` files in `brain/inbox/` or `brain/domains/`

Both Nugget (knowledge files) and a future AI code reviewer (code files) need the same basic operations. The difference is what's _in_ those files, not how you read and write them.

### Layer 2: Review Engine (shared between products)

The review engine is the same for both products:

- **Proposal model**: directory-per-proposal with `proposal.md`, `comments.yaml`, `meta.yaml`
- **Comment threads**: author + timestamp + content, supports both human and AI authors
- **Review actions**: accept, reject, modify, comment, skip
- **Tree context view**: show the proposed change in the context of the full directory structure
- **MCP tools**: `get_review_feedback`, `respond_to_comment`, `update_proposal`

What differs between products:

|                      | Nugget                   | AI Code Reviewer      |
| -------------------- | ------------------------ | --------------------- |
| What's reviewed      | Markdown knowledge units | Code diffs/changesets |
| What "context" means | Brain directory tree     | Project file tree     |
| What "accept" does   | Move to domain folder    | Apply patch / commit  |

### Layer 3: Products

Each product is a thin layer on top of the review engine that knows about its specific content type and workflow.

### Build Sequence

**Don't build the shared engine first as an abstract framework.** Build Nugget all the way through. The review UX — tree view, comments, modify, AI feedback loop — gets built for knowledge review first. Then when the AI code reviewer is built, the shared pieces get extracted because you'll _know_ which parts are actually generic and which are knowledge-specific.

This is "extract, don't abstract" — you can't design the shared layer well until you've built at least one real product on it.

The one thing to keep in mind: when building the Phase 1.5 review UX for Nugget, keep the data model clean and generic where it's cheap to do so (directory-per-proposal, comment threads as a simple list, status enum). Don't hardcode knowledge-unit-specific assumptions into the proposal format. That way extraction later is a refactor, not a rewrite.
