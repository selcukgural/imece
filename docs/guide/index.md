---
title: What is imece?
description: One terminal for many coding agents from many vendors, on one codebase, with you as the last gate.
---

# What is imece?

imece runs a team of coding agents (Claude Code, Codex, Gemini CLI) on one codebase, on one screen. A leader agent takes your request, hires workers as the work needs them, and every change a worker makes waits for you before it touches your checkout. You meet imece when one coding agent is no longer enough and several of them, from several vendors, have to work on the same repository without getting in each other's way, or in yours.

## The problem

One coding agent in one terminal is simple. Several are not:

- **Many agents, many vendors.** Claude Code, Codex and Gemini CLI each have their own command line, their own permission prompts, their own idea of a session and their own way of saying what a turn cost. Running three of them means three windows and three sets of rules.
- **One codebase.** Two agents writing the same file at the same time break each other's work. A merge after the fact is late.
- **One screen.** Thirty agents that each print what they do are noise. What you need is the few decisions only you can make, and the rest folded away until you want it.
- **One bill.** Each vendor counts differently (dollars, tokens, a subscription window). The team has one budget, and it has to stop everyone when it runs out.

## What imece is

imece is a terminal program, written in Rust, that drives the vendors' own command-line agents as child processes and turns what they say into one vendor-neutral stream of events. The screen never sees a wire format. On top of that it adds what a team needs and a single agent does not have:

- **A leader** (`lead`) that reads your checkout, talks to you, splits a request into cards and hires workers with a `hire` tool, up to `--agents` (default 4). You never pick a team size.
- **Workers** (`claude-1`, `codex-2`, `gemini-3`, …) that each work in their own git worktree, detached at your HEAD, under imece's state directory. Your checkout is never touched by an agent. A worker may hire **helpers** (`claude-1.1`) in its own worktree, up to `--helpers` (default 2).
- **A bus** of typed messages between agents (`assign`, `ask`, `answer`, `inform`, `handoff`, `done`, `blocked`, `progress`, `notice`, `cancel`), every message numbered so a reply can name the one it answers; and a **board** of cards a free worker can claim.
- **Changes (`Δ`)**: a worker's `done` makes imece read its worktree as one patch and put it in WAITING FOR YOU. You read the diff, apply it to your checkout (staged, for you to commit), reject it with a reason, send notes line by line, or apply only the hunks you choose.
- **Approvals (`⚑`) and questions (`?`)**: what a vendor's session would ask before running a command comes to you instead; an agent can ask you something only you can decide.
- **One pot**: `--budget` (50 dollars the first time) is one budget for the whole team, kept between runs; once it is spent nobody takes another turn. `:budget USD` changes it while agents run. Beside it the header shows each vendor plan's usage window nearest to full.
- **Persistence**: projects, agents, cards, the journal, mail and the board live in one SQLite file. The next `imece` shows the same screen with every agent asleep, at no cost, until you write to one.
- **Many projects**: `:open PATH` opens another checkout as a project with a leader of its own; the salon groups agents by project.

## What imece is not

- **Not a sandbox.** imece switches on each vendor's own restrictions (Claude Code's `--restricted`, Codex's sandbox, Gemini's policy file) and checks, fail closed, that a session runs with what was asked: a session that reports other tools stops. But an agent with a shell can do what your account can. Isolation beyond that is a container's job. See [security](concepts/security.md).
- **Not a model and not a tool list as the gate.** The person is the last gate: a worker's change reaches your checkout only when you apply it (the `Δ` review), and a command the vendor would ask about runs only when you allow it.
- **Not a merge tool.** Two workers in one file are still a problem that the leader avoids by handing a shared file (a changelog, a version) to one worker at a time; symbol leases, the real fix, are not built yet. The code map (`3`) is a preview of them.
- **Not finished.** Researchers (a capped pool that answers questions from trusted sources) are designed but not built; the demo shows them as a preview. A colour per project is not done either.

## The loop

1. You write to the leader (`/`, then your request, `enter`). Plain text goes to the leader; `@name text` goes to one agent.
2. The leader reads the checkout (it cannot write), posts cards and hires workers by **tier** (`cheap`, `standard`, `strong`), never by model name; each vendor maps a tier to its own models.
3. Each worker works in its own worktree with its vendor's full coding toolset (read, write, shell, web) under the default `--confine standard`. What its vendor would ask permission for comes to you as `⚑`.
4. A worker's `done` becomes a change `Δ` in WAITING FOR YOU, with the runs imece saw on exactly that tree (`✓ ran clean on this tree: cargo test`, or `no run on this tree`).
5. You open it (`enter` or `W`), read the diff, and answer: `a` applies it to your checkout, staged, and marks the worktree so the next `done` shows only what came after; `r` rejects it and opens the composer so you say why; `c` and `n` send notes on lines instead; `x` chooses hunks for a partial apply. A change that does not fit your checkout is refused with git's words and nothing is half applied.
6. The journal says what happened; the terminal's title counts what waits (`imece · 2 waiting`).

## Three depths

Every part of the screen has three depths, so you can look as deep as the moment needs:

1. **The guild**: the salon, with projects, their leaders, workers and helpers, one row each, and what needs you on top.
2. **One card's story**: `enter` on an agent opens its focus, with its card's intent, decisions, assumptions, conversations and evidence; `#N` or `#word` opens a card's own screen.
3. **The raw stream**: `enter` again (or `t`) shows the trace, what the vendor's session actually sent.

`esc` goes back a depth; `^o` goes back to where you were, like vim's jump list.

## Screen principles

1. **Calm by default.** Nothing scrolls unless it means something; a change stands out for a moment, then calms down.
2. **You talk to the leader**, not to thirty agents.
3. **Three depths everywhere:** the guild, one card's story (intent, decisions, assumptions, conversations, evidence), the raw stream.
4. **An attention budget:** only decisions that need you reach the inbox; calm agents fold into one line.
5. **Coming back is cheap:** after a pause, the first key shows what happened while you were away; back in the terminal after a few minutes in another window, it shows at once, without a key.

Everything is done with keys, vim style (`hjkl`, `gg` `G`, `^d` `^u`, `esc` back, `/` to write); a mouse is never needed and never captured, so your terminal's own selection keeps working. `?` lists every key in English and Turkish (`L` switches).

## Status

imece is early. It runs real Claude Code, Codex and Gemini CLI sessions, and a seeded simulation with `--demo` that shows the same screen without a vendor or a cost. Install is from source (a Rust toolchain and `cargo`); there are no packages yet. It is used to write itself.

Every behaviour on this site comes from a decision record in [`docs/decisions/`](https://github.com/selcukgural/imece/tree/main/docs/decisions) on GitHub: one file per decision, with the reasons, the facts the probes found and the gaps left. Where this site says *why*, that is where the why comes from; where you want the exact version a fact was probed on, read the record.

## Where to go next

- [Install](getting-started/install.md): the toolchain, the vendor CLIs and where state lives.
- [First run](getting-started/first-run.md): a walkthrough from `imece` to the first applied change.
- [The demo](getting-started/demo.md): `imece --demo`, the whole screen at work, for free.
- [Scenarios](scenarios/index.md): one fix, mixed vendors, a review with notes, many projects, the next morning.
- [Keyboard](screen/keyboard.md): every key and every `:` command.
- [Videos](videos.mdx): the screen moving.
- [How imece works](concepts/how-imece-works.md): the pieces in more detail.
