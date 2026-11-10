# Acceptance catalogue

What a release of imece must do on a real machine, case by case. How to
run it, how a case is written and what lets a release go:
[README.md](README.md). `tools/acceptance.py check` reads this file: a
case is a `### AREA-N · Title` heading with its `- records:`, `- commands:`
and `- tier:` lines, a `Steps:` and an `Expect:`; every decision record and
every `:` command is named by a case or under [Not covered](#not-covered).

Cases run in this order; `after:` names a case whose state one goes on
from. Unless a case says otherwise the run's own imece is started as
`tools/acceptance/drive.py start imece -- imece-acc` (README: the
environment) on `shelf`, at 120×40.

## Automatic

### AUTO-1 · The workspace's own checks on the candidate

- records: 130
- commands: —
- tier: auto

Steps:
1. On the candidate's commit: `cargo fmt --all --check`,
   `cargo clippy --workspace --all-targets --locked -- -D warnings`,
   `cargo test --workspace --locked`, `cargo deny check`.

Expect: all four exit 0. The tests include `live::end_to_end` (the real
`Live` on the stand-in: hire, worktree, the change waiting, resume, asks,
questions, helpers, faults, plan) and `tests/stand_in.rs`.

### AUTO-2 · The installer end to end

- records: 69
- commands: —
- tier: auto

Steps:
1. `cargo build --release --locked -p imece`, then
   `website/test-install.sh target/release/imece`.
2. `shellcheck website/static/install.sh website/test-install.sh tools/acceptance/*.sh`.

Expect: `test-install` passes (download, checksum, install, PATH line once
per shell, refusals); shellcheck says nothing.

### AUTO-3 · The site describes the screen, in both languages

- records: —
- commands: —
- tier: auto

Steps:
1. `website/check.sh`, then `npm ci --ignore-scripts && npm run build` in
   `website/`.

Expect: both exit 0: every page has its Turkish copy, every `:` command and
help key is on its page, no broken link or anchor.

### AUTO-4 · CI and the nightlies are green on the candidate

- records: 139
- commands: —
- tier: auto

Steps:
1. `gh run list --commit <candidate> --limit 20` and
   `gh run list --workflow=fuzz.yml --event schedule --limit 1`.

Expect: every workflow of the candidate's commit passed (ci, docs, deny);
the newest nightly fuzz run passed, or what it found is fixed in the
candidate.

### AUTO-5 · The acceptance tools refuse what they must

- records: 147
- commands: —
- tier: auto

Steps:
1. `python3 -I tools/acceptance.py check`, then
   `python3 -I -m unittest discover -s tools -p 'test_*.py'`.

Expect: both pass: the catalogue names every record and command, and the
gate's tests (a missing report, a failed or unexplained skip, a row
missing, a change after the run's commit) refuse.

## Install and first start

### INST-1 · The one-line install into an empty home

- records: 69
- commands: —
- tier: real

Steps:
1. Pack the candidate as release.yml does (`imece-<target>.tar.gz` with
   `imece` and `LICENSE`, `SHA256SUMS`) into a directory `rel/`.
2. `env -i HOME=$ACC/home0 PATH=/usr/bin:/bin SHELL=/bin/zsh
   IMECE_DOWNLOAD_URL=file://$ACC/rel sh website/static/install.sh`.
3. A new `zsh -l` with that `HOME`: `imece --version`.

Expect: installed in `~/.local/bin`, one `# imece` PATH line in `.zshrc`;
the new shell finds it and it says `imece <candidate version>`; a second
install adds no second PATH line.

### INST-2 · The command line refuses in words

- records: 02, 05, 28
- commands: —
- tier: real

Steps:
1. `imece --version`, `imece -h`, `imece --agents`, `imece --agents 17`,
   `imece --helpers 9`, `imece --budget abc`, `imece --confine loose`,
   `imece --lang de`, `imece --projects 2`, `imece --model "x y"`,
   `imece --tier huge=opus`, `imece --repo /`.

Expect: `--version` exits 0 with `imece <version>`; `-h` and each wrong
flag exit 2 with the words of `docs/guide/reference/cli.md` (a missing
value and a demo flag given live followed by the usage); `--repo /` exits 1
with `imece: …` and the terminal as it was.

### INST-3 · imece doctor, outside and inside the screen

- records: 138
- commands: :doctor
- tier: real

Steps:
1. `imece doctor --repo $ACC/repos/shelf`, then `--verbose`, `--json`,
   `--lang tr`.
2. `PATH` without `claude`: `imece doctor`.
3. In the screen: `:doctor`, `esc`.

Expect: one line per category with `[✓]`, `[!]` or `[✗]`, a fix under each
that is not right, the count of categories with issues last; exit 0 with
only notes and warnings; without `claude` the Claude Code category is `[✗]`
and the exit 1; `--json` names stay English under `--lang tr`; the window
shows the same report for the project at hand.

### INST-4 · The first start ever: welcome, mark, news, farewell

- records: 75, 101
- commands: :welcome, :news
- tier: real

Steps:
1. With an empty `$XDG_STATE_HOME` start imece on `shelf`.
2. Read the welcome window; `esc`. Then `:news`, `esc`; `:welcome`, `esc`.
3. Quit with `q q`.

Expect: the welcome window opens by itself with imece's mark, the version,
Claude Code's version and who is signed in, how to sign in, the keys; the
mark's dots come one a beat, whole after a key; Home's free rows show the
mark while no worker exists; `:news` names this version; after the quit
the shell's title is back and one farewell line says what waits, what
sleeps, what this run spent and `resume: imece`.

### INST-5 · The tab: title, icon, progress

- records: 16, 82, 101
- commands: —
- tier: real
- skip-if: the run's terminal is not iTerm2, kitty, Ghostty or Konsole (say which)

Steps:
1. Run imece in the terminal itself (not tmux), give a worker a task.
2. Look at the tab while it works, while an ask waits, when it is calm.
3. Quit; scroll the terminal's scrollback up.

Expect: the calm title is `⁙ imece`; it starts `● ` while working, `! `
while something waits; the tab's progress bar follows (where the terminal
has one); iTerm2 shows imece's icon (`IMECE_NO_ICON=1` keeps it off); after
the quit no screen of imece's is left in the scrollback and the progress
bar is cleared.

## The team on the git path

### TEAM-1 · The leader alone answers from what it reads

- records: 02, 03, 14, 26, 88, 109, 110
- commands: —
- tier: real
- after: INST-4

Steps:
1. Put `Always end a reply with the word "selvi".` in
   `$XDG_CONFIG_HOME/imece/instructions.md`; start imece on `shelf`.
2. `/` and write: `what does this repository do, and how are its tests run?`
3. Then: `edit README.md yourself and add a line "hello"`.

Expect: the leader answers from the files (Python, `python3 -m unittest`,
from `CLAUDE.md`) and ends with `selvi`; the reply is a line of the
conversation and of Home, not something to decide; the leader does not
write (no file of the checkout changes: `git status` clean) and says it
gives such work to a worker; the journal says the session's confine level.

### TEAM-2 · The first task: a cheap worker in its worktree

- records: 05, 10, 36, 61, 62, 63, 64, 85, 97, 105, 136, 135
- commands: —
- tier: real
- after: TEAM-1

Steps:
1. Write to the leader: `find() should ignore case and match authors too.
   One worker on the cheap tier.`
2. While the worker works, write to it `@<worker> also keep the old tests`
   (a message mid-turn).
3. Watch Home and the worker's row to the end of its turn.

Expect: the leader hires one worker (`hire`, cheap tier: haiku) named by
the task, not `claude-N`, on Home; its worktree is under
`$XDG_STATE_HOME/imece/worktrees` and its scratch beside it; the row shows
`◌ starting…`, then `● ` with `▸` the to-do step, `≡ N/M`, the activity
bar, `⎇ branch +N −M`; `python3 -m unittest` runs without an ask (the run
tool or a routine command); the message mid-turn is taken into the turn
under way: one turn, the row goes idle when it ends, the `done` reaches the
leader.

### TEAM-3 · The change waits, with its verdict, evidence and cost

- records: 11, 54, 55, 106, 112, 113, 124, 125, 137
- commands: :changes, :review
- tier: real
- after: TEAM-2

Steps:
1. Home: the change in TO DECIDE. `gc` (`:changes`), `enter`.
2. The first `.imece/check` of the run asks: `enter` on it, read its
   bytes, `a`.
3. In the diff: `w`, `h l`, `zM zR`, `J`/`K` or `[ ]` between files.
4. `:review`; when its note comes, `R`; `:review` again on the same change.

Expect: the change's row and the diff's first line are one verdict
(`ready` or `look first` with why); the worker's why with its bullets kept;
the files above the patch by kind (source, tests); `cost · $… · N min` on
the row and above the diff; the check runs once approved and its result
is above the runs (`✓ check exit 0`); `key=lambda` in `shelf.py` is shown
as it is, not as `[***]`; `:review` puts a tool-less session's note above
the files (`R` shows it whole), a second `:review` of the same change is
refused in words, never a gate.

### TEAM-4 · Notes on lines, a quoted hunk, the change sent back

- records: 47, 49
- commands: —
- tier: real
- after: TEAM-3

Steps:
1. In the change: move to a line, `c`, write a note, enter; another on a
   second file.
2. `>` on a hunk: the line opens with `@<worker>` and the hunk quoted; add
   `why this?` and send.
3. Back in the change: `n`.

Expect: the notes show under their lines and `✎ 2` in the title; the
quoted message reaches the worker with `file:line`; `n` sends both notes in
one message and the change goes back (gone from TO DECIDE); the worker's
answer brings a new change.

### TEAM-5 · Part applied, then the rest; commit from imece's own shell

- records: 13, 50, 56, 66, 68
- commands: :commit, :output, :history
- tier: real
- after: TEAM-4

Steps:
1. In the new change: `x` on one hunk, `a`.
2. The rest comes back as a new change: `a`.
3. `:commit`; read the suggested line; enter.
4. `!git log --oneline -3`; then `O`; then `esc`.
5. `↑` on an empty line, `^r` and `log`; `:history git`.

Expect: only the chosen hunk is staged in the checkout, the rest waits as
its own Δ; after the second `a` all is staged; `:commit` puts
`!git commit -m '…'` on the line (cheap tier, once a day per staged tree
free after the first) and the commit is made; the `!` line's lead shows as
a badge; the journal has the command, its exit code and time; `O` opens
`@<agent>` with the output quoted; `↑` and `^r` bring back the lines sent.

### TEAM-6 · A clash with the person's own edit is sent back

- records: 108
- commands: —
- tier: real
- after: TEAM-5

Steps:
1. Ask the leader for a small change to `shelf.py`'s `listing`.
2. Before applying, edit the same lines in the checkout yourself, commit.
3. `a` on the change; then `b`.

Expect: the apply is refused and says which file and hunk clash, a red
line above the change, `b  send back` in the footer; `b` sends the clash
to the worker in one message and the change goes back; the worker's next
change applies.

### TEAM-7 · Finished rows and the team finished

- records: 35, 98, 140, 145
- commands: —
- tier: real
- after: TEAM-6

Steps:
1. Apply what waits until nothing does; look at Home and the header.

Expect: the worker's row says `idle · finished, applied`; the AT WORK
heading counts `finished 1`; while something of the round waited Home
said `◐ TEAM FINISHED … N wait for you`; once nothing waits `ALL DONE` with
the round's cards, cost and time; the header keeps what is to decide and
the pot whole; a done the person has not opened shows `✓` until opened.

### TEAM-8 · A worker's helper

- records: 09
- commands: —
- tier: real
- after: TEAM-7

Steps:
1. Start imece with `--helpers 1`; ask the leader: `add a remove() test;
   the worker should have a helper write the test`.

Expect: the helper's row stands under its worker (`└`); it works in the
worker's worktree, says `done` to its worker and leaves; the worker's
change holds the test.

### TEAM-9 · Fork a worker to try another way

- records: 94
- commands: :fork
- tier: real
- after: TEAM-8

Steps:
1. On the worker's row (between turns): `:fork`.
2. Write to the fork: `try the same with a generator`.

Expect: a new worker named as `hire` names, on the worker's model, its
worktree at the worker's HEAD with the worker's change carried; its
session is a fork (it knows what the worker did); the journal says
`forked`; `:fork` on the leader or mid-turn is refused in words.

### TEAM-10 · Mail, reply numbers and mail held while waiting on you

- records: 58, 107
- commands: —
- tier: real
- after: TEAM-9

Steps:
1. Give a worker a task whose command needs a yes (TEAM-8's
   `python3 -c 'print(1)'` style), leave the ask open.
2. While it waits, have the leader write to it (`ask the worker how far it
   is`).
3. Answer the ask; open the conversation `v`; `%` on an answer.

Expect: while the ask waits the worker's row says `✉ 1 held` and its
mail starts no turn; `:why` says it waits on you; after the yes the mail
is delivered; the conversation marks answers with `↩` and `%` goes to the
message answered and back.

### TEAM-11 · A blocked worker is the leader's first

- records: 143, 146
- commands: —
- tier: real
- after: TEAM-10

Steps:
1. To the leader: `assign a worker: import the books from data/books.csv
   (the file does not exist; if so it must send blocked, not guess)`.
2. Watch the leader; if it asks you, answer with a digit.

Expect: the worker sends `blocked`; its row says the leader has it; the
leader either solves it (journal `lead unblocked … after N min`) or asks
you with what it tried and the risk, as an item with digits; an answer
reaches the worker and it goes on; nothing about it is silent.

### TEAM-12 · Health notices to the leader

- records: 128
- commands: :health
- tier: real
- after: TEAM-11

Steps:
1. `:health`, then `:health idle 1m`.
2. Leave a worker with no card idle for over a minute.
3. `:health idle 10m`.

Expect: bare `:health` says `stall 10m idle 10m loop 3`; after a minute
the journal has the health line and the leader hears of the idle worker
in its next prompt (not woken for `Free`); the values are kept for the
next start.

### TEAM-13 · The issue a card closes

- records: 142
- commands: :closes
- tier: real
- skip-if: no throwaway GitHub repository and `gh` signed in for it (say which)
- after: TEAM-12

Steps:
1. A throwaway repository with `shelf` pushed and an open issue N; imece on
   its checkout.
2. To the leader: `fix issue #N: <its words>`.
3. Apply the change, `:commit`. Then `:closes off`, `:closes`.

Expect: the card names `closes N`; the commit message ends `Closes #N`;
with `:closes off` the next commit has no `Closes` line and bare `:closes`
says where it stands; the setting is kept per project.

## Asks and questions

### ASK-1 · A permission ask, a yes, a no and a standing yes

- records: 30, 45, 89
- commands: :allow
- tier: real

Steps:
1. To the leader: `a worker should serve the folder with python3 -m
   http.server 8765 --bind 127.0.0.1 in the background, then say done`.
2. The ask `⚑` in TO DECIDE: `r` the first time; the worker asks again:
   `A`.
3. `:allow`; a second worker asked for the same command.

Expect: the ask names the command whole; `r` tells the worker no, and it
says so; `A` says yes for the project to `python3 -m`'s rule (or the line
alone, as `:allow` lists it) and the next same command passes without an
ask; the worker's row shows `http://127.0.0.1:8765` as a link that opens.

### ASK-2 · A yes for the rest of a turn

- records: 104
- commands: :yes
- tier: real
- after: ASK-1

Steps:
1. Give a worker a task with three different non-routine commands.
2. On its first ask: `e`.

Expect: `✓ turn` first on its status; its later asks in that turn go on
unasked (traced, counted); the journal says how many when the turn ends;
the next turn asks again. `:yes @<worker>` does the same.

### ASK-3 · A question with answers

- records: 27, 125
- commands: —
- tier: real

Steps:
1. To the leader: `ask me, with options, whether find() should match the
   year too; then do what I answer`.
2. On Home: `2`. Next time: `/` and write an answer of your own.

Expect: the question's row lists its answers with their digits; the
preview shows it whole with `so far:`; a digit answers at once and the
row goes (a yes moves down: the next row is at hand); a written answer
reaches the asker as written.

### ASK-4 · A focus decides its agent's ask, and says where it works

- records: 103
- commands: —
- tier: real
- after: ASK-1

Steps:
1. While a worker's ask waits, open its focus (`enter` on its row, the log
   tab); `a`.
2. On a focus with nothing waiting: `a`.

Expect: the footer starts with the ask's keys; `a` decides that agent's
oldest ask, not the one at hand on Home; nothing waiting flashes
`focus_no_ask`'s words; the focus has a `folder` line (`~/…/worktrees/…
· worktree of shelf`).

### ASK-5 · How much a session may do: strict and open

- records: 26, 28
- commands: —
- tier: real

Steps:
1. `imece --confine strict` on `shelf`; give a worker a task with a test
   run.
2. Quit; `imece --confine open`; the same.

Expect: strict: the worker has file tools only and tests through the run
tool, no shell; the journal says strict. Open: the person's own Claude
Code setup is loaded (their `~/.claude` settings and MCP servers; the
journal says so at every start); the leader writes in neither.

### ASK-6 · The worktree's setup and teardown scripts

- records: 92, 96
- commands: :tidy
- tier: real

Steps:
1. `shelf` has `.imece/worktree-setup.sh`; a worker is hired.
2. The `▶` ask: `enter`, read the bytes, `a`.
3. Later, with a worktree no agent owns (an agent gone): `:tidy`, `d` on
   it; the teardown script's ask: `a`.

Expect: the worker starts without the script until the yes, then it runs
in that worktree (`.setup-ran` there); `:tidy` lists the orphaned worktree,
says whether it holds work, the teardown runs before its removal; a start
says the count of orphans in a flash.

## Sessions and what is kept

### SESS-1 · The quit question, the farewell and where you left off

- records: 19, 20, 24, 25, 34
- commands: :pending, :quit
- tier: real

Steps:
1. With one agent mid-turn and a change not reviewed: `q`; then `s`
   (`:pending`); `esc`.
2. `:quit` and answer yes.
3. Start imece again on `shelf`.

Expect: the quit question counts the agents mid-turn and the changes not
reviewed; `q s` lists them; the farewell says what waits; at the next
start WHERE YOU LEFT OFF lists what happened and what waits; the leader
and workers resume their sessions (`--resume`; the journal says so, a
fresh one says why); the pot, cards, journal and marks are as left, the
pot not `+?`.

### SESS-2 · Forget a session, with a grace to undo

- records: 23, 78
- commands: :forget, :undo, :now
- tier: real
- after: SESS-1

Steps:
1. On a worker: `:forget`; within ten seconds `u`.
2. Again `:forget`, then `U`.
3. Quit; start with `--fresh`.

Expect: the bottom line counts down `◷ … in N s · u keeps it · U now`;
`u` keeps it; `U` forgets at once and its next turn is a new session told
where it left off; `--fresh` does it for every agent with a journal line
each and their transcripts removed.

### SESS-3 · Archive, browse, view and restore

- records: 80
- commands: :archive, :archives, :restore
- tier: real
- after: SESS-2

Steps:
1. `:archive lending try`; wait out the grace.
2. `A`: the browser; `enter` on the archive; move around; `a` on something;
   `esc`.
3. `:restore lending try`.

Expect: a fresh project of the same name takes its place; the view is
read-only (`ARCHIVE · shelf “lending try” …` banner, a refused key flashes
`read_only`'s words, `ctrl-c` would quit); `:restore` brings it back
asleep, its agents told, the fresh project archived or closed as record 80
says.

### SESS-4 · An account that is signed out

- records: 79
- commands: —
- tier: real

Steps:
1. `CLAUDE_CONFIG_DIR=$ACC/nobody imece` on `shelf` (an empty directory).
2. Write to the leader.

Expect: the welcome says Claude Code is not signed in and how to; the
leader's turn fails as the account's fault: a `✕` item in TO DECIDE at
once, no `↻` retries; writing to it after signing in closes it.

### SESS-5 · The pot and the usage windows

- records: 02, 48
- commands: :budget
- tier: real

Steps:
1. `:budget 0.05` while a worker works; then ask the leader to hire.
2. `:budget 15`.

Expect: the header shows the pot loud when low; when spent no new turn
starts and `hire` is refused in words, a team stop says so on Home with
choices; `:budget` raises it and the work goes on; the usage window of the
plan (`5h`, `7d`) shows in the header once a session said it, loud from
90 %.

### SESS-6 · The cost ledger

- records: 132, 133, 134
- commands: :ledger
- tier: real
- after: TEAM-7

Steps:
1. `:ledger`; scroll it; `esc`.
2. Open a card's log tab.

Expect: per project `applied $… · wasted $… · at work $… · lead $…`; one
row per attempt with card and try, worker, model, what became of it; each
model's part; figures marked `≈` or `at least` where a price was not
known, never `$0` for an unknown; the card's log tab lists its attempts.

### SESS-7 · The agents' processes, and those an earlier run left

- records: 129
- commands: :ps
- tier: real
- after: ASK-1

Steps:
1. `:ps` while ASK-1's server runs; `d` on the `python3` line, `y`.
2. `kill -9` imece's own process; start imece again; `:ps`.

Expect: each session's processes with cpu and memory, read again every few
seconds; `d y` ends the server, never the session or the relay; after the
kill the next start flashes what an earlier run left, the window lists it
under `LEFT FROM AN EARLIER RUN` and `a y` ends them all.

### SESS-8 · The terminal goes away

- records: 86
- commands: —
- tier: real

Steps:
1. A worker mid-turn; `drive.py stop imece` (the pane's terminal goes).
2. `pgrep -fl 'imece|claude'` after five seconds.

Expect: imece quits as on `:quit`: sessions stopped, the bus's sockets
gone, nothing of the run left running; the next start resumes them.

## Solo

### SOLO-1 · A solo beside the team, its model and effort

- records: 59
- commands: :solo, :effort, :model
- tier: real

Steps:
1. `:solo`: the pick screen; choose `haiku`, then `low`.
2. Write to it: `add a docstring to Shelf.add`.
3. `:effort medium`; `:model sonnet`; write again.
4. Quit; `imece --solo claude:haiku@low` on `shelf`.

Expect: the solo works in its own worktree `solo-shelf` and every turn
that answers shows its Δ; `:effort` and `:model` end the session and
resume it on the new choice (journal `Chose`); the same `--solo` choice
the next morning goes on with the same solo; a different choice opens
`-2` beside it.

### SOLO-2 · A solo in a folder without git

- records: 73
- commands: —
- tier: real

Steps:
1. `:solo $ACC/repos/plain haiku`.
2. Write: `add a line to notes.txt`.
3. `:commit`; `:open $ACC/repos/plain`.

Expect: the header says `⌀ no git`; the solo writes in the folder itself,
no Δ; `:commit` is refused; a team on the folder is refused in words.

## Projects

### PROJ-1 · A second project, pinned, renamed, closed with a grace

- records: 18, 43, 78
- commands: :open, :pin, :rename, :close, :settle, :unsettle, :fold, :unfold
- tier: real

Steps:
1. `:open $ACC/repos/tally`; ask its leader for a small change; let its
   worker run `npm test`.
2. Home: `[ ]` narrows to a project and back; `h l za zM zR`, `:fold`,
   `:unfold` on the projects.
3. `:pin tally`; `:rename counter`; `s` on a row, `:unsettle`.
4. `:close`, `u`; `:close`, wait the grace.

Expect: two leaders, Home's AT WORK headed by project; `npm test` needs no
ask (record 136); the pin puts `▴ counter` first and is kept; `s` hides the
row and the heading counts it; `:close` counts down and `u` keeps it; a
closed project's sessions sleep and come back with `:open`.

### PROJ-2 · Ignored files copied into a worktree

- records: 57
- commands: :copy
- tier: real

Steps:
1. `shelf` has an ignored `.env` and a `.worktreeinclude` naming
   `local.cfg`; `:copy .env`; `:copy`.
2. A worker is hired; look in its worktree.

Expect: `:copy` lists `.env` and says a `.worktreeinclude` is there; the
worktree has both files, copied (never a link); the journal says what was
copied and what skipped.

## Cards, the board and the plan

### CARD-1 · The plan the person sees first

- records: 114
- commands: :plan
- tier: real

Steps:
1. `:plan on`; to the leader: `plan lending: a Loan model, then lend and
   return commands; one card each, the second after the first`.
2. `▤` in TO DECIDE: `enter`, read; `r` with a word; the next plan: `a`.

Expect: before the yes the leader's `hire` past the first worker is
refused; the plan window wraps long lines, lists each card with its paths,
`after 1 above` and the cost; `r` tells the leader no and why; `a` lets the
cards out; the board shows the second card `⧗ after #1`.

### CARD-2 · Paths a card owns, and a write outside them

- records: 114
- commands: —
- tier: real
- after: CARD-1

Steps:
1. To the leader: `assign a worker the paths shelf.py only: make
   listing() sort by author; it must also touch cli.py`.

Expect: the assign carries `owns shelf.py`; the worker's write to `cli.py`
is noted once (`Strayed`, in its next prompt), and the Δ header says
`1 outside #N: cli.py`.

### CARD-3 · Attempts, a failed card, what waits on it, withdraw

- records: 126, 146
- commands: —
- tier: real
- after: CARD-2

Steps:
1. To the leader: `assign card X; the worker should say done with outcome
   failed`; then `hand it again to another worker`; then `withdraw it`.
2. A card after X (`after`) exists before X fails.

Expect: the card shows ` · try 2` when handed again, `✕` FAILED when it
failed (no change shown), the card after it waits and its leader is told;
withdraw puts it in WITHDRAWN, tells its holder, and what waited on it
says `✕ #X withdrawn` and never begins.

### CARD-4 · A card's screen, the board, the code map and the timeline

- records: 04, 17, 32, 100, 125
- commands: :board, :code, :timeline
- tier: real
- after: CARD-3

Steps:
1. `#N` on the line: a card's screen (the log tab of its owner); `n p`.
2. `:board`, `r` (ready only), `esc`; `:code`, `esc`; `:timeline`, `[ ]`,
   `j k`, `enter`, `esc`.

Expect: each is a `:` window that `esc` and `q` close and that takes every
key; the board shows columns with `→ name`, `⧗`, `✕`, ` · try N`; the code
map one row per file with the leases' glyphs and a legend (no leases off
Lonca: says so); the timeline one lane per agent with its scale.

## Lonca

### LONCA-1 · Sign in, import, and the policy signed

- records: 115, 116, 117, 121
- commands: :lonca
- tier: real

Steps:
1. The run's own daemon (README); `imece doctor` names Lonca.
2. On `shelf`: `:lonca login`, then `:lonca import`; read the window (the
   checkout as `~/…`, branch, commit); `a`.
3. `:lonca`.

Expect: the import runs `lonca` as a child with a spinner, then the
project is on Lonca; the policy is signed with `unobserved_critical =
"escalate"` and `mcp_servers = ["imece"]` (the journal says so);
`:lonca` opens the Lonca window (state, `WAITING FOR A HUMAN · N`,
`HELD · N`); a second import is refused in words.

### LONCA-2 · Workers in workspaces, leases on the map

- records: 119, 123
- commands: —
- tier: real
- after: LONCA-1

Steps:
1. To the leader: `plan lending in two cards, the second after the
   first`; `a` on the plan.
2. While they work: `:code`.

Expect: each worker works in a Lonca workspace (its focus's folder says
so; no git run in it by imece); `:code` shows only the files held, each
worker's lease and `· name` for others; a worker proposes before `done`.

### LONCA-3 · The Δ on Lonca: verdict, approve, reject with a reason

- records: 120, 122, 125, 127
- commands: :checks
- tier: real
- after: LONCA-2

Steps:
1. Set `[verification]` `test = "python3 -m unittest"` with
   `lonca policy set` (README).
2. The first proposal: read the verdict (`after your yes Lonca runs:
   tests`), `a`.
3. The second: `r`, write a reason, enter.
4. `:checks tests` on the next change, then `a`.

Expect: the Δ is the newest pending proposal, shown once its worker's turn
is over; the verdict names what runs after the yes, overlaps with another
proposal and a stale base; `a` approves (journal `on trunk as …`, the
queue's checks with exit codes); `r` asks for the reason first and the
worker hears it; `:checks` changes what the yes asks for.

### LONCA-4 · Behind trunk and `:sync`

- records: 118
- commands: :sync
- tier: real
- after: LONCA-3

Steps:
1. After a landing the header says `↓N`; `:sync`.
2. Make a commit of your own in the checkout; land another change;
   `:sync`.

Expect: `:sync` fetches and fast-forwards the checkout (`git log` shows the
landed commits), `↓` goes; with commits of your own it says the branch and
trunk went apart and changes nothing of yours.

### LONCA-5 · The daemon goes away and comes back

- records: 116
- commands: —
- tier: real
- after: LONCA-4

Steps:
1. Stop the daemon; wait 20 s; ask the leader to hire.
2. Start the daemon again.

Expect: the header says `⌀ no lonca` after three looks; hire, fork, apply
and reject are refused in words; when it is back the journal says so and
the work goes on.

## The screen

### SCR-1 · Home: what to decide first, then who works

- records: 33, 38, 124, 125
- commands: :home, :next, :defer, :waiting, :salon
- tier: real
- after: ASK-3

Steps:
1. With a permission, a question and a change waiting: look at Home.
2. `W` (`:next`), `S` (`:defer`), `gd`, `gw`, `:waiting`, `:salon`,
   `:home`; `g` and wait.

Expect: TO DECIDE grouped permissions, questions, changes, later; each row
says the whole ask with the task's name; the row at hand ends with two
keys; the status line at most three keys; `g` shows which-key; `W` opens
the next in that order from anywhere, `S` puts one off to later;
`:salon` lands on AT WORK.

### SCR-2 · The detail: diff, talk and log tabs, reading the log

- records: 29, 102, 141
- commands: :trace
- tier: real
- after: TEAM-3

Steps:
1. `enter` on a worker: `tab` and `⇧tab` through its tabs.
2. On the log: `j k gg G`, `PgUp PgDn`, `l` on a cut entry, `h`, `zM zR`.
3. `enter` on a `changed shelf.py` line.
4. `t` (`:trace`): the raw stream; `esc`.

Expect: one head line over the tabs; the log's entries cut to their rows
with `… +N · l shows all`; Markdown tables fit their room; `enter` on a
change line opens the diff at that file, lit for a moment, `← the log's
line` in its header, `esc` back to that log line; the raw stream scrolls.

### SCR-3 · Writing: the conversation above the line

- records: 12, 37, 39, 67, 95, 144
- commands: :edit
- tier: real

Steps:
1. `/`: the leader's chat above the line; write two lines (`shift-enter` or
   `\` enter), `^a ^e ^w ^u ^k ^y`, `alt-b alt-f`; send.
2. `@` + first letters of a worker, `tab`; write `see shel` + `tab`.
3. `^x ^e` (with `EDITOR=vi`): write, `:wq`.
4. `:edit some text`.

Expect: the line stays open in a chat after a message went; `@name`
brings that agent's conversation above while the line names it; names,
commands and files complete (`shelf.py`); the editor's text comes back on
the line, never sent by itself; a failure to start the editor is said
until a key.

### SCR-4 · A conversation: turns, folds, copy, notes, export

- records: 42, 44, 70, 71
- commands: :talk, :export
- tier: real
- after: TEAM-10

Steps:
1. `v` on a worker (`:talk`); `[ ]`; `za`, `zM`, `zR`.
2. `v` (choose), `j`, `w`, `y`; paste somewhere.
3. Choosing: `c`, a note; `n`.
4. `:export $ACC/out/talk.md`; again to the same path.

Expect: turns jump, folds show `▸ N lines`; `y` copies the chosen lines as
drawn (through `clean`); notes on words go as one message per agent; the
export is Markdown with days on every time, cleaned; a second export over
it is refused.

### SCR-5 · The journal, the story, marks, clearing, recall, why

- records: 14, 46, 87, 90, 93, 99
- commands: :journal, :story, :mark, :clear, :unclear, :recall, :why
- tier: real
- after: TEAM-7

Steps:
1. `J` (`:journal`); `j k g G`; `m` on a line; `esc`; `'`.
2. `:story` (by card, then as it is).
3. `:clear journal`, wait; `:unclear journal`.
4. `H` (`:recall`) `lambda`; `enter` on a hit.
5. `:why` on a worker.

Expect: the journal window pages; `'` opens at the mark; the story groups
by card with `✓ done` / `Δ waits for you` headers; `:clear` hides (marked
lines stay) and `:unclear` brings back; recall finds every project's and
archive's lines and `enter` goes there; `:why` flashes `name: rung · since
HH:MM · …`.

### SCR-6 · Jump, peek, settle and back

- records: 51, 65, 72
- commands: :goto, :peek
- tier: real
- after: PROJ-1

Steps:
1. `&`, a digit; `&!`; `:goto counter`.
2. `K` (`:peek`), `j k`, `esc`.
3. `^o` several times.

Expect: the jump line lists agents by what needs you with notes; a digit
goes at once and unfolds what hides it; the peek shows a focus over Home
and steps through agents; `^o` walks back the places left.

### SCR-7 · Help, the `:` menu, Turkish

- records: 40, 41, 125
- commands: :help, :lang
- tier: real

Steps:
1. `?` on Home, on a change, in a window; `?` again; `/` and `ileri` and
   `undo`; `esc`.
2. `:` alone; `tab` through; `:q` style short names (`:b`, `:t`).
3. `L` (`:lang tr`); walk Home, a change, the journal, the help; `L`.

Expect: `?` shows the keys of where you are, `?` again all; the filter
folds case and Turkish letters; the `:` menu shows values and `$` on what
costs, `‹ N` / `+N more ›`; in Turkish no English word is left on any of
those screens and nothing is cut that was whole in English.

### SCR-8 · A narrow screen

- records: 131
- commands: —
- tier: real
- after: SCR-1

Steps:
1. With something to decide: `drive.py size imece 100 40`, `60 20`,
   `48 18`, `39 11`, back to `120 40`.

Expect: at 100×40 nothing is cut that decides; under 80 columns Home's
entries stack; the status line keeps `esc` and `?` last; the header keeps
what to decide and the pot; under 40×12 one line says the size and the
floor and only `ctrl-c` acts; growing back draws at once.

### SCR-9 · Themes

- records: 15
- commands: :theme
- tier: real

Steps:
1. `T` several times; `:theme nord` (write it slowly: it previews);
   `:theme $ACC/config/mine.theme` (a Ghostty file); `:theme mono`.
2. Quit, start again.

Expect: each theme draws the whole screen in its colours; mono has no
colour and the `!` badge reversed; the last choice is kept; a wrong name
says the list.

### SCR-10 · Motion, the cursor, drawing less when away

- records: 77, 81, 84, 85
- commands: :motion
- tier: real

Steps:
1. `:motion full`, `calm`, `off` while a worker works.
2. Write on the line; open a window; look at the terminal's cursor.
3. Focus another window for a minute (`tools/measure.py --blurred` on the
   demo for numbers).

Expect: calm: a working row's glyph breathes, starting rows count dots, a
row that comes to wait blinks three times; full adds the header's spinner;
off moves nothing; the terminal's own cursor stands where you type and is
hidden where nothing is typed; blurred nothing moves and imece draws at
most once a second.

### SCR-11 · A key file of your own

- records: 76
- commands: :keys
- tier: real

Steps:
1. Write `$XDG_CONFIG_HOME/imece/keys`: `gt = G`, `[journal]`, `x = gg`,
   and one wrong line.
2. `:keys`; remove the wrong line; `:keys`; `?`.

Expect: the wrong file is refused whole with why and the last good keys
stay; the fixed one is taken (`taken N`); `gt` acts as `G`; `?` lists the
file's keys first; a change to the file is read again by itself within two
seconds.

### SCR-12 · Back after being away, and words in another window

- records: 52, 60, 83, 91
- commands: :away, :digest, :notify
- tier: real

Steps:
1. `:away 1m`; focus another window for over a minute while a worker
   works; come back.
2. `:digest`.
3. `:notify bell`; away, an ask comes. `:notify text`; again.
4. `:notify push` with a `push` file for a throwaway ntfy topic.

Expect: back in the window the digest of what happened opens at once (not
over writing); bell rings once, text sends one notice in imece's words
with the agent's name (through tmux's passthrough inside tmux); a click on
a kitty notice opens the item; push sends to the topic once with the
project, or says once why it failed.

### SCR-13 · Keep the machine awake mid-turn

- records: 53
- commands: :awake
- tier: real

Steps:
1. `:awake` while a worker works: `pgrep -fl caffeinate`
   (`systemd-inhibit` on Linux); when nothing works, again.

Expect: one `caffeinate -i -w <imece>` child while an agent is mid-turn,
gone when none is and at quit; `:awake` again turns it off; not kept for
the next start.

### SCR-14 · A secret-looking token is hidden, code is not

- records: 137
- commands: —
- tier: real

Steps:
1. To the leader: `have a worker print the line GITHUB_TOKEN=ghp_<36
   letters> with echo, and add max_tokens=5 to a test`.

Expect: the token's value is hidden on every screen that shows it (trace,
log, ask, journal); `max_tokens=5` and `key=lambda` are shown as they are
in the diff.

## The demo

### DEMO-1 · The demo shows every screen, on fake agents

- records: 01, 31
- commands: :pause, :faster, :slower
- tier: demo

Steps:
1. `imece --demo`; two minutes; `space` (`:pause`), `+ -` (`:faster`,
   `:slower`).
2. Every screen: Home, a change (`a`, `r`, `x`), a question (digits), an
   ask (`a`, `A`, `e`), a plan, the windows, the journal, researchers'
   rows.
3. `imece --demo --projects 1 --agents 3` at 100×40; `imece --demo --lang
   tr`; quit each.

Expect: nothing is started or spent; every screen of the real cases above
has its demo counterpart; the demo quits at once on `q q` and on SIGTERM.

### DEMO-2 · The demo on the real Live, through the stand-in

- records: 130
- commands: —
- tier: stand-in

Steps:
1. `imece --demo-live --agents 3`; five minutes; answer the asks it makes.
2. Quit; `pgrep -fl imece`.

Expect: the leader hires workers that work through to-do lists, edits and
tests, every third asks a yes; their changes wait in TO DECIDE and apply;
nothing is spent; the demo's directory is gone after the quit and nothing
is left running.

## Not covered

What no case runs, and why. A record or command here is named by no case.

- record 06: replaced by record 109 (Claude Code only); on the `multi-vendor` branch
- record 07: replaced by record 109; on the `multi-vendor` branch
- record 08: replaced by record 109; on the `multi-vendor` branch
- record 21: replaced by record 109; on the `multi-vendor` branch
- record 22: replaced by record 109; on the `multi-vendor` branch
- record 74: hint letters, gone with record 125's step 5
- record 111: a direction of the work, no behaviour of its own (its parts are records 112-114)

## Retired

Case ids that are gone, so they are not used again.
