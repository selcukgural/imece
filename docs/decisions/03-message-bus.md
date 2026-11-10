# Message bus v0

**Changed by [record 109](109-claude-only.md): Claude Code only; what is below about choosing a vendor is history.**

Message bus v0 (2026-10-28): one tool, `send(to, type, body)`, served by
imece itself over MCP: one Unix socket per agent in a fresh 0700 temp
directory; the session starts `imece mcp-relay <socket>`, which only
copies bytes. The sender is the socket, never an argument. Allowed by
exact name (`--allowedTools mcp__imece__<tool>`, each tool); `init` must report
exactly imece's server, connected, and exactly its tools. Mail waits per
agent; waking types start the next turn when the agent has no prompt
left, inform/progress/notice ride along. Caps: body 4000 chars, 20
unread, 12 message-driven turns per agent until the person speaks again;
a stopped agent gets no waking mail, nobody does once the budget is spent,
and mail to an agent that waits on the person stays queued (record 107).
Each message gets a number, and a reply may name the one it answers
(`re`): see Reply-to numbers (record 58).

**A socket serves its own session (2026-11-03).** The sender is the
socket, so a socket must take calls only from its agent: each call's
peer (`mcp::peer_pid`: macOS `LOCAL_PEERPID`, Linux `SO_PEERCRED`) must
be the process imece started for the session (`Agent::pid`: Claude's and
Gemini's CLI, Codex's app-server, followed across a Codex restart) or
one of its descendants (`child::descends`, at most `PARENTS_MAX` 64
parents up, a process gone or a parent untold is a no). Anything else is
refused with `this socket takes calls from its own agent's session only`
and a trace line on the agent (`send: called on this socket from outside
its session, refused`); nothing of it runs or reaches the journal.
`Bus.owners` holds each socket's session pid, updated every step
(`own_sockets`); an agent with no session yet (pid 0) takes no call.
A session being started is `STARTING` (named at once by the prompt that
starts it, 2026-11-09): its first calls can come before the main loop
takes its start (imece's stand-in, record 130, did on CI), so such a
call waits for the pid (`owner_of`, at most `OWNER_LOOKS` 250 ×
`OWNER_STEP` 20 ms) and is then judged as any other; still starting
after that, it is refused. A socket with no session refuses at once.
Probe facts: Claude's and Codex's `mcp-relay` are direct children of
the vendor process; the live Codex test passes with the check, the live
Gemini test could not finish on a free-tier key that day (not verified
live; its unit tests check the pid). The demo has no sockets: nothing to
show.

**A call is done once (backlog F18 in #59, 2026-11-02).** A tool call
waits for imece's answer (`call_wait`: 30 s, a run its timeout + 30 s, an
approval 24 h), and imece answers when its main loop steps. While a
`!cmd` holds the screen the loop does not step, so calls waited out and
the agent heard "try again", but the call stayed in the queue and was
done when the command ended: a retried `hire` made two workers, a
retried `send` two messages. Now a call carries a flag (`Call`'s
`Arc<AtomicBool>`), and either imece takes it or its caller gives up on
it, whichever turns the flag first (`live::answer_of`): a call given up
is dropped when imece gets to it, with a trace line on the caller
(`hire: given up before imece got to it, not done`); a call imece took
is waited for once more, and if that is late too the caller hears that it
was done and must not be made again. The demo has no bus to wait on, so
there is nothing for it to show. A model that calls `hire` twice on
purpose still makes two workers (within `--agents`): that is a choice,
not a retry.

**whoami (backlog F17 in #59, 2026-11-03).** Every role has a `whoami`
tool that changes nothing and says, in plain lines, who the caller is in
imece: its name, its role, its project, who it answers to (a helper the one
that hired it, a worker or researcher the leader, the leader and a solo the
person, `send to: you`), what is left of the one pot of the budget and of
how much, and whether it may hire: `yes, N more workers/helpers (you
have H of at most C)` or `no:` with the very words `hire` would refuse
with, since both read `Live::hire_room`. An agent that lost track of its
place after a compaction, or a model that would guess at the caps, can
ask instead. The other half of F17 was there already: every tool's answer
says what it did and what comes next (`posted as #3; an idle worker will
be offered it`, `delivered as message 8; claude-1 starts on it now`,
`claude-1.1 joined …; give it work with send (type assign); 1 more may
be hired`, a claimed card's `When it is done, send lead a done
message`). The demo has no bus, so there is nothing for it to show.

**Group addresses (backlog F12 in #59, 2026-11-03).** The leader may
`send` to a group of its project's workers instead of one: `workers`
(every one not gone), `idle` (idle with no turn under way), or a vendor
it leads with (`claude`, `codex`, `gemini`: its workers of that vendor),
`bus::Group::parse`. Only `inform` and `notice` go to a group (work goes
to one worker with `assign`, or onto the board), and no `re`. Each
worker gets its own message through `Live::message`, so every cap and
check of a message to one holds per worker (stopped, the unread cap,
the budget); the answer lists who got it with its number and who did
not with why, and is an error only when nobody got it. A teammate whose
name is a group's word is written to as itself: a name comes first. The
caps: a group is at most the team (`--agents`), each worker's unread
cap (`MAX_MAIL`) holds, and nothing in a group wakes anyone, so it
starts no turn and costs nothing until the next turn reads it. Other
roles are refused (`only the leader writes to a group`). The demo's
leader tells the other workers, one inform each, when the person applies
a change (`Sim::tell_the_others`).


**Later (record 143).** A worker's `blocked` to its leader opens a block
the leader is to solve first; `re` may go on the leader's ask to the
person when it names that blocked, and nowhere else to the person.
