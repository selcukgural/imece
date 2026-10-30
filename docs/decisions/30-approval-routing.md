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

Known gaps: Codex's approval requests and Gemini's
`session/request_permission` still stop or refuse (parts 2 and 3); an
`Other` ask shows what the tool touches, not all of its input (a
`Write` outside the directory shows the path, not the content); no
"allow for the session" rule, every non-routine ask waits; Claude Code's
own read-only rules decide what never reaches imece.
