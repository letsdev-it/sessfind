---
name: sessfind
description: >-
  Search, browse, and resume past AI coding sessions across GitHub Copilot, Claude Code, and OpenCode
  using the sessfind CLI. Use this skill whenever the user wants to find a previous conversation,
  search through session history, resume or continue an old session, check what was discussed before,
  look up past AI interactions, see session statistics, or re-index their sessions. Also use it when
  the user asks things like "have I worked on this before?", "find that conversation about X",
  "what did I discuss last week?", or "open my last Claude session about Y". Even if the user doesn't
  mention sessfind by name, use this skill whenever they want to search or navigate their AI session history.
---

# sessfind — AI Session Search & Resume

`sessfind` is a CLI tool that indexes and searches AI assistant sessions from **GitHub Copilot**, **Claude Code**, and **OpenCode** in one place. It lets users find past conversations by keyword and resume them directly.

## Prerequisites

`sessfind` must be installed and available in PATH. If it's not installed, tell the user:

```bash
cargo install sessfind
```

Requires Rust 1.85+. The user needs to have had at least one session with a supported AI tool for there to be anything to search.

## Core Workflow

The typical flow is: **index → search → show → resume**. The index step only needs to happen once (or when the user wants to pick up new sessions).

### 1. Index Sessions

Before searching, sessions need to be indexed. This scans all supported sources and builds a full-text search index.

```bash
sessfind index
```

This is incremental — it only processes new or changed sessions. It's fast and safe to run repeatedly. If the user says their recent sessions aren't showing up, suggest re-indexing.

### 2. Search Sessions

Search across all indexed sessions using natural language queries:

```bash
sessfind search "<query>"
```

The search uses BM25 ranking (like a search engine) to find the most relevant sessions.

**Available filters** — combine as needed:

| Flag | Purpose | Example |
|------|---------|---------|
| `--source` / `-s` | Filter by AI tool | `--source claude`, `--source copilot`, `--source opencode` |
| `--project` / `-p` | Filter by project name (substring match) | `--project my-app` |
| `--after` | Only results after date | `--after 2025-01-01` |
| `--before` | Only results before date | `--before 2025-06-01` |
| `--limit` / `-n` | Max results (default: 10) | `-n 5` |

**Example 1** — Find sessions about authentication in Claude:
```bash
sessfind search "authentication JWT" --source claude
```

**Example 2** — Find recent sessions about a specific project:
```bash
sessfind search "database migration" --project my-api --after 2025-03-01
```

**Example 3** — Broad search across all sources:
```bash
sessfind search "refactoring strategy"
```

The output shows a table with score, source, project, date, and a preview snippet. At the bottom, session IDs are listed for use with `sessfind show`.

### 3. Show Full Session

To read the full content of a session found via search:

```bash
sessfind show <SESSION_ID>
```

Use the session ID from the search results output. This displays the entire conversation — useful when the user wants to review what was discussed or find specific details from a past session.

### 4. Resume a Session

To resume (continue) a session in the original AI tool, use the appropriate resume command based on the **source** shown in the search results:

| Source | Resume Command |
|--------|---------------|
| **copilot** | `copilot --resume=<SESSION_ID>` |
| **claude** | `claude --resume <SESSION_ID>` |
| **opencode** | `opencode --session <SESSION_ID>` |

Note the syntax differences: Copilot uses `=`, Claude uses a space, OpenCode uses `--session`.

When the user asks to resume a session, identify the source from the search results and run the correct command. If you're already running inside one of these tools, warn the user that resuming will start a new process — they may want to copy the command and run it manually after exiting.

### 5. Check Index Statistics

To see how many sessions are indexed and from which sources:

```bash
sessfind stats
```

This is useful for diagnosing issues ("why can't I find my session?") or just getting an overview of the user's session history.

## Tips for Effective Searching

- **Be specific**: "JWT authentication middleware" works better than just "auth"
- **Use filters to narrow down**: if the user remembers roughly when or which tool, add `--source` or `--after`/`--before`
- **Try different terms**: if a search returns nothing useful, try synonyms or related terms — the conversation might have used different wording
- **Increase the limit**: if the top 10 results don't have what the user wants, try `-n 20` or higher
- **Re-index if needed**: if recent sessions are missing, run `sessfind index` first

## Combining Steps

For common workflows, chain commands for efficiency:

```bash
# Index and then search in one go
sessfind index && sessfind search "my query"
```

When the user asks to "find that conversation about X", the natural flow is:
1. Run `sessfind search "X"` — see if results look right
2. If the user wants details, run `sessfind show <id>` on the most promising result
3. If the user wants to continue that conversation, resume it with the appropriate tool command
