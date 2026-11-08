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

`.lonca/workspace.json` is the mark, a plain file, no
link (`lonca::is_workspace`). `Agent.workspace` says which repository, set when
a session is ready and read anew each run; it is what `tree_at` looks at, so a workspace gives git nothing to read, stand or move, and its Δ is a proposal (record 120). `:copy`, `.worktreeinclude` and the setup script run as they
do in a worktree: a workspace honours `.gitignore`, so what they bring is no
part of a proposal, and a tracked file they change is one, which is the truth.

**The session** (`claude::Spec.store`): `--settings` puts `lonca hook` on every
tool with `onFailure: block`, `--mcp-config` adds `lonca mcp` beside the bus,
Lonca's tools are allowed by exact name, `LONCA_SOCKET` is set in the session's
environment. `--restricted` ignores settings *files*, `--settings` holds. `init`
must show Lonca's server connected, once, and every tool of `LONCA_TOOLS`, or
the session stops; one more tool of that server is no stop, since a newer Lonca
may add one, it comes from the server asked for, and not being on
`--allowedTools` it asks. **The hook must have started** (2026-11-08): `lonca hook`
comes by `--settings`, and a Claude Code that skips the hooks it is given would
run the session with Lonca's server connected and nobody watching its tools.
`--bare` does exactly that (and reads no OAuth login, so a subscription session
fails loudly while an API-key one would not), and Claude Code's headless docs say
`--bare` may become the default for `-p`. Its `SessionStart` hook starts before
`init` (`system` `hook_started`, `hook_event: SessionStart`; probed on 2.1.319, at
start and at `--resume`), so `Parser::init` stops a session on Lonca that saw none
(`Parser.hooked`, kept across `/clear`'s new `init`). Under `open` a person's own
`SessionStart` hook would count too; `--bare` skips those as well.
imece holds no token of a worker's: it is in
`.lonca/session`, and the hook and the server find it from `cwd`.

**What it is told** (`prompts::lonca_note`, beside the role's text, which still
says worktree: the note says that means this): a `.git` of its own that reaches
nothing (below), lease,
preview, propose, propose *before* `done`, `blocked` when a denial can not be
put right. Lonca's own rules come by its `SessionStart` hook.

**It names its harness** (`lonca::HARNESS`, `claude-code`): `session start`,
fresh or `--continue`, passes `--harness claude-code --no-setup`, and the hook
runs as `lonca hook --harness claude-code`. Since lonca `a6d214c` an
observation level is granted only to the harness a signed `session.start`
names; a session that names none is `unobserved` on any machine, a hardened
one with Claude Code's hooks locked included. `--no-setup` because imece
brings the hooks and the MCP entry itself; Lonca writes nothing in the
workspace. A cooperative daemon changes nothing (every session there is
`unobserved`). An older `lonca` refuses the flag, and the start says so.
Checked 2026-11-08 on lonca `010ba7e`: the daemon's `status.sessions` carry
`harness: claude-code` for a fresh seat and a continued one.

**The scope is the whole tree** (`--paths=**`), for 8 h, Lonca's most (9 h is
`session_ttl`). A session's paths could be a card's owned paths (record 114),
but one session outlives many cards and `--continue` refuses a wider scope;
the person left it the whole tree (record 123). An agent's name goes as
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
  can not add a dependency. lonca#103 made that a human's decision when the
  policy says so, and `:lonca import` says so (record 121); on another repository a
  worker says `blocked` and the person makes that change.
- `[taint] mcp_servers` is empty by default, so imece's bus taints a session, as
  does the web with `fetch = true`, and a tainted session is refused a critical path
  however that word stands. `:lonca import` names the bus; the web stays (record 121).

**Known gaps**, the second half of this step:

- `session start` is signed. A Secure Enclave key wants a finger at every hire and
  every session over, at the leader's moment, not the person's, within 120 s.
  Record 116 kept a login to a moment the person chose; this can not, a worker
  needs a session to start. A file key signs alone.

**The workspace's `.git`** (lonca#111, read on Lonca `9bf19e0`). `lonca session
start` now makes one unless told `--no-git`, and imece does not tell it: a build
or a tool that reads git works, which was a gap here. It is a repository of its own
with one base commit, not a worktree of anything: a commit stays in it, `propose`
sends what differs from the base, committed or not, `sync` moves it, and `git clean
-x` takes `.lonca/` with it, which ends the workspace (the note says so). Unlike a
worktree's, whose config is in the person's repository and out of a worker's reach,
this `.git` is the agent's own to write, so its config is a stranger's: git run there
outside the session runs what it says. So imece runs none: `tree_at`, `:tidy`,
`:fork`, the start and the Δ already kept out, and the `run` tool, the one path that
did not, now ties no run to a tree in a workspace and refuses `git`
(`tools::no_git_in_workspace`). Under `strict` that leaves the agent Lonca's `preview`
and `status`; from `standard` on its shell has git, as its own, seen by the hook.

**`lease_interest` and `events`** joined `LONCA_TOOLS`: waiting in line for a holder
and reading what happened, which would each have asked the person.

**No `Stop` hook** (lonca#121, read on `9bf19e0`), though `lonca harness setup` writes
one. It sends an agent that stops with unproposed work back to propose, up to
`stop_rounds` (3) since its last proposal. imece's workers stop a turn in mid-work on
purpose: a worker that has asked and waits. The hook would push it to propose half a
change, which is a Δ before its time (record 120: an approval then races a worker
still editing). `done`'s gate already refuses a done with changes no proposal carries,
and `lonca session end` reports what was left (the CLI sends `files` itself). To look
at again after the first real run, for a worker that ends a turn saying nothing.

**A session runs out; a harness does not** (record 02: it stays up across turns).
Every look reads `status.sessions`, so `Watch.ends` knows when each agent's newest
session ends (the daemon's clock, kept as an `Instant`). `Live::prompt` first calls
`renew_seat`: a worker in a workspace, between turns, within `SEAT_MARGIN` 30 min of
the end, ends its Claude Code, and the prompt wakes it the way any sleeper wakes,
resuming its conversation; `start` then passes `seat_short` to `seat`, which goes on
with `--continue` though the heartbeat says alive. It takes a new harness because
`lonca mcp` holds the token it started with. It is done before a prompt, not on a
timer, so that a team asleep overnight asks nobody for a finger, and a worker hears
`SEAT_RENEWED`: files and proposals stand, leases are gone. A helper shares its
worker's session, so it never renews and a worker waits for its helpers: `--continue`
would end the session under them. A session goes by the owner's name, a helper seated
first too, or the owner's would be lost to `Watch.ends`. An expiry mid-turn is told by
the worker's next Lonca tool (`session_inactive`), and its next prompt renews it. The
demo has no clock on sessions: this can not be seen there.

**Leaving and tidying.** A worker that leaves (`Live::dismiss`) ends its session on
a thread (`stand_up`, `lonca session end`): its leases go, its proposals stay, and a
helper leaving ends nothing of its worker's. Its workspace stays, as a worktree does,
for `:tidy`, which lists it beside the worktrees (`tidy::orphans_under`); its checkout
is the one `Live::roots` knows its directory by, else that directory, since imece
asks a workspace's `.git` nothing. What it holds comes from `lonca diff` (`Place::left_in`).
A workspace as its base left it says `no local changes`, locally, with no session and
no daemon (a real run found this; `diff` has no JSON for it). **One with changes wants
a live session to count them**: after `session end` it says `unauthenticated` (probed).
So what can not be read is what holds something: `Orphan.unread` is not clean, the
window says it may hold work, `a` passes it by and `d` asks `y`.
Each such `lonca diff` under a dead token is an `auth_failure` in the daemon's
`security.log` (lonca `010ba7e`), so a session the daemon says is over is read
first (`Place::session_at`: the workspace's `session` id, looked up in
`status.sessions` as the person): one that left work no proposal carries has its
count there (`unproposed.total`, recorded at `session end`), and no command runs
under its token. Since lonca#189 (`dfec4ff`) a clean end, or one with every change
proposed, reads `total: 0`: the workspace is clean. `null` now means nobody
reported since the session began or last proposed (it expired, or ended without
its files): the workspace is unread, and still no `lonca diff` runs (#363), so an
expired session's untouched workspace asks `y` too, the price of no dead token
noted as an attack. The same holds for `done`'s gate (`proposed`): a worker whose
session is over and said nothing is told to propose first, which the renewal
before a prompt (`renew_seat`) leaves to a turn longer than its margin. A daemon older
than `dfec4ff` reads `null` for a clean end, so its clean workspaces are unread.
`clear_workspace` reads again when it removes: without the person's yes only `Some(0)`
goes. Its session ends first unless the daemon says it is over (`Place::ended`),
where `session end` would only be refused under the dead token; its proposals are
in the store and do not go with it. Its teardown script runs as a worktree's does.

**The demo** (record 31). A worker hired on a project on Lonca has
`Agent.workspace`, the trace line and the focus's `· workspace of lonca NAME`;
its helpers too; `:fork` refuses it as live; `:tidy` lists a workspace of a project
on Lonca that can not be counted. What can not be faked: a daemon, a
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
