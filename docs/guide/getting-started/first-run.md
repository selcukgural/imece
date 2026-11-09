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

The directory must be a git checkout with a commit (`--repo DIR` names another); in a folder without git, only a solo starts (`imece --solo claude`, see [solo sessions](../concepts/solo.md#in-a-folder-without-git)).

### The welcome window

On the first start ever (the store remembers no earlier run), a window opens before anything moves: what imece is, the coding agents on this machine, and the keys to start with.

```text
┌ WELCOME TO IMECE 0.1.0 ──────────────────────────────────────────────────────────┐
│                                                                                  │
│       ██        imece v0.1.0                                                     │
│   ██  ▄▄  ██    one terminal, many agents, one leader                            │
│     ▄▄▀▀▄▄      your-checkout · claude leads                                     │
│     ▀▀  ▀▀                                                                       │
│                                                                                  │
│ One terminal for many coding agents. A leader hires workers; each works          │
│ in a worktree of its own, and what it changes waits for your yes.                │
│                                                                                  │
│ Coding agents on this machine:                                                   │
│   ✓ claude  2.1.316 (Claude Code) · signed in                                    │
│                                                                                  │
│ Keys to start with:                                                              │
│   /  i            write to the leader: say what you want built                   │
│   @name text      write to one agent                                             │
│   W               the next thing to decide: a change, a question                 │
│   …                                                                              │
│ any key: close · :welcome shows this again                                       │
└──────────────────────────────────────────────────────────────────────────────────┘
```

At the top is imece's mark, five hands round one job, beside its version, what it is and the project at hand with who heads it. Where it comes on the screen its dots come one a beat, the job last (under two seconds); any key shows it whole at once, and so do [`:motion off`](../reference/commands.md#motion-offcalmfull), another window and SSH. On a screen too narrow for it the words stand alone, and on one too short the window goes without them. Home shows the same mark in its free rows while no project has a worker yet. The terminal's title carries the mark too, `⁙ imece`, while all is calm.

The line says Claude Code's version and whether it is signed in, or how to sign in: `claude auth login`. Signed in through a claude.ai plan it says `signed in, a plan · ≈ API prices`: the dollars imece shows are what the turns would cost at API prices, marked `≈` ([what a figure means](../concepts/budget-and-limits.md#what-a-figure-means)). imece asks it itself (`claude auth status`), on a thread of its own, without the network and without spending; it says `checking…` until its answer comes, `not on PATH` when the program is not there.

On every later start imece asks again, and opens the same window when it says it is not signed in (while you are writing, a line says `claude is not signed in: :welcome says how` instead). `:welcome` opens it any time and asks again. Any key closes it and does nothing else.

### What's new

The first start of a new version shows, once, what is new since the version that started before it (at most three versions), before WHERE YOU LEFT OFF:

```text
┌ WHAT'S NEW IN IMECE 0.1.0 ───────────────────────────────────────────────────────────────┐
│ 0.1.0                                                                                    │
│   · The first release, for macOS and Linux: one curl line installs and updates it.        │
│   · …                                                                                    │
│ any key: close · :news shows this again                                                  │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

The version that started last is kept in the store (`seen`); the very first start shows the welcome window instead. Any key closes it; `:news` shows the latest versions' news any time.

### Where you left off

The first time there is nothing to show. On a later start that remembers an earlier run, one window opens before anything moves:

```text
┌ WHERE YOU LEFT OFF · 2 h 10 min ago ────────────────────┐
│ $3.10 of $50.00 spent; every agent sleeps until spoken to │
│ imece: 3 agents, 1 open card                              │
│ ! 1 to decide:                                            │
│ Δ 11:42 claude-1 finished: parser: accept trailing commas │
│ the last lines before:                                    │
│ 11:42 claude-1 finished: change ready · 2 files +31 −4  │
│ 11:43 you ran `cargo test` → exit 0                       │
│                                any key: close             │
└───────────────────────────────────────────────────────────┘
```

It lists the pot, one line per open project, the oldest items still to decide (six at most; replies, only to read, are counted apart: `» 1 reply to read on Home`), any turns that were cut short when imece closed, and the last journal lines. Any key closes it and does nothing else.

## Home

What opens is [Home](../screen/home.md): what is yours to decide on top (TO DECIDE), who is at work under it (AT WORK), and from 120 columns the row at hand read beside the list. With nothing yet it says so, and imece's mark stands in the free rows:

```text
 imece  your-checkout │ $0.00 of $50.00  10:02
Nothing needs you and nobody is at work. / writes to the lead.

 home · your-checkout    enter  open   s  hide   @  write to this agent   ?  help
```

The header has four things: the open projects, what is to decide (nothing yet), the pot (spent of the budget) and the time. The bottom line is the status line: where you are, then at most three keys for what is at hand; `?` lists the keys of the screen you are on, `?` again every key.

The team is the leader alone, idle until you write; an idle leader is not listed. The journal (`J` opens it over the screen, `esc` closes it) says on its first line, once per start that is not `--confine strict`, that isolation is not imece's.

## Write to the leader

Press `/` (or `i`). The footer becomes the composer, ` › to lead: `. Type what you want and press `enter`:

```text
 › to lead: the CSV parser rejects a trailing comma on the last column; make it
            accept one, with a test
```

A line longer than the screen goes on in the next row. Before you type, the footer says `enter send · ⇧enter new line · esc cancel`.

Plain text goes to the leader of the project at hand. `@claude-1 text` would go to one agent. A line that starts with `/` after the prompt is the agent's own command (`/cost`, `/compact`, `/clear`), not a message. `⇧enter` (or `\` `enter`) starts a new line in a longer request; the footer says which works in your terminal. `tab` completes names after `@` and commands after `:`.

## What the leader does

The leader reads the checkout (it has read tools, the web, and read-only commands; it cannot write: it works in your checkout, where a write would skip the review). It splits the request into cards, posts them, and **hires** a worker with its `hire` tool when a card needs hands. It picks a **tier**, not a model: `cheap` for small mechanical work, `standard` (the default) for ordinary fixes, `strong` for cross-cutting design; they stand for haiku, Claude Code's default and opus. Up to `--agents` workers (default 4).

```text
 AT WORK 1
 ● #1   accept a trailing comma   Read the code around it
```

The journal says `lead posted #1 accept a trailing comma`, `lead hired claude-1 (default model)`, `⇄ lead → claude-1 [assign]: #1 …`. The leader's closing text comes to you as a reply in TO DECIDE, under replies, after anything to decide (`answered: …`): `enter` reads it whole, `r` dismisses it, or you just write back. The header counts it as `1 to read`, apart from `! N to decide`.

A tier model your account does not have fails its first turn; imece then restarts that worker on the standard model and the journal says `claude-1: model opus is not available here, using the default`.

## A worker's row

```text
 ● #1   accept a trailing comma   Make the change · 4 min
```

The row is the task, not the agent. From left to right: the glyph (`●` working, `○` idle, `!` it needs you, `✓` done, `■` stuck: blocked or its session ended), its card and title, the step of its to-do list it is on (or what it does now), and since when. A worker that asks for your yes leaves AT WORK while it waits: its ask is a row of its own in TO DECIDE, under permissions (`run Bash: …?`), where `a` allows it and `r` refuses it (routine commands, the build, the tests, git's reading commands run on their own).

`enter` on the row opens the worker's work on its log tab: its card's story (intent, assumptions, decisions, talk, evidence, changes) and its to-do list; `t` opens the trace, the raw stream of its session. The head line names its card, project, the agent, its time and cost; `tab` goes to the next tab, `esc` goes back.

## The change (Δ)

When the worker sends its `done`, imece reads its worktree as one patch and puts it in TO DECIDE. The journal says `claude-1 finished: change ready · 2 files +31 −4`, the header says `! 1 to decide`, the terminal's title becomes `! imece · 1 to decide`. On Home the change is a row under changes:

```text
 TO DECIDE 1 change
 changes                                                                                 1 ready to approve
▸! #1   accept a trailing comma   ✓ cargo test · 2 files +31 −4, both inside #1     a  approve   r  send back
```

The row is the change's verdict: the runs imece's `run` tool saw on exactly this tree (`✓ cargo test`, `✗ cargo test: exit 101`, or `nothing ran`), its files against the paths its card owns, and any other waiting change it overlaps. One proved clean, inside its card's paths and overlapping nothing is ready to approve, and the ready ones come first. It is shown, never a gate: you decide.

- `W` opens the next thing to decide from anywhere (a card's screen, a diff, a window); again, the one after. `S` puts the item at hand off till later and opens the next.
- `enter` on the change opens it on its diff tab:

```text
#1 accept a trailing comma · your-checkout · claude-1 · 6 min · $0.41 · 2 files +31 −4       change 1 of 1

diff · talk · log                                                                             tab switches
┌────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│✓ ready to approve · ✓ cargo test · 2 files, both inside #1                                             │
│ cost · $0.41 · 6 min                                                                                   │
│ source +24 −4 (1) · tests +7 −0 (1)                                                                    │
│ src/csv.rs +24 −4                                                                                      │
│ tests/csv.rs +7 −0                                                                                     │
│src/csv.rs                                                                                              │
│@@ -41,7 +41,9 @@ fn fields(line: &str) -> Vec<&str> {                                                  │
│-    line.split(',').collect()                                                                          │
│+    let line = line.strip_suffix(',').unwrap_or(line);                                                 │
│+    line.split(',').collect()                                                                          │
└────────────────────────────────────────────────────────────────────────────────────────────────────────┘
 change · your-checkout · #1 accept a trail…    a  approve   r  send back   c  note   esc  back   ?  help
```

The head line names the card, the project, the worker, its time and cost, the files and lines, and where the change stands among those waiting. The diff starts with the verdict line, `✓ ready to approve` or `! look first` and why, then what the change cost, its lines by kind when there are several kinds, and its files (six at most). `j` `k` scroll, `^d` `^u` half a page, `gg` `G` the ends, `[` `]` jump file to file, `w` wraps long lines, `tab` goes to the talk and the log.

### `a`: approve

`a` applies the change to your checkout with `git apply --index`: it lands **staged**, for you to look over and commit. The flash says `approved: applying claude-1's change to your checkout; the journal says when it is in, staged`; git runs on a thread of its own, so the screen never waits. It applies one change at a time: one approved while another still goes in says `… goes into your checkout after the one being applied now …`, and `a` with nothing to approve meanwhile says a change is still being applied. Then the journal says `you applied claude-1's change (2 files) to the checkout, staged; worktree mark 3f9a1c2`: the reviewed tree is committed in the worktree as a mark (author `imece`), so the worker's next `done` shows only what came after. A change that does not fit (you edited the same lines, or your checkout moved on) is tried three-way on a copy of the index first and refused with git's words when it conflicts; nothing is half applied. `x` on hunks and then `a` applies only the chosen hunks; the rest waits as a change again.

Then commit as you always do: `!git commit` runs in your own shell with the screen stepped aside (any key brings it back); `:commit` asks a cheap model for a message for what is staged and puts `!git commit -m '…'` on the command line.

### `r`: send back

`r` closes the item, leaves the change in the worktree, and opens the composer as `@claude-1 ` so you say why. Your reasons are the worker's next turn. Between apply and reject there is a third answer: `c` notes the top line of the diff, `n` sends all notes in one message, each under its `file:line`, and the change goes back to the worker the same way. `>` quotes the hunk at the top into a message.

## Quit

`q` asks before quitting. When something is in flight, the question says how much:

```text
 quit imece? agents mid-turn: 1, changes not reviewed: 1 · q again quits · s shows them · any other key stays
```

`q` again quits; `s` opens BEFORE YOU GO, which lists the agents mid-turn (quitting cuts their turns short) and the changes not reviewed; there `q` quits at once, `W` opens the next waiting item, any other key goes back. `:q` and `:quit` ask the same way.

Once the screen is gone, imece leaves one line in your terminal, what waits for you first:

```text
⁙ imece · 1 to decide · 3 agents asleep · $2.40 spent this run · resume: imece
```

A part with nothing to say is left out; the demo's line ends `the demo keeps nothing`.

## The next `imece`

The next start in the same checkout shows the same screen, every agent asleep (`◦ asleep since the last run, write to it to wake it`): no process and no cost until you write to one, or a teammate's message or a new card wakes it. A woken agent goes on in the session it had when Claude Code still has its transcript (`claude-1 goes on in the session it had, with all it knew`); otherwise it starts a fresh one and is told where it left off: its card's story, a turn that was cut short, its worktree's changes, the board (`claude-1 starts a new session, told where it left off`). A worktree found clean at an older commit is moved to your HEAD; one with changes or commits of its own stays, and the journal says which.

## See also

- [How imece works](../concepts/how-imece-works.md)
- [Changes](../concepts/changes.md) and [the change screen](../screen/changes.md)
- [Approvals and questions](../concepts/approvals-and-questions.md)
- [Home](../screen/home.md)
- [Sessions](../concepts/sessions.md)
- [Keyboard](../screen/keyboard.md)
- [Scenario: one fix](../scenarios/one-fix.md)
