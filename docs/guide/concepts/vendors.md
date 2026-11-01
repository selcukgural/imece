---
title: Vendors
description: What an agent session is for Claude Code, Codex and Gemini CLI, how much of each vendor the --confine dial gives, what imece checks fail-closed, the vendors' own command lines and their usage limits.
---

# Vendors

imece looks for `claude`, `codex` and `gemini` on PATH and drives all three. An agent is one long-lived session of its vendor's CLI; imece switches on that vendor's own restrictions for the agent's role and the `--confine` level and checks, fail closed, that the session runs with exactly that. This page says what a session is per vendor and what is checked.

## The confine dial

`--confine standard|strict|open` says how much of its vendor each agent gets. At every level the leader never writes and the vendor's own sub-agent tool stays off.

| Level | Workers and helpers | Leader | Your own vendor setup |
| --- | --- | --- | --- |
| `standard` (default) | the vendor's full coding toolset: read, write, shell, web search and fetch | read tools and the web | not loaded |
| `strict` | file tools only; commands through imece's `run` tool; nothing asks, what would ask is refused | read tools | not loaded |
| `open` | the vendor as you have it set up (settings, hooks, MCP servers, skills, extensions), less the sub-agent tool | its read list | loaded |

imece sandboxes nothing: it switches on each vendor's own restrictions and verifies them; isolation is a container's job. The journal says so once at every start that is not strict: `confine standard: workers get their vendor's shell and web; isolation is a container's job, not imece's`.

## Claude Code

A session is a long-lived `claude -p` stream (stream-json), one per agent.

- **Standard and strict** run in `--restricted` mode: no user or project settings (so no hooks), no MCP server but imece's own, file access confined to the agent's directory. The tools are named exactly: `Glob`, `Grep`, `Read` for a reader, plus `Edit`, `Write` for a writer; from standard on `WebFetch`, `WebSearch` for everyone and `Bash` for writers. The web is allowed outright (it only reads); a shell command Claude Code would ask about comes to imece's `approve` tool ([approvals](../concepts/approvals-and-questions.md)). Strict runs `--permission-prompts none`: what would ask is refused.
- **Open** drops `--restricted`: a writer gets Claude Code's default toolset with `--disallowedTools Agent,Task`, your settings, hooks and MCP servers load; a reader keeps its list.
- **Checked on every turn:** the `init` event must report exactly the asked tools (under open: at least them, imece's server once and connected, no sub-agent tool), imece's MCP server with exactly its tools. Anything else stops the session.
- Each session gets `--max-budget-usd`: what was left of the pot when it started, a hard cap on a session running away ([budget](../concepts/budget-and-limits.md)). Costs come in dollars.
- A choice with an effort (`opus@xhigh`, `@max`) becomes `--effort`: `low`, `medium`, `high`, `xhigh` or `max`. Claude Code only warns about an effort it does not know and runs at its default, and its `init` event says only that a per-turn effort is active, not which; so imece refuses an unknown effort before the session starts ([command line](../reference/cli.md)).
- Instructions are appended to Claude Code's own system prompt, with the repository's `AGENTS.md` or `CLAUDE.md` (16 KiB cap) at the end, because `--restricted` drops them.

## Codex

A session is a long-lived `codex app-server` (JSON-RPC on stdio).

- Codex always has a shell, so the **sandbox** does the confining: `read-only` for the leader, `workspace-write` (the agent's directory) for workers and helpers. Under strict the sandbox has no network and web search is off; under standard a worker's sandbox reaches the network and web search is live (a Codex leader has web search but no network in its read-only shell).
- Writers under standard and open run `approval_policy = "on-request"` and ask before what needs more than the sandbox gives; the leader and everything under strict stay `never`, where what the sandbox refuses simply fails.
- Standard and strict turn plugins, apps, hooks, sub-agents, memories and the other features off, write no session files (`history.persistence = "none"`; the thread's own rollout is kept for resume), and name off the MCP servers and skills your own `~/.codex` config brings: Codex loads your config whatever imece does, so a first start reads what is on and, if anything is, imece starts Codex a second time with each turned off by name. Open keeps your config, with only the sub-agent feature (`multi_agent`) off.
- **Checked before the thread starts:** `account/read` (ChatGPT login only), `config/read` echoes every override, no plugin enabled, no hooks, exactly imece's MCP server with exactly its tools, the thread's answer echoes the approval policy, sandbox and directory. The MCP servers are checked again before every turn. An approval request where none may come, an elicitation, a dynamic tool call, a web search or sub-agent item stops the session.
- **Tokens, not dollars.** On a ChatGPT plan a turn costs the pot nothing; the trace says so and shows the tokens. An API-key login is refused, since imece cannot count its cost yet. There is no per-session dollar cap for Codex; the pot still stops new turns.
- Tiers are reasoning efforts on Codex's default model: `@low`, the default, `@high`; a `--tier codex:T=MODEL@EFFORT` value sets both. The efforts Codex has are `low`, `medium`, `high` and `xhigh`; the effort rides on every turn, and one Codex lacks is refused before the session starts.
- Codex reads `AGENTS.md` on its own, so it is given only a `CLAUDE.md`.

## Gemini CLI

A session is a long-lived `gemini --acp` (Agent Client Protocol v1, JSON-RPC on stdio). Gemini CLI has no effort setting: a choice names a model alone, and `gemini:pro@high` is refused.

- Gemini has nothing like `--restricted` and takes its settings from `~/.gemini` and the `.gemini` directory of the folder it runs in. So imece runs it in a **private directory** of its own (next to the worktrees, `<worktrees root>-gemini/<agent>`, trusted) and gives it the agent's files as an included directory: a repository's `.gemini` settings, hooks and `.env` never reach the session.
- A **policy file** imece writes there allows exactly the tools of the role and level (read tools for the leader; read and edit tools for writers; from standard on the web for everyone without a prompt, and a shell for writers that asks before every command) plus imece's own tools, and under strict and standard denies every other tool, which is then not even shown to the model. Extensions are off and no MCP server but imece's is allowed. Under open your extensions, MCP servers and policies run; imece denies by name only Gemini's agent tools and, for the leader, the writing tools.
- The agent's instructions go in a `GEMINI.md` in that private directory, the only way ACP has to give them. Your own `~/.gemini/GEMINI.md` and user-level hooks still apply.
- **Checked before the first turn:** `initialize`, `session/new` (mode `default`, the model asked for), then the zero-token commands `/about` (model, auth type), `/extensions list` (none active) and `/memory list` (imece's GEMINI.md in use). A tool call of a kind the role may not have, an MCP tool not offered, or a mode change stops the session.
- **Tokens, not dollars.** With an API key a turn is booked from the list prices imece carries (prompts up to 200K tokens, thought tokens as output, a turn on several models at the dearest; an alias such as `auto` at the dearest model it may mean). A model imece cannot price stops the session: name one it knows with `--model` or `--tier`. With a Google login a turn costs the pot nothing, and the trace says so. Vertex AI and gateway logins are refused.
- Tiers: `flash`, the default (`auto`), `pro`. Gemini keeps its chat files under `~/.gemini/tmp` as it always does.

## The vendor's own command line

A line that starts with `/` is a command of the agent's own CLI, not a message: `/clear`, `/compact`, `/cost`, `/context`, whatever the vendor knows; `@claude-2 /compact` sends it to one agent. It runs as a turn and its answer comes back like a reply. After `/` the completion menu offers that agent's own commands.

| Vendor | Commands |
| --- | --- |
| Claude Code | its slash commands as they are; `/clear` starts a new session id and the cost counter goes on counting |
| Codex | the app server has no command line: only `/compact` works (`thread/compact/start`; a refusal is a failed turn); `/clear` ends the session, the next message starts a fresh one |
| Gemini CLI | its commands as they are |

## Usage limits

Claude Code sends `rate_limit_event` and Codex `account/rateLimits/updated`; Gemini reports none. imece keeps one window per vendor and length (five hours, a week) for the account, and the header shows each vendor's window nearest to full after the pot: `claude 5h: 48% left, resets in 1 h 59 min`, in the attention colour from 90% used. A subscription's limit can stop the team before the pot does; imece does not stop a turn itself, it shows the vendor's refusal. Windows are not kept between runs; a session says them again on its first turn.

## Which vendor leads, which are hired

`--lead` names the leader's vendor; the leader may hire workers from any vendor on PATH (`hire` takes a vendor). Mixed teams are the point: see [mixed vendors](../scenarios/mixed-vendors.md) and [the team](../concepts/team.md). Resume per vendor is on [sessions](../concepts/sessions.md).

## See also

- [Approvals and questions](../concepts/approvals-and-questions.md): what each vendor asks and when.
- [Budget and limits](../concepts/budget-and-limits.md): how tokens are booked.
- [Security](../concepts/security.md): what the checks guard and what they do not.
- [CLI reference](../reference/cli.md): `--lead`, `--model`, `--tier`, `--confine`.
