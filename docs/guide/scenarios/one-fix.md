---
title: "A failing test, one worker"
description: "One task from start to commit: the leader hires a worker, the worker asks for a command, sends its change, and you apply and commit it."
---

# A failing test, one worker

This is the whole loop once, with one agent: you tell the leader what is wrong, it hires a worker, the worker's change (`Δ`) comes to WAITING FOR YOU, you read it, apply it and commit. You meet every part of imece that a single task touches, in the order it happens.

The checkout is a Rust crate with one failing test. Claude Code is on PATH.

## 1. Start

```sh
cd ~/code/parser
imece
```

With one coding agent on PATH there is no question; with several, the WHO LEADS? screen asks (`j k` choose, `enter` this time, `p` always). The salon opens with the team as the leader alone, `lead`, which reads your checkout and never writes to it. The first journal line says what the agents get, because the default level is `standard`:

```text
 JOURNAL
 14:00 ⚑ confine standard: workers get their vendor's shell and web; isolation is a container's job, not imece's
```

Nothing costs anything yet: `lead` has no process until you write to it.

## 2. Tell the leader

Press `/`. The footer becomes a command line labelled ` › to lead: `. Type the request and press `enter`:

```text
 › to lead: the parser test fails on empty input, fix it
```

The journal shows your line (`›` marks what you did) and the leader starts its first turn: its row reads `◆ lead  claude  thinking…`, then what it does. It reads the test with its read tools and may run `cargo test` through imece's `run` tool. It does not change a file: the leader is non-writing at every confine level, since it works in your checkout where a write would skip the review.

## 3. The leader hires

The leader calls the `hire` tool with a tier, never a model name: `cheap` for small mechanical work, `standard` (the default) for ordinary fixes, `strong` for design that cuts across the code. For this fix it picks `standard`. The worker is named after its vendor, `claude-1`, and the journal says who hired whom and on what:

```text
 14:01 ◆ lead hired claude-1 (default model)
 14:01 ⇄ lead → claude-1 [assign]: parser: empty input returns Err instead of an empty tree; src/parser.rs, test parse_empty
```

"default model" is what `standard` means for Claude: the model your Claude Code runs on. A `strong` hire would say `(opus)`, a `cheap` one `(haiku)`. The leader's answer from the tool was `claude-1 joined (claude, the default model). Give it work with send (type assign). 3 more may be hired.`: `--agents` (default 4) is the cap, and you never pick a team size.

The `assign` message opens a card, `#1`, titled by the message's first line. The worker's row shows it:

```text
 ▾ parser                      1 working  $0.09
   ◆ lead       claude          coordinating                       $0.09
   ● claude-1   #1   parser: empty input    reading src/parser.rs
```

Before its first turn imece makes `claude-1` a git worktree under `~/.local/state/imece/worktrees/`, detached at your HEAD. Your checkout is never touched by an agent.

## 4. Watching it work

Two things change on the worker's row as it goes.

Its status is what it does right now (reading, editing, running), in the words its vendor reports. After its first tool use the row also shows the worktree's stand, read every 15 s while the worker is mid-turn:

```text
   ● claude-1   #1   parser: empty input    editing src/parser.rs   $0.14  +12 −3
```

`+12 −3` are the lines its next `done` would hand you, counted as they are written. `⎇ name` would appear only if the agent made a branch, `↑N` for commits of its own, `↓N` for commits of yours it does not have (the legend in `?` names them all).

A `progress` message is one the worker sends its leader without waking it: it rides along until something else starts the leader's turn. It shows in the journal as a conversation line:

```text
 14:04 ⇄ claude-1 → lead [progress]: found it: parse() reads tokens[0] before checking len; fixing and adding a test
```

## 5. A `⚑` ask

Under `standard`, Claude Code's own shell is on, and a command Claude Code's rules would ask about comes to you instead of being refused. A **routine** command runs at once: one line, no shell syntax (no pipes, redirections, `&&`, `$` or globs), and a program the `run` tool's policy allows for the role (cargo without the commands that change dependencies, git's reading commands, the programs you named with `--run`). `cargo test` is routine; the trace says `allowed on its own: Bash: cargo test`. Anything else waits:

```text
 WAITING FOR YOU (1)
 ⚑ 14:07 claude-1 waits for your yes: Bash: cargo test 2>&1 | tail -20
   a allow · r refuse · enter focus
```

The pipe makes it non-routine. The command is shown whole, never read as markdown: what you allow is what runs. The worker's row says `⚑ waits for your yes` meanwhile, and the terminal's title reads `imece · 1 waiting`.

Press `tab` (or `w`) to the panel, or `W` from anywhere, and `a`. The flash says `allowed: claude-1 goes on`; the journal keeps your decision:

```text
 14:07 › you allowed claude-1: Bash: cargo test 2>&1 | tail -20
```

`r` would say `refused: claude-1 is told so`, and the model gets "The person refused this. Do not try it another way; if you need it, ask them". A call waits for your answer up to a day; the session's end withdraws it.

## 6. `done` becomes a change

The worker checks its work with the `run` tool (its instructions say to: only imece's own runs count as evidence, because imece knows which tree they ran on) and sends `done` to the leader, replying to the assign (`↩`). imece reads the worktree against your HEAD and the change waits for you:

```text
 14:12 ⇄ claude-1 → lead [done] ↩: parse() returns Ok(Tree::empty()) on no tokens; test parse_empty added
 14:12 ! claude-1 finished: 1 files, +14 −3 → waits for you
```

```text
 WAITING FOR YOU (1)
 Δ 14:12 claude-1 finished: parse() returns Ok(Tree::empty()) on no tokens (1 files, +14 −3) · ✓ ran clean on this tree: cargo test
   enter see the diff · a apply to the checkout · r reject
```

The item's text is the first line of the worker's `done`; it is the change's title from here on. `✓ ran clean on this tree` names the runs imece made on exactly this tree. Had the worker tested only in its own shell, the line would say `no run on this tree`: a vendor's shell carries no exit code imece can trust. Nothing is refused for lack of a run; you decide.

## 7. Reading the diff

Press `W`. It opens the next item waiting for you from wherever you are: here, the diff view.

```text
 CHANGE · claude-1 · 1 files +14 −3 · parse() returns Ok(Tree::empty()) on no tokens
 ✓ cargo test
 diff --git a/src/parser.rs b/src/parser.rs
 @@ -40,7 +40,9 @@ impl Parser {
 -    let first = tokens[0];
 +    if tokens.is_empty() {
 +        return Ok(Tree::empty());
 +    }
 j k  w  scroll · wrap   [ ]  file   a x  apply · choose a hunk   r  reject   >  quote to it   c n  note · send notes   esc  back
```

The first lines are the runs on this tree, then the patch. `j k` scroll, `gg G` to the ends, `^d ^u ^f ^b` by pages. `[` `]` go from file to file; with one file there is no file list above the patch (a change of two or more files lists them). `w` wraps long lines under themselves and `w` again cuts them at the edge. `esc` goes back.

## 8. Apply

Press `a`. The flash says `applying claude-1's change to your checkout; the journal says when it is in, staged`. Git runs on a thread of its own, so the screen never waits. Then:

```text
 14:13 ✓ you applied claude-1's change (1 files) to the checkout, staged; worktree mark 3f2a9c1
```

Applied means `git apply --index` in your checkout: the files are changed and staged, nothing is committed. If you had edited the same lines, the apply is refused with git's words and nothing is half done; if your checkout only moved on, the patch is merged three-way first into a copy of the index and applied only when that is clean.

The mark: the worker's worktree is committed at `3f2a9c1` as `imece <imece@localhost>`, with the files left as they were. So the worker's next `done` shows only what comes after, and the row's `+14 −3` drops to nothing. At its next prompt the worker is told "The person applied your change (parse() returns Ok(Tree::empty()) on no tokens) to their checkout. Your worktree is committed at 3f2a9c1 as a mark of that; your files are as you left them. Go on from here with the next task."

## 9. Look in your own shell

Press `!`. The command line opens as ` › shell: ` with `!` already there. Type and press `enter`:

```text
 › shell: !git diff --cached
```

The screen steps aside; `git diff --cached` has the terminal to itself (so a pager works). Under its output:

```text
[imece] exit 0 · 40 ms · any key: back to the screen
```

Any key brings the screen back. The journal keeps the command, how it ended and how long it took; the output never reaches an agent:

```text
 14:14 › you ran `git diff --cached` → exit 0 · 40 ms
```

## 10. A commit message

Type `:commit` and press `enter`. The flash says `asking haiku for a commit message for what is staged in parser`. imece reserves $0.05 from the pot before the call (the most a suggestion may cost) and asks Claude Code's cheap tier, with no tools and its own short system prompt, for a message for `git diff --cached`. When the answer comes, the command line opens with it, single-quoted, for you to edit:

```text
 › shell: !git commit -m 'Return an empty tree for empty input

The parser read tokens[0] before checking the length. parse_empty covers it.'
```

```text
 14:15 ! haiku suggested a commit message for what is staged, for $0.01
```

Nothing is committed by imece: `enter` runs the line in your own shell as `!` does, `esc` drops it. A day holds 20 suggestions and $1.00 across every project; the same staged tree asked again costs nothing. The `:` menu marks `:commit` with `$` because it raises the bill.

Press `enter`. The commit lands; the screen comes back on any key.

## 11. The worktree follows

Your HEAD moved. Before the worker's next prompt, a clean worktree whose own commits are all imece's marks is moved to your HEAD, and the journal says so:

```text
 14:20 ⚑ claude-1: worktree moved from 3f2a9c1 to HEAD 9b1d2e0
```

A worktree with uncommitted changes or commits of its own stays where it is, and the journal says that instead (`worktree kept at …, it has uncommitted changes`).

## 12. Quit

Press `q`. With nothing mid-turn the question is the plain one:

```text
 quit imece? q again to quit · any other key to stay
```

`q` again quits. The team, the cards, the journal and the pot are saved in one SQLite file; the next `imece` in this checkout shows the same screen with every agent asleep, costing nothing until spoken to. [Coming back](next-morning.md) continues from here.

## See also

- [Changes](../concepts/changes.md): what a `Δ` is and how apply works
- [Approvals and questions](../concepts/approvals-and-questions.md): the routine rule
- [The team](../concepts/team.md): leader, workers, helpers, tiers
- [The change view](../screen/changes.md)
- [Your shell next to the team](own-shell-and-commit.md)
