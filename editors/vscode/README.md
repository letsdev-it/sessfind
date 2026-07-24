# Sessfind for VS Code

Search, browse and resume your AI coding-assistant sessions (Claude Code,
OpenCode, Copilot, Cursor, Codex) from within VS Code. The extension is a thin
UI over the [`sessfind`](https://github.com/letsdev-it/sessfind) CLI — it shells
out to the binary and renders its JSON.

## Requirements

- The `sessfind` binary. The extension automatically detects Cargo's standard
  installation path (`~/.cargo/bin/sessfind`) when VS Code is launched from
  the desktop and does not inherit your shell `PATH`: it tries `PATH` first,
  then this Cargo location. Set
  `sessfind.binaryPath` only for a non-standard installation.
- Run `sessfind index` once (or use **Sessfind: Refresh Index**) to build the index.

## Features

- One sidebar hub with recent sessions, projects grouped by directory, and
  effective session/project tags.
- Full-text, fuzzy, semantic and LLM search (only methods advertised by the
  installed binary are shown). Instant methods search as you type;
  semantic/LLM run on Enter.
- Rename sessions, tag sessions or directories, and inspect project statistics.
- Open any session as a rendered Markdown conversation in a tab.
- Resume a session, or start a new session in its project directory, in an
  integrated terminal.

## Settings

| Setting | Default | Description |
| --- | --- | --- |
| `sessfind.binaryPath` | `sessfind` | Override path for a non-standard binary installation. |
| `sessfind.searchLimit` | `50` | Max search results to fetch. |
| `sessfind.defaultSearchMethod` | `fts` | Default search method. |
