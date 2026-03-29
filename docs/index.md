![Rust](https://img.shields.io/badge/rust-stable-orange?logo=rust)
![License](https://img.shields.io/badge/license-MIT-blue)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey)

# sessfind

**CLI tool to search and resume AI sessions across GitHub Copilot, Claude Code, OpenCode, Cursor, and Codex.**

*GitHub Copilot · Claude Code · OpenCode · Cursor · Codex*

[letsdev.it](https://letsdev.it)

![sessfind interactive TUI — split-pane search and session preview](https://raw.githubusercontent.com/letsdev-it/sessfind/main/tui.png)

---

## What is sessfind?

`sessfind` indexes and searches your AI assistant sessions from **GitHub Copilot**, **Claude Code**, **OpenCode**, **Cursor**, and **Codex** in one place, and lets you **resume** a session from the UI or CLI. Ever had a conversation about a topic days ago and could not find it? `sessfind` is for that.

## Features

- Full-text search (BM25 ranking via tantivy) across all your sessions
- Interactive TUI with split-pane layout, real-time filtering, and session preview
- Fuzzy substring matching as alternative search mode
- **Semantic search** — find conceptually similar sessions using ML embeddings (optional plugin)
- **LLM search** — agentic search using installed AI CLI tools (Claude Code, OpenCode, Copilot)
- Resume any session directly from the search results
- Incremental indexing — only processes new/changed sessions
- Zero external runtime dependencies — single static binary

## Supported Sources

| Source | Session Location | Resume Command |
|--------|-----------------|----------------|
| **GitHub Copilot** | `~/.copilot/session-state/*/events.jsonl` | `copilot --resume=SESSION_ID` |
| **Claude Code** | `~/.claude/projects/*/` | `claude --resume SESSION_ID` |
| **OpenCode** | `~/.local/share/opencode/opencode.db` | `opencode --session SESSION_ID` |
| **Cursor** | `~/.cursor/projects/*/agent-transcripts/` | `cursor PROJECT_PATH` |
| **Codex** | `~/.codex/sessions/YYYY/MM/DD/*.jsonl` | `codex resume SESSION_ID` |

## Quick Links

<div class="grid cards" markdown>

- :material-download: **[Installation](getting-started/installation.md)**

    Install from crates.io, GitHub Releases, or build from source.

- :material-rocket-launch: **[Quick Start](getting-started/quick-start.md)**

    Index your sessions and launch the TUI in two commands.

- :material-monitor: **[Interactive TUI](usage/tui.md)**

    Keybindings, pane layout, and TUI usage guide.

- :material-console: **[CLI Commands](usage/cli.md)**

    Full reference for all CLI commands and flags.

</div>
