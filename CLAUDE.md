# imece — notes for Claude

- One terminal for many coding agents from many vendors (ACP), led by a
  leader agent, talking to each other, sharing files through symbol leases.
- Stage: the screen (`crates/imece-tui`) runs on fake events (`sim.rs`) or
  on live Claude Code sessions (`--live`, `live.rs`); both feed the same
  model (`model.rs`). `crates/imece-agent` drives vendor CLIs as child
  processes and turns their output into vendor-neutral events; the screen
  never sees a wire format.
- Chat with the user in Turkish; code, comments and repo docs in English.
- The screen speaks English (default) and Turkish. Every word on screen
  lives in `i18n.rs` (both languages, same `{}` placeholders); the model
  stores events as data and `ui/words.rs` words them at draw time.
- Everything an agent wrote reaches the terminal only through `i18n::fill`
  or `i18n::clean` (control characters removed, secret-looking tokens
  hidden). Never draw agent text raw.
- Run cargo with `export PATH="$HOME/.cargo/bin:$PATH"`.

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Code from Lonca and Kule

imece never changes Lonca or Kule (they move together and must stay in
sync), and does not depend on them. Code that helps is **copied** into imece
and owned here from then on:

- Every copied file starts with `Copied from <repo>@<sha> <path>`.
- `VENDORED.md` lists source, commit, date and local changes.
- A scheduled check lists upstream commits that touched copied paths since
  the recorded sha, so security fixes are not missed.

## Decisions so far

- Default screen: the salon (guild tree, conversations, inbox, journal);
  board and code map one key away. The command line goes to the leader.
- Messages: typed envelope + free-text body. Types: assign, ask, answer,
  inform, handoff, done, blocked, progress, notice, cancel. No `ack`. Only
  the envelope drives routing, waking and cost; the body is never
  interpreted.
- Organisation: a leader assigns by default; agents may also claim from the
  board.
- Researchers: coders do not read the outside world; a capped pool of
  short-lived researchers does. Identical questions in flight are merged and
  answers cached.
- Graded taint: reading a report makes a coder soft-tainted (only sensitive
  changes need the person); researchers are fully tainted.
- Trusted sources: official docs by default; user-content sites never;
  package-registry docs only for packages in the lockfile.

- Live v0 (2026-10-28): one long-lived `claude -p` stream-json session per
  agent (never a process per turn), `--restricted`, exact `--tools`, no MCP
  server but imece's own, `--permission-prompts none`; the `init` event is
  checked every turn and a mismatch stops the session (fail closed). Leader: read tools, the
  checkout itself. Workers: edit tools, one detached worktree each under
  `$XDG_STATE_HOME/imece/worktrees`, kept between runs. Budget split evenly
  up front as each session's `--max-budget-usd`. Sessions start off the UI
  thread; events carry a session generation so late ones are dropped.
- Message bus v0 (2026-10-28): one tool, `send(to, type, body)`, served by
  imece itself over MCP: one Unix socket per agent in a fresh 0700 temp
  directory; the session starts `imece mcp-relay <socket>`, which only
  copies bytes. The sender is the socket, never an argument. Allowed by
  exact name (`--allowedTools mcp__imece__send`); `init` must report
  exactly imece's server, connected, and exactly its tools. Mail waits per
  agent; waking types start the next turn when the agent has no prompt
  left, inform/progress/notice ride along. Caps: body 4000 chars, 20
  unread, 12 message-driven turns per agent until the person speaks again;
  a stopped agent or one over its share gets no waking mail.
- Board v0 (2026-10-28): `post(title, body)` and `board()` for everyone,
  `claim(card)` for workers only (the leader's socket does not offer it).
  An open card is offered to one idle worker at a time as a turn of its
  own (titles only; the body comes with the claim); it goes to the next
  worker only after that one's turn ended without taking it, so a card
  wakes each worker at most once, never a broadcast. One card at a time
  per worker; the worker sends `done` to whoever posted it. Caps: title one
  line of 120 chars, 20 open cards per project, 10 posts per agent until
  the person speaks again; offers count as message-driven turns.
- Projects: the salon groups agents by project, each with its own leader;
  groups fold to one line that still shows what waits for the person. The
  researcher cap is shared by all projects.

## Neighbours

- **voff** (github.com/orhun/voff, MIT/Apache-2.0, by ratatui's maintainer)
  is the closest competitor: lead + team, Codex/Claude/Ollama, ratatui TUI
  with a node-graph team map, SQLite state, team generated from a repo scan.
  It drives Claude Code and Codex as stateless completion engines (fresh
  process per turn, built-in tools off, history re-sent, one tool call per
  turn), splits work by disjoint paths (overlap means serial), and a model
  reviews every patch and command. imece differs on purpose: real agent
  sessions over ACP with a message bus between peers, symbol leases in the
  same file, a sandbox instead of model review as the last gate, graded
  taint, many projects at once, a calm screen.
- Taken from voff (deep review 2026-10-28, be322a2), to build when the
  matching part exists:
  - Done needs evidence: one entry per acceptance criterion citing what
    proves it, and a passing check after the last change; a later change
    voids the check.
  - Rejections say whether new evidence or a different action is needed;
    an identical action after a rejection is refused without a model call.
  - Repeated unchanged reads (path + range + file digest) are caught; the
    second repeat stops the agent and asks its leader.
  - Writes carry the base digest they were made against and are refused if
    the file moved on.
  - Passing test lines are dropped and repeats collapsed before test output
    reaches an agent.
  - An interrupted review or decision is denied on restart (fail closed);
    a state change and its event are written in one transaction.
  - Child processes: argv only, cleared environment plus an allowlist,
    prompt on stdin (never argv), byte caps on every pipe, process-group
    kill. A tool call is acted on only after the vendor's final event;
    streamed text is display only.
  - Before driving a vendor CLI, check its effective config as the vendor
    reports it, not the flags we passed; fail closed.
  - Onboarding: find vendors on PATH, scan the repo without sensitive
    files, propose a team through a forced tool call (here per-agent vendor
    and model, edited in place before it starts).
  - Screen: headline glyphs per event kind, speaker-coloured "A → B"
    threads, display-only folding with nothing lost, a compact approval
    line that opens into route / action / request / preview, a per-agent
    token and cost table, composer history per workspace and drafts per
    view.
- Rules that come from what voff lacks: hard caps on turns, revisions and
  money per agent and per project, reserved before each call; an empty
  scope is nothing, never everything; no decision of a model widens a
  grant; redraw on change, never on a timer; history kept per agent, no
  global cap; Esc never quits and nothing modal blocks typing; colours
  fixed per agent id.

## Security

Same standing rules as Kule: deny by default, nothing weakens silently, every
boundary has a test that fails without it.
