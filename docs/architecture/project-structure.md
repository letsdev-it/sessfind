# Project Structure

## Crates layout

```
crates/
├── sessfind/           # main binary (TUI, CLI, indexer, sources)
├── sessfind-common/    # shared types (Source, SearchResult, SearchParams)
└── sessfind-semantic/  # optional plugin (embedder, sqlite-vec store)
```

- **`sessfind`** — the main binary. Handles the TUI, CLI commands, indexing pipeline, source adapters (Copilot, Claude, OpenCode), FTS/Fuzzy/LLM search, and session resume.
- **`sessfind-common`** — shared data types used across crates: `Source`, `SearchResult`, `SearchParams`, and the plugin communication protocol.
- **`sessfind-semantic`** — optional standalone binary. Implements the semantic search plugin: generates embeddings and stores them in `sqlite-vec`. Discovered and invoked by the main binary via `PATH`.

## Data storage paths

```
~/.local/share/sessfind/
├── index/          # tantivy search index
├── state.db        # SQLite tracking indexed sessions
└── semantic.db     # sqlite-vec embeddings (if plugin installed)

~/.config/sessfind/
└── config.json     # LLM model overrides (optional)
```

!!! info
    All data is stored locally. sessfind does not send any session content to remote servers (except when you explicitly trigger an LLM search, which invokes the AI CLI tool you have installed).
