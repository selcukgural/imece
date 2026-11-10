---
title: Claude Code
description: What an agent session is, how much of Claude Code the --confine dial gives, what imece checks fail-closed, Claude Code's own command line and its usage limits.
---

# Claude Code

imece looks for `claude` on PATH and drives it. An agent is one long-lived Claude Code session; imece switches on Claude Code's own restrictions for the agent's role and the `--confine` level and checks, fail closed, that the session runs with exactly that. This page says what a session is and what is checked.

## The confine dial

`--confine standard|strict|open` says how much of Claude Code each agent gets. At every level the leader never writes and Claude Code's own sub-agent tool stays off.

| Level | Workers and helpers | Leader | Your own Claude Code setup |
| --- | --- | --- | --- |
| `standard` (default) | Claude Code's full coding toolset: read, write, shell, web search and fetch | read tools and the web | not loaded |
| `strict` | file tools only; commands through imece's `run` tool; nothing asks, what would ask is refused | read tools | not loaded |
| `open` | Claude Code as you have it set up (settings, hooks, MCP servers, skills), less the sub-agent tool | its read list | loaded |

imece sandboxes nothing: it switches on Claude Code's own restrictions and verifies them; isolation is a container's job. The journal says so once at every start that is not strict.

## A session

A session is a long-lived `claude -p` stream (stream-json), one per agent.

- **Standard and strict** run in `--restricted` mode: no user or project settings (so no hooks), no MCP server but imece's own, no commands of yours, file access confined to the agent's directory. The tools are named exactly: `Glob`, `Grep`, `Read` for a reader, plus `Edit`, `Write` for a writer; from standard on `WebFetch`, `WebSearch` for everyone and `Bash` for writers; the task tools (`TaskCreate`, `TaskGet`, `TaskList`, `TaskUpdate`, the agent's own to-do list, which changes no file) for everyone at every level. A Claude Code without the task tools runs on; the trace says so once. The web is allowed outright (it only reads); a shell command Claude Code would ask about comes to imece's `approve` tool ([approvals](../concepts/approvals-and-questions.md)). Strict runs `--permission-prompts none`: what would ask is refused.
- **Open** drops `--restricted`: a writer gets Claude Code's default toolset with `--disallowedTools Agent,Task`, your settings, hooks and MCP servers load; a reader keeps its list.
- **Checked on every turn:** the `init` event must report exactly the asked tools (under open: at least them, imece's server once and connected, no sub-agent tool), imece's MCP server with exactly its tools. Anything else stops the session.
- Each session gets `--max-budget-usd`: what was left of the pot when it started, a hard cap on a session running away ([budget](../concepts/budget-and-limits.md)). Costs come in dollars.
- A choice with an effort (`opus@xhigh`, `@max`) becomes `--effort`: `low`, `medium`, `high`, `xhigh` or `max`. Claude Code only warns about an effort it does not know and runs at its default, and its `init` event says only that a per-turn effort is active, not which; so imece refuses an unknown effort before the session starts ([command line](../reference/cli.md)).
- imece's instructions are appended to Claude Code's own system prompt. The notes are Claude Code's to read: `--restricted` loads no `CLAUDE.md` by itself, so imece sets `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1` and adds the agent's own directory with `--add-dir`, and the loader reads its `CLAUDE.md` whole, with its imports and `.claude/rules`. Your `~/.claude/CLAUDE.md` comes as a copy in a directory of imece's, added the same way, since adding `~/.claude` would open it to the file tools; an `@import` in it is not followed there. Under open Claude Code reads all of it itself. The details are in [files](../reference/files.md).
- A session goes on from where it was in a later run, and a worker can be forked: [sessions](../concepts/sessions.md).

## Claude Code's own command line

A line that starts with `/` is a command of Claude Code, not a message: `/clear`, `/compact`, `/cost`, `/context`, whatever it knows; `@claude-2 /compact` sends it to one agent. It runs as a turn and its answer comes back like a reply. After `/` the completion menu offers that agent's own commands. `/clear` starts a new session id and the cost counter goes on counting.

## Usage limits

Claude Code sends `rate_limit_event`. imece keeps one window per length (five hours, a week) for the account, and the `:budget` note shows the window nearest to full after the pot: `claude 5h: 48% left`; from 90% used it is in the header too, `claude 5h: 10% left, resets in 1 h 59 min`, in the attention colour. A subscription's limit can stop the team before the pot does; imece does not stop a turn itself, it shows Claude Code's refusal. Windows are not kept between runs; a session says them again on its first turn.

Each agent's context window is reported too: every message carries the request's tokens (the context so far) and the result the model's window. The agent's line on its card's screen shows `◔ 72%` from 70% on, and its log tab the figure; the trace notes the fill by the tenth, and a compaction when Claude Code says one (`context compacted by the session: it held 181000 tokens`, or without `by the session` after your `/compact`). Gone with the session.

## Other coding agents

imece drove Codex and Gemini CLI too, until this was cut to keep it simple; that code stays on the `multi-vendor` branch (tag `multi-vendor-last`).

A project saved by a version that drove them still opens. On the first start of this version a leader or solo on Codex or Gemini carries on as Claude Code in a new session, told where it left off, and the other agents on them count as gone. A `<worktrees root>-gemini` directory left on disk is no longer used and may be deleted by hand.

## See also

- [The team](../concepts/team.md): tiers and who hires whom.
- [Approvals and questions](../concepts/approvals-and-questions.md): what a session asks and when.
- [Budget and limits](../concepts/budget-and-limits.md): how a turn is booked.
- [Security](../concepts/security.md): what the checks guard and what they do not.
- [CLI reference](../reference/cli.md): `--model`, `--tier`, `--confine`.
