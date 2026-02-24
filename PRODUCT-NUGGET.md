# Nugget

**A personal knowledge brain that AI agents can read and write to.**

---

## What is it?

Nugget is where you store how you think. It's a collection of markdown files — concepts you understand, patterns you use, decisions you've made, bugs you've fixed — organized by domain, versioned with Git, and queryable by AI.

When you connect Nugget to Claude Code (or any AI tool), the AI stops giving you generic answers and starts giving you answers informed by _your_ knowledge, _your_ team's conventions, and _your_ past decisions.

When you connect to a colleague's brain, you can browse what they know and cherry-pick the knowledge that's useful to you — without needing to schedule a meeting.

---

## Who is it for?

- **Individual developers** who want their AI tools to know their preferences, patterns, and past decisions
- **Engineers learning from senior peers** who want async access to how smart people think
- **Teams** who want to share institutional knowledge without it living in someone's head
- **Anyone** who accumulates knowledge across domains (coding, fashion, management, hobbies) and wants it organized and searchable

---

## The Core Problem

Tacit knowledge requires synchronous experience to acquire. When a senior engineer helps you debug a race condition, you absorb how they think: what they check first, what they suspect, how they narrow down. That mental model transfer only happens in the moment, and it's lost the second the conversation ends.

Nugget makes that transfer async.

---

## How It Works

### Your brain is a folder of files

```
brain/
  domains/
    coding/
      concepts/cache-invalidation.md
      patterns/retry-with-idempotency.md
      decisions/2024-01-15-redis-for-sessions.md
    fashion/
      concepts/capsule-wardrobe.md
      patterns/seasonal-rotation.md
    management/
      patterns/one-on-one-framework.md
  identity/
    voice.md          # How you communicate
    beliefs.yaml      # What you believe
  inbox/              # New knowledge waiting for your review
```

Each file is a knowledge unit with structured metadata:

```markdown
---
type: pattern
domain: coding
tags: [stripe, payments, reliability]
confidence: 0.9
source: experience
related:
  - id: concept/idempotency-keys
    relation: uses
---

# Retry with Idempotency Keys

When retrying Stripe API calls, ALWAYS use idempotency keys...
```

Files encode a knowledge graph through their relationships. Nugget indexes this into a searchable, traversable structure. But the files are always the source of truth — human-readable, Git-versioned, editable in any tool.

### Domains keep things organized

Domains are top-level folders that act as namespaces. Think of them like separate notebooks.

- **Scoped search**: Search just your coding knowledge, or across everything
- **Sharing boundaries**: Share your coding brain without exposing your fashion knowledge
- **Natural routing**: When AI captures knowledge, it suggests which domain it belongs to
- **Cross-domain links**: A concept in coding/ can link to one in management/. The graph doesn't care about folder boundaries.

You create domains as you need them. There's no fixed taxonomy.

### Everything flows through the inbox

Every piece of captured knowledge — from a URL, a conversation, an observation, or a manual entry — lands in the **inbox**. Nothing auto-accepts. You review everything.

```
$ nugget inbox
  Inbox: 12 items (7 coding, 3 fashion, 2 unclassified)

  1. [coding]  Distributed Locking Comparison    (from: URL, 10:30am)
  2. [coding]  Stripe Webhook Retry Patterns      (from: pairing w/ Sarah, 11:00am)
  3. [fashion] Summer 2024 Linen Layering         (from: URL, 12:15pm)
```

Review is fast. Most items need a 2-second glance: accept or reject. Bulk-accept a batch with `nugget accept 1-7`. The AI already did the extraction work — you're just triaging.

---

## Six Ways Knowledge Gets Into Your Brain

### 1. Web capture

You're browsing the web and find something worth remembering.

```
$ nugget capture --url "https://blog.example.com/distributed-locking"

  Added to inbox: "Distributed Locking Comparison" (coding)
```

Background agent fetches the page, extracts the key knowledge, suggests a domain and tags. You review it later.

### 2. Post-session capture

After a pairing session or debugging call, paste your notes.

```
$ nugget capture --from-text --source "pairing with Sarah" < notes.md

  Added to inbox:
    1. "Flaky Payment Test Diagnosis" (pattern)
    2. "Stripe Webhook Mock Behavior" (concept)
    3. "Stripe Client Retry Bug" (bug)
```

AI breaks your raw notes into atomic knowledge units, each with suggested domain, tags, and relationships to your existing knowledge.

### 3. AI conversation capture

After a Claude Code session, the AI captures what was learned and sends it to your inbox.

### 4. Continuous observation

Nugget watches your Git commits. When it notices patterns (e.g., "you've fixed the same kind of bug 3 times this month"), it suggests a knowledge unit.

### 5. Interview mode

Nugget asks you questions to extract your tacit knowledge.

```
$ nugget interview --topic "how I debug production issues"

  Q: "When you get paged, what's the first thing you check?"
  A: "Error rate dashboard, then recent deploys..."

  Added to inbox: "Production Incident Triage" (pattern)
```

### 6. Cross-brain cherry-pick

Add a colleague's brain as a Git remote. Browse what they know. Pull what's useful.

```
$ nugget remote add sarah git@github.com:sarah/brain.git
$ nugget diff sarah --domain coding

  32 units in Sarah's coding brain that aren't in yours:
    1. pattern/circuit-breaker-tuning
    2. concept/stripe-idempotency-keys
    ...

$ nugget pull sarah concept/stripe-idempotency-keys
  Added to inbox.
```

---

## How AI Gets the Right Knowledge

When Claude Code is helping you with a Rust caching problem, you don't want it pulling your fashion knowledge. Nugget uses a three-layer retrieval pipeline:

**Layer 1 — Similarity search.** Find the ~50 knowledge units most semantically similar to the current task. Fast but imprecise.

**Layer 2 — Graph expansion.** For the top results, walk their relationship links. Your `cache-invalidation` concept links to `ttl-strategy` pattern and `redis-choice` decision. Those wouldn't have matched on text alone but are exactly what you need.

**Layer 3 — Relevance ranking.** A fast LLM scores the remaining candidates for actual relevance to the specific task. Filters out false positives. Factors in confidence, freshness, and source quality.

The result: the 5-10 most relevant pieces of YOUR knowledge, ranked from most to least useful.

Domain context helps too. When you're working in a coding context, coding knowledge gets boosted — but not exclusively. A management insight about "technical debt prioritization" might still surface if it's genuinely relevant.

---

## How You Interact With It

**CLI** — for managing your brain directly.

- `nugget init` / `nugget domain add coding`
- `nugget capture --url "..."` / `nugget capture --from-text`
- `nugget inbox` / `nugget review` / `nugget accept 1 2 3`
- `nugget search "cache invalidation" --domain coding`
- `nugget remote add sarah ...` / `nugget diff sarah` / `nugget pull sarah ...`

**MCP server** — for AI agents.

- Claude Code, PR Reviewer, or any MCP-compatible tool can search your brain and capture new knowledge
- Connect it once, and your AI tools automatically have access to your knowledge

**Your editor** — for direct editing.

- Knowledge files are just markdown. Open them in VS Code, Obsidian, or any text editor.

**Git** — for sharing and collaboration.

- Push your brain to GitHub. Others pull it. Standard workflows.

---

## What Makes It Different

| Other tools                                           | Nugget                                                        |
| ----------------------------------------------------- | ------------------------------------------------------------- |
| Notion/Obsidian are for humans to read                | Nugget is for both humans AND AI agents to read               |
| Note-taking apps require you to organize as you write | AI extracts and organizes for you; you just approve           |
| Knowledge bases are write-once, read-never            | Nugget's retrieval pipeline makes knowledge actually get used |
| Tribal knowledge stays in people's heads              | Nugget makes it browsable, shareable, and cherry-pickable     |
| AI tools use generic training data                    | AI tools connected to Nugget use YOUR knowledge               |

---

## The Flywheel

```
You work → AI observes → knowledge captured to inbox
    ↓
You review inbox → knowledge enters your brain
    ↓
AI uses your knowledge → gives better answers
    ↓
You work better → AI observes more → ...
```

The more you use Nugget, the smarter your AI tools become. The smarter they become, the more useful knowledge they capture. The knowledge compounds.
