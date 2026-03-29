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
| `Enter` | Resume selected session (or trigger search) |
| `PgUp/PgDn` | Scroll session preview |
| `Ctrl+U` | Clear search input |
| `?` | Show help popup |
| `Esc` | Quit |

!!! note
    `Semantic` mode is only available when the [`sessfind-semantic`](../plugins/semantic-search.md) plugin is installed.
