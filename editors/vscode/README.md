# Sessfind for VS Code

Search, browse and resume your AI coding-assistant sessions (Claude Code,
OpenCode, Copilot, Cursor, Codex) from within VS Code. The extension is a thin
UI over the [`sessfind`](https://github.com/letsdev-it/sessfind) CLI — it shells
out to the binary and renders its JSON.

## Requirements

- The `sessfind` binary on your `PATH` (or set `sessfind.binaryPath`).
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

## Publishing

Publishing is handled by `.github/workflows/vscode.yml`. It uses the exact VSIX
produced by the tested build job. Release automation mirrors the Cargo
`release-plz` flow: conventional commits affecting the CLI or extension update
a release PR, and merging that PR creates a `vscode-v<version>` GitHub Release.
That release is then published automatically to the Marketplace. Ordinary pull
requests and pushes only build and test.

Configure a GitHub Actions secret named `VSCE_PAT` with an Azure DevOps token
that has the Marketplace **Manage** scope. The publish job targets the
`vscode-marketplace` GitHub environment, so environment protection rules can be
added without changing the workflow.
