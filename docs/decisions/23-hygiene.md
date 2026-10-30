# Hygiene v0

Hygiene v0 (2026-10-30, persistence PR 5): a session imece forgets
leaves nothing behind at its vendor. `Live::forget(id, whole, settle)`
drops `sessions[id]` and removes, on a thread of its own (the trace
says how it went), what the vendor kept: Claude the transcript file
(`claude::forget`, the path of `claude::transcript`); Codex the thread
through an app server started for it (`codex::delete`: `initialize`,
`thread/delete {threadId}`, 30 s cap; probed 2026-10-30 on 0.165.0: a
live thread answers `{}`, an unknown or deleted one error -32600 "no
rollout found for thread id …", and the thread leaves Codex's SQLite
index too, which deleting the rollout file by hand would not); Gemini
the chat files of the session under `<GEMINI_CLI_HOME|~/.gemini>/tmp/
<name>/chats/` (`gemini::forget`: the name comes from Gemini's
registry `projects.json` keyed by the private directory as given or
canonical, a name that is not one plain path segment is refused; a
file counts when it ends with `-<first 8 of the id>.jsonl` AND its
first line's `sessionId` is the id; 0-byte `session-*.jsonl` strays
go too), or, when `whole`, everything under that name plus the
private directory (`gemini::forget_all`). Only a UUID reaches a path
or a vendor. `settle` waits `SETTLE` (4 s, two child grace periods)
before touching anything when the session's process was just asked
to stop; tests set it to zero. Called from `dismiss` (`:close`, a
helper's `Left`; whole), imece's `/clear`, `fall_back` and `renew`
(the replaced session); `restart` has nothing to remove (the vendor
said it is gone). A Gemini LEADER's private directory is `lead`
whatever the project is called (was `lead-NAME`), so `:rename` keeps
its session. `:forget [@NAME]` (`:f`, completion offers the agents of
open projects, `@` or not; no name = the agent at hand; live only):
the running session ends (its run killed, its unfinished prompts kept
as `interrupted`, told at the wake and not run again), the remembered
one goes, the agent sleeps until spoken to and its next message
starts a new session with the full briefing (`Restarted`); refused
for an agent with nothing to forget or one that is gone; journal
`Forgotten` (`EntryKind::Person`). `--fresh` does it for every
remembered agent when the store is taken in (`adopt`), journaled per
agent that is not gone. Known gaps: a Claude session's other traces
(`~/.claude/debug`, `file-history`, `todos`) are not looked for (an
imece session of 2026-10-28 had only the transcript); Codex's delete
takes a process (~1 s) per thread, `--fresh` with many Codex agents
starts them all at once; a session stopped for a mismatch is still
the one remembered (and forgotten only by these paths).
