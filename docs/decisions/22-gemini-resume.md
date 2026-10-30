# Gemini resume

Gemini resume (2026-10-30, persistence PR 4): Gemini CLI records every
ACP session's chat under `~/.gemini/tmp/<project short id>/chats/
session-<time>-<id8>.jsonl` (the short id comes from a registry,
`~/.gemini/projects.json`, keyed by the directory the session runs in:
imece's private one, stable per agent; the file grows as the turn goes;
a second, empty `session-…` file appears per start, Gemini's own stray),
so `gemini::Spec.resume` carries the session id and the start sends
`session/load {sessionId, cwd, mcpServers}` instead of `session/new`
(probe of 2026-10-30 on Gemini CLI 0.65.0: the answer carries the same
modes/models as `session/new` but NO sessionId, so the id asked for is
the session's, and one named differently is refused; the history is
streamed back as `session/update` notifications AROUND the answer: the
first came before it, the rest right after, in one millisecond, ending
with `available_commands_update`; an id Gemini has no chat of, or the
right id from another cwd, answers error -32603 "Internal error" with
`data.details` "Invalid session identifier …", before any turn). So
`Protocol.replay` is `Streaming` from the load request until the command
list arrives: every update but the command list and a mode change
passes unread (no text, no tool checks: a replayed `tool_call` carries
`status` completed/failed and was the screen's when it happened),
`waiting()` stays true (a list that never comes is a timeout, 60 s), and
`/about` goes out once BOTH the answer and the list are in (either
order). The same checks follow (mode default, the model asked for,
`/about`, `/extensions list`, `/memory list`). Error answers now carry
Gemini's `data.details` first line in parentheses (turn failures too:
the free tier's 429 says "daily quota"). `Live::resumable` is UUID-only
for Gemini, like Codex (the chat's path is registry-given, so Gemini
says when it is gone and `Live::restart` briefs a new session). Known
gaps: the live memory check (a loaded session answering from its
history) could not be run on 2026-10-30, the API key's daily quota was
spent; the source (`resumeChat(clientHistory)`) says it does, and the
ignored test `IMECE_LIVE_GEMINI=1 cargo test -p imece-agent --
--ignored live_gemini_resumes` is written for it; Gemini keeps at most
50 history messages per chat file (`MAX_HISTORY_MESSAGES`), so a long
session resumes with its tail; a resumed session re-reads GEMINI.md
(the role prompt written fresh at every start), unlike Codex's fixed
developer instructions.
