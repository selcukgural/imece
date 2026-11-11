---
title: How imece works
description: The parts of imece a user meets - the screen and its model, the Claude Code adapter, one session per agent, the bus, the git thread, the SQLite store, the gate between a worker's worktree and your checkout, and Lonca underneath a project.
---

# How imece works

imece is one terminal program that runs several Claude Code agents on one codebase and shows them on one screen. This page is the architecture you need to read the rest of the guide: what runs where, what touches your files, and what never does.

## Two crates

| Crate | What it holds |
| --- | --- |
| `imece` (`crates/imece-tui`) | The screen and its model. The model (`model.rs`) stores what happened as data; the screen words it at draw time in English or Turkish. The same model is fed by live sessions (`live/`, the default) or by the fake agents of `--demo`. |
| `imece-agent` | The Claude Code adapter. It drives `claude` as a child process and turns its output into events (a turn started, text, a tool call, a cost, a limit, an ask). The screen never sees a wire format. |

Everything an agent wrote reaches the terminal only after it is cleaned: control characters are removed and secret-looking tokens are hidden. See [security](../concepts/security.md).

## One session per agent

Every agent is one long-lived child process, never a process per turn:

- The child is `claude -p` (stream-json). What it is and what imece checks is on [Claude Code](../concepts/claude-code.md).
- The child gets a cleared environment (plus what Claude Code needs to log in), its own process group (killed when imece ends), and capped pipes (a line over 8 MiB ends the session).
- Sessions start off the screen's loop, so the screen never waits for a session to start.

A session that reports it runs with anything other than what imece asked for (another tool, another MCP server, a hook, another sandbox) is stopped: fail closed.

## The bus: imece's own MCP server

Agents talk to each other and to you through tools imece serves itself over MCP:

- One Unix socket per agent, in a fresh `0700` temporary directory. Claude Code starts `imece mcp-relay <socket>`, imece's own binary, which only copies bytes. The tools run inside imece.
- The sender is the socket the call came through, never something the agent says. No agent can speak as another or as you.
- The tools are `send`, `post`, `board`, `run`, `whoami` (who the agent is in imece: name, role, project, who it answers to, the budget left, whether it may hire), and by role `claim` (workers), `hire` (the leader hires workers, a worker hires helpers) and `approve` (a Claude session only; Claude Code calls it itself before a tool call that needs a yes).

What the tools do is on [messages and the board](../concepts/messages-and-board.md) and [approvals and questions](../concepts/approvals-and-questions.md).

## Where the agents work

- **The leader** works in your checkout and reads it. It never writes: a write there would skip the review and collide with your working copy. This is a role, not a security boundary.
- **Workers** each get a git worktree under `$XDG_STATE_HOME/imece/worktrees/` (`~/.local/state/imece/worktrees/` by default), detached at your HEAD. Your checkout is never touched by an agent.
- **Helpers** work in their worker's worktree, on the files their worker names.

A worktree is kept between runs. One found clean at an older commit is moved to your HEAD when its agent next starts and between its turns; one with uncommitted changes or commits of its own stays where it is. The journal says which.

## The Δ gate

When a worker sends `done`, imece reads its worktree (untracked files included, through a private index, the agent's own index untouched) and shows the diff as a change (`Δ`) in TO DECIDE. You apply it (`a`: `git apply --index` into your checkout, staged, three-way when a plain apply does not fit), reject it (`r`) or send notes (`n`). Nothing reaches your checkout without you. The whole path is on [changes](../concepts/changes.md).

## The screen

The screen draws the model when something on it changed, and never waits on a session, git or the store. It has two screens:

- **Home**, where imece opens, is one list. TO DECIDE comes first, grouped by what deciding costs: permissions (a command an agent asks to run, a script to say yes to), questions (an agent's question, the leader's plan, a card out of tries), changes (a diff to read, ready ones first). AT WORK comes under it: the workers and solos of each project, stuck first, then at work, then idle, each with its helpers under it. Every row leads with its card. See [Home](../screen/home.md).
- **One piece of work** opens with `enter` on a row: a head line with the card, the project, the agent, the time and the cost, and three tabs, `diff · talk · log`. A change starts with its verdict line, the Δ gate in one line; a question with its ask and the answers it offers; an agent with its log, from its card's story to the raw stream of its session.

The board, the code map, the timeline, the journal, the ledger, the processes and the Lonca window are windows over either (`:board`, `:code`, `:timeline`, `J`, `:ledger`, `:ps`, `:lonca`), and `esc` closes them. The header has four things: the open projects, `! N to decide`, the pot and the time.

## On Lonca

[Lonca](https://github.com/selcukgural/lonca) is a daemon that is the only writer of a repository, with leases, a merge queue and a signed log. A project can stand on one (`:lonca NAME` names a repository, `:lonca import` makes one of the checkout); imece is then a client of the daemon's socket, never a second writer:

- A worker hired from then on works in a Lonca **workspace** instead of a worktree, with Lonca's hook and tools in its Claude Code beside imece's bus. It leases what it edits and proposes its change.
- Its Δ is its newest pending proposal, once its turn is over. `a` approves it and `r` rejects it with a reason, both signed by the `lonca` program; nothing lands in your checkout.
- After your yes Lonca's merge queue runs the repository's checks on the merged tree and puts the change on trunk, or sends it back to the worker. Your checkout is a client of trunk: `:sync` brings it up.
- imece reads the daemon's state every 5 seconds while a worker works on Lonca, else every 30 seconds, and at once when the daemon tells it something happened to one of your agents' sessions: the project's line (`lonca mobile-app 9d3f1c2 · 3 pending`), the leases on the code map, the `:lonca` window. While the daemon does not answer, the project is closed: nobody is hired and no Δ is decided.

The details are on [projects](projects.md#on-lonca) and [changes](changes.md#on-lonca). A project on no repository works with worktrees and your checkout as above.

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
                           imece reads the worktree ─▶ Δ in TO DECIDE
you  ─a─▶ staged in your checkout; the worktree marked (author imece)
```

Only the envelope of a message (its type, who it is to, which message it answers) drives anything; the body is never interpreted by imece.

## See also

- [The team](../concepts/team.md): leader, workers, helpers, hiring.
- [Claude Code](../concepts/claude-code.md): what a session is and what is checked.
- [Changes](../concepts/changes.md): the Δ gate end to end.
- [Sessions](../concepts/sessions.md): what is kept between runs.
- [Files](../reference/files.md): every path imece writes.
