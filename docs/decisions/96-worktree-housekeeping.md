# Worktree housekeeping: sparse checkouts, worktrees no agent owns

Issue #245 (from the study of agent-deck), in two parts.

## Sparse checkouts

On a large
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

## Worktrees no agent owns

A worker's worktree is kept between runs (record 02) and imece removed
none (record 92). One whose agent imece no longer knows stays for ever:
after a crash between `ensure` and the save, a store the person removed
(`--fresh` keeps worktrees; a removed `imece.sqlite` forgets every
agent), a fork that never started. agent-deck lists and prunes such
orphans; imece does it on the person's word only.

What is one (`live::tidy::orphans_under`): a directory
`state/worktrees/<checkout>-<hash>/<name>` (links, files left out) that
`worktree::held` takes, i.e. git's top directory there and a worktree
added to a checkout (`--absolute-git-dir` is not `--git-common-dir`),
whose path is not in `Live::owned`: `places[project].root.join(
Live::tree_of(agent))` for every agent of every project, open, closed
or archived (a helper's is its worker's, a solo's `solo-<id>`; a
leader's and a git-less solo's count too: owning too much only keeps a
directory). Both sides are built from `Live.state` by the same joins, so
no path is canonicalised to compare. Gemini's private directories
(`<root>-gemini/…`) and anything git does not know are passed over. The
store's lock (one imece per state directory) makes the world the whole
truth of who owns what.

What it holds (`worktree::Held`): `git status --porcelain` lines
(ignored files not counted; they go with the worktree) and the authors
of `git log HEAD --not --branches --tags --remotes` that are not
imece's own `imece@localhost` (applied changes: in the checkout already,
as record 11 has it for following HEAD). Clean is neither. Removing
(`worktree::remove`) reads it again: without `force` only a clean one,
with `git worktree remove` (which refuses an untracked file itself);
with `force`, the person's yes to that row, `--force`. git removes a
worktree it runs in.

The screen: `:tidy` (no key; record 41's names are for keys that act,
and this is rare) opens `App.tidy: TidyWin { at, sure }` over the view
(`overlay::tidy`, after the setup script's window in `ui::draw`) and asks
the engine to look (`Live::look_orphans` / `Sim::look_orphans`): a
thread lists them and `Live::take_tidy` in `step` puts them in
`World.orphans: Orphans { found, busy, tidied }` (memory only, nothing
saved, no journal line: no agent is in it, and every `Event` is about
one). `j k g G` (and the arrows, home, end) move, `a` removes the clean
ones (none: `tidy_no_clean`), `d` the one at hand, at once when clean,
else `sure` and the foot asks `y` (any other key keeps it), `esc`/`q`
close, `?` the help. `Live::tidy(at, force)` takes indexes into the
last look's list, never a path from the screen; refused while a look or
a removal runs (`Refusal::Busy`) and for nothing asked (`Nothing`). The
removal thread looks again after, and `App::watch_tidy` flashes
`words::tidied` once (`removed …`, `not removed: name (why)`) and keeps
the selection on the list. At each start (`App::start_tidy`, from
`main` for live and demo) the engine looks once, and when it found any
and the window is not open the flash says how many (`tidy_found`);
imece removes nothing by itself.

An agent's worktree is never removed. Ownership is read when the look
starts, again when it answers (`take_tidy` drops what an agent came to
own meanwhile), and again when a removal is asked: one owned by then
stays with no why of git's (`tidy_owned`). That matters because
`worktree::ensure` reuses a worktree that is there: the next hire's
name in a fresh store is the old one's, and it takes the old worktree
over, its work too. A start that comes after the check while the
removal runs waits for it: `Live.tidy.removing` (`tidy::Removing`, a
set of paths under a mutex with a condvar) holds the paths being
removed, and a start's thread calls `wait` on its tree's path before
`ensure`, so it makes the worktree anew once it is gone instead of
reusing one being deleted under it.

Growth: the list is what is on disk and shrinks with each removal;
nothing kept. Read cost: one look at start and per `:tidy` on a thread,
five git calls per directory; the window draws the list it has.
Concurrency: the removal thread and the start threads meet only in
`Removing`; another imece cannot use the same state (the store's lock).
Failure: a removal that fails says git's words and leaves the rest of
its list going; a look that fails on a directory passes over it. The
demo has two made-up ones of its first project (`demo_orphans`, one
clean, one with work), answered on the next step, removed as live
removes them. Not done: `git worktree prune` for ones removed by hand
(record 02's message says what to run).

## The teardown script before a removal

Issue #248 (the person, 2026-11-03: "your recommendation"):
`.imece/worktree-teardown.sh` of the worktree's checkout runs in it
before `:tidy` removes it. Its other half, imece removing a worker's
own worktree (`:forget` of one with no change, or letting a worker
go), is not done, by the person's choice the same day: a clean
worktree costs only disk, `:tidy` takes the ones left behind, and
letting a worker go gets an issue of its own if it is ever needed. So
a teardown runs only before a `:tidy` removal.

The same terms as setup's (record 92), the same code: `setup::Kind`
(`Setup`/`Teardown`: the file, the key's first word), `Script.kind`,
`setup::look_for(checkout, Kind::Teardown)`; the approval key is
`teardown /bin/sh SHA REALPATH`, kept with the setup keys in
`World.setup_approved` (meta, the newest 64); what runs is the private
copy `state/scripts/<sha>.sh` (`copy_of`), through `setup::run_in` in
the worktree with `IMECE_REPO` = the checkout git names
(`worktree::held`) and `IMECE_WORKTREE`, its own group, 60 s, killed
with imece (`Tidy.running`, in `Live`'s `Drop`).

When: `Live::tidy` reads each checkout's script once, on the main
thread, before anything starts (`teardown_plan`: none, `Run(copy)`,
`Refused` after a no this run (`Tidy.refused`), `Failed(why)` when it
can not be read or copied). One not answered refuses the whole removal
with `Refusal::Asks(script)`; nothing goes, nothing is busy. The
screen keeps the removal asked (`App.teardown: TeardownAsk { script,
scroll, at, force }`) and shows the bytes in a window over `:tidy`'s
(`overlay::teardown`, the setup window's body shared as
`script_body`): `a` yes (`Live::decide_teardown` → `approve_setup`),
`r` no for the run, and the removal is asked again with the same rows
(the next checkout's script, if another is unanswered, asks next);
`esc`/`q` remove nothing (`teardown_held`). No inbox item: an orphan
has no agent to hang one on, and the person is at the window anyway.

The removal thread runs the plan in each worktree just before
`worktree::remove`; how it went (`TornDown::Ran { ok, end, last }`,
`Failed`, `Refused`) is in `Tidied.torn` and flashed by
`words::tidied` after what was removed. A teardown that fails is said
and the removal decides on its own: a clean removal reads the worktree
again, so one the script left a file in stays (`not removed: … holds
work`). The demo's checkouts have a teardown (`sim::DEMO_TEARDOWN`,
`npm run db:drop`), asked at the first removal, its run made up
(`dropped database NAME`).
