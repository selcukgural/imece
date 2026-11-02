# Message bus v0

Message bus v0 (2026-10-28): one tool, `send(to, type, body)`, served by
imece itself over MCP: one Unix socket per agent in a fresh 0700 temp
directory; the session starts `imece mcp-relay <socket>`, which only
copies bytes. The sender is the socket, never an argument. Allowed by
exact name (`--allowedTools mcp__imece__<tool>`, each tool); `init` must report
exactly imece's server, connected, and exactly its tools. Mail waits per
agent; waking types start the next turn when the agent has no prompt
left, inform/progress/notice ride along. Caps: body 4000 chars, 20
unread, 12 message-driven turns per agent until the person speaks again;
a stopped agent gets no waking mail, nobody does once the budget is spent.
Each message gets a number, and a reply may name the one it answers
(`re`): see Reply-to numbers (record 58).

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
