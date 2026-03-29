# Quick Start

## Step 1 — Index your sessions

Run this the first time, and again whenever you start new AI sessions:

```bash
sessfind index
```

## Step 2 — Launch the interactive TUI

```bash
sessfind
```

Start typing to search across all your indexed sessions. Press `Enter` to resume a session.

## Combine both steps

```bash
sessfind --index
```

This indexes all sources and immediately opens the TUI.

!!! tip
    After the first index, subsequent runs are fast — `sessfind` only re-indexes new or modified sessions.
