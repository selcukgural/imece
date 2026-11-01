---
title: The change view
description: Reading a worker's change (Δ) - the diff, its files and kinds, moving through it, notes on lines, choosing hunks, quoting a hunk to the worker, applying and rejecting.
---

# The change view

When a worker sends `done`, imece reads its worktree and the change waits for you as `Δ` in WAITING FOR YOU. `enter` on it (or `W` when it is the next item) opens the change view in the salon's place: the whole diff, with what ran on that tree above it. You decide here: `a` applies it to your checkout, staged; `r` sends it back; `n` sends it back with your notes on its lines; `x` and `a` apply only the hunks you chose.

```text
┌ CHANGE · claude-1 · 3 files +41 −6 · parser returns an error on an empty file ─────────────┐
│✓ cargo test                                                                                 │
│source +23 −6 (2) · tests +18 −0 (1)                                                         │
│  src/parser/manifest.rs  +19 −6                                                             │
│  src/parser/mod.rs  +4 −0                                                                   │
│  tests/manifest.rs  +18 −0                                                                  │
│diff --git a/src/parser/manifest.rs b/src/parser/manifest.rs                                 │
│--- a/src/parser/manifest.rs                                                                 │
│+++ b/src/parser/manifest.rs                                                                 │
│@@ -12,7 +12,9 @@ pub fn read(path: &Path) -> Result<Manifest>  ✓                            │
│     let text = fs::read_to_string(path)?;                                                   │
│-    let doc: Doc = toml::from_str(&text).unwrap();                                          │
│+    if text.trim().is_empty() {                                                             │
│+        return Err(Error::Empty(path.to_owned()));                                          │
│  ✎ say which file in the message                                                            │
│+    }                                                                                       │
│+    let doc: Doc = toml::from_str(&text)?;                                                  │
└─────────────────────────────────────────────────────────────────────────────────────────────┘
  j k  w scroll · wrap  [ ] file  a x apply · choose a hunk  r reject  > quote to it  c n note · send notes  esc back  ? help
```

The title: the worker, the files and lines, the done's first line. The border is in the worker's vendor colour.

## What is above the patch

1. **The runs on this tree.** Every `run` the worker made through imece's `run` tool on exactly this change, each with how it ended: `✓ cargo test`, `✗ cargo clippy: exit 1`, killed at the deadline, killed. With none it says, in the warning colour, that no imece run was made on exactly this change, so whether it builds and passes its tests is not known. The same evidence follows the item's row in WAITING FOR YOU.
2. **The lines by kind**, when the files are of more than one kind: `source +23 −6 (2) · tests +18 −0 (1) · generated +7 −5 (1)`. A file is sorted by its path alone: generated when it is a lock file (`Cargo.lock`, `package-lock.json`, `yarn.lock`, `pnpm-lock.yaml`, `go.sum`, `poetry.lock`, `Gemfile.lock`, `composer.lock`), minified or generated code (`.min.js`, `.min.css`, `.pb.go`, `_pb2.py`, `.g.dart`, `.Designer.cs`) or under `generated/`, `dist/` or `vendor/`; tests when under `tests/`, `test/`, `__tests__/` or `spec/`, named `test_*` or ending in a test suffix (`_test.go`, `_test.rs`, `_test.py`, `.test.ts`, `.spec.ts`, `Tests.cs`, `Test.java`, …); source otherwise. Generated wins over tests. Nothing is read from the files. A change of `+400 −20` that is twenty lines of code and a regenerated lock file says so here.
3. **The file list**, for a change of two files or more: each file with `+N −M` or `binary`, six at most and the rest counted (`… 2 more files`). A change of one file shows no list; its title says it.

## Moving

| Key | What it does |
| --- | --- |
| `j` `k` / `↑` `↓` | a line down / up. The top row is the line at hand, drawn as a selection; the lines move under it |
| `gg` / `G` | the first / last line |
| `^d` `^u` | half a page down / up |
| `^f` `^b`, `pgdn` `pgup`, `space` | a page down / up |
| `[` `]` | previous / next file: the `diff --git` headers |
| `w` | wrap long lines under themselves (a minified file, a long string); again cuts them at the edge. The scroll still counts patch lines, so `j` moves a whole line however many rows it takes. On for every change until turned off; not kept between runs |
| `esc` | back to WAITING FOR YOU |

Added and removed lines have their own colours; headers stand out. Every line goes through the cleaning everything drawn gets.

## Deciding

| Key | What it does |
| --- | --- |
| `a` | apply the change to your checkout, staged, for you to look over and commit. With hunks chosen (`x`), only those |
| `r` | reject: the item closes, the change stays in the worktree, the composer opens `@worker ` so you say why |
| `c` | a note on the top line |
| `n` | send the notes to the worker in one message; the change goes back |
| `x` | choose the hunk at the top; `a` then applies only the chosen hunks |
| `>` | write to the worker with the hunk at the top quoted |

### Applying

`a` only starts the apply: the flash says it is under way and the journal says how it went. Git runs on a thread of imece's own, so the screen never waits. The patch goes in with `git apply --index` after `update-index --refresh`; when that fails for a reason other than your own unstaged edits, it is merged three-way into a copy of the index first and applied for real only when the copy is clean, so a conflict is refused as `conflicts with the checkout in a, b` and nothing is half applied. Your own unstaged edit to a file the change touches is refused with git's words (`does not match index`): no merge goes over your work. After the apply the worktree is committed as a mark (author `imece`), so the worker's next `done` shows only what came after, and a clean worktree follows your HEAD once you commit. The worker hears of it at its next prompt.

Refused: `a` while that change is being applied, or while a newer `done` of the same worker is still being read.

### Rejecting

`r` closes the item and leaves the worktree as it is; the composer opens `@claude-1 ` for your reasons, and the worker gets them with a note from imece that its change was rejected. Its next `done` is a new Δ.

### Notes on lines

`c` on a line of a hunk (context, added or removed; not a header or an `@@` line) opens the command line for a note, labelled with its place, `note on src/parser/manifest.rs:14 ▸`. `enter` keeps it, `esc` gives it up, `c` on a line that has a note writes it again, an empty note takes it away. No names or commands complete while a note is written, and the footer says `enter keep`, not send. A note shows under its line after `✎`. A place is the file's new name and the line's number there, or for a removed line its number in the old file with a minus, `src/p.rs:−7`.

`n` sends the change's notes, in line order, to its worker in one message: a line saying the change stays in its worktree, not applied, to change as they say and send `done` again; then each note under its place and the line itself in a `diff` fence. The change goes back as `r` sends it back, so the journal says it was rejected and the message follows; the flash counts the notes. With no notes `n` says how to write one. Notes live in the screen's memory for the run, kept against the patch by its hash: a newer Δ from the worker has other lines, so older notes are not shown or sent with it.

### Part of a change

`x` chooses the hunk at the top (the one the top row is in; on a file's header, the next one); `x` again lets it go. A chosen hunk's `@@` line wears `✓` and the flash counts them: `1 of 3 hunks chosen; a applies only them, the rest waits`. With hunks chosen, `a` applies only those; with none, the whole change.

On the git thread the part becomes a change of its own before anything is applied: the chosen hunks under their file's header (a file with none is left out, and so is a binary file, which has no hunks), read through an index of imece's own so the worker's files and index are not touched, and applied like a whole change, three-way when clean. The worktree is marked with only that part, and the rest is read again at once and waits in WAITING FOR YOU as a change again under the same title. The worker hears that part of its change was applied (`1 of its hunks`) and that the rest waits. A part that does not fit is refused like a change that does not apply. The choice is kept against the patch's hash for the run; a newer Δ starts with none. Single lines inside a hunk cannot be chosen.

### Quoting a hunk

`>` starts a message to the change's worker, `@claude-1 ` with the cursor after it, and under your line a `diff` fence with the file's `---` and `+++` lines and the hunk at the top: at most 40 lines, the rest counted as `… +N` (the worker has the whole hunk in its worktree). You edit all of it before `enter`; it is a plain message, journaled as said. With no hunk to quote (an empty change) the flash says so. In this view `>` quotes and no longer narrows the side panel; `.` still does.

## What the journal says after

```text
14:05 › you applied claude-1's change (3 files) to the checkout, staged; worktree mark 7f3a2c1
14:06 ✕ claude-1's change did not apply: conflicts with the checkout in CHANGELOG.md
14:07 › rejected: claude-1
14:07 › you → claude-1: the empty-file check belongs in `read`, not in the caller …
```

A `done` arrives as `claude-1 finished: 3 files, +41 −6 → waits for you`; an apply as `you applied …, staged; worktree mark <sha>`; a failure as `… did not apply: <git's words>`, and the item stays; a rejection as `rejected: claude-1`, followed by your message. After a partial apply the rest comes back as a new `finished` line.

## Demo

`--demo` makes the same changes from its cards and runs, and takes `a`, `r`, `n`, `x` and `>` the same way (one apply in eight fails there, to show the failure).

## See also

- [Changes](../concepts/changes.md)
- [WAITING FOR YOU](waiting.md)
- [Reviewing with notes](../scenarios/review-with-notes.md)
- [Own shell and commit](../scenarios/own-shell-and-commit.md)
- [Keyboard](keyboard.md)
