# Sessfind for VS Code

Search, browse and resume your AI coding-assistant sessions (Claude Code,
OpenCode, Copilot, Cursor, Codex) from within VS Code. The extension is a thin
UI over the [`sessfind`](https://github.com/letsdev-it/sessfind) CLI — it shells
out to the binary and renders its JSON.

## Requirements

- The `sessfind` binary on your `PATH` (or set `sessfind.binaryPath`).
- Run `sessfind index` once (or use **Sessfind: Refresh Index**) to build the index.

## Features

- **Projects** view: sessions auto-grouped by directory.
- **My Projects** view: user-defined projects (a root directory plus extra
  directories and pinned sessions). Create them, add directories, pin sessions.
- **Tags** view: organise sessions with your own tags.
- **Search Sessions** command: full-text, fuzzy, semantic and LLM search (only
  the methods your binary supports are offered). Instant methods search as you
  type; semantic/LLM run on Enter.
- Open any session as a rendered Markdown conversation in a tab.
- Resume a session, or start a new session in its project directory, in an
  integrated terminal.

## Settings

| Setting | Default | Description |
| --- | --- | --- |
| `sessfind.binaryPath` | `sessfind` | Path to the binary. |
| `sessfind.searchLimit` | `50` | Max search results to fetch. |
| `sessfind.defaultSearchMethod` | `fts` | Default search method. |

## Development

```bash
npm install
npm run build      # bundle with esbuild
npm test           # vitest unit tests
npm run package    # produce a .vsix
```

Press <kbd>F5</kbd> to launch an Extension Development Host. Point
`sessfind.binaryPath` at `../../target/debug/sessfind` to test against a local
build.
