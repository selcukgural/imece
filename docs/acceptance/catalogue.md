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
- tier: stand-in

Steps: `scenarios/INST-1.scn`: `install.sh` with `IMECE_DOWNLOAD_URL=file://$ACC/rel` (what a release would put up, packed by `setup.sh`) into an empty `HOME`, twice.

Expect: `installed imece <version>`; one `# imece` PATH line in bash's file (`.bash_profile` on macOS, `.bashrc` elsewhere); the installed binary says its version; a second install adds no second line.

### INST-2 · The command line refuses in words

- records: 02, 05, 28
- commands: —
- tier: stand-in

Steps: `scenarios/INST-2.scn`: `imece --version`, `-h`, `--agents`,
`--agents 17`, `--helpers 9`, `--budget abc`, `--confine loose`,
`--lang de`, `--projects 2`, `--model "x y"`, `--tier huge=opus`, then
a screen started with `--repo /`.

Expect: `--version` says `imece <version>`; `-h` and each wrong flag exit
2 with the words of `docs/guide/reference/cli.md` (a missing value
followed by the usage); `--repo /` exits 1 and says how to make the
folder a checkout.

### INST-3 · imece doctor, outside and inside the screen

- records: 138
- commands: :doctor
- tier: stand-in

Steps: `scenarios/INST-3.scn`: `imece doctor` plain, `--verbose`, `--json`, `--lang tr --json`; then with no `claude` on PATH; then `:doctor` on the screen.

Expect: `[✓]` categories and exit 0; `--json` names (`claude.login`) stay English under `--lang tr`; without Claude Code a `[✗]` and exit 1; the window shows the report.

### INST-4 · The first start ever: welcome, news, the way out

- records: 75, 101
- commands: :welcome, :news
- tier: stand-in

Steps: `scenarios/INST-4.scn`: a start with an empty state, the welcome,
`esc`, `:news`, `:welcome`, `q q`.

Expect: the welcome opens by itself with imece's mark, Claude Code's
version and that it is signed in, and `any key: close · :welcome shows
this again`; an empty Home says nobody is at work; `:news` opens WHAT'S
NEW; `q` asks and `q` again quits with exit 0. The farewell's words and
the mark's dots one a beat are EYE-1's.

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

### TEAM-1 · The leader alone answers from what it reads, and writes nothing

- records: 02, 03, 14, 26, 88, 109, 110
- commands: —
- tier: real
- after: INST-4

Steps: `scenarios/real/TEAM-1.scn`: the person's `instructions.md` asks every reply to end with `selvi`; `--agents 0`; the welcome; a question about the repository; a request that the leader edit `shelf.py` itself; `J J`.

Expect: the welcome names Claude Code's real version, signed in; the answer comes from the files (`unittest`) and ends with `selvi` (the person's instructions reach the session, record 110); the leader writes nothing (`git status` clean after both); the journal says `confine standard:`.

### TEAM-2 · The first task: a cheap worker, a message mid-turn, the sessions resumed

- records: 05, 10, 20, 36, 61, 62, 63, 64, 85, 97, 105, 136, 135
- commands: —
- tier: real
- after: TEAM-1

Steps: `scenarios/real/TEAM-2.scn`: a task to the leader for one cheap worker; the worker's start; `@claude-1 …` while it works; its change; a stop and a start; a question to the leader; `J J`.

By hand, in the same run: the row while it works (`▸` the step, `≡ N/M`, `⎇ branch +N −M`, the context fill), the header's usage window once a session said it.

Expect: `claude-1` is hired and works in a worktree with its scratch beside it; the message mid-turn is taken into the turn under way (the change comes, the row rests); no ask was for `unittest` (record 136); after the restart the journal says the session `goes on in the session it had`.

### TEAM-3 · The change waits, with its verdict, evidence and cost

- records: 11, 54, 55, 112, 113, 124, 125, 137
- commands: :changes
- tier: stand-in

Steps: `scenarios/TEAM-3.scn`: a task to the leader, the setup script's
and the check's asks said yes to, `:changes`, `enter`, `w`, `esc`.

Expect: Home's change row and the preview say one verdict (`✓ ready to
approve · ✓ check · 2 files`, `1 ready to approve`), the cost (`$0.12`);
the diff view has the files by kind (`source +1 −0 (1) · tests +1 −0
(1)`), the worker's why and its test run, the patch; `esc` is back on
Home with the change still waiting. That `key=lambda` in `shelf.py` is
shown as it is, never `[***]`, is SCR-14's.

### TEAM-4 · A note on a change's line, sent with n

- records: 47, 49
- commands: —
- tier: stand-in

Steps: `scenarios/TEAM-4.scn`: a change waiting; in the diff `]`, `j j`, `c`, a note, enter; `n`; `J J`.

Expect: the note goes on the hunk line at the top (`note on test_shelf.py:…`), `note kept (1 on this change)`; `n` sends it (`1 note went to claude-1; the change went back to them`, one note said as one) and the change goes back: nothing of it left to decide, the journal says `sent back: claude-1`. (`>` quoting a hunk to the worker is SCR-3's line, not scripted.)

### TEAM-5 · Part applied, then the rest; commit from imece's own shell

- records: 13, 50, 56, 66, 68
- commands: :commit, :output, :history
- tier: stand-in

Steps: `scenarios/TEAM-5.scn`: a change; `x` on a hunk, `a`; the rest,
`a`; `:commit`, enter; `/` `!git log --oneline -3`; `O`; `/` `↑`; `^r`
`log`.

Expect: only the chosen hunk is staged, the rest waits as its own change
(the first key on it chooses it: record 125's hold on a new item); then
all is staged and ALL DONE; `:commit`'s line runs on imece's own pty and
says `[imece] exit 0 · … any key`, the commit is in the checkout; the
`!` line's output comes back with `O` as `I ran `git log --oneline -3``;
`↑` and `^r` bring back the lines sent. The message's words are
TEAM-14's.

### TEAM-6 · A clash with the person's own edit is sent back

- records: 108
- commands: —
- tier: stand-in

Steps: `scenarios/TEAM-6.scn`: a change waiting; a commit of the person's own on the same lines (`sh`); `gd`, `a`; the journal; the change opened; `b`.

Expect: the apply is refused (`did not apply: conflicts with the checkout in shelf.py`), the change says `✗ does not fit your checkout at … · b sends it back`, the footer `b  send the clash back`; `b` sends it and the change goes.

### TEAM-7 · Finished rows and the team finished

- records: 35, 98, 140, 145
- commands: —
- tier: stand-in

Steps: `scenarios/TEAM-7.scn`: a task, its asks said yes to, the change
applied.

Expect: while the change waits Home says `◐ TEAM FINISHED … 1 waits for
you`; once applied `ALL DONE` with `1 landed`, the worker's row
`idle · finished, applied` and the AT WORK heading `AT WORK 1 · finished
1`.

### TEAM-8 · A worker's helper

- records: 09
- commands: —
- tier: stand-in

Steps: `scenarios/TEAM-8.scn`: `--helpers 1`, a task with `[helper]`.

Expect: the helper `claude-1.1` stands under its worker (`└`), writes the tests and leaves; the worker's change waits with them.

### TEAM-14 · The one-shot sessions: a second read and a commit message

- records: 56, 106
- commands: :commit, :review
- tier: real
- after: TEAM-3

Steps: `scenarios/real/TEAM-14.scn`: a cheap worker's docstring change; its check; `:review` (the note above the files), `:review` again; the yes; `:commit`.

Expect: a tool-less session's note stands above the change's files (`✎ review · …`); a second `:review` is refused (`no review: …`); `:commit` puts `!git commit -m '…'` on the line with a message of the model's, not the stand-in's words.

### TEAM-9 · Fork a worker to try another way

- records: 94
- commands: :fork
- tier: stand-in

Steps: `scenarios/TEAM-9.scn`: a task done; `:fork @claude-1`; `J`; `:fork @lead`.

Expect: a new worker `claude-2` with the line opened `@claude-2 `; the journal says forked; a fork of the leader is refused in words.

### TEAM-10 · Mail held while a worker waits on the person

- records: 58, 107
- commands: —
- tier: real
- after: TEAM-9

Steps: `scenarios/real/TEAM-10.scn`: a cheap worker told to run `python3 -c 'print(6*7)'`; its ask left open; the leader asked to send it an inform; `J J`; its focus; the yes.

By hand, in the same run: `v` on the worker and `%` on an answer (reply numbers, `↩`).

Expect: the leader's `[inform]` is in the journal while the ask waits, and the worker's focus says the message is `queued`; after the yes the worker goes on and the round ends.

### TEAM-11 · A blocked worker is the leader's first

- records: 143, 146
- commands: —
- tier: real
- after: TEAM-10

Steps: `scenarios/real/TEAM-11.scn`: a cheap worker told to read `data/books.csv`, which does not exist, and to send `blocked` if so.

Expect: the worker sends `blocked`; the leader solves it (`unblocked`) or it comes to the person as a question in TO DECIDE (`… is still blocked · lead did not solve it`); the journal has the `blocked`.

### TEAM-12 · Health notices to the leader

- records: 128
- commands: :health
- tier: stand-in

Steps: `scenarios/TEAM-12.scn`: a change applied; `:health`; `:health idle 1m`; a minute and more with the worker idle and no card; `J`.

Expect: bare `:health` says `stall 10m · idle 10m · loop 3`; after the minute the journal says `claude-1 has been idle 1 min with no card` and that its leader hears of it.

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

### ASK-1 · A permission ask, a no and a standing yes

- records: 30, 89
- commands: :allow
- tier: stand-in

Steps: `scenarios/ASK-1.scn` (the stand-in's `[ask]` cue runs a command
that is no routine): `r` on the first ask; the next task, `A`; `:allow`;
a third task.

Expect: the ask names the command (`run Bash: python3 -m pip …`) and the
status line offers `a  allow once   r  refuse   e  yes this turn`; `r`
tells the worker no (its why says `did not run: The person refused
this`); `A` says `yes from now on to python3 -m pip …`, `:allow` lists
it, and the third time nothing asks.

### ASK-2 · A yes for the rest of a turn

- records: 104
- commands: :yes
- tier: stand-in

Steps: `scenarios/ASK-2.scn`: a task with `[ask]`; on its ask `gd`, `e`; `J`.

Expect: the journal says `you say yes to everything claude-1 asks until its turn ends`; the ask goes on.

### ASK-3 · A question with answers

- records: 27, 125
- commands: —
- tier: stand-in

Steps: `scenarios/ASK-3.scn` (the `[question]` cue): the question's row,
`gd`, `2`, the check's ask said yes to.

Expect: the row lists the answers with their digits (`1  yes, a line`,
`2  no, it is internal`); the preview reads it whole with `so far:`; the
digit answers at once and the row goes; the worker's change says `The
person said: no, it is internal`.

### ASK-4 · A focus decides its agent's ask, and says where it works

- records: 103
- commands: —
- tier: stand-in

Steps: `scenarios/ASK-4.scn`: a task with `[ask]`; `:goto claude-1`, `enter` (its focus); `a`.

Expect: the focus says `folder … · worktree of …`; `a` there decides that agent's ask and it is gone from TO DECIDE.

### ASK-5 · How much a session may do: standard, open, strict

- records: 26, 28
- commands: —
- tier: stand-in

Steps: `scenarios/ASK-5.scn`: a start (standard), `J J`; a start with `--confine open`, `J J`; a start with `--confine strict` and a task to the end.

Expect: the journal says `confine standard: workers get Claude Code's shell and web …` and `confine open: agents run with your own Claude Code setup …`; under strict the sessions run (no tool of imece's they were not given: record 30) and the change waits. Until 2026-11-11 every strict session was stopped at its start.

### ASK-6 · The teardown script and :tidy

- records: 92, 96
- commands: :tidy
- tier: stand-in

Steps: `scenarios/ASK-6.scn`: a worktree no agent owns made under the state with `git worktree add`; `:tidy`; `d`; the teardown's ask `a`.

Expect: `WORKTREES NO AGENT OWNS` lists it; `d` asks for the teardown script first, `a` runs it, the worktree is `removed` and gone from the disk. (The setup script's ask and run are in every scripted case.)

### ASK-7 · A worker's local server: its link on its row

- records: 45
- commands: —
- tier: real

Steps: `scenarios/real/ASK-7.scn`: a cheap worker told to start `python3 -m http.server 8765` in the background; its asks said yes to; the server answering; `:ps`; the terminal closed.

Expect: the server answers on `http://127.0.0.1:8765` and the address shows on the worker's row; though it left the worker's session (`nohup … &`), `:ps` lists it under the worker (the session's mark, record 129); closing the terminal quits imece and the server goes with it, and no `claude` of the case outlives the quit.

## Sessions and what is kept

### SESS-1 · The quit question, where you left off, the sessions resumed

- records: 19, 20, 24, 25, 34
- commands: :pending, :quit
- tier: stand-in

Steps: `scenarios/SESS-1.scn`: a change waiting; `q`, `s`, `esc`;
`:quit`, `q`; a start again; a line to the leader; `J`.

Expect: the quit question counts `changes not reviewed: 1`, `s` names the
worker; the quit is exit 0; the next start shows WHERE YOU LEFT OFF (`every
agent sleeps until spoken to`, `! 1 to decide:`); the leader's next turn
`goes on in the session it had, with all it knew`.

### SESS-2 · Forget a session, with a grace to undo

- records: 23, 78
- commands: :forget, :undo, :now
- tier: stand-in

Steps: `scenarios/SESS-2.scn`: `:forget @claude-1`, `u`; again, `U`; `J`;
a start with `--fresh`, `J`.

Expect: the bottom line counts down `… u keeps it · U now`; `u` keeps
the session; `U` forgets it at once and the journal says `you made
claude-1 forget its session`; a start with `--fresh` forgets every
agent's, the leader's too (`you made lead forget its session`).

### SESS-3 · Archive, browse, view and restore

- records: 80
- commands: :archive, :archives, :restore
- tier: stand-in

Steps: `scenarios/SESS-3.scn`: a task done; `:archive lending try`, `U`; `A`, `enter` on it, `a`, `esc`; `:restore lending try`; `J`.

Expect: the browser lists `lending try`; its view says `ARCHIVE · shelf “lending try”` and a key that would act says read-only; the journal says `you brought back shelf “lending try”`.

### SESS-4 · An account that cannot pay waits for the person

- records: 79
- commands: —
- tier: stand-in

Steps: `scenarios/SESS-4.scn`: a task with `[account]` (the leader's turn fails at Claude's API as the account's).

Expect: TO DECIDE at once: `API error, write to it once mended: Credit balance is …`; no `↻` try again. (Claude Code signed out is its own words of the same: the doctor's case, INST-3.)

### SESS-5 · The pot runs out and comes back

- records: 02, 48
- commands: :budget
- tier: stand-in

Steps: `scenarios/SESS-5.scn`: `:budget 0.05`; a task; `:budget 15`.

Expect: the team stops (`STOPPED` atop Home, its choices as digits) when the pot is spent; `:budget 15` says `of $15.00`. (The plan's usage windows come from real sessions: TEAM-2's header.)

### SESS-6 · The cost ledger

- records: 132, 133, 134
- commands: :ledger
- tier: stand-in

Steps: `scenarios/SESS-6.scn`: a change waiting; `:ledger`; apply it; `:ledger`.

Expect: `LEDGER · 1 attempt`, per project `applied $0.00 · wasted $0.00 · at work …`, the row `#1 try 1  claude-1`, `by model:`; after the apply `applied $0.12`.

### SESS-7 · The agents' processes

- records: 129
- commands: :ps
- tier: stand-in

Steps: `scenarios/SESS-7.scn`: a change waiting; `:ps`; `d` on the first row, `j`, `d`.

Expect: `PROCESSES` lists each session with what runs under it; `d` on the session itself (`that is claude-1's session itself: …`) or on imece's relay (`that is imece's bus for …: it stays`) is refused at once, with no `y` asked; nothing ended (`PROCESSES · 4` still). `d` on a command under a session asks (`y ends it, any other key keeps it`) and `y` ends it: by hand, with a worker's test running. What an earlier run left is SESS-8's runner check and the real run's.

### SESS-8 · The terminal goes away

- records: 86, 129
- commands: —
- tier: stand-in

Steps: `scenarios/SESS-8.scn`: a task done; a process started with a
session's mark that left it (`marked.py start`, as a `nohup … &` server);
then `/` `!true`, and the terminal closed while imece waits for a key
after it.

Expect: imece quits as on `:quit`: the marked process is gone within 15 s
(`marked.py gone`), and nothing that worked in the case's directory is
running ten seconds later (the runner checks this after every scripted
case). Until 2026-11-10 imece spun at full CPU here, deaf to SIGTERM;
until 2026-11-11 its last words to a terminal gone panicked, the process
aborted and the marked one lived on (ASK-7, the real run).

## Solo

### SOLO-1 · A solo beside the team, its model and effort

- records: 59
- commands: :solo, :effort, :model
- tier: stand-in

Steps: `scenarios/SOLO-1.scn`: `:solo`: the pick (haiku, its own effort); a task to the solo, its asks; `:effort medium`; `J`.

Expect: the solo `solo-shelf-2` works in its own worktree and its change waits (`ready to approve`), titled by the person's words (`add a docstring to shelf.py`), not by the solo's answer (`Changed shelf.py.`); `:effort medium` ends its session and resumes it on the new choice, the journal says so.

### SOLO-2 · A solo in a folder without git

- records: 73
- commands: —
- tier: stand-in

Steps: `scenarios/SOLO-2.scn`: `:solo $ACC_CASE/repos/plain claude:haiku`; a task; `:commit`; `:open` the folder as a team.

Expect: the header says `⌀ no git`; the solo writes in the folder itself (`Changed notes.txt.`, the line in the file); `:commit` and a team there are refused, each saying git.

## Projects

### PROJ-1 · A second project: narrowed, settled, pinned, renamed, closed

- records: 18, 43, 78
- commands: :open, :pin, :rename, :close, :settle, :unsettle, :fold, :unfold
- tier: stand-in

Steps: `scenarios/PROJ-1.scn`: `:open` tally, a task (its `npm test` asks nothing); `]` `[`; apply, `G`, `s`, `:unsettle`, `:fold`, `:unfold`; `:pin tally`; `:rename counter`; `:close`, `u`; `:close`, `U`; `:open` again.

Expect: `opened tally at …`; the change comes with no permission asked, and says its tests ran in the worker's own shell (`npm test passed in its own shell`), never `nothing ran`; `]` narrows Home to shelf, `[` to tally; `s` says `hidden until it needs you`, `:unsettle` `1 back in the team`; `tally is pinned to the top of the team`; the header says `shelf · counter`; `:close` counts down `u keeps it open · U now`, `u` keeps it (`counter stays open`), `U` closes it at once and `:open` brings it back.

### PROJ-2 · Ignored files copied into a worktree

- records: 57
- commands: :copy
- tier: stand-in

Steps: `scenarios/PROJ-2.scn`: `:copy .env`; `:copy`; a task; the journal as it is (`J J`); the worktree on the disk.

Expect: `.env is copied into shelf's worktrees …`, the list `copied into shelf's worktrees: .env`; the journal says `copied .env into claude-1's worktree` and `copied local.cfg …` (the `.worktreeinclude`); the file is there, not a link.

## Cards, the board and the plan

### CARD-1 · The plan the person sees first

- records: 114
- commands: :plan
- tier: stand-in

Steps: `scenarios/CARD-1.scn`: `:plan on`; a task with `[cards 2]`; the asks; the plan's row `enter`; `a`.

Expect: the first card goes to a worker, the rest waits in a plan (`plans 1 card for $0.05`); its window says `the leader hires a second worker only once you say yes`, each card's paths (`owns cli.py`); `a` lets it out and a second worker's change `#2` waits beside the first.

### CARD-2 · Paths a card owns, and a write outside them

- records: 114
- commands: —
- tier: stand-in

Steps: `scenarios/CARD-2.scn`: a task with `[cards 2] [stray]`; the asks.

Expect: a change says `outside #…`: its worker wrote a file its card does not own.

### CARD-3 · A card that failed, handed again, then withdrawn

- records: 126, 146
- commands: —
- tier: real
- after: CARD-2

Steps: `scenarios/real/CARD-3.scn`: a cheap worker on a card `Import books` told to say done with outcome failed; the leader asked to assign it again (a plan's yes if it hires); then to withdraw it; `:board`.

By hand, in the same run: a card after it (`after`) waits and is told `✕ #1 withdrawn`.

Expect: the card says `#1 failed` (no change); the journal says the card's `try 2`; after the withdrawal the journal says the leader `withdrew` it, Home `1 withdrawn`, and the board shows it marked `–` with its try.

### CARD-4 · A card's screen, the board, the code map and the timeline

- records: 04, 17, 32, 100, 125
- commands: :board, :code, :timeline
- tier: stand-in

Steps: `scenarios/CARD-4.scn`: a change waiting; `:board`, `r`, `q`; `:code`, `esc`; `:timeline`, `]`, `esc`; `/` `#1`.

Expect: the board `BOARD · cards: 1 · r ready only`, `TO REVIEW (1)`; the code map off Lonca says `nothing to show of who edits what`; the timeline `a column is 5 s`, `]` makes it 30 s; `#1` opens the card's screen in its owner's detail. Each window closes with `esc` or `q`.

## Lonca

### LONCA-1 · Import into Lonca, the policy signed, the Lonca window

- records: 115, 116, 117, 121
- commands: :lonca
- tier: real

Steps: `scenarios/real/LONCA-1.scn`: a daemon of the case's own (`bg lonca daemon`, its own `LONCA_HOME` and identities, with a session file of another daemon's in them); `:lonca import lonca-one`, `a`; `J J`; `:lonca`; `:lonca import` again; `lonca policy show`.

Expect: a session file another daemon left (written first) does not count: the import signs in anew and nothing stops as `not signed in`; the header says `lonca lonca-one …`; the journal `went into Lonca as repository lonca-one`; the window `is on Lonca repository lonca-one: trunk …`, `WAITING FOR A HUMAN · 0`, `HELD · 0`; a second import is refused (`… already`); the policy imece signed holds `escalate`.

### LONCA-2 · A worker in a workspace: its proposal is the Δ

- records: 119, 123
- commands: —
- tier: real
- after: LONCA-1

Steps: `scenarios/real/LONCA-2.scn`: its own daemon; an import; a cheap worker's change; `:lonca`; the change opened.

By hand, in the same run: `:code` while a worker holds a lease (its file, `· name` for others).

Expect: the window says `WAITING FOR A HUMAN · 1` and `Δ shown`; the change says `nothing runs after your yes` and, from Lonca, why it needs the person (`needs your yes: Lonca does not watch this agent's tools, so a person reads each change · proposal N`, no `approvals=1`).

### LONCA-3 · The Δ on Lonca: the yes lands it on trunk

- records: 120, 122, 125, 127, 148
- commands: :checks
- tier: real
- after: LONCA-2

Steps: `scenarios/real/LONCA-3.scn`: its own daemon; an import; a cheap worker's change; its verdict; `gd` `a`; `J J`.

By hand, in the same run: `r` on another proposal, its reason written (the worker hears it); `[verification]` set with `lonca policy set` and the verdict naming it; `:checks NAMES` on a change; with nobody at work, another proposal approved from a shell (`lonca approve SEQ`): the journal says `on trunk as …` within a couple of seconds, not half a minute.

Expect: the verdict says what runs after the yes (`nothing runs after your yes`, or Lonca's checks by name); the yes lands it: the journal says `on trunk as …`.

### LONCA-4 · Behind trunk, and :sync

- records: 118
- commands: :sync
- tier: real
- after: LONCA-3

Steps: `scenarios/real/LONCA-4.scn`: its own daemon; an import; a cheap worker's change, approved; `:sync`.

By hand, in the same run: a commit of your own in the checkout first: `:sync` says the branch and trunk went apart and changes nothing of yours.

Expect: the header says `↓1` once it landed; `:sync` takes it away and the checkout's log has the landed commit.

### LONCA-5 · The daemon goes away and comes back

- records: 116, 148
- commands: —
- tier: real
- after: LONCA-4

Steps: `scenarios/real/LONCA-5.scn`: its own daemon; an import; the daemon ended (`pkill`); started again; `J J`.

By hand, in the same run: a hire asked for while it is gone is refused in words.

Expect: the header says `no lonca` while it is gone and `lonca lonca-five` again after; the journal says the repository `can not be read` and then `answers again`.

## The screen

### SCR-1 · Home: what to decide first, then who works

- records: 33, 38, 124, 125
- commands: :home, :next, :defer, :waiting, :salon
- tier: stand-in

Steps: `scenarios/SCR-1.scn`: a task with `[cards 2] [question]`; `g` (and wait), `d`; `S`; `:waiting`, `:salon`, `:home`; `W`.

Expect: TO DECIDE groups `permissions` (`one key each`) and `questions`; `g` shows its second keys (`to decide`, `at work`); `S` puts the item off (`1 put off`) and opens the next; `:salon` is the team, `W` opens the next to decide in its detail.

### SCR-2 · The detail: diff, talk and log tabs

- records: 29, 102, 141
- commands: :trace
- tier: stand-in

Steps: `scenarios/SCR-2.scn`: a change waiting, opened; `tab`, `tab`, `⇧tab`, `tab`; three up the log and `enter`; `esc`; `t`.

Expect: `diff · talk · log`; the log lists `changed shelf.py`; `enter` on it opens the change at that file with `← the log's line` in its header, `esc` back to the log; `t` the raw stream.

### SCR-3 · Writing: the conversation above the line

- records: 12, 37, 39, 67, 95, 144
- commands: :edit
- tier: stand-in

Steps: `scenarios/SCR-3.scn`: `/`, `first line\` enter `second`, `^w`; `esc`; `@claude` `tab`; `see shel` `tab`; `^x ^e` with `VISUAL` a script; `:edit some words`.

Expect: the line takes two lines and `^w` deletes the last word; names and files complete (`@claude-1`, `shelf.py`); the editor's text comes back on the line, never sent by itself.

### SCR-4 · A conversation: turns, folds, choosing, export

- records: 42, 44, 70, 71
- commands: :talk, :export
- tier: stand-in

Steps: `scenarios/SCR-4.scn`: a change waiting; `:talk @lead`; `[ ]`, `zM zR`; `v`, `j`, `esc`; `:export $ACC_CASE/talk.md` twice.

Expect: the conversation with the leader; `v` chooses lines (`COPY ·`), `esc` leaves; `written to …` and the file holds the conversation; the second export over it is refused (`could not write`). Notes on words (`c`, `n` while choosing) are not scripted.

### SCR-5 · The journal, the story, a mark, clearing, recall, why

- records: 14, 46, 87, 90, 93, 99
- commands: :journal, :story, :mark, :clear, :unclear, :recall, :why
- tier: stand-in

Steps: `scenarios/SCR-5.scn`: a change waiting; `J`, `J`, `m`, `esc`, `'`; `:clear journal`, `U`; `:unclear`; `H` `mend`; `:why @claude-1`.

Expect: `STORY · J journal`, then `JOURNAL`; `marked`, `'` opens at it; `:clear` counts down and takes the lines off (`journal lines off the screen`), `:unclear` brings them back; recall finds `mend shelf.py`; `:why` says `claude-1: …`.

### SCR-6 · Jump, peek and back

- records: 51, 65, 72
- commands: :goto, :peek
- tier: stand-in

Steps: `scenarios/SCR-6.scn`: a change waiting; `&`, `esc`; `:goto claude-1`; `K`, `esc`; `^o`.

Expect: the jump line lists the agents; `:goto` goes to the worker; `K` shows its focus over Home (`FOCUS · claude-1`); `^o` goes back to where you were.

### SCR-7 · Help, the `:` menu, Turkish

- records: 40, 41, 125
- commands: :help, :lang
- tier: stand-in

Steps: `scenarios/SCR-7.scn`: `?`, `?` again, `/` `undo`, `esc`; `:` and
`tab`; `L`, `?`, `esc`, `L`.

Expect: `?` opens the help of where you are, `?` again all of it, the
filter keeps what matches; the `:` menu counts `· 1 of …`; `L` turns
Home and the help Turkish (`Sizi bekleyen yok, çalışan da yok.`,
`YARDIM`) and back. That no English word is left and nothing is cut is
EYE-1's.

### SCR-8 · A narrow screen

- records: 131
- commands: —
- tier: stand-in

Steps: `scenarios/SCR-8.scn`: a question waiting; the screen at 100×40,
60×20, 48×18, 39×11, then 120×40.

Expect: the question and its answers stay on the screen at every size
down to 48×18; under 40×12 one line says the size and no key acts
(`a` there decides nothing: the question is still there at 120×40).
How each size reads is EYE-1's.

### SCR-9 · Themes

- records: 15
- commands: :theme
- tier: stand-in

Steps: `scenarios/SCR-9.scn`: `:theme nord`; `:theme` a Ghostty colour file; `:theme zzzz`; `:theme mono`.

Expect: each is said (`theme: nord`, the file's name, `theme: mono`); a name that is none says it is `not one of` the list. How each draws, and that the choice is kept for the next start, is EYE-1's.

### SCR-10 · Motion off, calm, full

- records: 77, 81, 84, 85
- commands: :motion
- tier: stand-in

Steps: `scenarios/SCR-10.scn`: `:motion off`, `calm`, `full`.

Expect: each says what it does (`nothing moves on the screen`, `calm motion: a working row breathes …`, `full motion …`). The breath, the blinks, the cursor and drawing less in another window are EYE-1's.

### SCR-11 · A key file of your own

- records: 76
- commands: :keys
- tier: stand-in

Steps: `scenarios/SCR-11.scn`: a key file with a wrong line; `:keys`; the file mended; `:keys`; `?`.

Expect: the wrong file is refused whole, by its line (`line 2: …`); the mended one is `keys taken (1)`; `?` lists `gt`.

### SCR-12 · Away, the digest, how you are told

- records: 52, 60, 83, 91
- commands: :away, :digest, :notify
- tier: stand-in

Steps: `scenarios/SCR-12.scn`: `:away 1m`, `:away off`, `:digest`, `:notify bell`, `text`, `off`.

Expect: each says what it does (`back after …`, `what happened never shows by itself`, `nothing new since you last left`, `the bell rings when something waits …`, `a desktop note when something waits …`, `no word in another window`). A word or a ring in another window, a click on it, and a push are EYE-1's.

### SCR-13 · Keep the machine awake mid-turn

- records: 53
- commands: :awake
- tier: stand-in

Steps: `scenarios/SCR-13.scn`: `:awake`, `:awake`.

Expect: `the machine stays awake while an agent is mid-turn`, then `the machine may sleep again`. The `caffeinate` child while a turn runs is EYE-1's.

### SCR-14 · A secret-looking token is hidden, code is not

- records: 137
- commands: —
- tier: stand-in

Steps: `scenarios/SCR-14.scn`: a task whose words hold `GITHUB_TOKEN=ghp_…` and `max_tokens=5`, which the stand-in writes into `shelf.py`; the change opened.

Expect: the diff shows `max_tokens=5` as it is and never the token's value, which is in the file on the disk all the same.

### EYE-1 · What only an eye can judge

- records: 15, 53, 75, 77, 81, 83, 84, 85, 91, 101, 131
- commands: :lang
- tier: real
- skip-if: the push part, with no throwaway ntfy topic at hand
- after: SCR-14

Steps:
1. Over the real run's screens: Home, a change, a question, the help,
   the journal, at 120×40 and at 60×20, in English and with `L` in
   Turkish.
2. The first start's welcome (its mark's dots one a beat) and the
   farewell line after `q q`.
3. While a worker works: `:motion calm` and `full` (the breath, the
   spinner), the terminal's cursor where you type, `T` through the
   themes and a restart (the last one kept), `:awake` (`pgrep -fl
   caffeinate`), another window focused for a minute with `:notify text`
   and `:away 1m` (the note, its click, the digest on return), the
   activity bar on a working row; `:notify push` to a throwaway topic.

Expect: in Turkish no English word is left and nothing is cut that was
whole in English; at 60×20 every row that decides reads whole; the mark
comes dot by dot and is whole after a key; the farewell says what waits,
what sleeps, what this run spent and `resume: imece`; a working row
breathes, nothing moves with motion off or in another window; each theme
draws the whole screen; one `caffeinate` child while a turn runs and none
after; one note in another window, its click opens the item; the push
comes once.

## The demo

### DEMO-1 · The demo shows the screen on fake agents

- records: 01, 31
- commands: :pause, :faster, :slower
- tier: demo

Steps: `scenarios/DEMO-1.scn`: `imece --demo --projects 1 --agents 3`; `space`, `+`, `-`; `q q`; `imece --demo --lang tr`, `q q`.

Expect: shop-api's team AT WORK; it quits with exit 0; in Turkish it says `ana ekran`. Nothing is started or spent.

### DEMO-2 · The demo on the real Live, through the stand-in

- records: 130
- commands: —
- tier: stand-in

Steps: `scenarios/DEMO-2.scn`: `imece --demo-live --agents 3`, a task to
the leader, `gd` `a` on its plan, `q q`.

Expect: the leader plans the cards and its first worker's change waits in
TO DECIDE; at the yes more workers are AT WORK; the quit is exit 0 and the
demo's directory is gone.

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
