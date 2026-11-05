# How far behind trunk, and `:sync`

The second half of record 115's step 3, with Lonca at `832c161`. Trunk moves in
Lonca's store and the person's git checkout is a client of it (lonca#83), so the
person needs to see how far behind it is, and to bring it up with one word.
Nothing here decides anything about a change: this is a line and a fetch.

**The line.** A project on Lonca says `lonca kule 693d629 ↓3 · 2 pending`: three
integrations since the checkout left trunk. Nothing when level. `↓?` when its
commit never was trunk and nothing was fetched yet, so imece can not say. Bare
`:lonca` says it in words, and `:sync` shows it in the `:` menu.

**How it is read.** `StoreSeen::Up.behind` (`Behind`: `Unknown`, `Never`, `By`),
in the look (`behind_of`), for a project with git. The daemon's `trunk.distance`
counts integrations since a commit was trunk and answers `null` for a commit that
never was (probed: any commit of the person's own, on top of trunk), so asking
for HEAD would turn every local commit into `↓?`. So the base is
`worktree::near`: `git merge-base HEAD refs/lonca/trunk`, where the checkout left
trunk, else HEAD when nothing was fetched yet; level with trunk is said without
a call. What can not be read is `Unknown`, and shows nothing. This is deliberately
softer than the look's own status: **how far behind closes nothing**; only
`status` decides `store_closed`.

**`:sync`** runs `lonca fetch --repo=NAME --into CHECKOUT` (with `=`: a valid name
may start with a dash), which puts trunk in `refs/lonca/trunk` through the daemon,
then `worktree::fast_forward`: nothing when level (`Even`), `git merge --ff-only`
when HEAD is an ancestor of it (`Moved`), and when the checkout has commits of
its own, nothing (`Apart`): the journal says trunk is fetched and nothing moved,
since what to do with two histories is the person's. imece never merges, rebases
or resets the person's branch. A fast-forward that local changes stand in the way
of is git's refusal, shown in the journal; probed: an unrelated staged change
survives a fast-forward, and an overlapping edit stops it with the work intact.

Refused at once, in words: a project on no repository, a folder without git, a
store that does not answer (the same word as bare `:lonca`), a `lonca` command
under way. It takes the same `StoreJob` path as record 117: one at a time, a
thread of its own, the bottom line spinning, 2 minutes at most, the store asked
at once at the end.

**What a real daemon said.** `--ignored live_the_program_imports_signs_in_and_fetches`
runs the real `lonca` against a new daemon (by hand; CI has no daemon): a login
before any repository refused, an import under a name that starts with a dash,
then login, `status`, `trunk.distance` of trunk (0) and of a commit it never had
(`null`), `fetch`, `near`, `Even`, and an import of a name that exists refused.

**After `lonca propose --commit`** trunk has a new commit with the same tree, so
HEAD is not its ancestor and `:sync` says `Apart`, which is true: Lonca's guide
says fetch and merge or reset, and that stays the person's. imece does not use
`propose --commit`; the Δ moves in box 4.

**The demo** (record 31). `Sim::demo_sync` on the store's table: a row keeps how
many integrations the checkout has, so `↓N` grows as changes land and `:sync`
closes it; every third `:sync` finds commits of the person's own (`Apart`, then
`↓?` until the next one), so all three lines can be seen. What can not be faked:
git itself, and `refs/lonca/trunk`.

**Read cost.** One `git merge-base` and at most one `trunk.distance` a look, per
project, on the watch thread, every 5 s: never on the screen's thread, and a slow
git costs that look alone. **Concurrency.** Only the job touches the checkout's
branch, and only with `--ff-only`, which git refuses when the index or the work
tree is in the way; a worker's Δ landing at the same moment takes git's index
lock, and the loser fails with git's words. `refs/lonca/trunk` is Lonca's name,
in a namespace no other tool writes. **Growth.** A journal line a `:sync`; the
ref is one ref. **Ten times the scale.** Ten projects are ten `merge-base`s every
5 s on one thread; `next_seq` still lets a later box skip a look that found
nothing. **Migration.** `behind` is memory only, read anew each run; new journal
events, no schema change. **Failure.** A fetch that fails moves nothing, a
fast-forward is atomic in git, and a checkout gone mid-look is `Unknown`.
