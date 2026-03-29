# Automatic Indexing

By default, `sessfind` requires a manual `sessfind index` to pick up new sessions.
Below are several ways to automate this — pick whichever suits your workflow.

---

## Option 1: `sessfind watch` (recommended)

A background daemon that monitors session directories and re-indexes automatically when files change.
Uses OS-level file watching (FSEvents on macOS, inotify on Linux) — essentially zero CPU when idle.

### Run in foreground

```bash
sessfind watch
```

Prints a line every time it detects changes and re-indexes. Press `Ctrl+C` to stop.

### Install as a system service

```bash
sessfind watch install    # install and start
sessfind watch status     # check if running
sessfind watch uninstall  # stop and remove
```

On **macOS** this creates a launchd agent (`~/Library/LaunchAgents/dev.lets.sessfind.watch.plist`)
that starts on login and auto-restarts on crash. Logs go to `~/Library/Logs/sessfind-watch.log`.

On **Linux** this creates a systemd user service (`~/.config/systemd/user/sessfind-watch.service`).
View logs with `journalctl --user -u sessfind-watch -f`.

---

## Option 2: `--index` flag

Index right before launching the TUI:

```bash
sessfind --index
```

This runs a quick incremental index (only new/changed sessions) and then opens the interactive UI.
Good enough if you always search via the TUI.

---

## Option 3: Shell hook (index on terminal start)

Add one line to your shell config to index in the background every time you open a terminal:

**Bash** (`~/.bashrc`):

```bash
sessfind index >/dev/null 2>&1 &
```

**Zsh** (`~/.zshrc`):

```zsh
sessfind index >/dev/null 2>&1 &
```

**Fish** (`~/.config/fish/config.fish`):

```fish
sessfind index >/dev/null 2>&1 &
```

The incremental index is fast (milliseconds when nothing changed), so you won't notice any delay.

---

## Option 4: Cron / scheduled task

Run indexing on a fixed schedule (e.g. every 10 minutes):

### macOS / Linux (crontab)

```bash
crontab -e
```

Add:

```
*/10 * * * * /path/to/sessfind index >/dev/null 2>&1
```

Replace `/path/to/sessfind` with the actual path (find it with `which sessfind`).

### systemd timer (Linux)

Create `~/.config/systemd/user/sessfind-index.timer`:

```ini
[Unit]
Description=Index AI sessions periodically

[Timer]
OnBootSec=2min
OnUnitActiveSec=10min
Persistent=true

[Install]
WantedBy=timers.target
```

And `~/.config/systemd/user/sessfind-index.service`:

```ini
[Unit]
Description=sessfind index

[Service]
Type=oneshot
ExecStart=/path/to/sessfind index
```

Enable with:

```bash
systemctl --user daemon-reload
systemctl --user enable --now sessfind-index.timer
```

---

## Comparison

| Method | Latency | Setup | Always-on |
|--------|---------|-------|-----------|
| `sessfind watch` | ~5 seconds | `sessfind watch install` | ✓ (service) |
| `--index` flag | Before each TUI launch | None | ✗ |
| Shell hook | On terminal open | One line in rc file | ✗ |
| Cron | 1–10 minutes | `crontab -e` | ✓ |
