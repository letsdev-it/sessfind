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

## JSON output & session/project listing

Most read commands accept `--json` for machine consumption (used by the
[VS Code extension](vscode.md) and any other frontend). The JSON shapes are a
stable, additively-versioned contract; `sessfind capabilities` reports the
version and what the binary supports.

```bash
sessfind capabilities                 # features, search methods, data dir (always JSON)
sessfind search "auth" --json
sessfind show SESSION_ID --json
sessfind stats --json

sessfind sessions list                # all indexed sessions, newest first
sessfind sessions list --json --tag work --user-project backend --limit 20
sessfind projects list --json         # auto-grouped by directory
```

## Tags

Tags attach to individual sessions or to whole project directories — sessions
inherit their directory's tags, and `sessions list --tag` matches the
effective set.

```bash
sessfind tag add SESSION_ID work rust
sessfind tag rm SESSION_ID rust
sessfind tag add-project ~/code/backend work    # whole directory
sessfind tag rm-project ~/code/backend work
sessfind tag list --json
```

## Rename a session

```bash
sessfind sessions rename SESSION_ID "Payments refactor"
sessfind sessions rename SESSION_ID --clear     # back to the original title
```

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
