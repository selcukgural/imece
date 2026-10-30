# Approval routing v0

Approval routing v0 (2026-10-30, posture PR d, part 1: Claude): what a
Claude Code session would ask before doing it goes to the person instead
of being refused. Under standard and open, a session with imece's server
runs `--permission-prompts host --permission-prompt-tool
mcp__imece__approve`; strict, and a session without the server, keeps
`none` (what would ask is refused). `Bash` left `ALLOWED_OUTRIGHT`
(`WebFetch`, `WebSearch` stay: they only read), so a shell command
Claude Code's own rules would ask about (its read-only commands and
`acceptEdits`' file commands in the directory run without asking) comes
to imece's `approve` tool. Only a Claude agent's socket offers
`approve` (`Live::bind` knows the vendor): Claude Code hides the
permission tool from the model, so `init` still reports exactly the
asked tools, while Codex's and Gemini's exact-tools checks would stop a
session that saw it. `claude::ask` turns the call (`tool_name`, `input`)
into `imece_agent::Ask` (`Command { tool, command }` for Bash, else
`Other { tool, detail }`: the path, address, query or pattern it names,
or its input as JSON cut at 200 characters); `claude::decision` answers
`{"behavior":"allow","updatedInput":<input as asked>}` or
`{"behavior":"deny","message":REFUSED}` ("The person refused this. Do
not try it another way; if you need it, ask them ...").

A ROUTINE command is allowed at once, with a trace line "allowed on its
own": one line (a shell runs each line as a command), that `run::split`
takes (no pipes, redirections, `&&`, `$`, globs) and that the run tool's
policy (`allowed`: cargo less the dependency commands, git's reading
commands, the person's `--run` programs; a leader less cargo's writing
commands) lets the role run. Anything else is an
`InboxKind::Approval` item in WAITING FOR YOU (`⚑`, "X waits for your
yes: Bash: …", each ask an item of its own, `World::asks_you`) and a
`Pending { what, input, reply }` in `Live::pending[agent]`, in the same
order as the agent's open approval items: `Live::decide(item, allow)`
answers the n-th pending call for the n-th open item (`a` allow, `r`
refuse), `World::decided` closes it and journals `Event::Decided {
agent, what, allowed }` (Person). The selected ask is shown whole under
its row, its lines joined with ` ↵ ` and never read as markdown: what
the person allows is what runs. The salon line of a working agent with
an open ask reads `⚑ waits for your yes`. A session's end (`Live::end`)
answers every pending call "the session ended" and closes its items;
the start closes any the store kept open (`adopt`: the process that
asked is gone). The call waits up to a day (`APPROVE_WAIT`); the MCP
server serves one call at a time per connection, so a waiting ask holds
that session's other imece tools, which it could not use mid-ask
anyway. The writer's prompt (Claude, standard and open) says which
commands run at once, that most others wait for the person's yes, and
that a refusal is the person's decision, not an error. Store schema 4
(the new tags only).

With it, git through imece (the run tool and the routine rule) takes no
option before its command (`-c`, `-C`, `--git-dir` change what it runs
and where) and none that starts a pager or writes a file (`-O…`,
`--open-files-in-pager…`, `--output…`; `-O<orderfile>` of diff and log
goes with them).

Probe facts (Claude Code 2.1.314, haiku, the ignored test
`live_claude_asks_imece_before_a_command_and_does_what_it_hears`): the
session starts with the permission tool on its server and `init` does
not list it; `curl … | head` comes to the tool as `{tool_name: "Bash",
input: {command, description}, tool_use_id}`; a deny counts in the
result's `permission_denials` (`denied: 1`) and the model said it would
not retry; an allow ran `python3 -c …`; `printf | tr | cat` ran without
asking (Claude Code's own read-only rule). An answer 100 s late was
waited for (earlier probe).

Part 2, Codex (2026-10-30, the Codex approvals PR): a writer (worker,
helper) under standard and open runs `approval_policy = "on-request"`
(`codex::approval_policy`; `-c`, the thread, and both echoes check the
computed value), so what needs more than the sandbox gives (a command
the model asks to run outside it, a write outside the directory) comes
as a server request instead of failing; a leader, and everything under
strict, stays `never`, where an approval request still stops the
session. `item/commandExecution/requestApproval` becomes
`Ask::Command { tool: "shell", command }`, with the shell Codex wraps
it in taken off (`/bin/zsh -lc '…'` from `sh`/`bash`/`zsh` bare or in
`/bin`, `/usr/bin`: `codex::unwrapped`; the inner command is judged as
it is, so `&&` inside is not routine); input to a running command
(`kind: writeStdin`) and a managed-network ask are `Other` (never
routine). `item/fileChange/requestApproval` shows the files the item
named when it started (`Protocol::changes`) and `writes under <root>`
for a `grantRoot`; `item/permissions/requestApproval` shows the asked
profile as JSON cut at 200 characters and grants it for the turn, or
`{}`. The old protocol's `applyPatchApproval`/`execCommandApproval`
stop the session. Each ask is an `imece_agent::Event::Asks { request,
what }` (request = the JSON-RPC id as text); `Agent::decide(request,
allow)` (Codex: `Msg::Decide` to the driver, `Protocol::decide`)
answers `accept`/`decline` once, and an ask that is gone is answered by
nobody. `serverRequest/resolved` for an ask still waiting, and the
turn's end for every one still waiting, is `Event::Withdrawn {
request }`: `Live::withdrawn` closes its item without a decision
(`World::withdrew(agent, nth)`, no journal line) and traces "no longer
asks". In live.rs a `Pending` now carries a `Reply`: `Socket { input,
to }` (Claude) or `Session(request)` (Codex, answered through the
session; a session's end answers nobody). Codex's writer prompt says
what waits for the person's yes and that a refusal is a decision.

Probe facts (Codex 0.165.0, ChatGPT plan, `@low`, the ignored test
`live_codex_asks_before_a_write_outside_its_directory_and_does_what_it_hears`):
asked to `touch` a file under `$HOME` (outside the directory and the
temporary ones the sandbox lets it write), the model asked at once;
the command came as `/bin/zsh -lc 'touch …'`, the first request id was
`0`; declined, the item ended `declined` (`denied: 1`), no file, and
the model said "refused"; accepted, the file was made.

Known gaps: Gemini's `session/request_permission` still refuses
(part 3); a Codex command wrapped in a shell elsewhere
(`/opt/homebrew/bin/zsh`) is shown and judged whole, so it is never
routine; an
`Other` ask shows what the tool touches, not all of its input (a
`Write` outside the directory shows the path, not the content); no
"allow for the session" rule, every non-routine ask waits; Claude Code's
own read-only rules decide what never reaches imece.
