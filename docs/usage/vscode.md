# VS Code Extension

The Sessfind VS Code extension turns your indexed sessions into a searchable,
organisable hub inside the editor — no need to leave VS Code to find, browse,
resume, or organise past AI coding sessions.

It is a thin UI over the `sessfind` binary: it shells out to the CLI and renders
its `--json` output. Everything the extension does is also available from the
[command line](cli.md).

## Requirements

- The `sessfind` binary on your `PATH` (or set `sessfind.binaryPath`).
- An index built at least once: run `sessfind index`, or use the **Refresh
  Index** button in the Projects view.

## Installing

The extension lives in [`editors/vscode`](https://github.com/letsdev-it/sessfind/tree/main/editors/vscode).
Until it is published to the Marketplace and Open VSX (see the roadmap below),
build a `.vsix` locally:

```bash
cd editors/vscode
npm install
npm run package        # produces sessfind-<version>.vsix
code --install-extension sessfind-*.vsix
```

## What you get

The whole sidebar is a single custom hub: a sticky search field with
search-method chips on top, then Projects and Tags sections with hover
actions on every row.

- **Projects** — sessions auto-grouped by their directory, shown as a flat
  list or as the directory tree (toggle in the section header; tree mode
  disambiguates same-named projects in different locations).
- **Tags** — organise sessions with your own tags. Tags apply to individual
  sessions or to whole project directories (sessions inherit them); a tag
  node lists tagged projects first, then individually tagged sessions.
- **Rename** — give any session a custom name; it shows in the trees, in
  search, and as the preview tab title.
- **Project summaries (LLM)** — the ✨ action asks a detected backend
  (claude/opencode/copilot) to describe the project from its sessions; the
  summary shows in the project tooltip and details page.
- **Chat about a project** — the 💬 action opens claude/opencode/codex in the
  project root with the context pre-loaded (description, tags, recent
  sessions with ids, and hints to query sessfind for details).
- **Search results** — typing a query shows a ranked **Results** section with
  the matching snippet highlighted per hit (real search, not just tree
  filtering). With no query, a **Recent** section gives one-click resume of
  your latest sessions.
- **Keyboard & context menu** — `↓` from the search box moves into the list,
  `↑`/`↓` navigate, `Enter` opens, right-click (or the menu key) opens a
  context menu with every row action.
- **Statistics** — a data-based dashboard: sessions per source, 14-day
  activity, a 12-week contribution heatmap, busiest hours, a 7-day work log
  (grouped by day and project — handy for standups), most active projects,
  tags, engine status.
- **Git context** — the project details page appends recent commits and open
  pull requests when `git`/`gh` are available.
- **Project details** — open a project overview page with metadata and
  data-derived metrics: session counts per source, first/last activity, active
  days, top tags, and recent sessions.
- **Filter** — a filter across all three views: type a query and only matching
  sessions (full-content search plus title/path/tag substrings) stay visible;
  project and tag counts are recomputed from the matches.
- **Search Sessions** — full-text, fuzzy, semantic and LLM search from a
  QuickPick. Only the methods your binary reports are offered; instant methods
  search as you type, semantic/LLM run on Enter.
- **Session preview** — open any session as a rendered Markdown conversation.
- **Resume / new session** — resume in an integrated terminal in the session's
  project directory; starting a new session asks which installed tool to use
  (claude, opencode, copilot, cursor, codex — whatever is on your PATH).

## Settings

| Setting | Default | Description |
| --- | --- | --- |
| `sessfind.binaryPath` | `sessfind` | Path to the binary (absolute if not on `PATH`). |
| `sessfind.searchLimit` | `50` | Max search results to fetch. |
| `sessfind.defaultSearchMethod` | `fts` | Default search method. |

## Roadmap

The extension is the first step of a broader "session hub". Planned:

- LLM-generated project summaries (in the background, using a detected backend).
- A statistics dashboard and a richer conversation viewer (webview).
- Git / PR history per project.
- `sessfind serve` — launch a browser-based VS Code (`code serve-web`, falling
  back to a downloaded open-source server) for users without VS Code installed.
- Marketplace + Open VSX publishing.
- A dedicated standalone web UI built on the same JSON layer.
