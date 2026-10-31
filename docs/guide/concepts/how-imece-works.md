---
title: How imece works
description: The parts of imece a user meets - the screen and its model, the vendor adapters, one session per agent, the bus, the git thread, the SQLite store, and the gate between a worker's worktree and your checkout.
---

# How imece works

imece is one terminal program that runs several coding agents (Claude Code, Codex, Gemini CLI) on one codebase and shows them on one screen. This page is the architecture you need to read the rest of the guide: what runs where, what touches your files, and what never does.

## Two crates

| Crate | What it holds |
| --- | --- |
| `imece-tui` | The screen and its model. The model (`model.rs`) stores what happened as data; the screen words it at draw time in English or Turkish. The same model is fed by live sessions (`live/`, the default) or by the fake agents of `--demo`. |
| `imece-agent` | The vendor adapters. Each drives a vendor's CLI as a child process and turns its output into vendor-neutral events (a turn started, text, a tool call, a cost, a limit, an ask). The screen never sees a wire format. |

Everything an agent wrote reaches the terminal only after it is cleaned: control characters are removed and secret-looking tokens are hidden. See [security](../concepts/security.md).

## One session per agent

Every agent is one long-lived child process, never a process per turn:

- Claude Code as `claude -p` (stream-json), Codex as `codex app-server` (JSON-RPC on stdio), Gemini CLI as `gemini --acp` (the Agent Client Protocol). What each is and what imece checks is on [vendors](../concepts/vendors.md).
- The child gets a cleared environment (plus what the vendor needs to log in), its own process group (killed when imece ends), and capped pipes (a line over 8 MiB ends the session).
- Sessions start off the screen's loop, so the screen never waits for a vendor to start.

A session that reports it runs with anything other than what imece asked for (another tool, another MCP server, a hook, another sandbox) is stopped: fail closed.

## The bus: imece's own MCP server

Agents talk to each other and to you through tools imece serves itself over MCP:

- One Unix socket per agent, in a fresh `0700` temporary directory. The vendor's CLI starts `imece mcp-relay <socket>`, imece's own binary, which only copies bytes. The tools run inside imece.
- The sender is the socket the call came through, never something the agent says. No agent can speak as another or as you.
- The tools are `send`, `post`, `board`, `run`, and by role `claim` (workers), `hire` (the leader hires workers, a worker hires helpers) and `approve` (a Claude session only; Claude Code calls it itself before a tool call that needs a yes).

What the tools do is on [messages and the board](../concepts/messages-and-board.md) and [approvals and questions](../concepts/approvals-and-questions.md).

## Where the agents work

- **The leader** works in your checkout and reads it. It never writes: a write there would skip the review and collide with your working copy. This is a role, not a security boundary.
- **Workers** each get a git worktree under `$XDG_STATE_HOME/imece/worktrees/` (`~/.local/state/imece/worktrees/` by default), detached at your HEAD. Your checkout is never touched by an agent.
- **Helpers** work in their worker's worktree, on the files their worker names.

A worktree is kept between runs. One found clean at an older commit is moved to your HEAD when its agent next starts and between its turns; one with uncommitted changes or commits of its own stays where it is. The journal says which.

## The Δ gate

When a worker sends `done`, imece reads its worktree (untracked files included, through a private index, the agent's own index untouched) and shows the diff as a change (`Δ`) in WAITING FOR YOU. You apply it (`a`: `git apply --index` into your checkout, staged, three-way when a plain apply does not fit), reject it (`r`) or send notes (`n`). Nothing reaches your checkout without you. The whole path is on [changes](../concepts/changes.md).

## The git thread

Reading a change, applying one, moving a worktree to your HEAD and reading where a worktree stands are jobs for one git thread of imece's own, one at a time, in the order given. One thread, not one per job: two applies never meet in your checkout, and a `done` sent after an apply is read only once that apply has marked the worktree. The screen never waits on git; the journal says how a job went.

## The SQLite store

What imece knows is kept in one SQLite file, `$XDG_STATE_HOME/imece/imece.sqlite` (`0600`, yours alone): projects, agents, cards and their stories, the journal, the inbox, mail that waits, the board, the pot, and imece's own paid calls. A save runs after every step and every key and writes only what moved. One imece at a time has the file; a second one is told to use `:open` from the first. Details on [sessions](../concepts/sessions.md).

## What happens in a turn

```text
you  ─/ Fix the parser─▶  lead (reads the checkout)
                             │ hire(tier: standard)  → claude-1 in its worktree
                             │ send(to: claude-1, type: assign, body: …)
                           claude-1 edits, runs `cargo test` through run
                             │ send(to: lead, type: done, re: 1)
                           imece reads the worktree ─▶ Δ in WAITING FOR YOU
you  ─a─▶ staged in your checkout; the worktree marked (author imece)
```

Only the envelope of a message (its type, who it is to, which message it answers) drives anything; the body is never interpreted by imece.

## See also

- [The team](../concepts/team.md): leader, workers, helpers, hiring.
- [Vendors](../concepts/vendors.md): what a session is per vendor and what is checked.
- [Changes](../concepts/changes.md): the Δ gate end to end.
- [Sessions](../concepts/sessions.md): what is kept between runs.
- [Files](../reference/files.md): every path imece writes.
