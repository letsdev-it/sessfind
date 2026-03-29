![Rust](https://img.shields.io/badge/rust-stable-orange?logo=rust)
![License](https://img.shields.io/badge/license-MIT-blue)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey)

# sessfind

**Search and resume past AI coding assistant sessions**

*Claude Code · OpenCode · GitHub Copilot*

[letsdev.it](https://letsdev.it)

![sessfind interactive TUI — split-pane search and session preview](tui.png)

---

## What is sessfind?

`sessfind` is a single-binary CLI tool that indexes and searches your AI coding assistant sessions across multiple tools. Ever had a conversation about a specific topic days ago and can't find it? sessfind solves that.

**Features:**

- Full-text search (BM25 ranking via tantivy) across all your sessions
- Interactive TUI with split-pane layout, real-time filtering, and session preview
- Fuzzy substring matching as alternative search mode
- Resume any session directly from the search results
- Incremental indexing — only processes new/changed sessions
- Zero external runtime dependencies — single static binary

## Supported Sources

| Source | Session Location | Resume Command |
|--------|-----------------|----------------|
| **Claude Code** | `~/.claude/projects/*/` | `claude --resume SESSION_ID` |
| **OpenCode** | `~/.local/share/opencode/opencode.db` | `opencode --session SESSION_ID` |
| **GitHub Copilot** | `~/.copilot/session-state/*/events.jsonl` | `copilot --resume=SESSION_ID` |

## Installation

### From source (recommended)

```bash
# Requires Rust 1.85+ (edition 2024)
git clone https://github.com/letsdev-it/sessfind.git
cd sessfind
cargo install --path .
```

### Build manually

```bash
cargo build --release
# Binary at target/release/sessfind
cp target/release/sessfind ~/.local/bin/
```

## Quick Start

```bash
# 1. Index all your sessions
sessfind index

# 2. Launch the interactive TUI
sessfind
```

That's it. Start typing to search.

## Usage

### Interactive TUI (default)

```bash
sessfind
```

Opens a full-screen terminal UI with:

- **Left pane** — search results list (source, project, date)
- **Right pane** — session details and conversation preview
- **Bottom** — search input with mode indicator

#### Keybindings

| Key | Action |
|-----|--------|
| *Type* | Filter sessions in real-time |
| `Tab` | Switch focus between search and results |
| `Shift+Tab` | Toggle search mode (FTS / Fuzzy) |
| `Up/Down`, `j/k` | Navigate results |
| `Enter` | Resume selected session |
| `PgUp/PgDn` | Scroll session preview |
| `Ctrl+U` | Clear search input |
| `?` | Show help popup |
| `Esc` | Quit |

#### Search Modes

**FTS (Full-Text Search)** — default, powered by tantivy BM25:

```
kalkulator              single keyword
kalkulator b2b          any of these words (OR)
+kalkulator +b2b        all words required (AND)
"kalkulator b2b"        exact phrase
kalkulat*               prefix wildcard
```

**Fuzzy** — case-insensitive substring match across content, project name, and title.

### CLI Commands

```bash
# Index sessions
sessfind index                     # index all sources
sessfind index --source claude     # index only Claude Code
sessfind index --force             # re-index everything

# Search from CLI (non-interactive)
sessfind search "kalkulator b2b"
sessfind search "react hook" --source claude --limit 20
sessfind search "auth" --after 2025-01-01 --before 2025-03-01
sessfind search "deploy" -p my-project

# Show full session content
sessfind show SESSION_ID

# Index statistics
sessfind stats
```

### CLI Search Flags

| Flag | Description |
|------|-------------|
| `-s, --source` | Filter by source (`claude`, `opencode`, `copilot`) |
| `-p, --project` | Filter by project name (substring match) |
| `--after` | Only results after date (`YYYY-MM-DD`) |
| `--before` | Only results before date (`YYYY-MM-DD`) |
| `-n, --limit` | Max results (default: 10) |

## How It Works

1. **Indexing** — sessfind reads session files from each source, pairs user/assistant messages into chunks (~6000 chars), and indexes them with tantivy full-text search.

2. **Incremental updates** — file mtime/size is tracked in SQLite, so only new or modified sessions are re-indexed on subsequent runs.

3. **Search** — FTS mode queries the tantivy index with BM25 ranking. Fuzzy mode does in-memory substring matching on pre-loaded chunks.

4. **Resume** — selecting a session and pressing Enter replaces the current process (`exec()`) with the appropriate tool's resume command.

### Data Storage

```
~/.local/share/sessfind/
├── index/          # tantivy search index
└── state.db        # SQLite tracking indexed sessions
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| [tantivy](https://github.com/quickwit-oss/tantivy) | Full-text search engine |
| [ratatui](https://github.com/ratatui/ratatui) | Terminal UI framework |
| [crossterm](https://github.com/crossterm-rs/crossterm) | Cross-platform terminal |
| [clap](https://github.com/clap-rs/clap) | CLI argument parsing |
| [rusqlite](https://github.com/rusqlite/rusqlite) | SQLite (index state + OpenCode) |
| [serde](https://github.com/serde-rs/serde) / serde_json / serde_yaml | Serialization |
| [chrono](https://github.com/chronotope/chrono) | Date/time handling |
| [walkdir](https://github.com/BurntSushi/walkdir) | Directory traversal |
| [rayon](https://github.com/rayon-rs/rayon) | Parallel processing |

## Contributing

Contributions are welcome! Please open an issue or submit a pull request.

```bash
# Dev build (faster iteration)
cargo build

# Run directly
cargo run

# Run with args
cargo run -- search "query"
cargo run -- index --force
```

## License

[MIT](LICENSE) © [Let's Dev .IT](https://letsdev.it)
