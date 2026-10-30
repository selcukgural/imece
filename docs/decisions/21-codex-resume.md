# Codex resume

Codex resume (2026-10-30, persistence PR 3): a Codex thread is no
longer `ephemeral` (`thread/start` sends `ephemeral: false`, and the
answer's `thread.ephemeral` must be false, fail closed), so Codex keeps
its rollout under `~/.codex/sessions/YYYY/MM/DD/rollout-<time>-<id>.jsonl`
while `history.persistence="none"` still keeps imece's turns out of the
global message log (probe of 2026-10-30 on Codex 0.165.0: the rollout
is written with persistence none; a second process resumed the thread
and remembered the first turn). `codex::Spec.resume` carries the thread
id: the start sends `thread/resume {threadId, cwd, approvalPolicy,
sandbox, developerInstructions, model?, excludeTurns: true}` instead of
`thread/start`, the answer goes through the same `check_thread` and
must carry the id asked for; a thread whose rollout is gone is refused
by Codex (`no rollout found for thread id …`) before any turn, which
stops the session and `Live::restart` starts a new one with the full
briefing (`Restarted`), as for Claude. `Live::resumable` is per vendor:
Claude needs its transcript file, Codex only a UUID-shaped id (the
rollout's path has the day in it, and Codex says when it is gone),
Gemini none yet; `Live::resume_of` is vendor-neutral (`Resumable`) and
`start` maps it per vendor; `imece_agent::valid_uuid` is the shared id
check. Known gaps: a resumed thread keeps the developer instructions it
was started with (the ones sent again are not taken: probed, the
rollout records no new ones), so a role prompt that changed since
(renames, new teammates) reaches a resumed Codex agent only through
imece's notes; `thread/resume` with another cwd or sandbox echoes the
thread's own (the echo check is what refuses it); threads started
before this change were ephemeral and come back as `Restarted`. Live check:
`IMECE_LIVE_CODEX=1 cargo test -p imece-agent -- --ignored live_codex_resumes`.
