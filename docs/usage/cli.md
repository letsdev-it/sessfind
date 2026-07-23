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
# Native IDs are normally sufficient. Disambiguate a cross-tool collision:
sessfind show SESSION_ID --source claude
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
sessfind sessions list --json --tag work --limit 20
sessfind projects list --json         # auto-grouped by directory
```

## Tags

Tags attach to individual sessions or to whole project directories — sessions
inherit their directory's tags, and `sessions list --tag` matches the
effective set.

```bash
sessfind tag add SESSION_ID work rust
sessfind tag rm SESSION_ID rust
# Use --source when the same native ID exists in more than one tool:
sessfind tag add SESSION_ID --source codex work
sessfind tag add-project ~/code/backend work    # whole directory
sessfind tag rm-project ~/code/backend work
sessfind tag list --json
```

## Project summaries & chat

```bash
# Ask a detected LLM backend to describe the project from its sessions;
# stored and exposed as ProjectGroup.description in projects list --json.
sessfind projects summarize ~/code/backend --tool claude

# Print (or emit as JSON CommandSpec) a command that opens an interactive
# claude/opencode/codex session in the project root, pre-loaded with a brief:
# description, tags, recent sessions with ids, and sessfind usage hints.
sessfind projects chat ~/code/backend --tool claude
```

Project summarization sends session titles and excerpts from up to five recent
conversations to the selected provider. The CLI prints a notice before
invocation. Claude uses a USD 0.50 cap; other providers use their own billing
controls.

## Rename a session

```bash
sessfind sessions rename SESSION_ID "Payments refactor"
sessfind sessions rename SESSION_ID --clear     # back to the original title
sessfind sessions rename SESSION_ID --source claude "Payments refactor"
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
