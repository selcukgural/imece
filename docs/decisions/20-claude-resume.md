# Claude resume

Claude resume (2026-10-30, persistence PR 2): the store keeps each
agent's last vendor session (`agents.session`, `agents.session_spent`,
schema 2, added to a schema-1 file with `ALTER TABLE` when the column
is missing; `Resumable { session, spent }` in model.rs, `Live.sessions`,
set from every `Event::Started`, which Claude's parser now emits again
when `/clear` brings a new session id). At the wake of a remembered
Claude agent, `Live::resumable` says whether to go on with it: a UUID
(nothing else reaches argv or a path), and the transcript file is where
Claude Code keeps it for the directory the agent runs in
(`claude::transcript`: `$CLAUDE_CONFIG_DIR|~/.claude/projects/<cwd with
every non-alphanumeric as '-'>/<id>.jsonl`; the worktree path canonical,
as `worktree::ensure` makes it). If so, the spec carries `Resume {
session, spent }` → `--resume ID` (the same flags otherwise; probe of
2026-10-30 on Claude Code 2.1.314: `init` reports the SAME session id,
`total_cost_usd` is the WHOLE session's, an unknown id answers a
`result` `error_during_execution` with no `init` and "No conversation
found" on stderr), the parser starts `cost_last` at `spent` and emits
what lies above that floor (a process that counted only itself would
fall under the floor and be taken as a reset, so it still counts right),
and the briefing is short (the session knows its story: only the cut
turn and waiting mail; "Nothing; go on." otherwise). `Started` with the
asked id → journal `Resumed`, the cut prompts dropped; a different id →
the parser stops the session (fail closed); a `Stopped` while the wake
is still unconfirmed → `Live::restart`: the old session forgotten, the
first prompt put back as it was (without imece's notes), the notes
restored, `remembered` set again, so the same `prompt` path gives the
full briefing and journals `Restarted` (which every fresh wake does,
Codex and Gemini included). `end()` resets the floor and the attempt;
imece's own `/clear` forgets the session. Known gaps: `renew` and
`fall_back` start fresh (no resume mid-run), the `/clear` inside a
resumed session leaves `spent` as a guess (Claude's counter resets, the
floor does not), a stopped session (tools differ etc.) is still the one
resumed at the next start. Live check:
`IMECE_LIVE_CLAUDE=1 cargo test -p imece-agent -- --ignored live_claude`.
