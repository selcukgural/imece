# Worktree setup script

`.imece/worktree-setup.sh` (2026-11-03, #240, from the study of
agent-deck): a script of the person's checkout run in each new worktree
after `:copy` (record 57) and before the session starts, once the person
said yes to its exact bytes. The person's answers (2026-11-03): setup
only (imece never removes a worktree, `:forget` forgets a session, so a
teardown script has nothing to run before; it waits for worktree removal,
its own issue, #248; since record 96 `:tidy` removes worktrees no agent
owns, with no script); a worker never waits for the yes; `enter` on the ask shows
the bytes that were hashed.

Reading (`live/setup.rs`, `look`): `canonicalize` of `repo/FILE` (missing:
nothing), then the real path opened `O_NOFOLLOW | O_NONBLOCK`, looked at
as opened: a plain file, at most `SCRIPT_MAX` 64 KiB; `sha::hex` (an own
SHA-256, FIPS 180-4 vectors and every padding edge in its test; no new
dependency). The approval key (`Script::key`): `setup /bin/sh SHA REAL`,
in `World.setup_approved` (meta `setup_approved`, JSON, no schema change;
the newest last, `SETUP_KEPT` 64; a value that does not read is no yes),
never in the repository.

When (`Live::setup_plan` on the main thread at `start`, `Plan::carry_out`
on the start's thread): a worker's or a solo's own worktree only (not a
leader, not a helper, not without git), and only when `worktree::ensure`
made it just now (`Placed::Fresh | From`). Approved: `copy_of` writes the
bytes to `state/scripts/<sha>.sh` (dir `0700`, file `0600`, `O_NOFOLLOW`)
and the thread runs that copy (`run_in`: `run::run_env` with `/bin/sh`,
the cleared environment, `IMECE_REPO`/`IMECE_WORKTREE`, its own process
group in the agent's `Running`, `DEADLINE` 60 s, `OUTPUT_MAX` 8 KiB) before
the session; `Ready` carries `setup::Done` back. Not approved: the worker
starts without it and `ask_setup` puts one `InboxKind::Script` item per
script (`▶`, no card, the agent the first that went without; later ones
counted on `setup::Ask.agents`); refused this run (`setup_refused`): not
asked. The bytes asked about stay in `Live.setup_asks`; `enter` opens
`App.script` (`overlay::script`: what runs, the real path, numbered
cleaned lines in the code colour, never Markdown; `j k ctrl-d ctrl-u g G`,
`a`, `r`, `esc q`, `?`). `decide_setup`: yes → `approve_setup` and the
private copy of the asked bytes runs in each counted agent's worktree
still running (`cwds`), one thread each, ends through `setup_tx` →
`take_setups` in `step`; no → `setup_refused`. An ask's bytes are not
kept, so a start closes open `Script` items (`World::close_scripts`).

Journal: `Event::SetupRan { agent, ok, end, lines }` (`ended`: `exit N,
T s` / `killed after 60 s`; ok only exit 0 in time; `last_lines` the last
`OUTPUT_LINES` 6 non-blank lines, cleaned; the journal line shows the
last), `SetupFailed { agent, why }` (unreadable, too big, copy not
written, did not start; cleaned), `SetupAnswered { agent, yes }`. The
worker starts either way.

Demo (`sim.rs`): workers the demo's leader hires (`Extras.hired`) get a
new worktree: the demo's own script (`DEMO_SETUP`, `/demo/<project>/…`)
is asked about once per project, shown with `enter`, and on a yes "runs"
for each counted worker (`DEMO_SETUP_SAYS`); later hires run it at once;
a no holds for the run. The demo's first workers' worktrees are as old as
the demo and are not set up.

Growth: a copy per approved hash in `state/scripts` (never removed; a
script of 64 KiB at most), 64 keys in meta, a journal line per run.
Concurrency: the run is on the start's thread or one thread per agent
after a yes; the agent's `Running` lets imece's end kill it. Failure: a
run that hangs is killed at 60 s with its group; a script that cannot be
read or copied is said and the worker starts. Known gaps: a teardown
script (no worktree removal yet); the run may overlap the worker's first
turn when the yes comes after it started (the journal says when it ran);
another runner than `/bin/sh` is not offered.
