# Where a worker's worktree stands

Backlog item B5 of #55 (2026-10-31, from herdr and Warp): a worker's
salon row says where its worktree stands, `⎇ fix-parser ↑2 ↓1 +12 −3`.

What the parts mean, given that imece makes every worktree detached at
the checkout's HEAD (record 02):

- `⎇ name`: the branch the worktree is on. Only when an agent made one
  (`git checkout -b`); a detached worktree shows none. The name is the
  agent's text, so it goes through `fit` (cleaned, cut at 16).
- `↑N`: commits of the worktree's own the checkout's HEAD does not have.
  imece's marks of applied changes (author `imece@localhost`, record 11)
  are not counted: the checkout has their content already, staged.
- `↓N`: commits of the checkout's HEAD the worktree does not have. A
  clean worktree follows the checkout before its next prompt (record
  11), so this mostly shows while it works or when it holds work of its
  own.
- `+N −M`: lines added and removed against the worktree's own HEAD,
  untracked files included: what a `done` would hand the person now.
  After an apply it drops to nothing (the mark is the new HEAD).

Each part shows only when there is something to say; a worktree at HEAD
with nothing written shows nothing. Leaders, helpers and researchers
have no part: a helper works in its worker's worktree, so it counts on
its worker's row. The help's legend names the glyphs in both languages.

How it is read: `worktree::stand` (imece-agent) does `rev-parse`,
`symbolic-ref`, `log --format=%ae checkout..HEAD`, `rev-list --count
HEAD..checkout`, and `diff --cached --numstat HEAD` through an index file
of its own (like `change`, no patch made, the agent's index untouched).
It is a job on Live's git thread (`GitJob::Stand`), never on the
screen's loop, and one at a time per worker (`Live::standing`); asked
again while one is under way, one more read follows it, so the row ends
with what the worktree holds after the last ask. A failed read keeps the
last numbers and writes a trace line.

When it is read: as a worker's session starts (`ready`), when a turn of
the worker or one of its helpers ends, after an apply and after git
placed the worktree before a prompt, and while the worker or a helper is
mid-turn every 15 s (`STAND_EVERY`), so the lines grow as they are
written. Not kept in the store: the first read at a session's start
fills it again, and an asleep worker shows nothing until it wakes.

Demo: `sim.rs` counts lines on each `Edit` (from the clock, not the
random stream, so a seed's scenario stays the same), sets them to the
change's counts at `done`, drops them at an apply, and a worktree kept
at the session's start shows `⎇ wip-NAME ↑ ↓` (with commits) or `↓ +N
−M` (with changes), numbers taken from the commit ids.

Known gaps: `↓` counts the checkout's commits since the worktree's
base, also those that only commit what the person applied from this
worker (until the worktree follows at its next prompt). A 15 s read
costs an `add -A` of the worktree, as reading a change does.
