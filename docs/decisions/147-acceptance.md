# Acceptance before every release

The person, 2026-11-10: imece has acceptance criteria a release must
meet; each piece of development adds to the acceptance tests or takes
out of them what it changes; and every release runs them first, without
exception. The first real-project run (2026-11-09, the field entry
`2026-11-09-first-real-run.md`) found some thirty bugs and frictions that
the unit tests, the end-to-end tests on the stand-in and the demo had
never shown: a message mid-turn that stuck a worker for good, Python's
tests asking for a yes at every run, `key=lambda` hidden as a secret in a
Δ. Its plan lived in a temporary directory and was gone a day later.

## What is built

- **The catalogue** (`docs/acceptance/catalogue.md`): one case per thing
  a person does, in the order a run takes them, each with the records and
  `:` commands it runs, a tier (`auto`, `demo`, `stand-in`, `real`), its
  steps and what is to be seen. Cases are by area (automatic, install, the
  team on the git path, asks, sessions, solo, projects, cards, Lonca, the
  screen, the demo). Ids are never used again; a case that goes is
  retired with why.
- **What may stay out** is written down: a record or command no case
  names is under "Not covered" with its reason (the vendors replaced by
  record 109, hint letters gone with record 125, record 111 as a
  direction).
- **`tools/acceptance.py check`**, in CI's `acceptance` job on every pull
  request: every case well formed, every decision record and every `:`
  command (`complete::COMMANDS`) named by a case or not covered, never
  both, nothing named that does not exist. A feature added with its
  record or command and no case fails the build, as `website/check.sh`
  does for a page.
- **`tools/acceptance.py gate TAG`**, the first job of `release.yml`; the
  four builds need it, so nothing is built or published without it. It
  reads `docs/acceptance/runs/TAG.md`: the head filled in (version,
  commit, date, Claude Code's and Lonca's versions, the terminal, what was
  spent), one row per case and none other, no `fail` and no `todo`, a
  `skip` only for a case with `skip-if` and with why, each row's earlier
  commit an ancestor of the run's; the run's commit an ancestor of the
  tag, and nothing changed between them but `docs/acceptance/runs/`. The
  checkout is fetched whole (`fetch-depth: 0`) for that.
- **The run's world** (`tools/acceptance/setup.sh`): the candidate built
  and packed as a release would be, `XDG_STATE_HOME` and
  `XDG_CONFIG_HOME` of the run's own, three sample checkouts copied from
  `tools/acceptance/repos` (Python with `.imece/check`, setup and teardown
  scripts, `.worktreeinclude` and an ignored `.env`; JavaScript; a folder
  without git), an empty home and an empty Claude Code directory, a home
  for Lonca's daemon, `env.sh`, and `imece-acc` (sonnet standard, haiku
  cheap, the pot at $15). `tools/acceptance/drive.py` runs each program
  on a tmux server of its own and logs every key.

## What was to decide

- **A mechanical gate, not a rule alone.** A rule in `CLAUDE.md` is
  forgotten the day a release is in a hurry; the gate makes the tag fail
  instead. It checks the shape of the report, not that the run happened:
  the version pull request is where a person reads it.
- **What is tagged is what was run.** A report on an older commit would
  let a fix after the run go out unrun. The report is the last commit of
  the version branch and the gate refuses any other change between the
  run's commit and the tag. Rows run on an earlier commit of the same
  candidate are allowed, named, so a late fix does not cost the whole run
  again; which rows a fix touches is the runner's call, and the README
  says to run again when in doubt.
- **Real sessions, not the demo.** The demo and the stand-in prove the
  screen and the plumbing; what failed in the field were Claude Code's
  own ways (a prompt folded into a turn, a test command, Bash writes),
  which only real sessions show. The automatic suites are cases of their
  own (AUTO-1 to AUTO-5) so a run starts from green.
- **Coverage by records and commands.** Those two lists are what a
  feature leaves behind that a script can see. Keys are not counted: most
  live in the help per place and the cases name them in their steps.

## The points of the list

- **Growth**: one report of a few kilobytes per release in
  `docs/acceptance/runs`; the catalogue grows by a case or two per
  feature.
- **Failure**: the gate fails closed: no report, a report it cannot read,
  a row missing, a git command that fails. A run gone wrong is stopped by
  its pot ($15).
- **Migration**: the first release after this needs a run; earlier tags
  had none and are not looked at.

## Gaps

- A run is driven by hand (the person, or Claude through `drive.py`);
  nothing replays a case by itself. Cases whose steps are only keys and
  words on the screen could become scripts later.
- The terminal-specific cases (the tab's icon, notices by terminal) run on
  the terminal at hand; the others are skipped with why.
