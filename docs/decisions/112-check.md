# The repository's check on a change

The first piece of record 111's first round (#288, 2026-11-04): a change
that reaches the Δ gate says whether it builds and passes, before the
person reads it. Record 11 shows the runs the worker made through the
`run` tool on exactly the change's tree, which depends on the worker
remembering to run them; this adds a check of the repository's own.

What runs. `.imece/check` of the person's checkout (`setup::CHECK_FILE`),
a third kind of script beside the worktree setup and teardown scripts
(`setup::Kind::Check`, records 92 and 96), on their terms: read once with
`look_for` (`O_NOFOLLOW | O_NONBLOCK`, a plain file, at most 64 KiB,
SHA-256), approved by the person for its exact bytes (key `check /bin/sh
SHA REAL` in `World.setup_approved`, never in the repository), a private
copy `state/scripts/<sha>.sh` run by `/bin/sh` (`setup::run_in`, which now
takes its deadline) in the worker's worktree with the cleared environment
plus `IMECE_REPO` and `IMECE_WORKTREE`, in the agent's own `Running`
group, for the `run` tool's deadline (`Live::run_timeout`, 600 s by
default: a build and a test run are not a setup script's 60 s), its
output kept to 8 KiB. One thread per check, as setup runs are; the ends
come back through `check_tx` with the tree they ran on and are taken in
`Live::step` (`take_checks`).

When. `changes::read`, right after the change is put before the person
(`World::change_ready`) and git's copy kept (`Live.changes`), calls
`Live::check_change`: none in the repository, nothing; the tree checked
before, its result at once (`Live.checks`, the last `CHECKED_KEEP` 32
trees' results, the newest last: a change sent back and sent again as it
is runs nothing, a changed tree runs again); approved, it runs
(`run_checks`: `Check::Running` on the change, a trace line); not
approved, `ask_script` (the setup script's `ask_setup`, now for both
kinds) puts `Check::Asked` on the change and one `InboxKind::Check` item
per script (`▶`, detail `.imece/check`, the first worker whose change
went unchecked; later ones counted on `setup::Ask.agents`); refused this
run, nothing. `decide_setup` answers both kinds: yes approves and runs
the check on every counted change still waiting (`CheckAnswered`), no is
kept for the run and the counted changes go back to `Check::None`. A
result whose change is gone or replaced (`Live.changes[id].tree` differs)
is kept for a later change with that tree and traced, not journaled
(`check_done`). A check that was asked about or running when the earlier
run ended is asked about or run again at the load (`adopt`, after
`close_scripts` closed the old ask).

What the person sees. `Change.check: Check` (`None`, `Asked`, `Running`,
`Ran(Checked { ok, end, lines })`, `Failed(why)`), set through
`World::checked`. Above the diff's runs, `ui::diff::check_lines`: `✓ check
· exit 0, 41.2 s` (ok colour), `✗ check · exit 1, 38.7 s` (bad) with the
last `OUTPUT_LINES` 6 lines of its output dim under it, `◌ the check runs
on this change…`, `▶ the repository's check waits for your yes in WAITING
FOR YOU`, `check did not run: why`; nothing for `None`. On the Δ row in
WAITING FOR YOU, `words::check_mark` before the runs' evidence: `✓ check
·`, `✗ check ·`, `◌ check ·`, `▶ check ·` (nothing for none or failed: the
diff says). The journal: `Event::Checked { ok, end, lines }` (`claude-1:
check passed on its change, exit 0, 41.2 s` / `check failed on its
change, exit 1, 38.7 s · <last line>`), `CheckFailed { why }`,
`CheckAnswered { yes }`. The ask's window is the setup script's
(`overlay::script`, titled `CHECK`), `enter a r` the same keys. The check
is a note, never a gate: `a` applies a red change as any other. The worker
is not told: the result is the person's, `n` sends notes back. A
`InboxKind::Check` item holds no mail (record 107): the worker waits for
nothing.

Not done, by choice for now: telling the worker and holding a red change
back for one round (it would start a paid turn without the person; a
later record may add it once real sessions say the person wants it);
checks per card or per acceptance criterion (voff's "done needs
evidence", `docs/neighbours.md`); a line in the worker's instructions that
the repository has a check; a check that changes tracked files (the change
is read before it runs; what it writes shows up in the next `done`).

Growth: `Live.checks` is bounded at 32; one `changes.checked` column per
waiting change; the journal's lines as any event's. Concurrency: each
check runs on a thread of its own in the worker's worktree while the
worker may already be on its next turn and writing there (the check
reads the tree as it is then, as the `run` tool does; a result whose tree
is no longer the waiting change's is not shown); two workers' checks run
at once in their own worktrees. Read cost: `check_change` reads the
script file once per change read, nothing per frame. Ten times the scale:
ten changes at once are ten shells and ten builds at once, bounded by the
deadline and the agent's `Running` group (killed with the agent, the
project and imece); a queue per project is the first thing to add if the
machine minds. Migration: schema 23 adds `changes.checked` (`''` for an
older file: `Check::None`); the save plan keys a change's row on its tree
and the check's hash, so a result that comes later is written; an older
imece refuses the file as any newer file. Failure: a script that cannot be
read or copied is `Check::Failed(why)` on the change and `CheckFailed` in
the journal, and nothing else changes; a killed or failed check is a red
note; imece ending mid-check loses nothing but the run, which the next
start repeats.

Demo (record 31): every demo checkout has a check (`DEMO_CHECK`), asked
about at the first change of a project and run on each one after a yes,
two ticks long (`CHECK_TICKS`), every third failing with
`DEMO_CHECK_SAYS`; the same `decide_setup` and the same window.
