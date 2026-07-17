# Sessfind for VS Code

Search, browse and resume your AI coding-assistant sessions (Claude Code,
OpenCode, Copilot, Cursor, Codex) from within VS Code. The extension is a thin
UI over the [`sessfind`](https://github.com/letsdev-it/sessfind) CLI — it shells
out to the binary and renders its JSON.

## Requirements

- The `sessfind` binary on your `PATH` (or set `sessfind.binaryPath`).
- Run `sessfind index` once (or use **Sessfind: Refresh Index**) to build the index.

## Features (this release)

- **Projects** view in the activity bar: sessions auto-grouped by directory.
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
