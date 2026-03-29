# CLI Commands

## Index sessions

```bash
sessfind index                     # index all sources
sessfind index --source claude     # index only Claude Code
sessfind index --force             # re-index everything
```

## Search from CLI (non-interactive)

```bash
sessfind search "shopping assistant"
sessfind search "react hook" --source claude --limit 20
sessfind search "auth" --after 2025-01-01 --before 2025-03-01
sessfind search "deploy" -p my-project

# Semantic search (requires sessfind-semantic plugin)
sessfind search "how to handle authentication" --method semantic

# LLM search (uses first detected AI CLI tool)
sessfind search "how to handle authentication" --method llm
```

## Show full session content

```bash
sessfind show SESSION_ID
```

## Index statistics

```bash
sessfind stats
```

Shows number of indexed sessions per source, semantic plugin status, and active LLM backends.

## Dump all chunks as JSONL

```bash
sessfind dump-chunks
```

Used internally by plugins (e.g., `sessfind-semantic`).

## Configure LLM model per provider

```bash
sessfind llm-model-set claude sonnet
sessfind llm-model-set opencode anthropic/claude-sonnet-4-6
sessfind llm-model-unset claude    # revert to tool's default model
```

## CLI Search Flags

| Flag | Description |
|------|-------------|
| `-s, --source` | Filter by source (`claude`, `opencode`, `copilot`) |
| `-p, --project` | Filter by project name (substring match) |
| `--after` | Only results after date (`YYYY-MM-DD`) |
| `--before` | Only results before date (`YYYY-MM-DD`) |
| `-n, --limit` | Max results (default: 10) |
| `-m, --method` | Search method: `fts` (default), `fuzzy`, `semantic`, `llm` |
