# Acceptance

What a release of imece must do on a real machine before it goes out:
real Claude Code sessions, a real Lonca daemon, sample checkouts imece has
never seen, a real terminal. The unit and end-to-end tests prove the
parts; an acceptance run proves that a person who installs this release
can do their work with it. The first such run (2026-11-09,
`docs/field/2026-11-09-first-real-run.md`) found some thirty things the
tests and the demo never showed. Record 147 says why it is built this way.

- [catalogue.md](catalogue.md): the cases, in the order a run takes them.
- `runs/vX.Y.Z.md`: one report per release, the run that let it go.
- `tools/acceptance.py`: `check` (the catalogue is whole), `new` (a report
  to fill in), `lint` (a report as the gate reads it), `gate` (release.yml).
- `tools/acceptance/setup.sh`: the run's world; `drive.py`: the terminal;
  `repos/`: the sample checkouts.

## The two rules

The person's rules (2026-11-10):

1. **No release without a passing run.** `release.yml`'s first job runs
   `tools/acceptance.py gate vX.Y.Z` before anything is built. It fails when
   `docs/acceptance/runs/vX.Y.Z.md` is missing, when a case of the
   catalogue has no row, when a row failed, when a case is skipped that
   may not be (no `skip-if`) or without why, and when the tag holds any
   change after the commit the run was on, other than the report itself.
2. **Every change keeps the catalogue true.** A pull request that adds,
   changes or removes a feature adds, changes or retires the cases that
   cover it, in the same pull request, as it updates the site and the
   tests. CI's `acceptance` job runs `tools/acceptance.py check`: a new
   decision record or a new `:` command that no case names (and that is not
   under "Not covered" with why) fails the build. What the check cannot see
   (a changed key, a new word on a screen) is the author's to carry: before
   calling a change done, find the case that runs it and read whether its
   steps and `Expect:` still say the truth.

## A case

```markdown
### AREA-N · What the person does, in a few words

- records: 11, 54          (the decision records it runs; `—` for none)
- commands: :changes       (the `:` commands it runs; `—` for none)
- tier: real               (auto, demo, stand-in or real)
- skip-if: …               (only when a skip can be right, and when)
- after: TEAM-2            (the case whose state it goes on from)

Steps:
1. …

Expect: what is seen, in the screen's own words where they matter.
```

- **auto**: a command whose exit code decides. **demo**: `imece --demo`.
  **stand-in**: a script (`tools/acceptance/scenarios/<ID>.scn`) that
  `drive.py run` plays on the real `Live` with imece's stand-in for Claude
  Code: free, in seconds, nobody at the keys. **real**: Claude Code and,
  for the Lonca cases, a Lonca daemon; it costs, and someone drives it.
- A case is `stand-in` unless what it shows needs a real model (how Claude
  Code takes a prompt, what it writes, how it resumes, Lonca's
  workspaces) or an eye (`EYE-1`). A new feature's case starts as a script
  where it can.
- A case id is never used again: a case that goes is listed under
  "Retired" with why. Renumbering is not done; a new case takes the next
  free number of its area.
- A case is written for someone who has the screen in front of them: keys
  and commands as typed, what to look for as the screen words it. A case
  that only an eye can judge (colours, the tab's icon) says so.

## Scripted cases

```sh
tools/acceptance/setup.sh --stand-in /tmp/imece-acc-s
. /tmp/imece-acc-s/env.sh
tools/acceptance/drive.py run --jobs 4                       # every script
tools/acceptance/drive.py run TEAM-5 ASK-1                   # some
tools/acceptance/drive.py run --jobs 4 --report docs/acceptance/runs/vX.Y.Z.md
```

- A script is a step a line: `start [--size CxR] [--bare] [ARG …]`, `env
  NAME VALUE` (for what starts next), `type
  TEXT`, `key KEY …` (tmux names: `Escape`, `Enter`, `C-u`, `Up`), `see
  TEXT`, `wait SECONDS TEXT`, `gone TEXT`, `size COLS ROWS`, `sh COMMAND`,
  `fails COMMAND`, `says TEXT`, `write FILE TEXT`, `sleep SECONDS`, `stop`
  (`drive.py`'s docstring has them whole). `#` starts a comment; say in
  one why a key is pressed that a reader would not expect. `$ACC_CASE` is
  the case's directory, `$ACC_SOURCE` the checkout the tools come from.
- The stand-in does what the person's words cue: `[ask]` a command that
  asks first, `[question]` a question with answers, `[helper]` a helper,
  `[stray]` a file outside its card, `[cards N]` a plan of N cards,
  `[fault]` / `[account]` a failed turn (record 130). Nothing happens by
  chance in this world.
- Wait for words, never for time: `see` waits up to 30 s. A flash goes
  in a few seconds and another may take its place, so what must be
  proved is looked for where it stays (the journal `J`, a row, a file
  with `sh`). Decide on the first thing to decide with `gd`; `W` opens it.
- Each case runs in `$ACC/s/<ID>` with its own state and copy of the
  checkouts; `--jobs` runs several at once. At its end its terminal goes,
  and anything still working in its directory five seconds later fails
  it. A failure's screen is in `$ACC/out/<ID>.txt`, every key in
  `$ACC/drive.log`.
- The 49 scripts (48 stand-in cases and the demo's) take about 95 s at 6
  jobs, most of it TEAM-12's minute of waiting; 15 cases stay real.
- CI runs every script on every pull request, on Linux and macOS (the
  `acceptance-scripts` job); a failure keeps `out/` and `drive.log` as
  the job's artifact. A change that breaks a script is fixed in the same
  pull request, the script too when the screen's words changed on
  purpose.

## The run's world

Needs, on the machine of the run: Claude Code signed in (the run spends
about $10-15 on sonnet and haiku; `--budget 15` stops one gone wrong),
`lonca` 0.2.1 or later, `tmux`, `python3`, `node`, `git`, `gh`.

```sh
tools/acceptance/setup.sh /tmp/imece-acc      # builds the candidate, makes the world
. /tmp/imece-acc/env.sh
tools/acceptance/drive.py start lonca -- lonca daemon --no-notify --no-proxy
lonca human keygen acc && export LONCA_HUMAN=acc
tools/acceptance/drive.py start imece -- imece-acc
tools/acceptance/drive.py snap imece
```

- `$ACC` is the run's directory: `bin/imece` (the candidate) and
  `bin/imece-acc` (it on `repos/shelf`, sonnet as the standard tier, haiku
  as the cheap one, the pot at $15), `state/` and `config/` (imece's
  `XDG_*`), `repos/shelf` (Python, `unittest`), `repos/tally` (JavaScript,
  `node --test`), `repos/plain` (a folder without git), `rel/` (what a
  release would put up, for INST-1), `home0/` and `nobody/` (an empty home
  and an empty Claude Code directory), `lonca/` and `humans/` (the
  daemon's), `out/`, and `drive.log` (every key the driver sent).
- Not under a scratchpad directory of Claude Code's on macOS: a binary
  there may hang at exec. `/tmp/…` is fine.
- `drive.py` runs each program on a tmux server of its own, so the
  person's tmux is not touched; `wait NAME 'words'` waits for words on
  the screen instead of sleeping.
- A Lonca policy for LONCA-3: `lonca policy show --repo shelf`, its
  `lonca.toml` and `policy.cedar` into two files of `$ACC/out`, add
  `[verification]` with `test = "python3 -m unittest"` and
  `env_passthrough = ["HOME", "TMPDIR"]` to the first, then
  `lonca policy set --repo shelf --config lonca.toml --policy policy.cedar`.

## A run

1. The candidate is the version pull request's branch with its version and
   `:news` commits (`docs/guide`'s release notes too): the commit that
   would be tagged.
2. `tools/acceptance.py new` writes `runs/vX.Y.Z.md` with one `todo` row per
   case and the candidate's commit. Fill in the head: Claude Code's version,
   Lonca's, the terminal and its size.
3. The scripted cases first, in a stand-in world (above) with `--report`:
   their rows are written by the run. Then the `auto` cases, then the
   `real` ones in a world of their own (below), in order. Each row: `pass`; `fail` with what was seen;
   `skip` only where the case has `skip-if`, with why. What rubbed but did
   not fail goes under "Findings" as the field entries do; a field entry
   for the run goes in `docs/field/` the same day.
4. A failure is fixed in a pull request of its own, merged, and the version
   branch brought onto it. Then the cases the fix could touch are run again
   on the new commit, and the report's `commit` is that commit. A row that
   was not run again names the earlier commit it passed on (`commit`
   column); the gate takes that only for an ancestor of the run's commit.
   When in doubt, run it again.
5. `tools/acceptance.py lint docs/acceptance/runs/vX.Y.Z.md` until it says
   passed; commit the report as the last commit of the version branch;
   merge the pull request with a merge commit (not squashed: the run's
   commit must be in what is tagged); main's CI green; then the tag.
6. Clean up: `drive.py stop` every session, `pkill -f "$ACC"`, nothing of
   the run left running (`pgrep -fl 'imece|claude|lonca'`), then remove
   `$ACC` and the sample's worktrees with it.

The gate trusts the report's words: it checks that a run of every case is
written down for what is tagged, not that the run happened. The report is
reviewed in the version pull request like any other change.
