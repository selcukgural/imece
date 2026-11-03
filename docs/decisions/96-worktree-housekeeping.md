# Worktree housekeeping: sparse checkouts

Issue #245 (from the study of agent-deck), its first half. On a large
monorepo the person keeps part of the tree checked out
(`git sparse-checkout`); a worker's worktree that checks out everything
writes what they left out, often gigabytes, for every worker.

A checkout is sparse when `git config --type=bool core.sparseCheckout`
says true there (per-worktree config included); its patterns are what
`git sparse-checkout list` gives, and `core.sparseCheckoutCone` says
whether they are cone mode's directories (`worktree::sparse_of`).
`worktree::ensure` then adds the new worktree with `--no-checkout`, runs
`git sparse-checkout set --cone|--no-cone --stdin` in it with those
patterns, and checks it out with `reset --hard HEAD`
(`worktree::make_sparse`): nothing left out is ever written. The
patterns go into the worktree's own sparse-checkout file (git turns on
`extensions.worktreeConfig` for it), so the person's checkout keeps its
own; a later change on either side does not reach the other. A worktree
that was there already is reused as it is.

Probe (git 2.56.0, 2026-11-03): `git worktree add` copies the sparse
settings and patterns of the worktree it runs in by itself (git 2.36
and newer), cone mode and the older `core.sparseCheckout` +
`info/sparse-checkout` alike, and checks out sparsely. imece does it
explicitly anyway, so a git that does not copy (2.35) still never
writes the rest; the `--no-checkout` is what keeps that true, and on a
git that copies it changes nothing a test can see. `set --cone` needs
git 2.35.

Failure: when `set` or the checkout fails (git older than 2.35, a
pattern `set` refuses, as `../x`), the worktree, made just now and
holding nothing, is removed again (`git worktree remove --force`) and
`ensure` fails with why: the agent does not start and its start failure
says so. Writing the whole tree instead would weaken what the person
chose without a word.

Growth, read cost: one `git config` per worktree made (two and a
`sparse-checkout list` for a sparse checkout); nothing per frame.
Migration: none; worktrees made before stay as they are.

The demo shows nothing of it: there is no git under the demo, and a
sparse worktree looks like any other on the screen.
