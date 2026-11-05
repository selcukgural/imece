# Workers in Lonca workspaces

The first half of record 115's step 4, with Lonca at `9a13c50` (lonca#100 brought
`--continue`) and Claude Code 2.1.318. A writer of a project on Lonca
now works in a workspace: a real session of Lonca's, leases and proposals
included. What it proposes waits in Lonca; the person sees the count on the
project's line and decides with `lonca approve`, until step 5 brings the Δ.

**Where a writer works** is one function, `live::lonca::seat`, run on the
start's thread before anything else touches the tree:

- A project on no repository gets a worktree, and nothing asks the daemon.
- A tree that is there and no workspace stays a worktree, on Lonca or not: a
  worker with work in a worktree is not moved, record 115's "the git path stays
  until the Lonca path carries a team". So a project goes on Lonca for the
  workers it hires from then on.
- No tree on a project on Lonca: `lonca session start`, a new workspace at the
  same path a worktree would have, `<root>/<tree>`. So `cwd_of`, `tree_of`,
  Claude Code's transcripts, resume and the scratch directory did not change.
- A workspace there: a heartbeat. Alive, it is taken as it stands, leases and
  all, which is what a helper needs (it shares its worker's) and a restart
  wants. Over, `session start --continue`: the files, the base and a pending
  proposal stay, a taint stays, the leases are gone and the worker leases
  again. A workspace of another repository, or on a project on none, stops the
  start with why; so does a daemon that can not say, since nobody could work
  there.

Only `.git`-less is not the mark: `.lonca/workspace.json` is, a plain file, no
link (`lonca::is_workspace`). `Agent.workspace` says which repository, set when
a session is ready and read anew each run; it is what `tree_at` looks at, so a
workspace gives git nothing to read, stand or move and a `done` from one has no
Δ (`change_none`). `:copy`, `.worktreeinclude` and the setup script run as they
do in a worktree: a workspace honours `.gitignore`, so what they bring is no
part of a proposal, and a tracked file they change is one, which is the truth.

**The session** (`claude::Spec.store`): `--settings` puts `lonca hook` on every
tool with `onFailure: block`, `--mcp-config` adds `lonca mcp` beside the bus,
Lonca's tools are allowed by exact name, `LONCA_SOCKET` is set in the session's
environment. `--restricted` ignores settings *files*, `--settings` holds. `init`
must show Lonca's server connected, once, and every tool of `LONCA_TOOLS`, or
the session stops; one more tool of that server is no stop, since a newer Lonca
may add one, it comes from the server asked for, and not being on
`--allowedTools` it asks. imece holds no token of a worker's: it is in
`.lonca/session`, and the hook and the server find it from `cwd`.

**What it is told** (`prompts::lonca_note`, beside the role's text, which still
says worktree: the note says that means this): no `.git`, no commits, lease,
preview, propose, propose *before* `done`, `blocked` when a denial can not be
put right. Lonca's own rules come by its `SessionStart` hook.

**The scope is the whole tree** (`--paths=**`), for 8 h, Lonca's most (9 h is
`session_ttl`). A session's paths could be a card's owned paths (record 114),
but one session outlives many cards and `--continue` refuses a wider scope;
that belongs with the last step of record 115's plan. An agent's name goes as
Lonca takes it: `[A-Za-z0-9_.-]`, 64 at most, anything else `-`.

**Refused.** `hire`'s `from` (`ON_LONCA_FROM`): a workspace is made of trunk.
`:fork` of a worker in a workspace (`ON_LONCA_FORK`): it has no HEAD to fork from,
and a session owns its proposals; a real fork needs Lonca to split a workspace.

**What real runs said.** A real Haiku session under imece's flags plus Lonca's
hooks and server: `SessionStart` injected the rules, `init` listed
`mcp__lonca__*`, `lease_acquire`, `Edit`, `Bash`, `preview` and `lease_needed`
worked, `target/` and `.env` stayed out of the facts (a workspace reads
`.gitignore`), a new file imece wrote is a fact. Then imece itself on a pty, with a
`--solo` on Lonca: told one change, in 15 s it had a session, a lease, and a
pending proposal; the lease pinned to it. `--ignored live_a_seat_is_taken_kept_and_taken_again`
runs `sit`, `seated` and `sit_again` on a real daemon (by hand; CI has none).

**What cooperative mode means here.** `level_for` says `unobserved` unless the
hooks come from root-owned managed settings, so every worker is `unobserved`, and
with Lonca's default `unobserved_is_tainted = true`:

- every proposal escalates (`unobserved_session`, `approvals=1`): it waits for a
  human, which is what imece wants. **Do not turn it off**: nothing would wait
  for the person, the change would integrate unseen. Step 5 has to check this.
- a change to a `[paths] critical` path is **denied**, `unobserved_critical_path`:
  `Cargo.toml`, lockfiles, `package.json`, `.github/**`, `build.rs`. A worker
  can not add a dependency. lonca#103 asks for the repository's word to make that
  a human's decision too. Until then a worker says `blocked` and the person
  makes that change.
- `[taint] mcp_servers` is empty by default, so imece's bus taints a session, as
  does the web with `fetch = true`. Today it changes nothing an unobserved session
  does not already have; lonca#103 will make it matter. A repository that wants
  it says `mcp_servers = ["imece"]` and `fetch = false`.

**Known gaps**, the second half of this step:

- A session lasts 8 h and a harness stays up across turns (record 02): one
  past its time is told by its next Lonca tool, and imece does not yet renew it,
  which takes ending the harness: `lonca mcp` holds the token it started with.
- A worktree nobody owns is listed by `:tidy`; a workspace nobody owns is not, and
  a worker that leaves does not end its session.
- `session start` is signed. A Secure Enclave key wants a finger at every hire and
  every session over, at the leader's moment, not the person's, within 120 s.
  Record 116 kept a login to a moment the person chose; this can not, a worker
  needs a session to start. A file key signs alone.
- The worker's `git`: a build that reads it fails in a workspace.

**The demo** (record 31). A worker hired on a project on Lonca has
`Agent.workspace`, the trace line and the focus's `· workspace of lonca NAME`;
its helpers too; `:fork` refuses it as live. What can not be faked: a daemon, a
proposal; the demo's Δs stay as they are until step 5.

**Concurrency.** Starts of one tree are serial (`removing`, one slot per agent); a
helper's only reads the heartbeat. `--continue` ends the old session atomically
in the daemon, so one workspace never has two. **Read cost.** One heartbeat a
start; `workspace_repo` reads a file that lists every tracked file, 64 MiB at
most, on the start's thread. **Growth.** A workspace is a plain copy of the tree per
worker, where a worktree shared git's objects; Lonca's 96 MiB `tree` cap is paged.
**Ten times the scale.** A 78 MB tree takes seconds to write (Lonca's probe), each
worker's, within `SEAT_WAIT` 120 s. **Migration.** `Agent.workspace` is memory only,
nothing in the schema; a stored worker with a worktree keeps it. **Failure.** A
daemon gone: a start stops with its words, `lonca hook` blocks every tool of a
running session (`onFailure: block`, seen: exit 2), and `store_closed` closes the
project. A crash of imece loses nothing: the work is in the workspace and its
proposals in the store.
