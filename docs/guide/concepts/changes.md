---
title: Changes (the Δ gate)
description: How a worker's work reaches your checkout - its done becomes a change in WAITING FOR YOU, which you apply, reject, annotate or apply in part; what the worktree does meanwhile.
---

# Changes (the Δ gate)

A change (`Δ`) is what a worker's worktree holds when the worker says `done`. It waits for you in WAITING FOR YOU, and nothing of it reaches your checkout until you apply it. This is the gate between the agents and your working copy, and you are the one who opens it.

## From `done` to Δ

1. A worker (role Worker only; a helper's `done` is its worker's business, a leader changes nothing) sends `done`.
2. On imece's git thread, the worktree is read through a private index file: `read-tree HEAD`, `add -A`, `write-tree`, `diff --cached --binary` against its HEAD. Untracked files count; the agent's own index is untouched. A patch over 4 MiB is refused and the worker told.
3. The result is a `Δ` item in WAITING FOR YOU, its detail the `done`'s first line, and the journal says `claude-1 finished: 3 files, +34 −8 → waits for you`. A newer `done` from the same worker supersedes the older change.
4. The worker's `send` answer waits for the read, and tells it that you see the change.

```text
 WAITING FOR YOU (2)
 Δ 14:02 claude-1 finished: parser tokens (3 files, +34 −8) · ✓ ran clean on this tree: cargo test -p parser
 ? 14:05 codex-1 asks you: keep the old CLI flag?
  enter see the diff · a apply to the checkout · r reject
```

### Evidence on the line

A run of the worker or a helper through imece's `run` tool is tied to the tree it ran on (the worktree is read before and after; a run that changed the tree counts for none). When a `done` is read, the runs on exactly its tree go with it: the `Δ` line ends with `✓ ran clean on this tree: …`, `✗ failed on this tree: …` or `no run on this tree`, and the diff starts with one line per run. It is shown, never a gate: a change with no run can still be applied; you decide. A vendor's own shell does not count, since imece does not see what tree it ran on.

## Reading it

`enter` opens the whole diff: `j k` scroll, `^d ^u ^f ^b` pages, `gg G`, `[ ]` file to file. Every line is cleaned before it is drawn. Above the patch:

- the runs on its tree (or that whether it builds and passes is not known);
- when the files are of more than one kind, the lines per kind: `source +20 −1 (2) · tests +18 −0 (1) · generated +7 −5 (1)`. Kinds are by path alone: lock files, minified and generated code and `generated/`, `dist/`, `vendor/` directories are generated; `tests/`, `test/`, `__tests__/`, `spec/`, `test_*` and the usual test suffixes are tests; the rest is source. Generated wins over tests;
- for a change of two files or more, the file list with `+N −M` each (or `binary`), six at most, the rest counted.

`w` wraps long lines under themselves (`w` again cuts them at the edge). The top row is the line at hand: `c`, `>` and `x` act on it.

`h` (or `←`) folds the file of the line at hand to its `diff --git` header, which then says how many lines it hides (`▸ 42 lines`); `l` (or `→`) opens it, `za` turns it, `zM` (`:fold`) folds every file and `zR` (`:unfold`) opens them all. `j` `k`, the pages and `G` go over the lines shown, so a folded file is one line to step over; `[` `]` still go from header to header. Folding the file of a hidden line puts the line at hand on its header. `v` opens every file first, since lines are chosen from the whole patch. The folds hold for that patch: a new one from the worker starts open.

## Apply

`a` starts the apply on the git thread; the flash says it is under way and the journal says how it went.

- `git apply --index` in your checkout, atomic: the change lands **staged**, for you to look over and commit. `update-index --refresh` runs first so a touched but unchanged file is not a mismatch.
- **Three-way when clean:** when the plain apply fails for any reason other than your own unstaged edits, the patch is merged three-way into a copy of your index (`<git dir>/imece-apply-index`), which touches no file. Only when that is clean does the real `git apply --index --3way` run. A conflict is refused as `conflicts with the checkout in a, b`; an unstaged edit of yours in the same file is refused with git's own words (no merge goes over your work). Nothing is half applied.
- **The mark:** after an apply the worktree's reviewed tree is committed there as author `imece <imece@localhost>` and the worktree is reset to it (files untouched, index = HEAD). The worker's next `done` then shows only what came after, and a worktree whose own commits are all imece's keeps following your HEAD.
- The journal: `you applied claude-1's change (3 files) to the checkout, staged; worktree mark 4f3a2c1`. The worker gets a note from imece at its next prompt ("From imece, not from the person or a teammate").

`a` is refused while that change is being applied, or while a newer `done` of the worker is still being read; `r` is refused while it is being applied.

## Reject

`r` closes the item; the change stays in the worktree, and the composer opens `@claude-1 ` for you to say why. The worker hears at its next prompt that the change was not applied.

## Notes

The third answer beside apply and reject:

- `c` on a line of a hunk (context, added or removed; not a header or `@@` line) opens the command line labelled with the line's place, `note on src/p.rs:12 ▸`. `enter` keeps it, `esc` drops it, `c` again rewrites it, an empty one takes it away. A note shows under its line after `✎`.
- A place is the file's new name and the line's number there; a removed line is its number in the old file with a minus sign: `src/p.rs:−7`.
- `n` sends all notes of the change to its worker in one message, in line order, each under its `file:line` and the line itself in a `diff` fence, with a line saying the change stays in its worktree, not applied, to change as they say and send `done` again. The change goes back as a reject does: the item closes, the journal says rejected, the worker's next `done` is a new Δ.

Notes live in the screen's memory, keyed by the patch's hash: a newer Δ starts without them, and they are not kept between runs. A worked example is in [review with notes](../scenarios/review-with-notes.md).

## Quote a hunk

`>` in a change starts a message to its worker, `@claude-1 ` with the cursor after it, and under it the hunk the top row is in, in a `diff` fence with its file's `---`/`+++` lines: at most 40 lines, the rest counted as `… +N`. You can edit all of it before `enter`; it is a plain message to the worker.

## Apply part of a change

- `x` chooses the hunk at the top (on a file's header, the next one); its `@@` line wears `✓`; `x` again lets it go. The flash counts: "1 of 3 hunks chosen; a applies only them, the rest waits".
- `a` with hunks chosen applies only those: the chosen hunks become a change of their own on the git thread (a file with none is left out, so is a binary file), read through an index of imece's own, applied like a whole change (three-way when clean) and marked in the worktree with only that part.
- The rest is what the worktree holds against its new HEAD; it is read again at once and waits in WAITING FOR YOU as a change again, under the same title. The worker hears which part was applied and that the rest waits.

The choice is kept against the patch's hash for this run only. Single lines inside a hunk cannot be chosen.

## The worktree meanwhile

- A worker's row says where its worktree stands: `⎇ fix-parser ↑2 ↓1 +12 −3`. `⎇ name` is a branch the agent made (none while detached), `↑N` its own commits your HEAD lacks (imece's marks not counted), `↓N` your HEAD's commits it lacks, `+N −M` the lines its next `done` would hand you, untracked files included. Each part shows only when there is something to say; read as the session starts, when a turn ends, after an apply, after a move, and every 15 s while mid-turn. Not kept between runs.
- **Following HEAD:** a clean worktree whose own commits are all imece's is moved to your HEAD before every prompt of a worker that is between turns (helpers too), journaled `worktree moved from … to HEAD …`. One with uncommitted changes or commits of its own is kept, and the journal says so. So once you commit what you applied, the workers go on from your new HEAD.
- A worktree removed by hand that git still lists is refused with what was lost and `git worktree prune`, and the agent stops with that reason.

## Committing

imece commits nothing in your checkout. `:commit` asks Claude's cheap tier for a message for what is staged and puts `!git commit -m '…'` on the command line for you to edit and run in your own shell; it has ceilings of its own ([budget](../concepts/budget-and-limits.md)) and is walked through in [own shell and commit](../scenarios/own-shell-and-commit.md).

## See also

- [The changes screen](../screen/changes.md): every key in the diff view.
- [WAITING FOR YOU](../screen/waiting.md): `W`, `S`, the order of items.
- [One fix](../scenarios/one-fix.md) and [review with notes](../scenarios/review-with-notes.md).
- [Security](../concepts/security.md): why the gate is yours.
