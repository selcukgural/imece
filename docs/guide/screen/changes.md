---
title: The change view
description: Reading a worker's change (Δ) - the diff, its files and kinds, moving through it, notes on lines, choosing hunks, quoting a hunk to the worker, applying and rejecting.
---

# The change view

When a worker sends `done`, imece reads its worktree and the change waits for you as `Δ` in TO DECIDE. `enter` on it (or `W` when it is the next item) opens the change, the diff tab of its [detail](#the-detail): the whole diff, with the verdict line on it above. You decide here: `a` applies it to your checkout, staged; `r` sends it back; `n` sends it back with your notes on its lines; `x` and `a` apply only the hunks you chose; `b` sends back one that clashed with your checkout.

```text
#6 Parse the manifest · shop-api · claude-1 · 12 min · $0.84 · 3 files +41 −6           change 1 of 3

diff · talk · log                                                                       tab switches
┌──────────────────────────────────────────────────────────────────────────────────────────────────┐
│✓ ready to approve · ✓ cargo test · 3 files, all inside #6 · no overlap with other changes        │
│ parser returns an error on an empty file                                                         │
│ cost · $0.84 · 12 min                                                                            │
│ source +23 −6 (2) · tests +18 −0 (1)                                                             │
│ src/parser/manifest.rs +19 −6                                                                    │
│ src/parser/mod.rs +4 −0                                                                          │
│ tests/manifest.rs +18 −0                                                                         │
│src/parser/manifest.rs                                                                            │
│@@ -12,7 +12,9 @@ pub fn read(path: &Path) -> Result<Manifest>  ✓                                 │
│     let text = fs::read_to_string(path)?;                                                        │
│-    let doc: Doc = toml::from_str(&text).unwrap();                                               │
│+    if text.trim().is_empty() {                                                                  │
│+        return Err(Error::Empty(path.to_owned()));                                               │
│  ✎ say which file in the message                                                                 │
│+    }                                                                                            │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
 change · shop-api · #6 Parse the manifest    a  approve   r  send back   c  note   esc  back   ?  help
```

The status line shows at most three keys for the change; every other key of this page is in `?`.

## The detail

A change, a conversation and a focus are the three tabs of one screen, the detail of a piece of work (record 125): above the tab's own frame a head line says the card and its title, the project, the agent, how long it has been as it is and what it cost; on the diff, the files and lines, and `change 2 of 5` at the right when several wait. When the agent has a question open, the question stands under the head with the answers it offers and their keys (`1`-`9` pick one, `/` writes your own), whatever the tab. Then the tab bar, `diff · talk · log`: the open tab bold, one with nothing to show (no change, nothing said yet) dim; `tab` and `⇧tab` go to the next and the previous tab that has something. The diff is this page; talk is the [conversation](conversations.md) with the agent (with you when there is one, else its newest with a teammate); log is its focus, or its raw stream once `enter` opened it. `esc` leaves the tab as it did before: the conversation closes back to what was under it, the diff and the focus back to Home.

```text
#6 E-mail templates · shop-api · claude-9 · 6 min · $0.06 · 3 files +41 −6                            change 1 of 3

diff · talk · log                                                                                   tab switches
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│✓ ready to approve · ✓ cargo test · 3 files, all inside #6 · no overlap with other changes   │
```

## What is above the patch

**The verdict line** comes first: the Δ gate in one line, `✓ ready to approve` or `! look first`, then the facts it rests on, each in its own colour, wrapped when the window is narrow:

```text
✓ ready to approve · ✓ check · ✓ dotnet test · 2 files, both inside #6 · no overlap with other changes · after your yes Lonca runs: build, tests
! look first · ✗ dotnet test: exit 1 · 3 files · 1 outside #18: Cargo.toml · overlaps #51 on Money.cs
```

- **What proved it:** the repository's [check](../concepts/changes.md#the-repositorys-check) when it ran (`✓ check`, or `✗ check · exit 1, 3.0 s` with the last lines of its output under the line), then every `run` the worker made through imece's `run` tool on exactly this tree, each with how it ended (`✓ cargo test`, `✗ cargo clippy: exit 1`, killed at the deadline, killed); with neither, the builds and tests the worker ran in Claude Code's own shell, said but never counted (`npm test passed in its own shell, not proved on this tree` in the warning colour, `✗ cargo build in its own shell: exit 101` in red), since imece does not see what tree they ran on; `nothing ran` in the warning colour when there is none of these.
- **The files against the card's paths:** `2 files, both inside #6` when the worker's card owns paths ([the paths a card owns](../concepts/messages-and-board.md#the-paths-a-card-owns)) and every file is in them; `3 files · 1 outside #18: Cargo.toml` when some are not, three names at most and the rest counted; `3 files` alone when the card owns no paths. The card is the one the change was made for, done or not.
- **Overlap:** `overlaps #51 on Money.cs` for every other change waiting in the project that touches a file this one touches (the rest is a merge for you to make); on Lonca also `overlaps proposal 12 (ayse) on Money.cs` for every proposal waiting in the repository that no change here shows, someone else's or a worker's whose turn is not over, by its number and who made it (imece reads the paths Lonca lists for it); `no overlap with other changes` when others wait and none shares a file; nothing when this is the only change waiting.
- **The checkout:** `does not fit your checkout` after a refused apply (the red line below says where, and `b` sends it back).
- **Lonca:** `the patch is not whole` when Lonca redacted a file or cut the patch; `based before trunk, merged after your yes` when trunk moved on since the proposal's base, a fact that leaves it ready, since the merge queue merges it ([on Lonca](../concepts/changes.md#on-lonca)); `after your yes Lonca runs: build, tests`, `nothing runs after your yes` or `Lonca did not say what runs after your yes` for a proposal ([what is checked on Lonca](../concepts/changes.md#what-is-checked-on-lonca)), then `the card also asks for api` when its card asks for checks Lonca would not run otherwise; your `a` asks for them, and `:checks` changes them first; `Lonca's policy no longer has smoke: :checks changes what the card asks for`, in red, when the card asks for one the policy in force does not define, which keeps the change from ready ([the checks a card asks for](../concepts/messages-and-board.md#the-checks-a-card-asks-for)).

It is ready when every proof passed, every file is inside the card's paths, nothing overlaps, it fits and it is shown whole; one thing short of that is `! look first`, and the line says which. The same facts, without the head, follow the change's row on [Home](home.md), and the ready ones come first there. It is a note, never a gate: `a` approves a change the line says to look at first.

Then, when the worker wrote more in its `done` than the first line, what it said: why, what it decided, what to look at; four rows at most, cut with `…`. With several changes waiting the head line's right end says which this is, `change 2 of 5`. Under that:

1. **The check's state** when it has not come to an end: `● the check runs on this change…` while it runs; `▶ the repository's check asks to run: it is in TO DECIDE` until you answer its ask; or why it could not run. A failed check's last output lines stand here too.
2. **What it cost**, once the worker's turn ended: `cost · $0.84 · 12 min`, dim, the worker's spend (its helpers' counted in) from its first prompt after its last change to this `done`, and the time it worked; a change sent back and sent again adds its rounds (`$1.20 · 25 min · 3 rounds`). The same figure ends the item's row in TO DECIDE and the journal says it (`claude-1's change cost $0.84 · 12 min`). A `done` read mid-turn shows it a moment later, when the turn's cost is known.
3. **The lines by kind**, when the files are of more than one kind: `source +23 −6 (2) · tests +18 −0 (1) · generated +7 −5 (1)`. A file is sorted by its path alone: generated when it is a lock file (`Cargo.lock`, `package-lock.json`, `yarn.lock`, `pnpm-lock.yaml`, `go.sum`, `poetry.lock`, `Gemfile.lock`, `composer.lock`), minified or generated code (`.min.js`, `.min.css`, `.pb.go`, `_pb2.py`, `.g.dart`, `.Designer.cs`) or under `generated/`, `dist/` or `vendor/`; tests when under `tests/`, `test/`, `__tests__/` or `spec/`, named `test_*` or ending in a test suffix (`_test.go`, `_test.rs`, `_test.py`, `.test.ts`, `.spec.ts`, `Tests.cs`, `Test.java`, …); source otherwise. Generated wins over tests. Nothing is read from the files. A change of `+400 −20` that is twenty lines of code and a regenerated lock file says so here.
4. **The file list**, for a change of two files or more: each file with `+N −M` or `binary`, six at most and the rest counted (`… 2 more files`). A change of one file shows no list; its title says it. From 120 columns on, a change of two files or more lists all of them beside the patch instead, the file at hand marked `▸`, and `[` `]` go through them.

The patch shows a file's path where git writes `diff --git`, and leaves out the `index`, `---` and `+++` lines above its first hunk. A line of the file that starts the same way stays. Copying, notes and quotes still work on the patch as git wrote it.

## Moving

| Key | What it does |
| --- | --- |
| `j` `k` / `↑` `↓` | a line down / up. The top row is the line at hand, drawn as a selection; the lines move under it |
| `gg` / `G` | the first / last line |
| `^d` `^u` | half a page down / up |
| `^f` `^b`, `pgdn` `pgup`, `space` | a page down / up |
| `[` `]` | previous / next file: the `diff --git` headers |
| `w` | wrap long lines under themselves (a minified file, a long string); again cuts them at the edge. The scroll still counts patch lines, so `j` moves a whole line however many rows it takes. On for every change until turned off; not kept between runs |
| `R` | the second reading (`:review`), above the file list: its first six rows, or all of it ([changes](../concepts/changes.md#a-second-reading)) |
| `v` | choose lines to copy (below) |
| `esc` | back to Home, where the change's row is; back to the log when a line of it opened the change (below) |

### From a line of the log

The log of a worker (its focus) has a line for every file it changed, `Δ change  changed src/parser/manifest.rs`. `enter` on such a line opens the change at that file: the view starts at the file's header, not at the top of the whole change, the file is unfolded if it was folded, and the header says ` ← the log's line` so that you see at a glance which lines you asked for. The file's lines are lit, softly, in the theme's accent, a step calmer every 300 ms for 1.5 s; then the faintest step stays until your first key, so the lines are still marked if you looked away. Nothing blinks. With `:motion off` or in a terminal that does not have the focus nothing fades: the lines have that faintest light from the start, until the first key. In a theme whose colours cannot be blended (`mono`, the terminal's own sixteen colours) nothing is lit and the words on the header mark the file. The words stay while you read that change; `esc` goes back to the log, at the line you came from.

The log has one line per file, however many times the worker edited it, so what is lit is the file's whole part of the change. When no change of the worker's waits yet (it is still mid-turn, or the change was applied), the line says so: `claude-1 shows no change yet: src/a.rs opens here once one waits`. When the change no longer has the file (the edit was undone), it says `src/a.rs is not in the change claude-1 shows now`. On such a line the raw stream is `t`; on every other line `enter` opens it as before.

Added and removed lines have their own colours; headers stand out. Every line goes through the cleaning everything drawn gets.

## Deciding

| Key | What it does |
| --- | --- |
| `a` | approve: the change is applied to your checkout, staged, for you to look over and commit. With hunks chosen (`x`), only those. Opened from [Home](home.md), the next change waiting opens; after the last, Home again |
| `r` | send back: the item closes, the change stays in the worktree, the composer opens `@worker ` so you say why |
| `c` | a note on the top line |
| `n` | send the notes to the worker in one message; the change goes back |
| `x` | choose the hunk at the top; `a` then applies only the chosen hunks |
| `>` | write to the worker with the hunk at the top quoted |
| `b` | after a refused apply: send the clash back (the change with what clashes); it goes back as with `r` |

### Applying

`a` only starts the apply: the flash says it is under way and the journal says how it went. Git runs on a thread of imece's own, so the screen never waits. The patch goes in with `git apply --index` after `update-index --refresh`; when that fails for a reason other than your own unstaged edits, it is merged three-way into a copy of the index first and applied for real only when the copy is clean, so a conflict is refused as `conflicts with the checkout in a, b` and nothing is half applied. Your own unstaged edit to a file the change touches is refused with git's words (`does not match index`): no merge goes over your work. After the apply the worktree is committed as a mark (author `imece`), so the worker's next `done` shows only what came after, and a clean worktree follows your HEAD once you commit. The worker hears of it at its next prompt.

### Sending a clash back

When git refuses a change, imece also finds which of its hunks do not fit your checkout: a copy of your index, the same plain and three-way tries as the apply, nothing of yours touched. The journal line then ends `· b in the change sends it back`, the change opens under a red line, and the status line has `b  send back`:

```text
 ✗ does not fit your checkout at 4f3a2c1: CHANGELOG.md · b sends it back to claude-1
```

`b` sends the worker one message: HEAD's short id, what to do (bring its worktree up to that commit with its change on top, still not committed, resolve, send `done` again), and each clash under its file, with its `@@` line and git's words. A file staged in your checkout and not committed is named, with the command that shows it. The item closes as with `r`, the journal says rejected, the worktree keeps the change, and the worker's next `done` is a new `Δ`. imece resolves nothing itself and does not move the worktree.

`b` says why and leaves the `Δ` as it is when nothing clashes (your own unstaged edits stop an apply and clash with no hunk: commit or stash them and press `a`), when the worker has gone or stopped, or when the budget is spent. What goes is what the last `a` found: press `a` again if your checkout has moved since. It tries at most 200 hunks and then says so.

Refused: `a` while that change is being applied, or while a newer `done` of the same worker is still being read.

### Rejecting

`r` closes the item and leaves the worktree as it is; the composer opens `@claude-1 ` for your reasons, and the worker gets them with a note from imece that its change was rejected. Its next `done` is a new Δ.

### A proposal on Lonca

A worker in a Lonca workspace has a proposal for a Δ ([more](../concepts/changes.md#on-lonca)). Above the patch it says why it needs you, in words (`needs your yes: Lonca does not watch this agent's tools, so a person reads each change · proposal 5`, or `it changes a critical path: Cargo.toml`, `it deletes or skips tests`, `2 people must say yes`; a session whose hooks did not run says `this session's hooks did not run`, and imece has stopped its worker ([more](../concepts/projects.md#on-lonca)); a reason imece has no words for stands as Lonca wrote it; `it takes the place of its earlier proposal` when the worker proposed again while one waited), and loud lines for files Lonca redacted or a patch it cut; the verdict line says what runs after your yes. `a` approves it. `r` opens the composer at `@claude-1 ` and flashes `say why`; the line you send rejects it and goes to the worker, and `esc` leaves it waiting. Notes sent with `n` are a rejection the same way. `x` says `a proposal is approved whole`, and `b` has nothing to send: a clash is the queue's, and it tells the worker itself.

### Notes on lines

`c` on a line of a hunk (context, added or removed; not a header or an `@@` line) opens the command line for a note, labelled with its place, `note on src/parser/manifest.rs:14 ▸`. `enter` keeps it, `esc` gives it up, `c` on a line that has a note writes it again, an empty note takes it away. No names or commands complete while a note is written, and the footer says `enter keep`, not send. A note shows under its line after `✎`. A place is the file's new name and the line's number there, or for a removed line its number in the old file with a minus, `src/p.rs:−7`.

`n` sends the change's notes, in line order, to its worker in one message: a line saying the change stays in its worktree, not applied, to change as they say and send `done` again; then each note under its place and the line itself in a `diff` fence. The change goes back as `r` sends it back, so the journal says it was rejected and the message follows; the flash counts the notes. With no notes `n` says how to write one. Notes live in the screen's memory for the run, kept against the patch by its hash: a newer Δ from the worker has other lines, so older notes are not shown or sent with it.

### Part of a change

`x` chooses the hunk at the top (the one the top row is in; on a file's header, the next one); `x` again lets it go. A chosen hunk's `@@` line wears `✓` and the flash counts them: `1 of 3 hunks chosen; a applies only them, the rest waits`. With hunks chosen, `a` applies only those; with none, the whole change.

On the git thread the part becomes a change of its own before anything is applied: the chosen hunks under their file's header (a file with none is left out, and so is a binary file, which has no hunks), read through an index of imece's own so the worker's files and index are not touched, and applied like a whole change, three-way when clean. The worktree is marked with only that part, and the rest is read again at once and waits in TO DECIDE as a change again under the same title. The worker hears that part of its change was applied (`1 of its hunks`) and that the rest waits. A part that does not fit is refused like a change that does not apply. The choice is kept against the patch's hash for the run; a newer Δ starts with none. Single lines inside a hunk cannot be chosen.

### Quoting a hunk

`>` starts a message to the change's worker, `@claude-1 ` with the cursor after it, and under your line a `diff` fence with the file's `---` and `+++` lines and the hunk at the top: at most 40 lines, the rest counted as `… +N` (the worker has the whole hunk in its worktree). You edit all of it before `enter`; it is a plain message, journaled as said. With no hunk to quote (an empty change) the flash says so.

### Copying lines

`v` chooses lines to copy, from the line at hand: the title becomes `COPY · 3 chosen · y as written · Y the new side · v esc leave` and the footer lists only these keys. `j` `k`, the page keys and `gg` `G` move the cursor; the lines between where you started and the cursor are marked, the cursor's in bold, and the view keeps where you started at the top while the lines fit, else it follows the cursor as little as it can.

| Key | What it does |
| --- | --- |
| `y` | copy the lines as the patch has them, each with its `+`, `-` or space |
| `Y` | copy the new side: only the hunks' added and kept lines, without their mark, ready to paste as code. Removed lines, file heads and `@@` lines are left out; with nothing left the flash says so and the lines stay chosen |
| `v` `esc` | leave without copying |

Other keys do nothing while you choose, so `a` does not apply the change by mistake; `?` and `q` still work. The copy goes to the system's clipboard or the terminal's (OSC 52), as `c` in a conversation does, and every line is cleaned first. A newer Δ from the worker, or leaving the change, ends the choosing.

## What the journal says after

```text
14:05 › you applied claude-1's change (3 files) to the checkout, staged; worktree mark 7f3a2c1
14:06 ■ claude-1's change did not apply: conflicts with the checkout in CHANGELOG.md
14:07 › rejected: claude-1
14:07 › you → claude-1: the empty-file check belongs in `read`, not in the caller …
```

A `done` arrives as `claude-1 finished: change ready · 3 files +41 −6`; an apply as `you applied …, staged; worktree mark <sha>`; a failure as `… did not apply: <git's words>`, and the item stays; a rejection as `sent back: claude-1`, followed by your message. After a partial apply the rest comes back as a new `finished` line.

## Demo

`--demo` makes the same changes from its cards and runs, and takes `a`, `r`, `n`, `x`, `b`, `>` and `v` the same way (one apply in eight fails there, to show the failure). A worker hired on the Lonca project proposes, and approving lands on the demo's trunk or, one time in eight, comes back as a merge the queue could not make. Every demo checkout has a `.imece/check`, asked about at the first change and failing on every third; each change says what its made-up spending came to. On a demo project on Lonca a landing says its checks in the journal, and each fifth fails its tests and the worker takes it up.

## See also

- [Changes](../concepts/changes.md)
- [TO DECIDE](waiting.md)
- [Reviewing with notes](../scenarios/review-with-notes.md)
- [Own shell and commit](../scenarios/own-shell-and-commit.md)
- [Keyboard](keyboard.md)
