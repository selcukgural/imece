# Lonca as the store

The person's decision (2026-11-04), on the question of where imece's git
work belongs. The end goal was always to run the repository through
Lonca (`../lonca`: agent-native version control; a daemon that is the
only writer, symbol leases, a merge queue that verifies the merged tree,
an operation log). imece was designed for many agents on one repository,
on the same file where the work asks for it; owned paths (record 114) and
one worktree per worker keep agents apart by file, which is the cheap
stand-in, and the first round of record 111 had started to build Lonca's
merge queue a second time inside imece: a change read against the
worktree, applied three-way into the checkout (record 11), what of it
clashes found and sent back (record 108), the repository's check run by
imece (record 112), and next in line a worker told when the checkout's
HEAD moved under its waiting Δ. Built that way, Lonca would have had no
place in imece. So:

- **imece's repository layer is Lonca's.** A worker works in a Lonca
  workspace; its `done` is a proposal; the Δ in WAITING FOR YOU is a
  pending proposal; `a` is the approval; the merge, the rebase onto a
  moved trunk, the conflict and the verification are the merge queue's.
  The person's checkout stops being where changes land: trunk lives in
  Lonca's store and the checkout is a client of it (`lonca sync`, or a
  fetch from the store, which is a bare git repository).
- **imece depends on Lonca's daemon, as a client.** The rule "imece never
  depends on Lonca" (the "Code from Lonca and Kule" section of
  `CLAUDE.md`) is lifted for Lonca and kept for Kule. The dependency is
  on a running `lonca` daemon and its protocol (a Unix socket, one JSON
  line a request, the same calls `lonca-cli` makes), never on Lonca's
  crates as libraries and never a second writer: the daemon is the
  sequencer, and imece writing to the store itself would break the model
  the audit log stands on. Code imece embeds (symbol parsing for the code
  map, the client's protocol shape) is still copied with its header and
  listed in `VENDORED.md`.
- **No second backend.** The git path imece has today stays until the
  Lonca path carries a team end to end, then goes; a project whose
  workers write asks for Lonca, fail closed (no daemon: no hire, no Δ
  decisions, the project read-only and said so). The demo fakes the store
  as it fakes everything (record 31).
- **What stays imece's**: the team (the leader, `hire`, the plan gate,
  the board, the bus and its mail, the pot, cards and their stories), the
  person's seat (WAITING FOR YOU, the salon, the journal, recall, the
  conversation), cost per Δ (record 113), cross-review (record 106), the
  field entries. **What becomes Lonca's**: worktrees and their housekeeping
  (records 02, 10, 36, 57, 96), reading and applying a change (11), the
  partial apply (50: a proposal is approved whole), clashes and their
  send-back (108), the repository's check (112), `:commit` (56: nothing
  is staged in a checkout any more), the "early rebase" item of record
  111. Record 111's sentence changes with it: the Δ gate is Lonca's;
  imece's product is the team and the person's seat on top of it.

## The probe (2026-11-04, Lonca `02220bb`)

Cooperative mode, an isolated `LONCA_HOME`, a file key, no Touch ID. Two
repositories: a three-function crate, and a copy of imece.

- **Setup.** `lonca daemon` (the socket path must be under 100 bytes:
  `LONCA_SOCKET`), `human keygen NAME`, `init NAME --import PATH` (0.9 s
  for imece), `session start --repo --agent --paths --task --ttl
  --workspace DIR`. The workspace is a plain directory holding trunk's
  tracked files, `.lonca/workspace.json` (the base commit and a manifest
  of blob ids) and `.lonca/session` (the token, 0600); no `.git`;
  symlinks become regular files holding their target. 315 files in
  0.07 s. The whole tree comes over the socket as one JSON line with
  contents in base64, and `MAX_LINE` is 96 MiB: imece's own repository,
  79 MB tracked with its tour videos, fails with `tree: io: request too
  large`; without them (6.7 MB) it works. A repository with large tracked
  binaries can not have a workspace until Lonca pages `tree`.
- **The worker's chain**, each call under 0.25 s on imece's `app.rs`
  (646 symbols listed in 0.05 s): `symbols FILE`; `lease acquire
  'FILE#Type.method' -i INTENT` (all or nothing, `lease_held:SYM:
  SESSION:INTENT` when another session holds it); edit; `diff` (the
  facts: symbols edited, files modified, lines); `propose -i INTENT
  --edits SYMS` (the claim must equal the facts: `claim_mismatch:edits`;
  a symbol not leased: `lease_missing:SYM`; `--accept-facts` takes the
  facts as the claim); `change ID` (JSON: actor, commit, base, claim,
  facts, status `pending` / `integrated` / `conflicted` with its symbols /
  `rejected` with reasons, provenance); `pending`; `approve SEQ --as
  HUMAN` (the file key signs; `nothing_pending` for a wrong seq);
  `sync` (trunk into the workspace, local edits kept, same-file edits
  merged structurally, a conflict kept local with trunk's version beside
  it as `FILE.trunk`).
- **The gate is there by default.** Every agent proposal escalates to one
  human approval in cooperative mode (`unobserved_session`). A human
  workspace (`workspace create DIR --repo --as HUMAN`) proposes with the
  same `propose` and integrates at once, no approval: that is how the
  person's own edits reach trunk; there is no staged checkout.
- **A stale base integrates by itself.** Proposal B, based on `c4acd4d`,
  approved after A had moved trunk to `75bc16f`: the queue made
  `d9aecdb` by structural merge; the worker did nothing. A real clash
  (two edits in one symbol's region; here both added a line above
  `add`, which the engine counts as `add`'s): after the approval,
  `change.integrate deny merge_conflict, conflict:src/lib.rs#add`, the
  change `conflicted`; the worker's `sync` wrote `src/lib.rs.trunk` and
  said `1 conflicts`; merged by hand, the side file removed, proposed
  again, integrated. Record 108's send-back becomes: show the conflicted
  status and tell the worker to `sync` and propose again.
- **The store is a bare git repository** (`$LONCA_HOME/repos/NAME.git`):
  `refs/heads/main`, `refs/lonca/changes/ID/SEQ` (the proposal's commit),
  `refs/lonca/oplog`, `refs/lonca/policy`, `refs/lonca/import/initial`.
  imece reads a Δ's patch with `git diff BASE COMMIT` in the store,
  read-only; `log --json` carries every operation with its evidence;
  evidence logs are blobs in the store (`cat-file -p`). A checkout can
  fetch `main` from it.
- **Verification.** `policy set --config lonca.toml --policy policy.cedar
  --as HUMAN` is allowed at once with one enrolled human.
  `[verification] build = … test = … required = true` runs on the merged
  tree in a seatbelt sandbox with the environment cleared but `PATH`:
  `cargo` needs `HOME`, `RUSTUP_HOME` and `CARGO_HOME` in the command
  itself, else rustup fails in 25 ms ("could not choose a version of
  cargo"); set, build 99 ms and tests 584 ms on the small crate. Evidence
  `syntax` / `build` / `tests` each with result, exit, duration, sandbox
  and a log blob. A failed verification is `change.integrate deny
  verification_failed:build` after the person's approval: the verifier,
  not the person, has the last word. This is record 112's check made a
  gate, in Lonca's hands.
- **Hooks, taint, MCP.** `lonca hook` (Claude Code's hook, the event on
  stdin) answers `SessionStart` with the rules as `additionalContext`
  (lease before editing, edit only inside the workspace, preview, propose
  with exactly the symbols changed, sync after integration, negotiate for
  a held symbol); a `PreToolUse` with `curl` tainted the session
  ("observed external shell curl") and its later proposals escalate as
  `tainted_session`; the hook also binds the session (`session.bind`,
  `observe`). `lonca mcp` is a stdio MCP server with nine tools (`status`,
  `symbols`, `lease_acquire`, `lease_release`, `preview`, `propose`,
  `change_status`, `sync`, `negotiate`): what a worker's Claude Code gets
  beside imece's own bus.
- **Limits** (`lonca.toml`): leases at most 30 min and 64 items; sessions
  8 h; 200 files and 2 MiB a file per change; `[paths] critical`
  (`Cargo.toml`, CI, `build.rs`, lockfiles) always needs a human and is
  closed to tainted sessions; one daemon per machine; file keys "have no
  user presence", `require_presence` is for Secure Enclave keys.
- **Gaps found.** The tree size cap above. Workspace creation is the
  CLI's (`Workspace::create` in `lonca-cli`), not a daemon call: imece
  runs `lonca session start` or copies that code. `propose` and `queue`
  take no `--repo`/`--as`: identity comes from the workspace and
  `LONCA_HUMAN`. `change` carries no diff: git in the store. Evidence logs
  only through `log --json` and `cat-file`. The MCP tool list has no lease
  renewal, and a lease lasts 30 min at most: a turn longer than that
  loses its lease mid-work (the protocol has `lease.renew`; to probe).
  The verifier's cleared environment needs the toolchain's homes in the
  command. What a human may do to another's pending proposal besides
  approve (reject with a reason; `change.withdraw` is the proposer's) is
  not probed.

## The plan

Each its own pull request and record, in this order; they replace the
git items of record 111's "after the first round" in epic #286.

1. This record; `CLAUDE.md`'s rule rewritten; records 108 and 111
   amended; the issue for the work.
2. (Done: record 116, which says what changed from this text.) The
   store client behind a boundary in `live/`: the socket at
   `LONCA_SOCKET` else `$LONCA_HOME/lonca.sock`, the JSON-line protocol
   (its shape copied from `lonca-daemon::protocol`, listed in
   `VENDORED.md`), `status`, `tree`, `change.get`, `pending`, `log`, the
   calls as `lonca-cli` makes them; a daemon missing or a repository not
   there fails closed (`⌀ no lonca` on the project's line, no hire, no Δ
   decision, said once in the journal). The demo's store is a table.
3. (Done: records 117 and 118, which say what changed from this
   text; the human workspace went, the checkout is a client since lonca#83.)
   A project on Lonca: `:open` names a Lonca repository or imports a
   checkout (`lonca init --import`, the person's yes first); the person's
   human key (`LONCA_HUMAN`); a human workspace per project for the
   leader's reads; the checkout stays the person's and the project's line
   says how far behind trunk it is, with `:sync`.
4. (First half done: record 119, which says what changed from this
   text.) Workers in workspaces: `hire` runs `lonca session start` with the
   card's owned paths as the session's paths; the worker's Claude Code
   gets `lonca mcp` and `lonca hook` beside imece's bus, and its
   instructions say lease, preview, propose, sync; the scratch directory
   stays; the worktree code (`worktree.rs`, `Placed`, `follow`, `stand`,
   `copy`, sparse, `:tidy`) goes when every team is on Lonca.
5. The Δ on Lonca: `done` is accepted once the worker's proposal is
   pending (a `done` without one is refused with "propose first");
   WAITING FOR YOU lists `pending`; the Δ view is `git diff BASE COMMIT`
   from the store, the evidence line is the log's evidence, the cost line
   stays imece's; `a` is `approve`; `r` sends the reason to the worker
   (and what Lonca offers a reviewer besides approval, once probed);
   `conflicted` and `rejected` reach the worker as the queue says them,
   with `sync` and propose again as the next step; `x` and `b` go.
6. Verification: `.imece/check` is retired in favour of `[verification]`
   in the repository's `lonca.toml`, documented with the toolchain
   environment the sandbox needs; the Δ shows the attestations.
7. Leases on the code map (record 17): `status` lists who holds what; the
   code map's lease columns come alive from it, and record 114's owned
   paths become the session's paths.

Growth: the store and its log grow with proposals, in Lonca's hands
(backups, redaction, retention are its). Concurrency: the daemon is the
one sequencer; imece is one client beside the person's own `lonca`
commands and the workers' MCP calls, and reads the state it needs rather
than keeping a copy; two imeces on one daemon see the same pending list.
Read cost: nothing on the screen's loop talks to the socket; `pending`,
`status` and `change.get` are polled on the engine's step at a few
seconds' pace, each a millisecond call, and a Δ's patch is read once and
kept. Ten times the scale: Lonca's own test integrates 20 agents on one
file in under a second; the first thing to break is the tree cap at a
large repository, which fails at the session's start and is said.
Migration: a project stored with worktrees opens as it does today until
it is moved to Lonca by the person (`:open` on the imported repository);
the old rows become archives (record 80); the schema gains the
repository's name per project. Failure: a daemon gone mid-turn fails
closed (no decision reaches the store, the worker's proposal waits in the
store, imece says `⌀ no lonca` until it answers); a crash of imece loses
nothing, the proposals and leases are the daemon's; a worker's token is
in its workspace and is revoked with its session.
