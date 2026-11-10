# The stand-in for Claude Code

Record 111 put the demo on the real `Live` with a `claude` stand-in that
speaks stream-json, so that `Engine::Sim` and its arms in `app/` can go.
The person's answers (2026-11-08): staged, `Sim` deleted in the last pull
request; the demo at `Live`'s limits (`--agents` up to 16 per project, a
small default, the 200-agent stress view gone); Lonca's fake daemon built
into the binary for `--demo` only. This record is the first step: the
stand-in, and the real `Live` run on it end to end in the tests. The demo
does not use it yet.

`imece stand-in ARGS…` (`imece_agent::stand_in`, dispatched in `main`
before anything else, as `mcp-relay` is) takes the command line imece gives
Claude Code (`claude::args`) and reads what it needs of it: `--tools` (a
list, or `default`: Claude Code's edit, shell, web and task tools less
`--disallowedTools` and the tools that start agents), `--permission-mode`,
`--permission-prompt-tool` (hidden from the tools it reports, as Claude
Code hides it), `--mcp-config`'s servers, `--model`, `--resume`,
`--fork-session` with `--session-id`; other flags that take a value are
passed over with it, a flag without its value or a config that does not
parse exits 2. `--version` says `0.0.0 (imece stand-in)`, `auth status
--json` says signed in. A session starts each MCP server in its
directory as their client (`Server`: `initialize`, `notifications/
initialized`, `tools/list`, then `tools/call`; an answer to another id or a
notification in between is passed over), reports `init` (session id,
model, permission mode, tools including `mcp__<server>__<tool>`, each
server `connected` or `failed`, two slash commands), then takes a prompt a
line (`claude::prompt_line`) and ends each turn with its text as an
assistant message and a `result`. Each tool call is a `tool_use` and its
`tool_result` (`is_error` when the call failed); a session without imece's
server calls none of its tools. `/clear` is a new session id and `init`, an
empty context and the cost counter from nothing, at no cost. imece's
parser (`claude::Parser`) and its checks run on all of it as on Claude
Code: the stand-in is checked, it checks nothing.

What it does (`Mind`): `whoami` once per conversation gives its role. A
leader hires one worker (`hire` with no arguments, the name the answer's
first word) and `send`s it an `assign` with the person's last paragraph
(`TASK_MAX` 200 characters); after that every prompt goes to the same
worker; mail from its workers is read and the turn names who said done. A
worker given an `assign` writes `stand-in/<name>.md` in its directory (the
task's first line as a heading, then the task) and sends its leader (from
`whoami`'s `you answer to: the leader NAME`) a `done`, `wrote PATH`; a
failed write or send is its turn's text. Anyone else says there is nothing
to do. The figures are made up and the same every run: a turn costs
`TURN_COST` $0.01 and each tool call `CALL_COST` $0.005, the context
starts at `CONTEXT_START` 2,000 tokens, grows 150 a message and
`CONTEXT_TURN` 1,500 a turn, in a `WINDOW` of 200,000. It asks no model,
spends nothing and reaches nothing but its MCP servers and its directory.

`stand_in::install(dir, imece)` writes `dir/claude`, 0700, `exec 'IMECE'
stand-in "$@"` (the path quoted for the shell): a session's program is one
path. The test `live::end_to_end` runs the real `Live` on it: a leader
told "Write a greeting", a real bus over `mcp-relay`, a worker hired
through it in a real worktree, its change read and waiting for the person
with the file and the words in its patch, every session's cost booked.
The unit tests find imece's binary one directory above theirs
(`target/<profile>/imece`); `tests/stand_in.rs` runs the binary's
stand-in, and being an integration test makes cargo build the binary
before the unit tests run. A test that runs without it built says so and
fails.

What the run found on Linux: `mcp-relay` copied with `io::copy`, which
there moves bytes between a pipe and a socket with `splice`, and the
server's reply to the first `initialize` never reached the session: the
run waited for good every time (macOS has no `splice` and passed). The
relay now copies with a read and a flushed write of its own
(`mcp::pump`); the same relay is what Claude Code starts, so this was a
bus that could hang a real session on Linux.

Growth: nothing kept; a session's context and counters are in its
process's memory. Concurrency: one stand-in per session, as Claude Code;
its MCP servers are its children, killed when it ends (their stdin
closed first). Failure: a server that does not start or answer is
`failed` in `init`, and imece stops the session, as it would Claude
Code's; stdin closing ends the stand-in.

Next, in this order: the demo (`--demo`) on `Live` with the stand-in on
temporary git checkouts and its own state directory, transcripts written
where `claude::transcript` looks so a resume is found, the demo's behaviours
moved from `sim.rs` into the stand-in's mind (asks to the person,
approvals through `approve`, helpers, faults, rate limits, to-do lists,
strays, the plan gate, the commit and review one-shots), Lonca's fake
daemon for the demo; then `Engine::Sim` goes.

## The demo on `Live`, first step (2026-11-09)

The person's answers (2026-11-09): `--demo` stays on `Sim` until the
stand-in does what `Sim` does; a hidden `--demo-live` runs the demo on the
real `Live` meanwhile, and the pull request that deletes `Sim` gives it
`--demo`'s name. The demo on `Live` runs on real time: no pause, no speed,
no 14:00 start; `--seed` will only seed the stand-in's choices.

`live::demo::world(projects, agents, imece)` makes one directory of the
run's own, `imece-demo-PID` under the temporary directory, 0700, named by
its real path (Claude Code keeps a transcript by the directory a session
ran in as it sees it, and macOS's temporary directory is a link): `bin/`
for the stand-in's program, `home/` as its `CLAUDE_CONFIG_DIR`, `state/`
for the store and the worktrees, `src/<name>` for each project (the
simulation's names and files, `sim::repos`, as many as asked, 1 to 4): a
git checkout on `main` with one commit, the person's hooks and signing
kept out. `live::world` takes `Options.claude: StandIn { program, home,
relay }` for it: the session's program, the configuration directory
`Live` looks for transcripts in, and the imece whose `mcp-relay` the
sessions start (imece's own binary; `current_exe` is not, in a test).
Projects after the first are opened as teams. There is no probe, no
welcome and no news; the farewell says the demo keeps nothing. `Demo`
removes the directory when dropped, after the app (its sessions end
with it, reaped).

`--demo-live` takes `--projects` and `--agents` (`Live`'s limit of 16, by
default `DEMO_LIVE_WORKERS` 3: the scale record 125 designs at);
`--researchers` and `--seed` stay `--demo`'s.

The stand-in keeps transcripts now: `install(dir, imece, home)` sets
`CLAUDE_CONFIG_DIR` in the program it writes (a session's environment is
cleared, so imece's own would not reach it), and under it each session
adds a line to `claude::transcript(home, cwd, session)` at its start and
after each turn: its mind (`Mind::to_json`: `whoami`'s lines and the
workers it hired), what it has spent and its context (`Kept`). A
`--resume` reads the last line of that session's transcript in whichever
directory it ran (`kept`; a fork goes on from another directory's) and
goes on from it, reporting the whole session's cost as Claude Code does;
one not found starts afresh (Claude Code would refuse; `Live` resumes only
what it found the transcript of). Without `CLAUDE_CONFIG_DIR` it keeps
nothing, so a test never writes in the person's `~/.claude`. The test
`a_session_of_an_earlier_run_resumes_from_its_transcript` runs `Live`
twice on the same state: the leader's session resumes, gives the new
work to the worker it hired before, and only the new turns are booked.

Growth: the transcripts grow a line a turn and go with the demo's
directory. Failure: a transcript that cannot be written is said on the
stand-in's stderr and leaves a resume to start afresh; a checkout or the
stand-in that cannot be made ends the start with what failed, the
directory removed.

Next, unchanged: the demo's behaviours from `sim.rs` into the stand-in's
mind, Lonca's fake daemon for the demo, then `Engine::Sim` goes and
`--demo-live` becomes `--demo`.

## A worker that works (2026-11-09)

The first of the simulation's behaviours moved into the stand-in's mind:
a worker's turn on an `assign` is the one the simulation draws, made of
real tool calls (`stand_in::work`). It makes its to-do list with
`TaskCreate`, one task per step of `work::STEPS` (the simulation's five),
and marks each `in_progress` then `completed` with `TaskUpdate`, so
`Event::Plan` and the row's `≡ 3/5` come from imece's own parser. It
`Read`s the file of its directory that names most of the task's words
(`work::pick`: letters and digits, three at least, folded; not a test or
JSON when another file is there; equals by the roll), adds the task to it
as comment lines of its kind (`mark_of`: `// ` for code, `# ` for scripts
and settings, plain for the rest) with one `Edit` of its whole text (a
`Write` when it is empty), and notes `covers: <title>` in the test file
whose path names that file's name (none when no test does). It runs the
tests of the file's kind with `Bash` (`kind_of`: `dotnet test`, `gradle
test`, `npm test`, `go test ./...`, `cargo test`; none for another file)
and sends its leader `done`: the task's first line, then `Changed …` and
what each command said. One assign in `asks` first runs a command imece
holds no routine (`Kind.asks`: a shell's operators, or a program not on
the run list), so imece asks the person through the permission prompt.
A directory with nothing to change gets the task written in
`stand-in/<name>.md`, as before. Its commands are never run: a test run
says `All 12 tests passed.`, anything else `Done.`.

The stand-in's own tools now act as Claude Code's do (`Busy::tool`): a
`tool_use` streamed before the work and its `tool_result` after; a tool
the session has not got is refused unstreamed (`No such tool available`);
paths are named whole in the stream and must stay in the session's
directory (no `..`, nothing outside); `Bash` and the web ask before they
run unless `--allowedTools` names them (now read), through the
`--permission-prompt-tool` server's tool with `tool_name`, `input` and
`tool_use_id`, hidden from the stream as Claude Code hides it; `allow`
runs it, `deny` is the call's error with its message, and nobody to ask
or an answer not understood is Claude Code's `Claude requested
permissions to use …, but you haven't granted it yet.` Edits ask nothing:
imece runs Claude Code under `acceptEdits`.

How it works comes from its program (`Habits`, written into the program
by `install` as `IMECE_STAND_IN_PAUSE_MS`, `IMECE_STAND_IN_ASKS`,
`IMECE_STAND_IN_SEED`): a pause between steps (at most `PAUSE_MOST` 10 s),
one assign in how many asks first (never at 0), and the seed of its
choices (`roll`: FNV-1a of seed, agent name and its assign's number, kept
in the transcript as `Mind.worked`). Tests install it with none of them;
the demo with 1.5 s, one in three and `--seed` (now `--demo-live`'s too),
and names the kinds' test programs as its run list
(`work::TEST_PROGRAMS`), so a test run is allowed at once and what asks
waits in TO DECIDE. `live::end_to_end` runs it on the real `Live`: the
change waits with the task in `a.txt` and the list all done; with `asks`
1 the command waits for the person, nothing changes before the yes, and
the `done` says what it heard.

What it found: `World::done_why` read the newest `done` from the journal,
where live keeps a message's first line only, so a change's "why it
waits" was never shown on live (the simulation journals whole messages).
It now reads the whole message from the cards' stories, by its number,
when the journal has only its first line.

Next: questions to the person with options, helpers, faults and retries,
rate limits and context, strays and the plan gate, the leader's board
and cards, the commit and review one-shots; then Lonca's fake daemon,
then `Engine::Sim` goes.

## A worker that asks the person (2026-11-09)

One assign in `Habits.questions` (`IMECE_STAND_IN_QUESTIONS`, never at
0; by the roll's bits from the 16th, so it falls apart from `asks`) has
the worker ask the person once it has read the code: `send` to `you`,
type `ask`, one of `work::QUESTIONS` about the file it read (by the
roll's bits from the 24th), every other with the answers it offers as
`options` (record 125), the rest none. It ends its turn there, nothing
changed: the card stays its own and working, its mail waits (record
107), and the question waits in TO DECIDE with its answers as digits.
The task and its to-do list's tasks wait in `Mind.waiting`
(`work::Waiting`, kept in the transcript, so a resumed session still
waits). The next prompt with the person's words (`work::heard`: its last
paragraph before the mail that came with it, at most `TASK_MAX`) goes on
from the list's second task, as before, and the `done` says `The person
said: …` after what changed; a prompt of mail alone keeps it waiting. A
question the bus refuses is not waited for: the work goes on in that
turn. The demo asks in one assign of four (`QUESTIONS` 4). The test
`a_worker_s_question_waits_with_its_answers_and_the_answer_goes_into_its_change`
runs it on the real `Live`: the question's text and options from the
journal (`World::question_of`), no change before the answer, the answer
given as a digit gives it, and the change waits on the same card with it
in its "why".

`Sim` also has its worker go on with what does not depend on the answer
after `QUESTION_WAIT`; a live session waits until it is told, so the
stand-in does too, and that goes with `Sim`. What the person is told
without a question (`Sim`'s assumptions, an `inform` to `you`) is not
moved yet.

Next: helpers, faults and retries, rate limits and context, strays and
the plan gate, the leader's board and cards, informs to the person, the
commit and review one-shots; then Lonca's fake daemon, then `Engine::Sim`
goes.

## A worker that hires a helper (2026-11-09)

One assign in `Habits.helpers` (`IMECE_STAND_IN_HELPERS`, never at 0; by
the roll's bits from the 32nd, apart from `asks` and `questions`) whose
file has a test file has the worker hand its tests on once it made the
change: `hire` with `tier: cheap` (the answer's first word is the name,
`claude-1.1`), then `send` an `assign` to it, `work::helper_task`: `Write
the tests in TEST for the change to FILE:` and the task. It ends its turn
there, its list's third task in progress; `Mind.waiting` keeps the task,
the list, the helper and what the person said (`Waiting.helper`,
`Waiting.said`, in the transcript). A hire refused or an assign not sent
waits for nobody: the worker writes the tests itself in that turn.

A helper (`role: a helper`, `Mind::help`) reads its part back
(`work::part_of`), notes `covers: …` in the test file as the worker would,
runs the tests of the file's kind, and sends its worker `done`: `Wrote the
tests in TEST.` and what the run said; imece lets it go when that turn
ends (record 09). A part it can not read, or a test file it can not
write, is said in its `done` instead.

The worker goes on at the prompt that carries its helper's `done`
(`work::done_from`, by the heading `[done …] from NAME:`): with `Wrote the
tests in` the test file is the helper's and the line goes in the worker's
`done`; any other `done`, or imece's note that the helper stopped before
it (`work::lost`, record 09's `helper_lost`), has the worker write them
itself and say so. Other mail, or the person's words, keeps it waiting.
Then the tests run and the `done` goes to the leader as before. The demo
hires in one assign of three (`HIRES` 3). The test
`a_worker_s_helper_writes_its_tests_says_done_and_leaves` runs it on the
real `Live`: the helper under its worker, gone after its `done`, one
change of the worker's with both files, and the helper's line in its
"why".

Five end-to-end tests in one process opened their buses in the same
microsecond (macOS's clock) and one refused the other's directory: a
bus's directory now ends with the process's count of buses
(`live::bus_dir`).

`Sim`'s helper is hired by chance at any step of a card and works for a
number of ticks; the stand-in's has a part of its own to do, so it is
hired only when there are tests to write. A helper lost mid-part is not
made to happen by the stand-in (its sessions do not fail yet): faults
come next.

Next: faults and retries, rate limits and context, strays and the plan
gate, the leader's board and cards, informs to the person, the commit and
review one-shots; then Lonca's fake daemon, then `Engine::Sim` goes.

## A turn that fails at Claude's API (2026-11-10)

One turn in `Habits.faults` (`IMECE_STAND_IN_FAULTS`, never at 0; by the
bits from the 40th of `roll(seed, --name, the session's turn)`, so any
session's turn, the leader's too) fails before any work, as Claude Code
says an API error (record 79's probe): of those, one in `ACCOUNT_IN` 3
(the bits from the 48th) is the account's, a bill unpaid refused at once
(`api_error_status` 400, `Credit balance is too low`), the rest an
overloaded API after Claude Code's own `TRIES` 3 tries, each an
`api_retry` line (`max_retries` 10, 500 ms doubling, the habits' pause
between them), then `is_error` true, `terminal_reason` `api_error`,
status 529. Nothing is spent on it. imece's parser and `World::turn_failed`
do the rest as with Claude Code: a passing fault is tried again after
30 s with `RETRY_PROMPT` (now `imece_agent`'s, so the stand-in knows it),
the account's waits in TO DECIDE until the person writes.

The failed prompt stays in the session, as Claude Code keeps it
(`Mind.failed: Failed { prompt, account }`, with `Mind.turns` in the
transcript, so a resumed session still owes it). The next turn never
fails, so imece's three tries are never used up, and does what
`Failed::then` says: imece's try again is the failed prompt alone; after
the account's fault the person's words are the word to go on (imece
holds the agent's mail while it waits, record 107); any other prompt
(mail, or the person's words after a passing fault) is done after the
failed one in the same turn, the two texts a blank line apart. `--name`
is read for the roll. The demo fails one turn in eight (`FAULTS` 8). The
test `a_turn_that_fails_at_the_api_is_tried_again_and_the_account_s_waits_for_the_person`
runs it on the real `Live`: the leader's first turn fails and is tried
again once the clock says so (moved in the test, not waited for), then
hires and assigns; its worker's first turn fails on the account, waits
with no retry, and does its assign at the person's word.

`Sim` fails only answers to the person, every third and seventh; the
stand-in fails any turn, so a worker's and a helper's fail too. A
passing fault past its third try is not made to happen (as in `Sim`),
and a helper is not lost mid-part by it: its turn is tried again.

## The plan's windows and a context that fills (2026-11-10)

**The windows.** With `Habits.limit` (`IMECE_STAND_IN_LIMIT`, millionths
of a dollar; none said at 0) and a home, each turn's end adds what the
turn spent, its calls and itself, to `<home>/stand-in-usage` (a line
`SECONDS MILLIONTHS`, appended: the plan's windows are one account's, so
every session of a run fills the same ones) and says Claude Code's
`rate_limit_event` just before its result (`stand_in::spend`, `limits`,
`Stream::done`): `status` `allowed`, `unifiedWindows` `five_hour` and
`seven_day`, each `utilization` a whole percent for every `limit` spent in
the five hours `now` is in (four times that for the week), never over a
hundred, `resetsAt` the next whole window of the clock, as `Sim::limits`
counts them. A failed turn and `/clear` spend nothing and say none. The
clock is `session`'s argument (`run` gives the machine's). imece's parser
and `World::limit` do the rest; the demo's part is a cent (`LIMIT`
10,000: a dollar fills five hours; the header names a window from 90%
used, `:budget` at any time, so a demo's first minutes show none).

**The context.** A message adds `Habits::message()` tokens
(`IMECE_STAND_IN_FILL`, at 0 the 150 it always was) and a turn ten times
that, added before the turn's last message so the message says it
(`Stream::grow`). Past `AUTO_COMPACT` (nine tenths of `WINDOW`) the
session compacts on its own: a `system` line `compact_boundary` with
`compact_metadata { trigger: "auto", pre_tokens }`, the shape Claude Code
2.1.322's own code writes (read in its program, not seen from a run), and
the context is `CONTEXT_START` again. `/compact` (it was in
`slash_commands` and did a turn's nothing until now) writes the boundary
with `trigger` `manual`, answers `COMPACTED` in a message that says the
context now, costs a turn (a model writes the summary) and keeps the mind;
not echoed, as no command is. The demo's message is 3,000 tokens (`FILL`):
a worker's second assign compacts.

**Live too.** imece's parser said nothing of a compaction; now
`compact_boundary` is a trace note (`claude::compacted`: `context
compacted by the session: it held 181000 tokens`, without `by the session`
for any other trigger, without the figure when none is said), as the
simulation's was. The test
`the_plan_s_windows_fill_with_every_session_s_turns_and_a_full_context_compacts`
runs all of it on the real `Live`.

Not made to happen: a window at a hundred refuses nothing (Claude Code's
`rejected` status and the turn that fails on it are not probed), and a
compaction is free but for `/compact`'s turn.

## A leader that plans, cards that own paths, the board (2026-11-10)

**The leader** (`stand_in/lead.rs`). With `Habits.cards`
(`IMECE_STAND_IN_CARDS`; under 2 the task goes whole to one worker, as
before) the person's task is cut into cards (`lead::parts`): one per
file of code in the leader's directory (of a kind that has tests: no
document, no settings), those the task names best first
(`work::code_for`), at most `cards` and never more than `CARDS_MOST` 8,
each owning its file and the test that names it (`work::test_for`; a
test an earlier card owns is left out, so no two cards own a path),
its task `FILE: the person's words`, its title that task's first line cut
to imece's 120 characters. Fewer than two files of code: no cards, the
task whole.

`Mind::hand_out` gives them out in order: to the workers it has (a new
task's cards), then to workers it hires, each with `send type assign`
and the card's `paths` (record 114). When `hire` is refused with a word
of a plan (imece's plan gate: no second worker before the person's yes)
the cards left go to the `plan` tool, five cents a card, are kept in
`Mind.planned` (in the transcript), and the turn ends: `claude-1 has
src/money.rs: …; my plan for 3 more cards waits for your yes.` When it is
refused for anything else (the team is full) they go on the board with
`post`, as they do when the plan itself is refused. With nobody hired
and no hire, it says so and does nothing.

The person's answer comes as imece's note (`PLAN_YES`, `PLAN_NO`, read
by how they begin): at a yes the planned cards are handed out, to new
workers only (the ones it has keep their cards), the rest posted once
the team is full; at a no the plan is dropped and said. Until then mail
is read as ever and the plan waits. Two things the leader did wrong
before and now does not: mail that came with a note of imece's was taken
for the person's task (it looked for the mail at the prompt's start),
and so was imece's bare wake (`GO_ON`).

**The worker.** A task that ends with what its card owns (`bus::owns`'
words, read by `work::owned_of`) is done in the code and the test it
owns (`work::chosen`; a card that owns no file that is there falls back
to what the task names, `pick`), and what imece added to the task stays
out of the notes it writes (`work::plain`). One assign in `Habits.strays`
(`IMECE_STAND_IN_STRAYS`, never at 0; the roll's bits from the 56th, the
file by those from the 60th) of a card that owns paths also changes one
file of code outside them (`work::outside`), before its tests run, and
its `done` says `Also changed FILE, which the card does not own.` imece
sees the write as it sees Claude Code's and does the rest (the card's
story, the journal, the note in the next prompt, the Δ's `outside` line).

Offered the board's open cards (`board_prompt`), a worker takes the
first with `claim` and does the task of its answer (`work::offered`,
`work::claimed`) as an assign; an assign in the same prompt comes first,
and a claim refused is said.

The demo cuts a task into four cards (`CARDS` 4: with the three workers
`--demo-live` allows by default, the gate holds the second hire and the
fourth card goes on the board) and strays one assign in three (`STRAYS`
3). The test
`a_leader_plans_its_cards_hands_them_out_at_the_yes_and_a_worker_takes_one_from_the_board`
runs it on the real `Live` with two workers and three cards: one hired
and assigned, the plan with the other two in TO DECIDE, the yes, a
second worker hired, the third card posted, offered and claimed, every
card's change waiting and every card's story with its one stray.
`the_stand_in_reads_imece_s_words_as_imece_writes_them` holds the words
the stand-in reads to imece's own.

Against `Sim`: its leader proposes a card per waiting card of the board
with a tier each and `after` between them; the stand-in's plan has no
tiers, no `after` and no checks, and its leader never withdraws a card,
hands one again or answers a failed one. A no to the plan ends the
planning (a real leader would ask); the person's words to it are a new
task.

Next: informs to the person, the commit and review one-shots; then
Lonca's fake daemon, then `Engine::Sim` goes.

Under `--replay-user-messages` (every session since record 135) it echoes each prompt as it takes it, `"isReplay": true`, before the turn's work; its commands (`/clear`) are not echoed, as Claude Code's are not.

## The person's cues (2026-11-10)

A scripted acceptance case (record 147) must make a thing happen when it
wants it, not one assign in so many. `stand_in::work::Cues::of` reads
words in brackets in the person's words, which the leader hands on with
the task: `[ask]` runs a command that is no routine (the kind's ask),
`[question]` asks the person with answers, `[helper]` hires a helper for
the tests, `[stray]` changes a file the card does not own, `[cards N]`
cuts the task into N cards (2 at least), `[fault]` and `[account]` fail
the turn the person's words start at Claude's API (passing, or the
account's; mail never fails by a cue, so a worker's assign goes through).
A cue acts whatever the habits say, so the scripted world runs with no
habit at all (`tools/acceptance/setup.sh --stand-in`: a pause of 150 ms,
nothing by chance). A Python kind joined the others (`.py`: `python3 -m
unittest`, which imece's runners make routine in a checkout with
Python's tests (record 136); its ask `python3 -m pip --version`), for the
acceptance run's sample `shelf`.

## A solo (2026-11-11)

A solo session (record 59) did nothing on the stand-in ("Nothing for me
to do here."), so no case of a solo could be a script. `Mind::solo`: the
person's words are its task, done at once: the file they name best
changed and its tests run, or, in a folder with no code, a note written
under `stand-in/`; what it did is the turn's text (`Changed shelf.py.`),
and it sends nothing, since it answers to the person alone. Mail alone,
or imece's wake, is no task.
