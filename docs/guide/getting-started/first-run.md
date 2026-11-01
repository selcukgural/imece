---
title: First run
description: From `imece` in a checkout to the first applied change, with the keys at each step.
---

# First run

This is a walkthrough of one sitting: you start imece in a checkout, ask the leader for a fix, a worker does it in its own worktree, you read its change and apply it, and you quit. Nothing here needs more than the keys named; every one of them is in the `?` help.

## Start

```sh
cd your/checkout
imece
```

The directory must be a git checkout with a commit (`--repo DIR` names another); in a folder without git, only a solo starts (`imece --solo claude`, see [solo sessions](../concepts/solo.md#in-a-folder-without-git)). imece first says `looking for coding agents on PATH…` while it runs each vendor's `--version`.

### Who leads

With one vendor on PATH, it leads and the salon opens. With several, and no `--lead` flag and no primary chosen before, a question comes first:

```text
┌ WHO LEADS? ─────────────────────────────────────────────┐
│ Several coding agents are on PATH. Which one leads?      │
│                                                          │
│ ▸ claude                                                 │
│   codex                                                  │
│   gemini                                                 │
│                                                          │
│  ↑↓ j k choose · enter this time · p always (primary) · esc quit │
└──────────────────────────────────────────────────────────┘
```

`enter` picks for this run; `p` picks and makes that vendor your **primary**, so the next start does not ask. The primary can also be set while imece runs with `:lead claude` (`:lead ask` forgets it, and `--lead ask` on the command line asks anyway). A primary that is no longer on PATH is said on this screen (`claude is your primary, but not on PATH`), never replaced silently. There is no silent default: this is a decision of yours.

### Where you left off

The first time there is nothing to show. On a later start that remembers an earlier run, one window opens before anything moves:

```text
┌ WHERE YOU LEFT OFF · 2 h 10 min ago ────────────────────┐
│ $3.10 of $50.00 spent; every agent sleeps until spoken to │
│ imece: 3 agents, 1 open cards                             │
│ 1 wait for you:                                           │
│ Δ 11:42 claude-1 finished: parser: accept trailing commas │
│ the last lines before:                                    │
│ 11:42 claude-1 finished: 2 files, +31 −4 → waits for you  │
│ 11:43 you ran `cargo test` → exit 0                       │
│                                any key: close             │
└───────────────────────────────────────────────────────────┘
```

It lists the pot, one line per open project, the oldest items still waiting for you (six at most), any turns that were cut short when imece closed, and the last journal lines. Any key closes it and does nothing else.

## The salon

What opens is the salon: the guild on the left, WAITING FOR YOU and the JOURNAL on the right, the header above with the pot and the fleet, the footer below with the keys that matter here.

```text
 spent $0.00 of $50.00 · all idle                                    imece
 SALON ──────────────────────────  WAITING FOR YOU (0) ─────────────────
 ▾ imece                           Nothing needs you right now.
   ◆ lead   claude   idle
                                   JOURNAL ───────────────────────────────
                                   10:02 confine standard: workers get their
                                         vendor's shell and web; isolation is
                                         a container's job, not imece's

 / write to the lead · @name to an agent · tab panel · ? help · q quit
```

The team is the leader alone (`◆ lead`). The journal's first line says, once per start that is not `--confine strict`, that isolation is not imece's.

## Write to the leader

Press `/` (or `i`). The footer becomes the composer, ` › to lead: `. Type what you want and press `enter`:

```text
 › to lead: the CSV parser rejects a trailing comma on the last column; make it accept one, with a test
   enter send · ⇧enter new line · esc cancel
```

Plain text goes to the leader of the selected project. `@claude-1 text` would go to one agent. A line that starts with `/` after the prompt is the agent's own command (`/cost`, `/compact`, `/clear`), not a message. `⇧enter` (or `\` `enter`) starts a new line in a longer request; the footer says which works in your terminal. `tab` completes names after `@` and commands after `:`.

## What the leader does

The leader reads the checkout (it has read tools, the web, and read-only commands; it cannot write: it works in your checkout, where a write would skip the review). It splits the request into cards, posts them, and **hires** a worker with its `hire` tool when a card needs hands. It picks a **tier**, not a model: `cheap` for small mechanical work, `standard` (the default) for ordinary fixes, `strong` for cross-cutting design; each vendor maps a tier to its own models (Claude: haiku, its default, opus; Codex: `@low`, its default, `@high`; Gemini: `flash`, `auto`, `pro`). It may hire from any vendor on PATH. Up to `--agents` workers (default 4).

```text
   ◆ lead      claude   coordinating                                $0.08
   ● claude-1  #1   accept a trailing comma   thinking…
```

The journal says `lead posted #1 accept a trailing comma`, `lead hired claude-1 (default model)`, `⇄ lead → claude-1 [assign]: #1 …`. The leader's closing text comes to you as a reply (`»`) in WAITING FOR YOU: `enter` reads it whole, `r` dismisses it, or you just write back.

A tier model your account does not have fails its first turn; imece then restarts that worker on the standard model and the journal says `claude-1: model opus is not available here, using the default`.

## A worker's row

```text
   ● claude-1  #1   accept a trailing comma   editing src/csv.rs   $0.41  +18 −3
```

From left to right: the glyph (`●` working, `○` idle, `◐` asking a teammate, `✕` blocked, `✓` done and not seen yet, `■` stopped, `◦` asleep), the name in its vendor's colour, its card, what it does now, what it has spent, and where its worktree stands: `+18 −3` are the lines its next `done` would hand you, `⎇ name` a branch it made, `↑N` its own commits, `↓N` your checkout's newer commits. A worker with an open permission ask shows `⚑ waits for your yes`, and the ask is an item in WAITING FOR YOU: `a` allows it, `r` refuses it (routine commands, the build, the tests, git's reading commands, run on their own).

`enter` on the row opens the worker's focus: its card's story (intent, assumptions, decisions, talk, evidence, changes). `enter` again (or `t`) opens the trace, the raw stream of the vendor's session. `esc` goes back.

## The change (Δ)

When the worker sends its `done`, imece reads its worktree as one patch and puts it in WAITING FOR YOU. The journal says `claude-1 finished: 2 files, +31 −4 → waits for you`, the header says `! 1 waiting for you`, the terminal's title becomes `imece · 1 waiting`.

```text
 WAITING FOR YOU (1) ───────────────────────────────────────────────────
 Δ 10:19 claude-1 finished: parser: accept a trailing comma (2 files, +31 −4) · ✓ ran clean on this tree: cargo test
   enter see the diff · a apply to the checkout · r reject
```

The end of the line is the evidence: the runs imece's `run` tool saw on exactly this tree. `✓ ran clean on this tree: cargo test` (green), `✗ failed on this tree: …` (red), or `no run on this tree` (a warning). It is shown, never a gate: you decide.

- `w` goes straight to WAITING FOR YOU; `tab` steps between the panels.
- `W` opens the next waiting item from anywhere (a card's screen, a diff, the board); again, the one after. `S` puts the item at hand off till later and opens the next.
- `enter` on the change opens the whole diff:

```text
 CHANGE · claude-1 · 2 files +31 −4 · accept a trailing comma ─────────────
 ✓ cargo test
 source +24 −4 (1) · tests +7 −0 (1)
 src/csv.rs +24 −4
 tests/csv.rs +7 −0
 --- a/src/csv.rs
 +++ b/src/csv.rs
 @@ -41,7 +41,9 @@ fn fields(line: &str) -> Vec<&str> {
 -    line.split(',').collect()
 +    let line = line.strip_suffix(',').unwrap_or(line);
 +    line.split(',').collect()
 j k scroll · [ ] file · a apply · r reject · c note · n send notes · x choose hunk · > quote · w wrap
```

`j` `k` scroll, `^d` `^u` half a page, `gg` `G` the ends, `[` `]` jump file to file, `w` wraps long lines. Several files are listed above the patch (six at most), and when they are of more than one kind, the lines by kind first.

### `a`: apply

`a` applies the change to your checkout with `git apply --index`: it lands **staged**, for you to look over and commit. The flash says `applying claude-1's change to your checkout; the journal says when it is in, staged`; git runs on a thread of its own, so the screen never waits. Then the journal says `you applied claude-1's change (2 files) to the checkout, staged; worktree mark 3f9a1c2`: the reviewed tree is committed in the worktree as a mark (author `imece`), so the worker's next `done` shows only what came after. A change that does not fit (you edited the same lines, or your checkout moved on) is tried three-way on a copy of the index first and refused with git's words when it conflicts; nothing is half applied. `x` on hunks and then `a` applies only the chosen hunks; the rest waits as a change again.

Then commit as you always do: `!git commit` runs in your own shell with the screen stepped aside (any key brings it back); `:commit` asks a cheap model for a message for what is staged and puts `!git commit -m '…'` on the command line.

### `r`: reject

`r` closes the item, leaves the change in the worktree, and opens the composer as `@claude-1 ` so you say why. Your reasons are the worker's next turn. Between apply and reject there is a third answer: `c` notes the top line of the diff, `n` sends all notes in one message, each under its `file:line`, and the change goes back to the worker the same way. `>` quotes the hunk at the top into a message.

## Quit

`q` asks before quitting. When something is in flight, the question says how much:

```text
 quit imece? agents mid-turn: 1, changes not reviewed: 1 · q again quits · s shows them · any other key stays
```

`q` again quits; `s` opens BEFORE YOU GO, which lists the agents mid-turn (quitting cuts their turns short) and the changes not reviewed; there `q` quits at once, `W` opens the next waiting item, any other key goes back. `:q` and `:quit` ask the same way.

## The next `imece`

The next start in the same checkout shows the same screen, every agent asleep (`◦ asleep since the last run, write to it to wake it`): no process and no cost until you write to one, or a teammate's message or a new card wakes it. A woken Claude agent goes on in the session it had when Claude Code still has its transcript (`claude-1 goes on in the session it had, with all it knew`); otherwise, and for Codex and Gemini when their thread or session is gone, it starts a fresh one and is told where it left off: its card's story, a turn that was cut short, its worktree's changes, the board (`claude-1 starts a new session, told where it left off`). A worktree found clean at an older commit is moved to your HEAD; one with changes or commits of its own stays, and the journal says which.

## See also

- [How imece works](../concepts/how-imece-works.md)
- [Changes](../concepts/changes.md) and [the change screen](../screen/changes.md)
- [Approvals and questions](../concepts/approvals-and-questions.md)
- [WAITING FOR YOU](../screen/waiting.md)
- [Sessions](../concepts/sessions.md)
- [Keyboard](../screen/keyboard.md)
- [Scenario: one fix](../scenarios/one-fix.md)
