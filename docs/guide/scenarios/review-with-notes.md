---
title: "Sending a change back, then taking half of it"
description: "The review keys in depth: notes on lines sent back in one message, hunks chosen and applied alone, a hunk quoted in a question, the rest rejected with a reason."
---

# Sending a change back, then taking half of it

A change (`Δ`) has more answers than apply or reject. You can note its lines and send the notes back in one message; you can take some of its hunks and leave the rest waiting; you can quote a hunk into a question. You meet these when a change is nearly right. This scenario reviews one worker's change twice.

`claude-2` holds card `#4 rate limiter` and has just sent `done`.

## 1. A change of three files

```text
 WAITING FOR YOU (1)
 Δ 15:02 claude-2 finished: token bucket in src/limit.rs, wired in lib.rs, tests (3 files, +45 −6) · no run on this tree
   enter see the diff · a apply to the checkout · r reject
```

`no run on this tree` says the worker ran nothing through imece's `run` tool on exactly this tree; whether it builds is not known. Press `W` (from anywhere) or `enter` on the item:

```text
 CHANGE · claude-2 · 3 files +45 −6 · token bucket in src/limit.rs, wired in lib.rs, tests
 no imece run on exactly this change: whether it builds and passes its tests is not known
 source +27 −6 (2) · tests +18 −0 (1)
 src/limit.rs +22 −6
 src/lib.rs +5 −0
 tests/limit.rs +18 −0
 diff --git a/src/limit.rs b/src/limit.rs
 @@ -1,6 +1,12 @@
 …
```

Above the patch: the runs (none here), then, because the files are of more than one kind, a line with the lines per kind, then the file list. Kinds are told by the path alone: `tests/`, `test_*` and test suffixes are tests; lock files, `.min.js`, `generated/`, `dist/`, `vendor/` are generated; the rest is source. A change of one kind shows no kinds line; a change of one file shows no file list. Six files are listed at most, the rest counted (`… 2 more files`). `[` `]` jump from file to file in the patch.

## 2. Notes on two lines

The top row of the view is the line at hand, drawn as a selection. Scroll with `j` until the line you want is at the top and press `c`. The command line opens, labelled with the line's place:

```text
 › note on src/limit.rs:12 ▸ 
```

Type `use Instant, not SystemTime: the clock may go back` and press `enter`. The flash says `note kept (1 on this change); n sends them` and the note shows under its line, after `✎`. Scroll to the second place and `c` again. A note on a removed line names its line in the old file with a minus sign (`src/limit.rs:−7`). `c` on a line that has a note rewrites it; an empty one takes it away. `c` off a hunk's lines (on a header, an `@@` line) says where a note goes.

Notes live in the screen's memory against this patch: a newer `Δ` from the worker has other lines and starts without them.

## 3. Send them with `n`

Press `n`. The flash says `2 notes went to claude-2; the change went back to them`, and the item closes. The worker gets one message from you, each note under its `file:line` and the line itself:

````text
Notes on your change, line by line. It stays in your worktree, not applied: change it as they say and send done again.

src/limit.rs:12
```diff
+    let now = SystemTime::now();
```
use Instant, not SystemTime: the clock may go back

tests/limit.rs:9
```diff
+    assert!(bucket.take(1));
```
also assert the refill after a sleep of the period
````

The change goes back the way `r` sends it back: the journal says `rejected: claude-2`, the worktree keeps the change, and the worker's next `done` is a new `Δ`. With no notes, `n` says `no notes on this change yet: c writes one on the top line`.

## 4. The worker answers

The worker reads the notes at its next turn and works. Its messages on the bus are numbered, and a reply names what it answers. It asks the leader about the refill period and the leader answers; its next `done` replies to the assign that gave it the card. Open their conversation with `enter` on one of its lines in the journal (`v` on a row opens that agent's conversation with you):

```text
 CONVERSATION · lead ⇄ claude-2 · 6/6
 15:02 lead → claude-2 [assign]: #4 rate limiter: token bucket, config v2
 15:20 claude-2 → lead [ask]: period in ms or s in config v2?
 15:21 lead → claude-2 [answer] ↩: ms, as the other durations
   ↩ re 12: period in ms or s in config v2?
 15:31 claude-2 → lead [done] ↩: Instant-based bucket; refill test added
   ↩ re 9: #4 rate limiter: token bucket, config v2
```

`↩` after the type marks a reply; the dim line under it is the first line of what it answers. `%` goes from a reply to the message it answers and from a message to its first reply; `[` `]` go from turn to turn. The `done` makes the next `Δ`:

```text
 Δ 15:31 claude-2 finished: Instant-based bucket; refill test added (3 files, +51 −9) · ✓ ran clean on this tree: cargo test
```

## 5. Choose two hunks with `x`

Open it with `W`. Scroll so a hunk's `@@` line, or any line in it, is at the top and press `x`. The hunk's `@@` line wears `✓`, and the flash counts: `1 of 4 hunks chosen; a applies only them, the rest waits`. Choose a second: `2 of 4 hunks chosen; …`. `x` again on a chosen hunk lets it go (`no hunk chosen; a applies the whole change` when none is left). On a file's header, `x` takes the file's first hunk.

```text
 diff --git a/src/limit.rs b/src/limit.rs
 @@ -1,6 +1,12 @@ ✓
 +use std::time::Instant;
 …
 @@ -40,8 +46,19 @@ impl Bucket { ✓
```

The choice is kept against this patch's hash, so a newer `Δ` starts with none.

## 6. Apply only them

Press `a`. On imece's git thread the chosen hunks become a change of their own (`git apply --cached` into an index file of imece's own, so the worker's files and index are not touched), applied to your checkout like a whole change, three-way when clean, and staged:

```text
 15:33 ✓ you applied claude-2's change (1 files) to the checkout, staged; worktree mark 7c0e4b2
```

The worktree is committed at the mark with only that part, files left as they were. What the worktree holds against that new HEAD is read again at once and waits as a change under the same title:

```text
 Δ 15:33 claude-2 finished: Instant-based bucket; refill test added (2 files, +30 −3) · no run on this tree
```

The worker hears at its next prompt: "The person applied part of your change (Instant-based bucket; refill test added) to their checkout: 2 of its hunks. Your worktree is committed at 7c0e4b2 with only that part; your files are as you left them, and the rest is not committed and waits for the person as a change again." A part that does not fit is refused like a change that does not apply, and the change still waits. Lines inside a hunk cannot be chosen; the hunk is the unit.

## 7. Quote a hunk in a question

Open the rest with `W`. Scroll the hunk in `src/lib.rs` to the top and press `>`. The command line opens as a message to the worker with the hunk quoted under your line, the cursor after `@claude-2 `:

````text
@claude-2 
```diff
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -88,6 +88,11 @@ pub fn build(cfg: &Config) -> App {
+    let limiter = Bucket::from(cfg.limit.clone());
```
````

Type the question above the fence, `why clone here, Config is already by reference?`, and `enter`. It is a plain message to the worker, journaled as said. At most 40 lines of a hunk go in, the rest counted as `… +N`; the worker has the whole of it in its worktree. With no hunk at the top (a binary file), the flash says `no hunk here to quote`. In the diff view `>` quotes; `.` still narrows the side panel.

## 8. Reject the remainder with a reason

Back in the diff (`W`), press `r`. The flash says `rejected claude-2's change; tell it why` and the command line opens with `@claude-2 ` for the reasons:

```text
@claude-2 drop the lib.rs wiring: the app builds the limiter itself in main; keep the test
```

The journal says `rejected: claude-2`. The change stays in the worktree, not applied; the worker is told "The person rejected your change (…). It stays in your worktree, not applied; they say why themselves. Do not send it again as it is." Your message follows. `r` is refused while that change is being applied (`claude-2's change was not rejected: their change is being applied`), and `a` while a newer `done` of the worker's is still being read.

## See also

- [Changes](../concepts/changes.md): reading, applying, the mark, three-way apply
- [The change view](../screen/changes.md): every key of the diff view
- [Conversations](../screen/conversations.md): `↩`, `%`, `[ ]`
- [Messages and the board](../concepts/messages-and-board.md): message numbers and `re`
- [A failing test, one worker](one-fix.md)
