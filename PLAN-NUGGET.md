# Nugget: Personal Knowledge Brain

## The Problem

Tacit knowledge requires synchronous experience to acquire. When a senior engineer helps you debug a race condition, you don't just learn the fix — you absorb how they think: what they check first, what they suspect, how they narrow down. That mental model transfer only happens in the moment, and it's lost the second the conversation ends.

Nugget makes tacit knowledge async. You record how you think. Others browse your brain, cherry-pick what's useful, and absorb your mental models on their own time. AI agents (Claude Code, etc.) query your brain for relevant context so they operate with _your_ knowledge, not generic training data.

---

## Files vs Knowledge Graph: Why Both (Sort Of)

**Files on disk are the source of truth.** Markdown files in a Git repo. Human-readable, versionable, portable, editable in any tool. No database to corrupt, no migration to run.

**But files alone can't answer "what does Sarah know about caching that I don't?"** For that you need relationships and traversal — graph operations.

The answer: **files encode the graph, indexes make it queryable.**

Each knowledge file contains typed relationships in its frontmatter:

```yaml
---
id: concept/cache-invalidation
type: concept
domain: coding
tags: [caching, distributed-systems, performance]
confidence: 0.9
source: experience
created: 2024-01-15
related:
  - id: pattern/ttl-strategy
    relation: implements
  - id: decision/2024-redis-choice
    relation: informed_by
  - id: concept/eventual-consistency
    relation: requires_understanding_of
---
# Cache Invalidation

When I debug cache issues, I check three things first...
```

The **frontmatter IS the graph**. Each file is a node. `related` entries are typed, directed edges. Tags are hyperedges grouping nodes. On startup, Nugget reads all files, builds an in-memory adjacency structure (SQLite + optional in-memory graph), and now you can traverse it: "give me everything 2 hops from `concept/cache-invalidation`" or "what concepts does Brain A have that Brain B doesn't?"

No Neo4j. No separate graph database. Just files with rich frontmatter and a derived index.

**Why not a pure graph database?**

- Can't Git-version it (no sharing via standard workflows)
- Not human-readable (can't open it in VS Code and browse)
- Creates a durability/sync problem (what if the DB and files disagree?)
- Overkill for the relationship density we actually need (most brains will have hundreds to low thousands of nodes, not millions)

**When would you upgrade to a graph DB?** If a brain exceeds ~50K nodes or if you need real-time graph algorithms (PageRank, community detection) at scale. That's a v3 problem.

---

## Architecture Overview

```
┌──────────────────────────────────────────────┐
│         Consumers                             │
│  ┌───────────┐ ┌──────────┐ ┌─────────────┐  │
│  │ Claude    │ │ PR       │ │ Any MCP     │  │
│  │ Code      │ │ Reviewer │ │ Client      │  │
│  └─────┬─────┘ └────┬─────┘ └──────┬──────┘  │
│        └─────────────┼──────────────┘         │
│              MCP Protocol                     │
└──────────────┼────────────────────────────────┘
               │
┌──────────────┼────────────────────────────────┐
│              ▼                                │
│  ┌──────────────────────────────────────┐     │
│  │       Nugget MCP Server (Rust)       │     │
│  │                                      │     │
│  │  ┌────────────┐  ┌───────────────┐   │     │
│  │  │ Retrieval  │  │ Capture       │   │     │
│  │  │ Engine     │  │ Engine        │   │     │
│  │  │            │  │               │   │     │
│  │  │ • Embed    │  │ • From URL    │   │     │
│  │  │ • Search   │  │ • From text   │   │     │
│  │  │ • Rank     │  │ • From convo  │   │     │
│  │  │ • Traverse │  │ • From commit │   │     │
│  │  └──────┬─────┘  └───────┬───────┘   │     │
│  │         │                │            │     │
│  │         │         ┌──────┴───────┐    │     │
│  │         │         │   Inbox      │    │     │
│  │         │         │ (all capture │    │     │
│  │         │         │  lands here) │    │     │
│  │         │         └──────┬───────┘    │     │
│  │         │                │            │     │
│  │  ┌──────┴────────────────┴───────┐    │     │
│  │  │         Index Layer           │    │     │
│  │  │  SQLite (metadata + graph)    │    │     │
│  │  │  Embeddings (similarity)      │    │     │
│  │  │  tantivy (full-text search)   │    │     │
│  │  └──────────────┬────────────────┘    │     │
│  │                 │                     │     │
│  │  ┌──────────────┴────────────────┐    │     │
│  │  │      Knowledge Store          │    │     │
│  │  │  Parse/write markdown+YAML    │    │     │
│  │  │  File watcher (notify)        │    │     │
│  │  └──────────────┬────────────────┘    │     │
│  │                 │                     │     │
│  └─────────────────┼─────────────────────┘     │
│                    │                           │
│   Git-versioned files on disk, organized by    │
│   domain (coding/, fashion/, personal/, ...)   │
└────────────────────────────────────────────────┘
```

**Primary interfaces**: MCP (for AI agents), CLI (for humans), your editor (for direct file editing).

No Tauri desktop app. No custom merge queue engine. The inbox IS the queue. Git IS the collaboration protocol.

---

## Knowledge Organization: Domains

A brain is one Git repo, organized into **domains** — top-level folders that act as namespaces. Think of them like repositories within a GitHub org, but inside one repo.

```
brain/
  brain.yaml                          # Brain metadata: owner, version
  domains/
    coding/
      domain.yaml                     # Domain config: description, default tags
      concepts/
        cache-invalidation.md
        goroutine-patterns.md
      decisions/
        2024-01-15-redis-for-sessions.md
      patterns/
        error-handling-go.md
        retry-with-idempotency.md
      bugs/
        stripe-client-retry.md
    fashion/
      domain.yaml
      concepts/
        capsule-wardrobe.md
        color-theory-for-skin-tone.md
      patterns/
        seasonal-rotation.md
    personal/                         # Private — never shared
      domain.yaml
      ...
  identity/
    voice.md                          # Writing style, tone
    beliefs.yaml                      # Core beliefs, values, principles
  config/
    tools.yaml                        # Tool preferences
    workflows.yaml                    # How I work
  inbox/                              # ALL captured knowledge lands here
    2024-01-15T10-30-cache-article.md
    2024-01-15T11-00-pairing-sarah.md
    2024-01-15T14-22-css-grid-fashion.md
    ...
  .nugget/                            # Gitignored — derived state
    index.db                          # SQLite: metadata + graph edges
    embeddings/                       # Vector embeddings for search
    search/                           # tantivy full-text index
```

### Why domains, not tags alone?

- **Sharing boundaries**: `nugget share coding` shares only the `coding/` domain. Your fashion knowledge stays private.
- **Scoped search**: `nugget search "caching" --domain coding` won't return fashion results. But `nugget search "caching"` still searches everything.
- **Scoped config**: Each domain can have its own `domain.yaml` with default tags, description, etc.
- **Cross-domain links still work**: A knowledge unit in `coding/` can link to one in `fashion/` via `related`. The graph doesn't care about folder boundaries.
- **Natural inbox routing**: When AI captures knowledge, it suggests a domain. When you review inbox items, you file them into the right domain.

### Domain examples

| Domain         | What goes here                                                                    |
| -------------- | --------------------------------------------------------------------------------- |
| `coding/`      | Programming concepts, architecture patterns, debugging techniques, tool knowledge |
| `coding/go/`   | Go-specific knowledge (sub-domains are just nested folders)                       |
| `coding/rust/` | Rust-specific knowledge                                                           |
| `fashion/`     | Style principles, brand preferences, seasonal strategies                          |
| `management/`  | Leadership patterns, team dynamics, hiring criteria                               |
| `personal/`    | Private knowledge, never shared                                                   |

You create domains as you need them. There's no fixed taxonomy.

---

## The Inbox: How Capture Volume Gets Managed

Every captured piece of knowledge — from a URL, a conversation, an observation, or a manual entry — goes to the **inbox**. Nothing auto-accepts. You review everything.

### What an inbox item looks like

```yaml
---
id: inbox/2024-01-15T10-30-cache-article
type: concept
suggested_domain: coding # AI's best guess
suggested_path: coding/concepts/ # Where it thinks this should go
tags: [caching, redis, distributed-systems]
confidence: 0.5
source: url
source_url: "https://blog.example.com/distributed-locking"
captured_at: 2024-01-15T10:30:00Z
capture_method: web-capture
---
# Distributed Locking Comparison

Redis Redlock vs Zookeeper vs etcd for distributed locking...

[AI-extracted summary of the article]
```

### The review workflow

```
$ nugget inbox
────────────────────────────────────────────────────────
  Inbox: 12 items (7 coding, 3 fashion, 2 unclassified)
────────────────────────────────────────────────────────

  1. [coding]  Distributed Locking Comparison        (from: URL, 10:30am)
  2. [coding]  Stripe Webhook Retry Patterns          (from: pairing w/ Sarah, 11:00am)
  3. [coding]  Subscription Nil Guard Pattern         (from: git observation, 11:45am)
  4. [fashion] Summer 2024 Linen Layering             (from: URL, 12:15pm)
  ...

  Commands:
    nugget review           # Review items one by one
    nugget accept 1 2 3     # Bulk accept (moves to suggested domain)
    nugget accept 1 --domain coding/go  # Accept to specific domain
    nugget reject 4         # Discard
    nugget defer 5 6        # Keep in inbox for later
    nugget edit 2           # Open in $EDITOR before accepting
```

**`nugget review`** walks you through each item:

```
$ nugget review

─── Item 1 of 12 ──────────────────────────────────────
  Distributed Locking Comparison
  Source: https://blog.example.com/distributed-locking
  Suggested domain: coding
  Tags: caching, redis, distributed-systems
  Confidence: 0.5

  [Shows condensed content...]

  [a]ccept  [e]dit  [r]eject  [d]efer  [m]ove domain  [s]kip
```

### Why everything-to-inbox works

- **You stay in control.** No AI-generated content pollutes your actual knowledge without your sign-off.
- **Review is fast.** Most items need a 2-second glance: accept or reject. The AI already did the extraction work.
- **Batch operations.** `nugget accept 1-7` to bulk-accept a batch. You don't have to review each individually if the summaries look right.
- **Inbox zero is achievable.** Unlike email, inbox items are small (a few paragraphs each). 15 items takes 5 minutes to triage.
- **No urgency.** The inbox can grow. Nothing breaks if you don't review for a week. The items are timestamped and searchable.

---

## How Intelligent Retrieval Works

This is the core technical challenge: when Claude Code is helping you with a Rust caching problem, how does it get _your_ knowledge about caching instead of _your_ knowledge about fashion?

### The Retrieval Pipeline (3 layers)

```
User's current task: "Help me implement a cache eviction policy for our Redis cluster"
                │
                ▼
┌─────────────────────────────┐
│  Layer 1: Embedding Search  │   "Find the ~50 knowledge units most
│  (fast, approximate)        │    semantically similar to this task"
│                             │
│  How: Embed the task desc,  │   Results: 50 candidates
│  cosine similarity against  │   (cache-invalidation, redis-config,
│  all unit embeddings        │    ttl-strategy, LRU-vs-LFU, ...)
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│  Layer 2: Graph Expansion   │   "For the top candidates, pull in
│  (enrich with neighbors)    │    their related knowledge"
│                             │
│  How: For top-20 results,   │   Results: 50 → ~30 unique units
│  walk 1-2 hops of related   │   (now includes prerequisite concepts,
│  edges. Deduplicate.        │    related decisions, linked patterns)
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│  Layer 3: Relevance Ranking │   "Of these ~30, which are actually
│  (precise, LLM-powered)     │    useful for THIS specific task?"
│                             │
│  How: Ask a fast LLM to     │   Results: Top 5-10, ranked
│  score each candidate for   │   with relevance explanations
│  relevance to the task.     │
│  Factor in: confidence,     │
│  recency, source quality.   │
└─────────────────────────────┘
```

**Layer 1 gets you in the neighborhood.** It knows "caching" is relevant but can't distinguish between your Redis knowledge and your browser caching notes.

**Layer 2 enriches with structure.** Your `concept/cache-invalidation` links to `decision/2024-redis-choice` and `pattern/ttl-strategy`. Those wouldn't have matched on embedding alone but are exactly what you need.

**Layer 3 filters the noise.** Of the 30 candidates, the LLM knows that your fashion knowledge about "capsule wardrobe caching" (yes, it might match on "caching" embeddings) is irrelevant.

### Domain as a retrieval signal

When you're working in a coding context (Claude Code, a coding project directory), the retrieval pipeline can automatically weight the `coding/` domain higher. It doesn't exclude other domains — cross-domain insights are valuable — but it knows your primary intent.

The MCP caller can specify a domain hint:

```
search_knowledge("cache eviction", { domain_hint: "coding" })
```

This boosts `coding/` results without filtering out others. A fashion article about "seasonal wardrobe rotation" won't surface. But a `management/` concept about "technical debt prioritization" might still surface if it's genuinely relevant to your caching decision.

### Ranking Signals

Each knowledge unit has intrinsic quality signals:

| Signal                 | Source                                                           | Weight |
| ---------------------- | ---------------------------------------------------------------- | ------ |
| **Confidence**         | Author self-assessment (0.0-1.0 in frontmatter)                  | High   |
| **Freshness**          | `modified` date vs now                                           | Medium |
| **Validation**         | Has it been confirmed/used successfully?                         | High   |
| **Source quality**     | `source: experience` > `source: article` > `source: speculation` | Medium |
| **Domain match**       | Does the unit's domain match the query context?                  | Medium |
| **Connection density** | More relationships = more central knowledge                      | Low    |
| **Access frequency**   | How often has retrieval returned this unit?                      | Low    |

---

## Agentic Workflows: How AI Adds to the Brain

The brain grows through multiple capture pathways. All captures land in the inbox.

### Workflow 1: Web Capture (Surfing → Brain)

The bread-and-butter workflow. You're reading something interesting online — paste the URL and a background agent does the rest.

```
$ nugget capture --url "https://blog.example.com/distributed-locking"

⟳ Fetching page...
⟳ Extracting knowledge...

  Added to inbox:
    "Distributed Locking Comparison" (concept, suggested: coding)
    Tags: distributed-systems, redis, zookeeper, etcd

  Run `nugget inbox` to review.
```

Or via MCP (so Claude Code or any agent can capture on your behalf):

```
capture_from_url({
  url: "https://blog.example.com/distributed-locking",
  context: "User was researching cache eviction strategies"
})
```

**What happens behind the scenes:**

1. Fetch the URL content (via reqwest, handle HTML → markdown conversion)
2. Send content + context to LLM with a structured extraction prompt
3. LLM returns: title, type (concept/pattern/decision), suggested domain, tags, confidence, condensed body, and suggested relationships to existing knowledge
4. Write the result to `inbox/` with all metadata
5. Index the inbox item (so it's immediately searchable even before review)

**Multiple URLs at once** (batch capture from a reading session):

```
$ nugget capture --urls urls.txt    # One URL per line
$ nugget capture --url "..." --url "..." --url "..."

⟳ Capturing 5 URLs in parallel...
  ✓ 5 items added to inbox
```

### Workflow 2: Post-Session Capture (After Pairing / Debugging)

```
$ nugget capture --from-text --source "pairing with Sarah" << 'EOF'
Sarah showed me that when you see flaky tests in the payment service,
it's almost always the Stripe webhook mock timing out. She checks the
mock server logs first, not the test output. She also mentioned that
the retry logic in stripe_client.rs has a subtle bug where...
EOF

  Added to inbox:
    1. "Flaky Payment Test Diagnosis" (pattern, suggested: coding)
    2. "Stripe Webhook Mock Behavior" (concept, suggested: coding)
    3. "Stripe Client Retry Bug" (bug, suggested: coding)

  Run `nugget inbox` to review.
```

### Workflow 3: Conversation Capture (Claude Code calls this)

After a Claude Code session, the agent can capture what was learned:

```
capture_from_conversation({
  summary: "Debugged race condition in order processing pipeline",
  learnings: [
    "Always check lock ordering when multiple goroutines access shared state",
    "The order_processor uses optimistic locking, not pessimistic"
  ],
  decisions: [
    "Chose optimistic locking over pessimistic because write contention is low"
  ]
})
```

Each learning and decision becomes a separate inbox item.

### Workflow 4: Continuous Observation (Background)

Nugget watches your Git commits in configured repos:

```yaml
# brain.yaml
observe:
  repos:
    - path: ~/workspace/my-project
    - path: ~/workspace/another-project
  schedule: daily # Analyze once a day, not on every commit
```

```
Nugget's observation agent notices:
  - 3rd nil-pointer fix in the subscription module this month
  - All three involve the same pattern: optional field access without guard

Creates inbox item:
  "Subscription Nil Guard Pattern" (pattern, suggested: coding)
  "The subscription module has multiple optional fields (plan, trial_end,
   payment_method) that are nil for free-tier users..."
```

### Workflow 5: Interview Mode (Extracting Your Own Tacit Knowledge)

```
$ nugget interview --topic "how I debug production issues"

Nugget asks you questions via CLI:
  Q: "When you get paged for a production issue, what's the first thing you do?"
  A: "I check the error rate dashboard, then look at recent deploys..."

  Q: "How do you decide whether to roll back vs. hotfix?"
  A: "If it's affecting >5% of requests, immediate rollback..."

  Added to inbox:
    1. "Production Incident Triage" (pattern, suggested: coding)
    2. "Rollback vs Hotfix Criteria" (decision, suggested: coding)
```

Interview captures go to inbox like everything else. But confidence is higher (0.9) since this is YOUR direct knowledge, not AI-extracted from a URL.

### Workflow 6: Cross-Brain Cherry-Pick

```
$ nugget remote add sarah git@github.com:sarah/brain.git

$ nugget diff sarah
  47 units in Sarah's brain not in yours:
    coding/    32 unique (you have 45 in this domain)
    devops/    15 unique (you have 0 — new domain!)

$ nugget diff sarah --domain coding
  Top suggestions (ranked by relevance to your existing knowledge):
    1. pattern/circuit-breaker-tuning     (confidence: 0.9)
    2. concept/stripe-idempotency-keys    (confidence: 0.8)
    3. decision/2024-grpc-over-rest       (confidence: 0.85)
    ...

$ nugget pull sarah concept/stripe-idempotency-keys
  Added to inbox: "Stripe Idempotency Keys" (from: sarah's brain)
```

Cross-brain pulls also go to inbox. When you accept, the file gets your own confidence score (0.5 by default — you haven't validated it yourself yet) and retains attribution to the original author.

---

## The MCP Interface (How Claude Code Connects)

Nugget's primary interface is an **MCP server**. This is how any AI agent gets your knowledge.

### MCP Tools Exposed

```
search_knowledge(query: string, options?: {
  max_results?: number,       // default 10
  min_confidence?: number,    // default 0.0
  domain?: string,            // filter to a domain ("coding", "coding/go")
  domain_hint?: string,       // boost a domain without excluding others
  tags?: string[],            // filter by tags
  types?: string[],           // "concept" | "pattern" | "decision" | "belief"
  brain?: string,             // search a specific remote brain
  include_graph?: boolean,    // expand with related nodes (Layer 2)
  rerank?: boolean,           // use LLM reranking (Layer 3)
})
→ Returns: ranked list of knowledge units with relevance scores

get_knowledge(id: string)
→ Returns: full knowledge unit (frontmatter + body)

capture_from_url(url: string, context?: string)
→ Fetches URL, extracts knowledge, adds to inbox

capture_from_text(text: string, source?: string)
→ LLM extracts knowledge units from raw text, adds to inbox

capture_from_conversation(summary: string, learnings: string[], decisions?: string[])
→ Extracts knowledge units from a session summary, adds to inbox

list_knowledge(filter?: {
  domain?: string,
  tags?: string[],
  type?: string,
  since?: string
})
→ Browse the brain with filters

inbox_status()
→ Returns: count of inbox items, breakdown by suggested domain

diff_brains(remote: string, domain?: string)
→ Returns units only in remote, only in local, and conflicts
```

### Example: Claude Code Using Nugget

Your Claude Code settings (`.claude/settings.json`):

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

Now when you ask Claude Code: _"Help me implement retry logic for our Stripe integration"_

Claude Code internally:

1. Calls `search_knowledge("retry logic Stripe integration", { domain_hint: "coding", rerank: true })`
2. Gets back your `pattern/retry-with-idempotency` and `decision/2024-exponential-backoff-config`
3. Uses YOUR documented patterns and decisions to write code that matches YOUR conventions

After the session, Claude Code can optionally call:

```
capture_from_conversation({
  summary: "Implemented retry logic for Stripe with idempotency keys",
  learnings: ["The Stripe client already has built-in retry, but without idempotency keys"],
  decisions: ["Used exponential backoff with jitter, max 3 retries"]
})
```

Those learnings land in your inbox for review.

---

## Knowledge File Format

Every knowledge file follows this format:

```yaml
---
id: pattern/retry-with-idempotency
type: pattern # concept | pattern | decision | bug | belief
domain: coding # which domain this belongs to
tags: [stripe, payments, reliability, retry]
confidence: 0.9 # 0.0-1.0, how sure you are
source: experience # experience | article | ai-extracted | pairing | observation
source_detail: "Built this for the payment service rewrite"
created: 2024-01-15
modified: 2024-03-20
related:
  - id: concept/idempotency-keys
    relation: uses
  - id: decision/2024-exponential-backoff-config
    relation: implements
  - id: concept/circuit-breaker
    relation: often_combined_with
---
# Retry with Idempotency Keys

When retrying Stripe API calls, ALWAYS use idempotency keys...
```

### Inbox item format

Same as above, plus inbox-specific fields:

```yaml
---
id: inbox/2024-01-15T10-30-cache-article
type: concept
suggested_domain: coding
suggested_path: coding/concepts/distributed-locking-comparison
tags: [caching, redis, distributed-systems]
confidence: 0.5
source: url
source_url: "https://blog.example.com/distributed-locking"
captured_at: 2024-01-15T10:30:00Z
capture_method: web-capture
capture_context: "User was researching cache eviction strategies"
---
```

When you accept an inbox item, the `inbox/`-specific fields are stripped and the file moves to its domain path.

---

## Tech Stack

| Component              | Choice                                                       | Why                                                       |
| ---------------------- | ------------------------------------------------------------ | --------------------------------------------------------- |
| MCP server             | **Rust** (via `rmcp` or custom)                              | Performance, single binary, same ecosystem as PR reviewer |
| CLI                    | **Rust** (clap)                                              | Same binary as MCP server, just different entry point     |
| Markdown parsing       | **pulldown-cmark**                                           | Rust-native CommonMark                                    |
| YAML parsing           | **serde_yaml**                                               | Standard Rust YAML                                        |
| Metadata + graph       | **SQLite** (rusqlite)                                        | Stores unit metadata, tag associations, graph edges       |
| Full-text search       | **tantivy**                                                  | Rust-native, fast rebuild from files                      |
| Embeddings             | **Local model** (fastembed-rs) or **API** (OpenAI/Anthropic) | fastembed for offline, API for quality                    |
| LLM (capture + rerank) | **Claude API** via reqwest                                   | For knowledge extraction and relevance reranking          |
| Web fetching           | **reqwest** + **readability** (html→text)                    | For URL capture workflow                                  |
| File watching          | **notify**                                                   | Cross-platform, triggers re-index                         |
| Git operations         | **git2**                                                     | For cross-brain operations (remote add, fetch, diff)      |
| Async runtime          | **tokio**                                                    | Standard Rust async                                       |

---

## Workspace Layout

```
nugget/
  Cargo.toml                    # Workspace root
  crates/
    nugget-core/                # Core types: KnowledgeUnit, KnowledgeId, Domain, etc.
    nugget-store/               # File parser/writer, brain directory ops, file watcher
    nugget-index/               # SQLite + tantivy + embeddings, graph traversal
    nugget-retrieve/            # The 3-layer retrieval pipeline
    nugget-capture/             # Capture engine: from URL, text, conversation, observation
    nugget-inbox/               # Inbox management: list, accept, reject, defer, move
    nugget-vcs/                 # git2 wrapper for cross-brain operations
    nugget-mcp/                 # MCP server implementation
    nugget-cli/                 # CLI entry point
```

---

## Implementation Phases

### Phase 1 — Core + Store + CLI (Week 1-2)

**Goal**: Can create a brain with domains, add knowledge files, and read them back.

- `nugget-core`: types (`KnowledgeUnit`, `KnowledgeId`, `KnowledgeType`, `Domain`, `Frontmatter`, `Relation`, `Tag`, `Confidence`)
- `nugget-store`: markdown+frontmatter parser/writer, brain directory operations (init, list, read, write), domain management
- `nugget-cli`: basic commands
  - `nugget init` — create a new brain directory with default structure
  - `nugget domain add coding` — create a new domain
  - `nugget add --domain coding concept "cache invalidation"` — interactive knowledge entry
  - `nugget list` / `nugget list --domain coding` — list knowledge units
  - `nugget show <id>` — display a unit

**Deliverable**: `nugget init && nugget domain add coding && nugget add --domain coding concept "my first concept"` creates a properly formatted markdown file at `brain/domains/coding/concepts/my-first-concept.md`.

### Phase 2 — Index + Search (Week 3-4)

**Goal**: Can search across knowledge by text, tags, domain, and graph traversal.

- `nugget-index`:
  - SQLite schema: units table (with domain column), tags table, relations table (from/to/type)
  - tantivy full-text index with stemming
  - Embedding index (fastembed-rs for local, API option for quality)
  - Full rebuild from files on startup, incremental updates via file watcher
- `nugget-retrieve`: the 3-layer retrieval pipeline (embed -> graph expand -> rerank)
- `nugget-cli`:
  - `nugget search "cache invalidation"` — searches all domains
  - `nugget search "cache invalidation" --domain coding` — scoped search
  - `nugget related <id>` — show graph neighbors
  - `nugget tags` / `nugget tags --domain coding` — list tags

**Deliverable**: A brain with 50+ knowledge files across 2-3 domains. `nugget search "how to handle retries"` returns relevant units from the right domain, ranked by quality.

### Phase 3 — MCP Server (Week 5-6)

**Goal**: Claude Code (and any MCP client) can query the brain.

- `nugget-mcp`: MCP server exposing `search_knowledge`, `get_knowledge`, `list_knowledge`
- Wire MCP tools to the retrieval pipeline and store
- Domain hint support: MCP clients can specify which domain is most relevant
- Test with Claude Code: add Nugget MCP to a project, verify context retrieval works

**Deliverable**: Add Nugget MCP to your Claude Code config. Ask Claude Code a domain question. It searches your brain, retrieves relevant knowledge, and uses it in its response.

### Phase 4 — Capture + Inbox (Week 7-9)

**Goal**: AI can capture knowledge from URLs, text, and conversations. Everything goes to inbox. User can review.

- `nugget-capture`:
  - `capture_from_url(url)` — fetch page, HTML→markdown, LLM extraction, write to inbox
  - `capture_from_text(text, source)` — LLM extracts knowledge units, writes to inbox
  - `capture_from_conversation(summary, learnings)` — post-session extraction to inbox
  - Domain suggestion: LLM suggests which domain each captured item belongs to
  - Relationship detection: LLM identifies links to existing knowledge in the brain
- `nugget-inbox`: inbox CRUD — list, accept (with domain routing), reject, defer, edit, batch operations
- MCP tools: `capture_from_url`, `capture_from_text`, `capture_from_conversation`, `inbox_status`
- `nugget-cli`:
  - `nugget capture --url "https://..."` — web capture
  - `nugget capture --from-text < notes.md` — text capture
  - `nugget inbox` — list inbox items
  - `nugget review` — interactive review workflow
  - `nugget accept 1 2 3` / `nugget reject 4` — batch operations

**Deliverable**: Paste a URL, get structured knowledge in your inbox. Review and accept into the right domain. Claude Code can capture learnings after a session.

### Phase 5 — Cross-Brain Operations (Week 10-11)

**Goal**: Browse other people's brains, cherry-pick knowledge.

- `nugget-vcs`: git2 wrapper (remote add, fetch, tree walking at a ref)
- Cross-brain diff: walk two brains' domain trees, compare by id, classify as unique/shared/conflicting
- Domain-scoped diffing: `nugget diff sarah --domain coding`
- `nugget-cli`:
  - `nugget remote add sarah git@github.com:sarah/brain.git`
  - `nugget diff sarah` — show what's different, grouped by domain
  - `nugget pull sarah concept/stripe-idempotency-keys` — cherry-pick to inbox
- MCP tool: `diff_brains`
- Sharing: `nugget share coding` — makes only `coding/` domain visible when others pull your brain

**Deliverable**: Add a colleague's brain as a remote, see their knowledge by domain, pull interesting units into your inbox.

### Phase 6 — Continuous Observation (Week 12-13)

**Goal**: Nugget watches your work and suggests knowledge to capture.

- File watcher mode: watch configured Git repos for commits
- Pattern detection: LLM analyzes recent commits, identifies recurring themes
- Auto-inbox: create inbox items from observed patterns
- `nugget-cli`:
  - `nugget observe add ~/workspace/my-project` — watch a repo
  - Inbox items show "from: git observation" as their source
  - CLI shows pending inbox count on startup

**Deliverable**: After a week of commits, your inbox has AI-suggested knowledge units like "You've fixed 3 nil-pointer bugs in the subscription module — here's the pattern."

---

## Verification Plan

1. **Unit tests**: Each crate. `insta` for snapshot testing parsed frontmatter, search results, inbox items.
2. **Integration test**: Init brain -> add knowledge across domains -> search -> verify domain scoping works -> capture from URL -> verify inbox item created -> accept -> verify file moves to correct domain.
3. **MCP test**: Start MCP server -> Claude Code calls `search_knowledge` with domain hint -> verify relevant results returned and wrong-domain results suppressed.
4. **Cross-brain test**: Two brain directories as Git repos -> add remote -> diff by domain -> pull -> verify item appears in inbox.
5. **Retrieval quality test**: Build a brain with `coding/` and `fashion/` domains. Search for a coding topic. Verify fashion knowledge doesn't leak into top results. Search without domain filter. Verify cross-domain results ranked appropriately.
6. **Inbox volume test**: Capture 50 URLs in batch. Verify inbox lists them all. Verify `nugget accept 1-50` handles batch correctly. Verify all items indexed after accept.

---

## Open Questions

- **Embedding model**: Local (fastembed, ~384 dim) vs API (OpenAI text-embedding-3-small, ~1536 dim)? Local is offline-friendly but lower quality. Could default to local with API upgrade option.
- **Brain size limits**: At what point does the 3-layer retrieval become slow? Probably fine up to ~10K units. Profile and optimize if needed.
- **Privacy in cross-brain**: Per-domain sharing is the default. Should we also support per-file privacy flags?
- **Inbox retention**: How long do rejected items stay (for "I changed my mind")? 30 days then purge?
- **Sub-domains**: Is `coding/go/` a sub-domain or just a subfolder within `coding/`? Leaning toward: it's just a folder, domains are only the top level. Sub-organization within a domain is free-form.
- **Duplicate detection**: When capturing from a URL you've already captured, should Nugget warn? Merge? Update the existing unit?
