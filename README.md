![Rust](https://img.shields.io/badge/rust-stable-orange?logo=rust)
![License](https://img.shields.io/badge/license-MIT-blue)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey)

# sessfind

**CLI tool to search and resume AI sessions across GitHub Copilot, Claude Code, OpenCode, Cursor, and Codex.**

*GitHub Copilot · Claude Code · OpenCode · Cursor · Codex*

[letsdev.it](https://letsdev.it)

![sessfind interactive TUI — split-pane search and session preview](tui.png)

📖 **[Full Documentation](https://letsdev-it.github.io/sessfind/)**

---

`sessfind` indexes and searches your AI assistant sessions from **GitHub Copilot**, **Claude Code**, **OpenCode**, **Cursor**, and **Codex** in one place, and lets you **resume** a session from the UI or CLI. Ever had a conversation about a topic days ago and could not find it? `sessfind` is for that.

## Features

- Full-text search (BM25 ranking via tantivy) across all your sessions
- Interactive TUI with split-pane layout, real-time filtering, and session preview
- Fuzzy substring matching as alternative search mode
- **Semantic search** — find conceptually similar sessions using ML embeddings (optional plugin)
- **LLM search** — agentic search using installed AI CLI tools (Claude Code, OpenCode, Copilot)
- Resume any session directly from the search results
- Incremental indexing — only processes new/changed sessions
- **Automatic indexing** — background watcher re-indexes on session changes ([details](docs/usage/automatic-indexing.md))
- **Agent skill** — use sessfind directly from GitHub Copilot CLI, Claude Code, or OpenCode ([details](docs/usage/agent-skill.md))
- Zero external runtime dependencies — single static binary

## Supported Sources

| Source | Session Location | Resume Command |
|--------|-----------------|----------------|
| **GitHub Copilot** | `~/.copilot/session-state/*/events.jsonl` | `copilot --resume=SESSION_ID` |
| **Claude Code** | `~/.claude/projects/*/` | `claude --resume SESSION_ID` |
| **OpenCode** | `~/.local/share/opencode/opencode.db` | `opencode --session SESSION_ID` |
| **Cursor** | `~/.cursor/projects/*/agent-transcripts/` | `cursor PROJECT_PATH` |
| **Codex** | `~/.codex/sessions/YYYY/MM/DD/*.jsonl` | `codex resume SESSION_ID` |

## Quick Install

```bash
cargo install sessfind
```

Requires Rust **1.88+**. See [Installation docs](https://letsdev-it.github.io/sessfind/getting-started/installation/) for prebuilt binaries and other options.

## Quick Start

```bash
# 1. Index your sessions
sessfind index

# 2. Launch the interactive TUI
sessfind
```

Combine both: `sessfind --index`. See [Quick Start docs](https://letsdev-it.github.io/sessfind/getting-started/quick-start/) for more.

## Documentation

- [Installation](https://letsdev-it.github.io/sessfind/getting-started/installation/)
- [Interactive TUI & Keybindings](https://letsdev-it.github.io/sessfind/usage/tui/)
- [CLI Commands](https://letsdev-it.github.io/sessfind/usage/cli/)
- [Search Modes](https://letsdev-it.github.io/sessfind/usage/search-modes/) (FTS, Fuzzy, LLM, Semantic)
- [LLM Configuration](https://letsdev-it.github.io/sessfind/usage/llm-configuration/)
- [Automatic Indexing](docs/usage/automatic-indexing.md) (`sessfind watch`, shell hooks, cron)
- [Agent Skill](https://letsdev-it.github.io/sessfind/usage/agent-skill/) (use sessfind from Copilot CLI / Claude Code / OpenCode)
- [Semantic Search Plugin](https://letsdev-it.github.io/sessfind/plugins/semantic-search/)
- [Architecture & How It Works](https://letsdev-it.github.io/sessfind/architecture/how-it-works/)
- [Contributing](https://letsdev-it.github.io/sessfind/contributing/)

## License

[MIT](LICENSE) © [Let's Dev .IT](https://letsdev.it)
