# A scratch directory per writing agent

Backlog E18 in #58 (from cmux). The design was decided by the person on
2026-11-03: the directory lives outside the worktree, in imece's state
directory, each vendor gets it as a writable root, and `:forget` removes
it. It was built on 2026-11-04.

**Why.** A worker that keeps notes, a log of a run or a download in its
worktree puts them in its change: they show in the Δ and are applied
with it unless the person picks them out. `/tmp` is no answer. Claude's
file tools cannot reach it under `--restricted`. It is shared with every
other process. And nothing ever clears it.

**Where.** `Live::scratch_of`: `<worktrees>-scratch/<name>` beside the
worktrees and Gemini's private directories
(`worktrees/kule-3f2a9c01-scratch/claude-1`). A helper has its own, not
its worker's. A solo's is `solo-<project id>`, as its worktree is: an
archived solo and the fresh one after it share a name (record 80). A
leader has none, since it writes nothing.

**Made** in the start thread before the session (`make_scratch`): 0700
with its parents, and kept when it is there already. Something there
that is not a directory, a link to one included, fails the start. Its
canonical path is what the vendors get:

- **Claude Code** `--add-dir DIR`. `--restricted` keeps file tools to
  the working directories, `--add-dir` included (its help says so).
  `init` does not report the added directories, so this is not checked.
- **Codex** `-c sandbox_workspace_write.writable_roots=["DIR"]`. This
  replaces the person's own roots: a writer's sandbox writes its
  workspace and the scratch directory, nothing else imece did not ask
  for. The probe of 2026-11-04 (codex-cli 0.165.0) showed it echoed in
  `config/read` (`sandbox_workspace_write.writable_roots`) and in the
  started thread's `sandbox.writableRoots`. Both are checked, fail
  closed, for writers (`Protocol::roots`). A reader's sandbox is
  read-only and is not asked about it. Codex still lets the sandbox
  write `/tmp` and `$TMPDIR` by default; imece leaves that as it is.
- **Gemini CLI** a second `--include-directories DIR`, so its file tools
  take it as part of the workspace. ACP reports nothing to check.

The agent's instructions name it (`prompts::scratch_note`): "yours
alone, outside the files you change … never among your files". The
focus has a `scratch` line under `folder` (`App::scratch_of`; live
through `shown_path`, the demo where live would put it).

**Removed** (`Live::drop_scratch`) by `:forget` (`forget_agent`), by
`--fresh` for each agent it forgets, and when an agent leaves
(`dismiss`: `:close`, a helper's `done`). It is not removed when a new
session goes on with the same work: a model fallback, a renewed cap, or
`/clear` on a vendor without one of its own. It is renamed aside at
once (`<name>.gone-<nanos>`), so a session started right after makes a
new one, and then removed on a thread of its own. The trace says `its
scratch directory was removed`, or why it could not be. Nothing is said
when there was none. An archive keeps its agents' scratch directories
with their sessions.

**Not done.** No size cap and no line in `:tidy` for a scratch
directory whose agent is gone, nor for one left aside by a crash
between the rename and the removal. imece checks none of what Claude
and Gemini report about it. A leader's notes go nowhere; it has no
scratch directory.
