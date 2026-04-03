# Semantic Search Plugin

## What is it?

`sessfind-semantic` is an optional standalone binary that adds ML-powered embedding search to sessfind. It finds conceptually similar sessions even when exact keywords don't match — great for vague queries like "that session about performance" when the actual content used words like "optimization" or "latency".

## Installation

```bash
# From crates.io
cargo install sessfind-semantic

# Or from source
cargo install --path crates/sessfind-semantic
```

!!! info "Auto-detection"
    Once installed, `sessfind` automatically detects the plugin on your `PATH` and enables semantic search — no configuration needed.

## How it works

1. **Embedding generation** — the plugin processes all indexed session chunks using the `multilingual-e5-small` model (~450MB, downloaded on first use). Each chunk is converted to a 384-dimension vector.
2. **Storage** — embeddings are stored in a local `sqlite-vec` database:

    === "macOS / Linux"

        ```
        ~/.local/share/sessfind/semantic.db
        ```

    === "Windows"

        ```
        %LOCALAPPDATA%\sessfind\semantic.db
        ```

3. **Search** — at query time, your input is embedded and compared against stored vectors via cosine similarity.

!!! note "First run"
    The first time you use semantic search, the ML model (~450MB) is downloaded automatically. Subsequent runs use the cached model.

## Supported languages

The `multilingual-e5-small` model supports **100+ languages**, including excellent support for Polish and English.

## Usage in TUI

1. Press `Shift+Tab` to cycle search modes until you see `Semantic`
2. Type your query
3. Press `Enter` to trigger the search (not instant — the ML model runs locally)

## Usage in CLI

```bash
sessfind search "database connection pooling" --method semantic
```

!!! warning "Performance"
    Semantic search is significantly slower than FTS or fuzzy because it runs a local neural network model. For real-time filtering, use FTS or fuzzy mode.
