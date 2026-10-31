# Dogfooding prep

Dogfooding prep (2026-10-29, user's decisions: imece will write imece):
a worktree found clean at an older commit is moved to the checkout's
HEAD when its agent starts (`worktree::ensure` returns `Placed`; the
journal says moved / kept with changes / kept with own commits); every
agent's instructions end with the repo's `AGENTS.md` else `CLAUDE.md`
(16 KiB cap; Codex gets only a CLAUDE.md, it reads AGENTS.md itself)
because `--restricted` (Claude) and the private Gemini directory drop
them (probe: a restricted session knew nothing of CLAUDE.md); a file
over the cap goes in cut and the journal says so (`Event::NotesCut`,
file and size in KiB, 2026-10-30: this CLAUDE.md is 50 KB and was being
cut silently); `run`
tool on imece's MCP server for every agent (`crates/imece-agent/src/
run.rs`: shell-less argv split that refuses `| & ; < > ( ) $ \` # * ? ~
{`, one pipe for both streams, process group killed at `--run-timeout`
(default 600 s) and in `Live::drop`, output tidied: ANSI stripped,
passing test lines dropped, repeats collapsed, middle cut past 48 KiB;
policy `allowed()` in `live/tools.rs`: cargo minus add/remove/install/publish/
login/owner/yank/uninstall, a leader also minus fmt-without-`--check`/
fix/clean/init/new, git read commands only, `--run PROG` extras for
workers and helpers only, nothing else; the screen's loop only checks
the call, the run answers from its own thread, the socket waits
timeout + 30 s for it); the vendor's own command line from imece's
(`/cmd` to the leader, `@name /cmd`; `Agent::command`; Claude: slash
commands on again, prompts starting with `/` get a `Text:\n` prefix
like Gemini's, `/clear` restarts the cost counter so the parser keeps
a monotonic total; Gemini: raw over session/prompt; Codex: only
`/compact` → `thread/compact/start`, which Codex may refuse: since
#52 (2026-10-31) a refusal is a failed turn (`TurnDone { ok: false }`,
"compaction refused: …") and the next queued turn runs, where before it
stopped the session like a failed start-up check, which still fail
closed; `/clear` without one of the vendor's own = imece ends the
session). Known gaps, in the order to
feel them: no session persistence across runs, `run` has no network
block (cargo may reach the registry if the lockfile is off), each
worktree has its own `target/` on purpose (a shared one thrashes
fingerprints across worktrees).
