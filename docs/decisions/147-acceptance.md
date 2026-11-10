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

## Scripted cases on the stand-in (2026-11-10)

The person found a few hours per release too long. Most of the time went
to keys typed one by one by whoever ran the cases, and to real model
turns where nothing of the model was looked at. So a case whose point is
imece's own screen and plumbing is a script, played on the stand-in
(record 130) as fast as the machine goes:

- **Tier `stand-in`** means scripted: `tools/acceptance/scenarios/<ID>.scn`,
  a step a line (`start`, `type`, `key`, `see`, `wait`, `gone`, `size`,
  `sh`, `fails`, `says`, `write`, `sleep`, `stop`: `drive.py`'s
  docstring). `check` refuses a stand-in case without its script, a
  script without its case and a step `drive.py` does not know.
- **The world** is `setup.sh --stand-in DIR`: its `claude` is imece's
  stand-in with no habit, steered by the person's cues (`[ask]`,
  `[question]`, … in the task's words).
- **`drive.py run [--jobs N] [--report REPORT]`** plays them, each in a
  directory of its own (state, config, Claude Code home, a copy of the
  checkouts), so they run side by side; a failure leaves its screen in
  `$ACC/out/<ID>.txt`; `--report` writes each row (`pass`, `scripted,
  12 s`). At a case's end its terminal goes, and a process still working
  in its directory five seconds later fails the case: that is how the
  first scripts found `!cmd`'s spin (record 86).
- **What a script can not judge** is a case of its own (`EYE-1`: the
  Turkish screens whole, the narrow ones read, the mark's dots, the
  farewell), and what only a real session shows stays `real`: the message
  taken mid-turn, the test runners, resume on Claude Code itself, Lonca's
  workspaces, the one-shot sessions' words (`TEAM-14`), a worker's local
  server (`ASK-7`).

Measured (2026-11-10, a laptop at load 40): the 13 scripted cases in 25 to
45 s at 4 to 6 jobs, 60 s one at a time; a person's or an agent's hand
took some 3 hours for the whole catalogue. Keys are sent one at a time
(150 ms apart, 500 ms after an Escape: an Escape and the next key in one
read are alt and that key), and a script waits for words on the screen,
never for time, except for what it can not see (`sleep`).

Findings of the first scripts: the spin above; and record 125's hold on
a new item (the first key on the rest of a part applied chooses it) is
what a person meets too, so the scripts say it in a comment where they
press a key twice.

CI's `acceptance-scripts` job (Linux and macOS) plays every script on
every pull request: `cargo build`, `setup.sh --stand-in` in the runner's
temporary directory, `drive.py run --jobs 4`; a failure's screens and
`drive.log` are the job's artifact. So a script breaks in the pull
request that breaks it, not on the day of a release, and the release
run's scripted rows only confirm what CI saw on the candidate.

## Most of the catalogue scripted (2026-11-11)

The person's yes to moving every case that needs no real model onto the
stand-in first: 36 more cases are scripts, 49 of 69 in all (48 `stand-in`
and the demo's), about 95 s at 6 jobs; 15 stay `real`: Claude Code's own
ways (TEAM-1, TEAM-2, TEAM-10, TEAM-11, TEAM-14, ASK-7, CARD-3), Lonca
(LONCA-1 to 5), a GitHub issue (TEAM-13), the tab (INST-5) and EYE-1,
which takes what a script can not see (the breath, the themes' colours,
a note in another window and its click, the `caffeinate` child, the
push). A case's `Expect:` says only what its script checks; the rest is
named where it goes.

The runner learnt `env NAME VALUE` (a variable for the next start: an
editor script), `$ACC_CASE` and `$ACC_SOURCE` in a step's text, and to
name a screen exactly (`=name:`): tmux took `-t case-scr-1` for
case-scr-14 once case-scr-1 was gone, and a case's cleanup ended another's
screen; the cleanup's `pgrep` matches a case's directory with its slash
for the same reason.

What the new scripts found:
- `--confine strict` stopped every session at its start (record 28):
  fixed here.
- `:ps` asks `end … ? y` for a session's own process and imece's relay,
  and only refuses after the `y`.
- `n` on one note says `1 notes went to claude-1`.
- A change whose worker ran its tests through Bash (`npm test`, no
  `.imece/check`) says `nothing ran`.
- A solo's change is titled by its reply (`Changed shelf.py.`), not by
  the person's words.

## The real cases scripted (2026-11-11)

With the person's yes to about $5 of trial runs, 12 of the 15 real cases
became scripts under `scenarios/real/`, played by `drive.py run --real`
on Claude Code itself: TEAM-1, TEAM-2, TEAM-10, TEAM-11, TEAM-14, ASK-7,
CARD-3 and LONCA-1 to 5. INST-5, TEAM-13 and EYE-1 stay by hand, and a
scripted real case keeps a "By hand" line for what its script does not do.

- A model takes its own way: the steps `allow SECONDS TEXT` (wait, saying
  yes to every ask meanwhile and keeping what was asked), `until SECONDS
  COMMAND` (a shell command until it succeeds, asks said yes to),
  `asked-none TEXT` (no ask held TEXT: the tests ran without one), and
  `A || B` in what is waited for (either right end). A script waits for
  imece's words, never the model's: a leader that repeats an address is
  not the server answering.
- `bg COMMAND` starts a case's own daemon, ended with the case before
  what outlived it is looked for: each Lonca case runs on a daemon and
  identities of its own and they run side by side.
- `run --real` refuses a stand-in's world and plain `run` a real one.
  Real sessions take longer to go: the runner waits 15 s, not 5, for what
  outlived a real case (`ACC_LEFT_WAIT`). A passed case's row says what
  its header said it spent.

Measured: a few cents a case, the 12 together about $0.50 and ten
minutes at 4 jobs. The whole catalogue now runs as 49 scripts on the
stand-in (about 95 s), 12 on Claude Code (about 10 min) and three cases
and the "By hand" lines by hand.

What the real scripts found:
- A server a worker starts with `nohup … &` leaves its session: `:ps`
  does not list it, imece's quit does not end it, and the next start's
  look for what an earlier run left does not find it (its command line
  names no worktree).
- A session of real Claude Code sometimes outlives imece's quit when the
  terminal closes: ASK-7's worker (`claude`) still ran 15 s after imece
  itself was gone, where record 02 kills a session within 1.5 s. Once in
  four runs; ASK-7 fails on it until it is mended. On the stand-in at six
  jobs, a quit once took more than 5 s too (SCR-8): the runner waits 10 s
  for a stand-in case, 15 s for a real one.
- A Lonca session file of another daemon (a reinstall) makes the import
  go through and the team stop at once as signed out; the stop offers
  "look again", not `:lonca login`.
- On Lonca a change says `needs your yes: nobody watched this session's
  tools, approvals=1`: Lonca's words, which a person may not follow.

