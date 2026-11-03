# Fork: `:fork` makes a new worker from one, to try another way

Issue #242 (2026-11-03, from the study of agent-deck's fork; backlog item
E5 of #58): a worker went one way; the person wants another way tried
without losing the first. `:fork [@NAME]` makes a new worker that goes on
from the worker's conversation and its uncommitted change, in a worktree
of its own, while the first goes on in its own.

## Probes (2026-11-03)

- **Claude Code 2.1.317**: `--resume OLD --fork-session --session-id NEW`
  works from another directory than the session was made in (Claude
  finds the transcript across its project directories); `init` reports
  NEW; the fork's transcript is kept for the directory the fork runs
  in; the old session is untouched. The fork's `total_cost_usd` includes
  the old session's whole cost (an old session of $0.01397 → the fork's
  first turn reported $0.01537), so the old session's spent is the
  fork's floor, as for a resume (record 20).
- **Codex 0.165.0 app-server**: `thread/fork` (the schema
  `generate-json-schema` writes): `threadId` plus the settings
  `thread/resume` takes (`cwd`, `sandbox`, `approvalPolicy`, `model`,
  `developerInstructions`, `ephemeral`, `excludeTurns`, `lastTurnId`);
  the answer is a thread's, with the fork's own id.
- **Gemini CLI 0.65.0 ACP**: `initialize` offers `loadSession` and no
  fork capability. A Gemini worker is refused, with why.

Both live probes are kept as ignored tests:
`live_claude_resumes_its_session_and_counts_above_what_it_had_cost` (the
fork part) and `live_codex_forks_a_thread_into_another_directory`.

## What `:fork` does

`:fork [@NAME]` (the agent at hand without a name; `$` in the menu: the
fork's turns cost) runs `Live::fork` (the demo `Sim::fork`):

- Refused, said as `NAME does not fork: …`: not a worker (a leader, a
  helper, a solo, a researcher; or gone), a vendor with no fork (Gemini,
  and the vendors imece does not drive), no session yet, mid-turn
  (prompts queued or a session starting), a team at `--agents`, or its
  worktree not there.
- A new worker joins the project, named after its vendor as `hire` names
  workers (`worker_name`), on the worker's model and tier, with a socket
  of its own; counted in `--agents` and its turns in the pot as any
  worker's. Its start point (`Agent.start`, schema 17) is the worker's
  HEAD commit now, so its worktree never follows the person's HEAD
  (record 05); the worktree itself is made at the worker's HEAD at the
  wake (below), with the change it carries, which is against that HEAD.
- `Live.forks[new] = Some(worker)`, kept in the store (`agents.fork_of`,
  schema 20) until the fork's own session starts; the fork is asleep
  (`remembered`), journaled `Event::Forked { from, agent }` and traced on
  both.
- The screen selects its row and opens the command line with `@NAME `,
  flashing `NAME is a fork of WORKER: write what it should try`.

## Its wake

At the fork's first prompt (`prompt` → `resumable`): the worker's session
**as it is then** (its id and spent), checked as a resume is (a UUID; for
Claude the transcript where the worker runs). Taken at the wake, not at
`:fork`, so the conversation and the files it carries are of one moment.
`Attempt.fork` holds a new UUID of imece's (`imece_agent::new_uuid`, from
`/dev/urandom`); the floor is the worker's spent.

- Claude: `claude::Resume { session, spent, fork: Some(NEW) }` →
  `--resume OLD --fork-session --session-id NEW`; the parser fails closed
  on any id but NEW (`forked session …, not …`).
- Codex: `codex::Spec { resume, fork: true }` → `thread/fork`, kept
  (`ephemeral: false`); the answer's id must be a new one, not OLD and
  not empty (`forked … into …, not a new thread`); its settings are
  checked as a new thread's.
- The worktree, made now at the worker's HEAD as it is now (a commit
  the worker made since `:fork` is in it), takes the worker's change
  first (`worktree::carry`: the worker's tree with untracked files, as
  `worktree::tree` reads it; `read-tree -m -u HEAD TREE` then `reset`, so
  the fork's index stays at HEAD and what was untracked is untracked
  again; ignored files are `:copy`'s). A failure stops the fork with
  `could not carry WORKER's change: …`.
- Its briefing says it is a fork: who it came from, that it remembers
  everything that worker did, that it works in a worktree of its own
  that started with that worker's change, and that the person forked it
  to try another way; the person's words follow.
- `Started` makes the session its own: `forks[new]` cleared, no
  `Resumed` journaled. A session that could not fork (`Stopped` before
  `init`) goes on as a new one (`Live::restart`, `forks` cleared): the
  journal says why, the worktree keeps what it carried.

Its Δ (record 11) is its own: against its HEAD, which is the worker's,
so the change it carried is in its Δ until the worker's is applied.

## The demo

`Sim::fork` refuses what live refuses (but the cap: the demo has no
`--agents`), makes the worker, marks its session as started (a copy),
traces `forked from WORKER · worktree from its HEAD, with its change` and
journals `Forked`; what it is told goes on as live's does.

## Not done

- A fork of a solo (a second solo on its worktree) or of a helper.
- Forking at an earlier turn (Codex's `thread/fork` takes a
  `lastTurnId`): the fork is of the whole conversation.
- Gemini: when its ACP offers a fork, the adapter takes it.
- No key for it: `:fork` and its `@NAME`.
