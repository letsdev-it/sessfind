# Agent Skill

`sessfind` ships with an **agent skill** that lets AI coding assistants (GitHub Copilot CLI, Claude Code, OpenCode) search and manage your sessions directly during a conversation — no need to leave your AI tool.

## What is an Agent Skill?

An agent skill is a set of instructions that teaches an AI assistant how to use a specific tool. When enabled, the AI can automatically invoke `sessfind` commands when you ask questions like:

- *"Find that conversation I had about database migrations"*
- *"What did I discuss last week about authentication?"*
- *"Resume my last Claude session about the API refactor"*

The skill handles the full workflow: **search → show → resume**.

## Installation

The skill file is located in the repository at `skills/sessfind/SKILL.md`.

The `~/.claude/skills/` directory is shared by **Claude Code**, **GitHub Copilot CLI**, and **OpenCode**, so installing the skill there makes it available in all three tools at once.

### Copy

```bash
cp -r skills/sessfind ~/.claude/skills/sessfind
```

### Symlink (recommended — stays in sync with the repo)

```bash
ln -s "$(pwd)/skills/sessfind" ~/.claude/skills/sessfind
```

### Alternative: add the repo's skill directory

In Copilot CLI or Claude Code, run:

```
/skills add
```

Then enter the path to the `skills/` directory in this repository.

### Verify

Run `/skills list` — you should see `sessfind` in the list.

## What the Skill Can Do

| Capability | Command Used | Example Prompt |
|-----------|-------------|----------------|
| **Search sessions** | `sessfind search` | *"Find sessions about React hooks"* |
| **Show full session** | `sessfind show` | *"Show me the full conversation from that result"* |
| **Resume session** | `copilot --resume` / `claude --resume` / `opencode --session` | *"Resume that session"* |
| **Re-index** | `sessfind index` | *"Index my latest sessions"* |
| **View stats** | `sessfind stats` | *"How many sessions do I have indexed?"* |

## Prerequisites

`sessfind` must be installed and available in your PATH:

```bash
cargo install sessfind
```

Sessions must be indexed before searching:

```bash
sessfind index
```
