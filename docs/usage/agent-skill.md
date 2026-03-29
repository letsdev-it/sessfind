# Agent Skill

`sessfind` ships with an **agent skill** that lets AI coding assistants (GitHub Copilot CLI, Claude Code) search and manage your sessions directly during a conversation — no need to leave your AI tool.

## What is an Agent Skill?

An agent skill is a set of instructions that teaches an AI assistant how to use a specific tool. When enabled, the AI can automatically invoke `sessfind` commands when you ask questions like:

- *"Find that conversation I had about database migrations"*
- *"What did I discuss last week about authentication?"*
- *"Resume my last Claude session about the API refactor"*

The skill handles the full workflow: **search → show → resume**.

## Installation

The skill file is located in the repository at `skills/sessfind/SKILL.md`.

### GitHub Copilot CLI

Copy the skill directory to your Copilot skills folder:

```bash
cp -r skills/sessfind ~/.copilot/skills/sessfind
```

Or add the repository's `skills/` directory as a skill location:

```
/skills add
```

Then enter the path to the `skills/` directory in this repository.

### Claude Code

Copy the skill directory to your Claude skills folder:

```bash
cp -r skills/sessfind ~/.claude/skills/sessfind
```

### Universal (works with both)

```bash
mkdir -p ~/.agents/skills
cp -r skills/sessfind ~/.agents/skills/sessfind
```

### Verify

In Copilot CLI, run `/skills list` — you should see `sessfind` in the list.

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
