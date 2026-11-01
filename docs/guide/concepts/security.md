---
title: Security
description: "What imece guards - who may call which tool on the bus, agent text on the screen, fail-closed session checks, the pot and the Δ gate - and what it does not: it is not a sandbox. How to report a problem."
---

# Security

imece drives Claude Code, Codex and Gemini CLI on your own machine with your own accounts. It guards its own boundaries and is honest about the one it does not have: it is not a sandbox. This page is for deciding where to run it and what to expect.

## What imece guards

The standing rules: deny by default, nothing weakens silently, every boundary has a test that fails without it.

### Who may call which tool on the bus

- The bus is imece's own MCP server, one Unix socket per agent in a fresh `0700` directory. **The sender is the socket**, never a name the agent gives: no agent can speak as another, or as you.
- Each socket offers only its role's tools: `claim` to workers alone, `hire` to the leader (workers) and to workers (helpers), `approve` only to a Claude session, which calls it itself. A helper hires nobody. A session's `init` must report exactly imece's server and exactly its tools.
- `re` on a reply is checked deny by default: a message that exists, sent to the sender, by the one the reply goes to. Messages to you are refused for the types that would assign you work.
- Caps on every write: body size, unread mail, message-driven turns and posts per agent until you speak again, open cards per project ([messages](../concepts/messages-and-board.md)).

### Agent text on the screen

Everything an agent wrote reaches the terminal only through `i18n::fill` or `i18n::clean`: control characters are replaced (an escape sequence cannot redraw the screen, retitle the window or spoof a row), and words that look like secrets are hidden as `[***]` (`sk-…`, `ghp_…`, `github_pat_…`, `API_KEY=`, `TOKEN=`, `SECRET=`, `PASSWORD=`). The rule holds for diffs, quoted hunks, notes, exported Markdown, the wake briefing and the `:commit` suggestion. An approval ask is shown whole and never read as markdown: what you allow is what runs.

### Fail-closed session checks

imece switches on each vendor's own restrictions for the role and `--confine` level and verifies what the session reports it runs with: Claude's `init` on every turn, Codex's config echo, plugins, hooks, MCP servers (again before every turn) and thread, Gemini's mode, model, `/about`, `/extensions list` and `/memory list`. Anything other than what was asked, a tool, a server, a hook, a sandbox, stops the session; nothing is downgraded quietly. The sub-agent tools are off at every level ([vendors](../concepts/vendors.md)).

### The pot

One budget for the whole team, kept in the store, checked before a waking message or a card offer is delivered; Claude sessions carry a hard cap of their own; imece's own paid call (`:commit`) reserves its cap before it runs and has a daily ceiling ([budget](../concepts/budget-and-limits.md)).

### The Δ gate

No agent writes in your checkout. The leader works there read only; workers work in worktrees; a change reaches your checkout only when you apply it, staged, and a change that does not fit is refused whole. Approvals of non-routine commands and questions from agents wait for you ([changes](../concepts/changes.md), [approvals](../concepts/approvals-and-questions.md)).

### Smaller rules

- Child processes get a cleared environment (plus what the vendor needs to log in), their own process group, and capped pipes.
- The `run` tool takes one program and its arguments, never a shell; git through it takes no option before its command, no pager, no output file.
- Only a UUID ever reaches a path or a vendor when a session is resumed or forgotten.
- `:export` never writes over an existing file; `:copy` refuses paths that leave the checkout or are links.
- The store is `0600`; a file imece cannot read is refused, not overwritten.
- The repository's `AGENTS.md`/`CLAUDE.md` goes to agents cut at 16 KiB, and the journal says when it was cut.

## What imece does not do

:::warning

imece is **not a sandbox** and never calls itself one. Under `--confine standard` (the default) a worker has its vendor's shell, and an agent with a shell can do what your account can: read your home directory, reach the network, run anything you could. What confines it is the vendor's own restriction (Claude's `--restricted` and permission prompts, Codex's sandbox, Gemini's policy file), switched on and verified by imece. Isolation beyond that is a container's job. Run imece inside one when the code or the task is not yours to trust.

:::

- **The leader not writing is a role, not security.** It works in your checkout because a write there would skip the Δ gate and collide with your working copy; it is a prompt and a tool list, verified fail closed, not an OS boundary.
- **You are the last gate**, not a model and not a tool list: the Δ review, the `⚑` asks and the `?` questions in WAITING FOR YOU are where the decisions are made.
- Under `open` your own settings, hooks, MCP servers and extensions load; imece checks less there by design.
- `run` has no network block of its own. The read-only sandbox lets a Codex leader read the whole disk. Your `~/.gemini/GEMINI.md` and user-level hooks still apply to Gemini. Claude Code's own read-only rules decide what never reaches imece as an ask.
- Secrets in an agent's context are the vendor's and the model's; imece hides what looks like one on its screen and in what it writes, no more.

## Reporting a problem

Please do not open a public issue, pull request or discussion for a security problem. Report it privately through GitHub: the repository's Security tab, "Report a vulnerability". Include the affected crate and commit, what an attacker can do and under which conditions (a compromised agent, a malicious repository, a local user), and steps or a test that reproduces it. You should get an answer within a week; a fix is published with an advisory that credits you unless you would rather stay anonymous.

Of particular interest: agent text that reaches the terminal uncleaned, one agent impersonating another or you on the bus, a session running with more than imece asked for without imece noticing, a worker's change reaching the checkout past your review, and secrets that end up on screen, in logs or in a model's context.

## See also

- [How imece works](../concepts/how-imece-works.md)
- [Vendors](../concepts/vendors.md): the confine dial and the checks per vendor.
- [Approvals and questions](../concepts/approvals-and-questions.md)
- [Changes](../concepts/changes.md)
