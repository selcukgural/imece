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
