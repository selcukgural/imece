---
title: What is imece?
description: One terminal for a team of Claude Code agents, on one codebase, with you as the last gate.
---

# What is imece?

imece runs a team of Claude Code agents on one codebase, on one screen. A leader agent takes your request, hires workers as the work needs them, and every change a worker makes waits for you before it touches your checkout. You meet imece when one coding agent is no longer enough and several of them have to work on the same repository without getting in each other's way, or in yours.

## The problem

One coding agent in one terminal is simple. Several are not:

- **Many agents.** A Claude Code session is one window with its own conversation and its own idea of what happened. Running several means several windows and nothing between them.
- **One codebase.** Two agents writing the same file at the same time break each other's work. A merge after the fact is late.
- **One screen.** Thirty agents that each print what they do are noise. What you need is the few decisions only you can make, and the rest folded away until you want it.
- **One bill.** Several sessions spend from one account. The team has one budget, and it has to stop everyone when it runs out.

## What imece is

imece is a terminal program, written in Rust, that drives Claude Code sessions as child processes and turns what they say into one stream of events. The screen never sees a wire format. On top of that it adds what a team needs and a single agent does not have:

- **A leader** (`lead`) that reads your checkout, talks to you, splits a request into cards and hires workers with a `hire` tool, up to `--agents` (default 4). You never pick a team size.
- **Workers** (`claude-1`, `claude-2`, `claude-3`, …) that each work in their own git worktree, detached at your HEAD, under imece's state directory. Your checkout is never touched by an agent. A worker may hire **helpers** (`claude-1.1`) in its own worktree, up to `--helpers` (default 2).
- **A bus** of typed messages between agents (`assign`, `ask`, `answer`, `inform`, `handoff`, `done`, `blocked`, `progress`, `notice`, `cancel`), every message numbered so a reply can name the one it answers; and a **board** of cards a free worker can claim.
- **Changes (`Δ`)**: a worker's `done` makes imece read its worktree as one patch and put it in TO DECIDE, headed by a verdict line that says whether it is ready to approve. You read the diff, apply it to your checkout (staged, for you to commit), reject it with a reason, send notes line by line, or apply only the hunks you choose. On a project on [Lonca](concepts/projects.md#on-lonca) the change is the worker's proposal, and your yes approves it.
- **Approvals (`⚑`) and questions (`?`)**: what a session would ask before running a command comes to you instead; an agent can ask you something only you can decide.
- **One pot**: `--budget` (50 dollars the first time) is one budget for the whole team, kept between runs; once it is spent nobody takes another turn. `:budget USD` changes it while agents run. The header shows it, and your plan's usage window beside it once that window is 90% used.
- **Persistence**: projects, agents, cards, the journal, mail and the board live in one SQLite file. The next `imece` shows the same screen with every agent asleep, at no cost, until you write to one.
- **Many projects**: `:open PATH` opens another checkout as a project with a leader of its own; Home groups agents by project.

## What imece is not

- **Not a sandbox.** imece switches on Claude Code's own restrictions (`--restricted`) and checks, fail closed, that a session runs with what was asked: a session that reports other tools stops. But an agent with a shell can do what your account can. Isolation beyond that is a container's job. See [security](concepts/security.md).
- **Not a model and not a tool list as the gate.** The person is the last gate: a worker's change reaches your checkout only when you apply it (the `Δ` review), and a command Claude Code would ask about runs only when you allow it.
- **Not a merge tool.** On a plain git checkout, two workers in one file are still a problem that the leader avoids by handing a shared file (a changelog, a version) to one worker at a time. A project on [Lonca](concepts/projects.md#on-lonca) has more: workers lease what they edit, Lonca's merge queue merges, and the code map (`:code`) shows who holds what.
- **Not finished.** Researchers (a capped pool that answers questions from trusted sources) are closed, not to be built: workers have the web themselves. A colour per project is not done either.

## The loop

1. You write to the leader (`/`, then your request, `enter`). Plain text goes to the leader; `@name text` goes to one agent.
2. The leader reads the checkout (it cannot write), posts cards and hires workers by **tier** (`cheap`, `standard`, `strong`), never by model name.
3. Each worker works in its own worktree with Claude Code's full coding toolset (read, write, shell, web) under the default `--confine standard`. What Claude Code would ask permission for comes to you as `⚑`.
4. A worker's `done` becomes a change `Δ` in TO DECIDE. Its verdict line says what proved it and where it reaches: `✓ ready to approve · ✓ cargo test · 2 files, both inside #1`, or `! look first` and why (a failed run, nothing ran, a file outside its card, an overlap with another change).
5. You open it (`enter` or `W`), read the diff, and answer: `a` applies it to your checkout, staged, and marks the worktree so the next `done` shows only what came after; `r` rejects it and opens the composer so you say why; `c` and `n` send notes on lines instead; `x` chooses hunks for a partial apply. A change that does not fit your checkout is refused with git's words and nothing is half applied.
6. The journal says what happened; the terminal's title counts what waits (`! imece · 2 to decide`).

## Two screens

imece has two screens, and windows you open over them:

1. **Home**, where imece opens: one list. On top, TO DECIDE: what waits for you, grouped by what deciding costs (permissions first, which one key settles, then questions, then changes, which want a diff read). Under it, AT WORK: who works, who is stuck, who is idle, by project. Every row leads with its card, `#12 Refunds`, not with an agent's name. The row at hand ends with the keys that decide it (`a yes  r no`), and a decision moves you to the next row.
2. **One piece of work**: `enter` on a row opens it, with a head line (card, project, agent, time, cost) and three tabs, `diff · talk · log`; `tab` switches. A change opens on its diff with the verdict line on top, a question on its ask with the answers as digits, an agent on its log: its card's story, its to-do list, and the raw stream of its session.
3. **Windows** over either: the board (`:board`), the code map (`:code`), the timeline (`:timeline`), the journal (`J`), the ledger (`:ledger`), the processes (`:ps`) and Lonca (`:lonca`). `esc` closes them.

`esc` goes back; `^o` goes back to where you were, like vim's jump list. The status line at the bottom says where you are and at most three keys for what is at hand; `?` lists the keys of the screen you are on, and `?` again every key.

## Screen principles

1. **Calm by default.** Nothing scrolls unless it means something; a change stands out for a moment, then calms down.
2. **You talk to the leader**, not to thirty agents.
3. **The task is the row:** every list leads with a card's number and title; the agent is a fact inside the detail.
4. **An attention budget:** only decisions that need you reach TO DECIDE, in the order deciding them costs; the header has four things: the projects, what is to decide (`! 2 to decide`), the pot, the time.
5. **Coming back is cheap:** after a pause, the first key shows what happened while you were away; back in the terminal after a few minutes in another window, it shows at once, without a key.

Everything is done with keys, vim style (`hjkl`, `gg` `G`, `^d` `^u`, `esc` back, `/` to write); a mouse is never needed and never captured, so your terminal's own selection keeps working. `?` lists the keys in English and Turkish (`L` switches).

## Status

imece is early. It runs real Claude Code sessions, and a seeded simulation with `--demo` that shows the same screen without a session or a cost. It installs with one line on macOS and Linux (`curl -fsSL https://selcukgural.github.io/imece/install.sh | sh`), or from source with `cargo`. It is used to write itself.

Every behaviour on this site comes from a decision record in [`docs/decisions/`](https://github.com/selcukgural/imece/tree/main/docs/decisions) on GitHub: one file per decision, with the reasons, the facts the probes found and the gaps left. Where this site says *why*, that is where the why comes from; where you want the exact version a fact was probed on, read the record.

## Where to go next

- [Install](getting-started/install.md): the toolchain, Claude Code and where state lives.
- [First run](getting-started/first-run.md): a walkthrough from `imece` to the first applied change.
- [The demo](getting-started/demo.md): `imece --demo`, the whole screen at work, for free.
- [Cheat sheet](cheat-sheet.md): the keys and commands of a first week, on one printable page.
- [Scenarios](scenarios/index.md): one fix, a review with notes, many projects, the next morning.
- [Keyboard](screen/keyboard.md): every key and every `:` command.
- [Videos](videos.mdx): the screen moving.
- [How imece works](concepts/how-imece-works.md): the pieces in more detail.
