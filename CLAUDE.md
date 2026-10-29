# imece — notes for Claude

- One terminal for many coding agents from many vendors (ACP), led by a
  leader agent, talking to each other, sharing files through symbol leases.
- Stage: the screen (`crates/imece-tui`) runs on live Claude Code sessions
  (`live.rs`, the default) or on fake events (`--demo`, `sim.rs`); both
  feed the same model (`model.rs`). `crates/imece-agent` drives vendor CLIs as child
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
cargo mutants --in-diff <(git diff main...HEAD)   # what the change touched; CI does this on every PR
cargo mutants                                      # everything (slow); CI does this nightly
```

- Mutation tests: a mutant that survives is a test that is missing, so a
  change comes with the tests that kill its mutants (`cargo-mutants` 27.1.0,
  pinned in `.github/workflows/mutants.yml`; exclusions with their reasons
  in `.cargo/mutants.toml`). They are CI's job (user, 2026-10-29): before
  a push run fmt, clippy and the tests, not mutants; run `cargo mutants`
  locally only when the PR job reports a missed mutant and closing it
  takes several tries. The PR job also fails on a timeout, so never write
  a loop a mutated operator could make endless (search a range instead).

- Keyboard first (user, 2026-10-29): everything is done with keys, vim
  style (hjkl, g/G, PgUp/PgDn, Esc back, / to type) next to the arrows; a
  mouse is never needed and never captured, so the terminal's own
  selection keeps working. Add a shortcut wherever it makes something
  easier; every key is listed in the `?` help in both languages.

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
  `$XDG_STATE_HOME/imece/worktrees`, kept between runs. Budget (default
  $50, user 2026-10-29) is one pot for the team: no turn once it is spent;
  each session's `--max-budget-usd` is what was left when it started, a
  last stop against a runaway session, not a share. `:budget USD` (key `:`
  opens a command line, `:b` is short) changes the pot while running: a
  session whose turn ends at its old cap (`error_max_budget_usd`) while
  the pot has room is ended and started again with the same prompts and
  the new remainder, once per session until a turn finishes (`renewed`).
  Sessions start off the UI
  thread; events carry a session generation so late ones are dropped.
- Message bus v0 (2026-10-28): one tool, `send(to, type, body)`, served by
  imece itself over MCP: one Unix socket per agent in a fresh 0700 temp
  directory; the session starts `imece mcp-relay <socket>`, which only
  copies bytes. The sender is the socket, never an argument. Allowed by
  exact name (`--allowedTools mcp__imece__<tool>`, each tool); `init` must report
  exactly imece's server, connected, and exactly its tools. Mail waits per
  agent; waking types start the next turn when the agent has no prompt
  left, inform/progress/notice ride along. Caps: body 4000 chars, 20
  unread, 12 message-driven turns per agent until the person speaks again;
  a stopped agent gets no waking mail, nobody does once the budget is spent.
- Board v0 (2026-10-28): `post(title, body)` and `board()` for everyone,
  `claim(card)` for workers only (the leader's socket does not offer it).
  An open card is offered to one idle worker at a time as a turn of its
  own (titles only; the body comes with the claim); it goes to the next
  worker only after that one's turn ended without taking it, so a card
  wakes each worker at most once, never a broadcast. One card at a time
  per worker; the worker sends `done` to whoever posted it. Caps: title one
  line of 120 chars, 20 open cards per project, 10 posts per agent until
  the person speaks again; offers count as message-driven turns.
- Hiring (2026-10-29, user's decisions): bare `imece` is live and the team
  starts as the leader alone; the leader hires workers with the `hire`
  tool (leader's socket only) as the work needs them, `--agents` is the
  cap (0-16, default 4), the sim sits behind `--demo`. The person never
  picks a team size. Workers are named `<vendor>-N`; a socket is bound per
  agent at hire time, before the agent exists. No budget share per agent
  (dropped 2026-10-29: $5 / 5 places gave an Opus leader $1, less than one
  task). Models: the leader runs on its vendor's default (or
  `--model`); the leader picks a TIER for a worker (cheap / standard /
  strong), never a model name; each vendor maps tiers to its models
  (Claude: haiku / default / opus), `--tier T=NAME` overrides. A tier model
  that fails its first turn at zero cost (that is how Claude Code reports
  a model the account lacks: init still succeeds, the first result is an
  `api_error`) is replaced once by the standard model with the same
  prompts, journaled as `Fallback`. Vendors are probed on PATH at start
  (`claude`, `codex`, `gemini`); only vendors imece has an adapter for are
  hireable, others give an honest error.
- Codex v0 (2026-10-29): `crates/imece-agent/src/codex.rs` drives Codex
  CLI (tested against 0.165.0) as one long-lived `codex app-server`
  (JSON-RPC over stdio) per agent; `imece_agent::Agent` is the trait both
  vendors' sessions implement, `Event::Note` carries vendor remarks (tokens,
  whose plan pays) to the trace. Codex has no tool allowlist and always a
  shell, so the SANDBOX confines: `read-only` for a leader,
  `workspace-write` (cwd only, `network_access=false`) for workers and
  helpers, `approval_policy="never"` (a refused command just fails, never
  a prompt). Set with `-c` overrides AND on the thread: web search off,
  features apps/plugins/browser_use/computer_use/hooks/image_generation/
  multi_agent/view_image/memories/goals/tool_suggest off, `notify=[]`,
  `history.persistence="none"`, thread `ephemeral`. Codex loads the
  person's own `~/.codex` (a private CODEX_HOME was tried and rejected:
  no auth, and the ChatGPT account re-installs remote plugins into it),
  so MCP servers and skills that config brings can only be turned off BY
  NAME: a first start reads them (`config/read`, `skills/list`) and, if
  any are on, the process is started again with
  `mcp_servers.<n>.enabled=false` / `skills.config=[{path,enabled=false}]`,
  once; still on → stop. Checked before the thread starts, fail closed:
  `account/read` (ChatGPT login only; an API key is refused because Codex
  reports tokens, not dollars, and imece can not price them yet),
  `config/read` echoes every override, `plugin/installed` none enabled,
  `hooks/list` empty, `mcpServerStatus/list` exactly imece's server with
  exactly its tools (tools listed = connected) and no tools elsewhere;
  imece's server carries `default_tools_approval_mode="approve"` (checked
  in the echo too): Codex asks approval for MCP tool calls, and under
  `never` every unapproved call fails with "MCP tool call requires
  approval" (the live smoke test found this);
  `thread/start`'s answer echoes approvalPolicy/sandbox/cwd; the MCP
  servers are checked again before every `turn/start`. Server requests:
  any approval request or elicitation or dynamic tool call stops the
  session; `item/tool/requestUserInput` is answered empty (the person is
  not there; the leader is, via send). Items: `commandExecution` →
  Tool `shell`, `fileChange` → Tool `apply_patch` per path (counts as a
  change), `mcpToolCall` must be imece's server and an offered tool,
  `webSearch`/`collabAgentToolCall`/`subAgentActivity`/`dynamicToolCall`
  stop the session. Cost booked is 0 on the ChatGPT plan (a Note says so
  once, and token totals per turn). Tiers are EFFORTS on Codex's default
  model (`@low` / default / `@high`), a tier value is `[MODEL][@EFFORT]`;
  `--model [V:]NAME` and `--tier [V:]T=NAME` take a vendor prefix, bare
  ones mean the leader's vendor (`team_vendors` in live.rs). Known gaps:
  git in a worktree writes to the main `.git` outside the sandbox (agents
  are told not to commit anyway); the read-only sandbox lets a leader read
  the whole disk, unlike Claude's `--restricted`; no per-session dollar cap
  for Codex (the pot still stops new turns). An ignored test
  (`IMECE_LIVE_CODEX=1 cargo test -p imece-agent -- --ignored live_codex`)
  runs one real turn with `send` over `nc -U`.
- Gemini v0 (2026-10-29): `crates/imece-agent/src/gemini.rs` drives Gemini
  CLI (tested against 0.65.0) as one long-lived `gemini --acp` (Agent
  Client Protocol v1, JSON-RPC over stdio) per agent. Gemini has no
  `--restricted`; its settings come from `~/.gemini/settings.json` and
  the `.gemini` directory of the folder it runs in, a system settings
  file counts only from a root-owned directory, and an UNTRUSTED folder
  starts no MCP server at all (so imece's bus would be gone). So: Gemini
  runs in a PRIVATE directory imece owns (`<worktrees root>-gemini/<agent>`,
  trusted via `GEMINI_CLI_TRUST_WORKSPACE=true`) and gets the checkout or
  worktree as `--include-directories`; the repo's `.gemini` settings,
  hooks and `.env` never load. Confinement is a POLICY file imece writes
  there (`--policy DIR`, the user tier's only policies once named; just
  root-owned system policies rank above): allow the role's file tools
  (read_file, list_directory, glob, grep_search; workers and helpers also
  write_file, replace) and imece's MCP tools by their FULL names
  (`mcp_imece_send`; the `mcpName` form matches when tools are shown to
  the model but not when one is called) at priority 999, deny `*` at 998
  (the highest matching rule wins, and a denied tool is hidden from the
  model). Argv: `--approval-mode default`, `-e none` (extensions off),
  `--allowed-mcp-server-names imece`, `-m MODEL`; env
  `GEMINI_TELEMETRY_ENABLED=false`, `NO_BROWSER=true`. The MCP server goes
  in `session/new` (`mcpServers`, stdio). Instructions: ACP has no system
  prompt, so the agent's role goes in `<private dir>/GEMINI.md` (project
  memory), plus a line naming its files' directory. Checks before the
  first turn, fail closed: `initialize` (ACP 1, `gemini-cli`),
  `session/new` (mode `default`, the model asked for), then the
  zero-token commands `/about` (model, auth type), `/extensions list`
  (none active) and `/memory list` (imece's GEMINI.md in use). Every
  `session/request_permission` is answered `cancel` and counted as
  denied; a `tool_call` of kind execute/fetch/switch_mode (or edit/delete/
  move for a leader), an MCP tool not offered, or a `current_mode_update`
  stops the session. A prompt starting with `/` or `$` would run as a
  Gemini command, so it is prefixed with `Text:`. Cost: `session/prompt`'s
  answer carries usage (input, cached, output, thought) and
  `_meta.quota.model_usage`; API key (`gemini-api-key`) → list-price
  table in gemini.rs (per-1M prices in thousandths of a dollar, <=200K
  tier, thoughts as output, cached at the cached rate, a turn on several
  models booked at the dearest), unknown model → session stops; Google
  login (`oauth-personal`) → 0 with a Note; vertex-ai/gateway refused.
  Tool calls: a `tool_call`'s id carries the tool's name
  (`write_file__call_7`, `mcp_imece_send__call_3`), its `title` is the
  path (file tools) or `send(to: lead, …)` (MCP), `locations` the paths;
  a write outside the workspace fails before it starts
  (`tool_call_update` failed, "Path not in workspace"), which counts as
  denied. Confirmed live on 2026-10-29 (API key, free tier): the checks,
  hidden tools (a lead's model sees no shell or write tool, a worker's
  no shell), `send` reaching imece's server, the outside write refused,
  a repo hook in `.gemini/settings.json` not running.
  Tiers: cheap=`flash`, standard=default (`auto`), strong=`pro`. Known
  gaps: Gemini writes chat files under `~/.gemini/tmp`; the person's own
  `~/.gemini/GEMINI.md` and user-level hooks still apply (hooks only turn
  off in settings files imece can not write); the free tier's rate limits
  (5 requests/min) make turns slow and retried. An ignored test
  (`IMECE_LIVE_GEMINI=1 cargo test -p imece-agent -- --ignored live_gemini`)
  runs one real turn with `send` over `nc -U`.
- Who leads (2026-10-29, user's decision): no silent default. `--lead
  VENDOR` wins; else the PRIMARY vendor from the preferences (`lead=` in
  the screen file) if it is on PATH and driveable; else the only driveable
  vendor; else a question before the salon (`ui/pick.rs`: j/k, enter =
  this time, `p` = make it the primary, esc = quit). `--lead ask` forces
  the question; `:lead VENDOR` sets the primary from the next start on,
  `:lead ask` forgets it. A primary that is not on PATH is said on the
  question screen, never replaced silently. Decision is `choose_lead` in
  `main.rs`, pure and tested.
- Helpers (2026-10-29, user's decision): a worker is to imece what an
  agent is to Claude Code, so it splits its task the same way: the
  worker's socket offers `hire` too, and a worker hires HELPERS (role
  `Helper`, `parent` = the worker, named `<worker>.N`), `--helpers N` is
  the cap per worker (0-8, default 2). A helper runs in its worker's
  worktree (same files; the worker says which), with the edit tools and
  send/post/board only (no claim, no hire: no helpers of helpers), is not
  offered cards and is not counted against `--agents`. Its `done` to its
  worker marks it leaving; it leaves when that turn ends (so the turn's
  cost is still reported): session ended, mail dropped, `gone`, journaled
  as `Left`. A worker that stops takes its helpers with it. The vendor's
  own sub-agent tool stays off the `--tools` list on purpose: a sub-agent
  spawned inside the CLI would skip the worktree, the budget, the bus and
  the screen. Salon: helpers sit under their worker at depth 3 (`├`/`└`),
  the worker line ends with `▾ 2 helpers` / `▸ 2 helpers`; `h`/`l`/`za`
  on a worker with helpers (or on a helper) fold its helpers before the
  project, `zM`/`zR` fold and unfold workers too.
- Dogfooding prep (2026-10-29, user's decisions: imece will write imece):
  a worktree found clean at an older commit is moved to the checkout's
  HEAD when its agent starts (`worktree::ensure` returns `Placed`; the
  journal says moved / kept with changes / kept with own commits); every
  agent's instructions end with the repo's `AGENTS.md` else `CLAUDE.md`
  (16 KiB cap; Codex gets only a CLAUDE.md, it reads AGENTS.md itself)
  because `--restricted` (Claude) and the private Gemini directory drop
  them (probe: a restricted session knew nothing of CLAUDE.md); `run`
  tool on imece's MCP server for every agent (`crates/imece-agent/src/
  run.rs`: shell-less argv split that refuses `| & ; < > ( ) $ \` # * ? ~
  {`, one pipe for both streams, process group killed at `--run-timeout`
  (default 600 s) and in `Live::drop`, output tidied: ANSI stripped,
  passing test lines dropped, repeats collapsed, middle cut past 48 KiB;
  policy `allowed()` in live.rs: cargo minus add/remove/install/publish/
  login/owner/yank/uninstall, a leader also minus fmt-without-`--check`/
  fix/clean/init/new, git read commands only, `--run PROG` extras for
  workers and helpers only, nothing else; the screen's loop only checks
  the call, the run answers from its own thread, the socket waits
  timeout + 30 s for it); the vendor's own command line from imece's
  (`/cmd` to the leader, `@name /cmd`; `Agent::command`; Claude: slash
  commands on again, prompts starting with `/` get a `Text:\n` prefix
  like Gemini's, `/clear` restarts the cost counter so the parser keeps
  a monotonic total; Gemini: raw over session/prompt; Codex: only
  `/compact` → `thread/compact/start`; `/clear` without one of the
  vendor's own = imece ends the session). Known gaps, in the order to
  feel them: no diff / apply / reject of a worker's change from the
  screen (still `git -C <worktree> diff`), no session persistence
  across runs, `run` has no network block (cargo may reach the registry
  if the lockfile is off), each worktree has its own `target/` on
  purpose (a shared one thrashes fingerprints across worktrees).
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
