# Interactive TUI

## Launching the TUI

```bash
sessfind            # launch TUI
sessfind --index    # index all sources first, then launch TUI
```

## Pane Layout

The TUI opens in full-screen mode with three areas:

- **Left pane** — search results list (source, project, date)
- **Right pane** — session details and conversation preview
- **Bottom** — search input with mode indicator

![sessfind interactive TUI — split-pane search and session preview](https://raw.githubusercontent.com/letsdev-it/sessfind/main/tui.png)

## Keybindings

| Key | Action |
|-----|--------|
| *Type* | Filter sessions in real-time |
| `Tab` | Switch focus between search and results |
| `Shift+Tab` | Toggle search mode (FTS / Fuzzy / LLM / Semantic*) |
| `Up/Down`, `j/k` | Navigate results |
| `Enter` | Resume selected session (opens confirmation dialog) |
| `PgUp/PgDn` | Scroll session preview |
| `Ctrl+U` | Clear search input |
| `?` | Show help popup |
| `Esc` | Quit |

!!! note
    `Semantic` mode is only available when the [`sessfind-semantic`](../plugins/semantic-search.md) plugin is installed.

## Resume Confirmation

When you press `Enter` on a selected session, a confirmation dialog appears showing:

- **Session summary** — source, date (in local time), and title
- **Directory choice** — where to resume the session:
    - **Session directory** — the original project directory (if it no longer exists, it will be created)
    - **Current directory** — your current working directory
    - **Cancel** — go back to search

Use `↑/↓` to select an option and `Enter` to confirm, or `Esc` to cancel.

All dates in the TUI are displayed in your computer's local timezone.
