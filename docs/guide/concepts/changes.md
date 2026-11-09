---
title: Changes (the Δ gate)
description: How a worker's work reaches your checkout - its done becomes a change in TO DECIDE, which you apply, reject, annotate or apply in part; what the worktree does meanwhile.
---

# Changes (the Δ gate)

A change (`Δ`) is what a worker's worktree holds when the worker says `done`. It waits for you in TO DECIDE, and nothing of it reaches your checkout until you apply it. This is the gate between the agents and your working copy, and you are the one who opens it.

## From `done` to Δ

1. A worker (role Worker only; a helper's `done` is its worker's business, a leader changes nothing) sends `done`.
2. On imece's git thread, the worktree is read through a private index file: `read-tree HEAD`, `add -A`, `write-tree`, `diff --cached --binary` against its HEAD. Untracked files count; the agent's own index is untouched. A patch over 4 MiB is refused and the worker told.
3. The result is a `Δ` item in TO DECIDE, its detail the `done`'s first line, and the journal says `claude-1 finished: change ready · 3 files +34 −8`. A newer `done` from the same worker supersedes the older change.
4. The worker's `send` answer waits for the read, and tells it that you see the change.

```text
 TO DECIDE (2)
 Δ 14:02 claude-1 finished: parser tokens (3 files, +34 −8) · ✓ ran clean on this tree: cargo test -p parser · $0.84 · 12 min
 ? 14:05 claude-2 asks you: keep the old CLI flag?
  enter see the diff · a approve · r send back
```

### Evidence on the line

A run of the worker or a helper through imece's `run` tool is tied to the tree it ran on (the worktree is read before and after; a run that changed the tree counts for none). When a `done` is read, the runs on exactly its tree go with it: the `Δ` line ends with `✓ ran clean on this tree: …`, `✗ failed on this tree: …` or `no run on this tree`, and the verdict line above the diff names each run with how it ended. It is shown, never a gate: a change with no run can still be applied; you decide. Claude Code's own shell does not count, since imece does not see what tree it ran on.

### The repository's check

Put a script at `.imece/check` in your checkout (a shell script, approved like the [worktree setup script](projects.md#a-worktree-setup-script)) and imece runs it in the worker's worktree every time a change is read from it, **once you said yes to its exact bytes**: `/bin/sh` on a private copy of the bytes you read, with the cleared environment plus `IMECE_REPO` and `IMECE_WORKTREE`, in its own process group, for the `run` tool's deadline (`--run-timeout`, ten minutes by default). What it came to goes on the change: `✓ check` or `✗ check · exit 1, 38.7 s` first in its verdict line, with the last lines of its output under it, a mark before the evidence on the `Δ` row in TO DECIDE (`✓ check ·`, `✗ check ·`, `● check ·` while it runs), and a journal line (`claude-1: check failed on its change, exit 1, 38.7 s · Tests: 1 failed, 211 passed`). It is a note, never a gate: `a` applies a red change as it applies any other; you decide.

- **Your yes:** the first change that finds a check you have not said yes to puts an ask in TO DECIDE (`▶ claude-1: run the repository's check .imece/check on its changes? enter shows it`); the change waits unchecked meanwhile and says so above its patch. `enter` shows the bytes, `a` says yes (it runs at once on every change that waited, then on each one read), `r` no for this run. The yes is bound to the bytes' SHA-256, the file's real path and `/bin/sh`, kept in imece's store; a changed byte asks again. Asks close at the next start; the next change asks again.
- **Once per tree:** a change read again with the same tree (sent back and sent again as it is) gets the result it had, without a run; a changed tree runs again. A check that was running, or asked about, when imece ended runs or asks again at the next start.
- **What it must not do:** change tracked files. The change is read before the check runs, so what the check writes is no part of it and would show up in the worker's next `done`; build output under paths git ignores is fine.
- **The worker is not told.** The result is yours: `n` sends your notes back, and a failed check's last lines make a first note. Whether a red check should hold a change back for one round, with the worker told first, is a question for a later record.
- Without a check in the repository nothing runs and nothing shows; one that cannot be read (not a plain file, over 64 KiB) says so on the change and in the journal.

### What it cost

Every change says what the worker spent on it, so you can judge the leader's hiring (a strong-tier worker on a rename) or a worker that burnt $4 on a one-line fix: `$0.84 · 12 min` on the `Δ` row in TO DECIDE, `cost · $0.84 · 12 min` above the diff, in the journal (`claude-1's change cost $0.84 · 12 min`) and so on the card's screen and in `:why`. The figure is the worker's spend, its helpers' counted in, from its first prompt after its last change to this `done`, and the time it worked (not the time the change waited for you); a `done` comes mid-turn, so the figure appears a moment later, when the turn's cost is known. A change sent back (`r`, `n`, `b`) and sent again adds up: `$1.20 · 25 min · 3 rounds`; a newer `done` that takes an older change's place carries its cost too, and so does the rest of a change you applied in part. Once a change is applied whole, the next one starts from nothing. The leader hears it in the worker's `done`, which it gets once the worker's turn ended, headed `[done 12 · $0.84]`, so it learns what its tiers cost. A solo's change at each turn's end says the same. What subscriptions would have cost at API prices, and a price per applied change across a project, are not here.

## Reading it

`enter` opens the whole diff: `j k` scroll, `^d ^u ^f ^b` pages, `gg G`, `[ ]` file to file. Every line is cleaned before it is drawn. Above the patch:

- the verdict line: `✓ ready to approve` or `! look first`, then what proved it (the repository's [check](#the-repositorys-check) when it ran, the runs on its tree, or that nothing ran), its files against the paths its card owns, what other waiting change it overlaps, a refused apply, a patch Lonca did not show whole, and what Lonca runs after your yes ([the change view](../screen/changes.md#what-is-above-the-patch));
- the check's state while it has not ended: that it runs, or that it waits for your yes;
- when the files are of more than one kind, the lines per kind: `source +20 −1 (2) · tests +18 −0 (1) · generated +7 −5 (1)`. Kinds are by path alone: lock files, minified and generated code and `generated/`, `dist/`, `vendor/` directories are generated; `tests/`, `test/`, `__tests__/`, `spec/`, `test_*` and the usual test suffixes are tests; the rest is source. Generated wins over tests;
- for a change of two files or more, the file list with `+N −M` each (or `binary`), six at most, the rest counted.

`w` wraps long lines under themselves (`w` again cuts them at the edge). The top row is the line at hand: `c`, `>` and `x` act on it.

`h` (or `←`) folds the file of the line at hand to its `diff --git` header, which then says how many lines it hides (`▸ 42 lines`); `l` (or `→`) opens it, `za` turns it, `zM` (`:fold`) folds every file and `zR` (`:unfold`) opens them all. `j` `k`, the pages and `G` go over the lines shown, so a folded file is one line to step over; `[` `]` still go from header to header. Folding the file of a hidden line puts the line at hand on its header. `v` opens every file first, since lines are chosen from the whole patch. The folds hold for that patch: a new one from the worker starts open.

## A second reading

`:review` has a fresh Claude Code session read the change at hand and say what it finds; `:review on` asks for one for every change as it comes (off by default, remembered). The reader knows nothing of the worker's session; it gets what the worker said it did, the file list and the patch (64 KiB at most), and no tools. Its answer stands above the file list:

```
✎ review · sonnet · $0.04
1. src/parse.rs:41: an empty input returns Ok(()) without a test for it.
2. The title says the lexer is fixed too; the patch does not touch it.
```

six rows of it, `… +N lines · R shows all` when there is more; `R` shows it whole. Until it comes, a line says it is being read. It is advice and nothing more: no change waits for it, `a` applies with or without it, and the worker is not told (quote it with `>`, or note its lines with `c`). A review costs at most $0.10 from the pot; 20 reviews and $2.00 a day; a change that has its review is not paid for twice. The review is kept in the journal and comes back with its change after a restart. See [`:review`](../reference/commands.md#review-onoff) and [budget and limits](budget-and-limits.md).

## Apply

`a` starts the apply on the git thread; the flash says it is under way and the journal says how it went.

- `git apply --index` in your checkout, atomic: the change lands **staged**, for you to look over and commit. `update-index --refresh` runs first so a touched but unchanged file is not a mismatch.
- **Three-way when clean:** when the plain apply fails for any reason other than your own unstaged edits, the patch is merged three-way into a copy of your index (`<git dir>/imece-apply-index`), which touches no file. Only when that is clean does the real `git apply --index --3way` run. A conflict is refused as `conflicts with the checkout in a, b`; an unstaged edit of yours in the same file is refused with git's own words (no merge goes over your work). Nothing is half applied.
- **The mark:** after an apply the worktree's reviewed tree is committed there as author `imece <imece@localhost>` and the worktree is reset to it (files untouched, index = HEAD). The worker's next `done` then shows only what came after, and a worktree whose own commits are all imece's keeps following your HEAD.
- The journal: `you applied claude-1's change (3 files) to the checkout, staged; worktree mark 4f3a2c1`. The worker gets a note from imece at its next prompt ("From imece, not from the person or a teammate").

`a` is refused while that change is being applied, or while a newer `done` of the worker is still being read; `r` is refused while it is being applied.

## Send a clash back

When git refuses a change, imece also finds which files and hunks do not fit (on a copy of your index, the same plain and three-way tries as the apply). `b` in the change view sends them to the worker in one message, with HEAD's short id, git's words on each, and what to do: bring its worktree up to that commit with its change on top, still not committed, resolve, send `done` again. The change goes back as a reject does: the item closes, the worktree keeps it, the next `done` is a new Δ. imece resolves nothing. Your own unstaged edits clash with no hunk, so nothing goes back for them; a worker that has gone or stopped, or a spent budget, leaves the Δ as it is. See [the change view](../screen/changes.md).

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
- The rest is what the worktree holds against its new HEAD; it is read again at once and waits in TO DECIDE as a change again, under the same title. The worker hears which part was applied and that the rest waits.

The choice is kept against the patch's hash for this run only. Single lines inside a hunk cannot be chosen.

## The worktree meanwhile

- A worker's row says where its worktree stands: `⎇ fix-parser ↑2 ↓1 +12 −3`. `⎇ name` is a branch the agent made (none while detached), `↑N` its own commits your HEAD lacks (imece's marks not counted), `↓N` your HEAD's commits it lacks, `+N −M` the lines its next `done` would hand you, untracked files included. Each part shows only when there is something to say; read as the session starts, when a turn ends, after an apply, after a move, and every 15 s while mid-turn. Not kept between runs.
- **Following HEAD:** a clean worktree whose own commits are all imece's is moved to your HEAD before every prompt of a worker that is between turns (helpers too), journaled `worktree moved from … to HEAD …`. One with uncommitted changes or commits of its own is kept, and the journal says so. So once you commit what you applied, the workers go on from your new HEAD. A worker hired from a start point of its own (`hire`'s `from`: a branch, a tag, a commit, `#N` a pull request) never follows: its worktree stays where it started, its change is against that point, and applying it is the same three-way apply ([team](team.md)).
- A worktree removed by hand that git still lists is refused with what was lost and `git worktree prune`, and the agent stops with that reason.

## Committing

imece commits nothing in your checkout. `:commit` asks Claude's cheap tier for a message for what is staged and puts `!git commit -m '…'` on the command line for you to edit and run in your own shell; it has ceilings of its own ([budget](../concepts/budget-and-limits.md)) and is walked through in [own shell and commit](../scenarios/own-shell-and-commit.md).

## On Lonca

A worker in a [Lonca workspace](projects.md#on-lonca) has no worktree to read. Its change is a proposal the daemon holds, and a `done` with changes no proposal carries is refused with `propose first`, so it never closes its card. Its Δ is that proposal: imece looks every few seconds, and once the worker's turn is over its newest proposal waits for you, read from the store. A workspace proposes all it holds, so the newest holds what the older ones did, and Lonca withdraws the one waiting when its workspace proposes again: the journal says `claude-1: proposed again; the newer proposal takes the place of the one you saw`, and the Δ becomes the newer one once the worker's turn is over.

`a` approves it and `r` sends it back (asked why first), both signed by the `lonca` program, so a Secure Enclave key wants your finger; a word that did not reach Lonca is said in the journal and the Δ still waits. Lonca takes no rejection without a reason, so `r` opens the line first and what you send is both the reason and your message to the worker; leaving the line with `esc` rejects nothing. A proposal is approved whole: `x` is refused, and nothing lands in your checkout, which [`:sync`](../reference/commands.md#sync) brings up to trunk.

What Lonca does next is said whoever decided, `lonca approve` in your own shell included: `on trunk as 174cfc1`, or a merge the queue could not make, or a refusal with its reasons. The checks of the merge queue go in the journal too: `claude-1: Lonca's checks · ✓ build exit 0, 12.4 s · ✗ tests exit 101, 31.0 s · its last line`. A worker sent back is woken, told to sync and propose again, with what the failed check printed; one on trunk hears it at its next prompt. A repository whose policy would let changes in unseen is closed: see [projects](projects.md#on-lonca). What it cost stays imece's and is carried into the next proposal until one lands.

### What is checked on Lonca

`.imece/check` does not run in a workspace. A repository on Lonca says what to run in `[verification]` of its `lonca.toml`, and Lonca's merge queue runs it on the merged tree after your yes, in a sandbox; imece runs nothing. The Δ's verdict line says which it is, as Lonca says it for that change: `after your yes Lonca runs: build, tests`, `nothing runs after your yes`, or `Lonca did not say what runs after your yes` (an older Lonca, or a policy it can not read: nothing promises a check then). `extended` is `on_signature_change`, for a change that reaches other code; a named check of the policy (`verification.checks`) shows by its name, here and in the journal line of what ran. The settings are signed, so changing them is `lonca policy set`, by you:

```toml
[verification]
build = "cargo build --offline"
test = "cargo test --offline"
required = true
env_passthrough = ["HOME", "CARGO_HOME", "RUSTUP_HOME", "TMPDIR"]
```

The commands see an empty environment but for `PATH`, a scratch `HOME` and `LANG`, so a toolchain under your home needs its directories in `env_passthrough`. With a check that fails the change does not land: Lonca refuses it, your yes stays in the log, and the worker takes it up.

A card may ask for more of the policy's named checks ([the checks a card asks for](messages-and-board.md#the-checks-a-card-asks-for)): its change's verdict line adds them (`after your yes Lonca runs: build, tests, api · the card also asks for api`), and your yes asks Lonca for them (`--also`).

## See also

- [The changes screen](../screen/changes.md): every key in the diff view.
- [TO DECIDE](../screen/waiting.md): `W`, `S`, the order of items.
- [One fix](../scenarios/one-fix.md) and [review with notes](../scenarios/review-with-notes.md).
- [Security](../concepts/security.md): why the gate is yours.
