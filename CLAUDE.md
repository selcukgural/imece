# imece — notes for Claude

- One terminal for many coding agents from many vendors (ACP), led by a
  leader agent, talking to each other, sharing files through symbol leases.
- Stage: the screen (`crates/imece-tui`) runs on live Claude Code sessions
  (`live/`, the default) or on fake events (`--demo`, `sim.rs`); both
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
  in `.cargo/mutants.toml`). Running them is CI's job, never local (user,
  2026-10-29: too slow on a laptop too); before a push run fmt, clippy
  and the tests. BUT before opening a PR, make sure the tests that kill
  the change's mutants are written (user, 2026-10-29: each push that
  comes back with missed mutants costs a CI round of 10+ minutes, and a
  big change came back twice). So write the tests with the mutants in
  mind from the start, and before the PR go over the diff once more for
  what a mutant would change: the equal case of every comparison, each
  operand of an arithmetic expression (with the number in the test, not
  the constant), every field a constructor sets, every arm of a match,
  every function with the same shape in a sibling file (a test written
  for Gemini's copy was not written for Codex's), with values that tell
  the variants apart. The full run is nightly only.
  The PR job also fails on a timeout, so never write
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
- Posture (user, 2026-10-30; this replaced "coders do not read the outside
  world"): imece's worth is the team (a leader that hires, the bus, the
  board, worktrees, one pot, the Δ review queue, persistence, many
  projects), not a locked-down agent. Workers and helpers get their
  vendor's full coding toolset (read, write, shell, web search and fetch);
  the person is the last gate (the Δ review, WAITING FOR YOU), not a model
  and not a tool list. imece sandboxes nothing: it switches on each
  vendor's own restrictions and checks, fail closed, that the session runs
  with what was asked; OS-level isolation is a container's or Kule's job
  (imece does not depend on Kule). The leader stays non-writing (read, web,
  read-only commands): a role, not security; it works in the person's
  checkout, where a write would skip the Δ gate and collide with the
  person's working copy; small jobs go to a cheap-tier worker. See
  `docs/decisions/26-posture-change.md` for what of this runs and what is
  still to come.
- Demo mirrors live (user, 2026-10-30): every fix, feature or change
  to what live does is shown by the demo (`--demo`, `sim.rs`) in the
  same PR, with its test; what cannot be faked is said in the PR. See
  `docs/decisions/31-demo-mirrors-live.md`.
- Projects: the salon groups agents by project, each with its own leader;
  groups fold to one line that still shows what waits for the person. The
  researcher cap is shared by all projects.

## Decision records

Each decision, with its reasons, probe facts and known gaps, is a file in
`docs/decisions/` (how to add one: its `README.md`). Read the record before
changing the part it covers; a change that alters a decision updates its
record in the same PR.

- 01 `researchers`: optional researcher layer, graded taint, trusted sources (not built).
- 02 `live`: one long-lived `claude -p` session per agent, worktrees, the one budget pot, `:budget`.
- 03 `message-bus`: `send(to, type, body)` over imece's own MCP server, a socket per agent, mail caps.
- 04 `board`: `post`/`board`/`claim`, one offer per idle worker, card caps.
- 05 `hiring`: the leader hires workers with `hire`, `--agents`, tiers per vendor, model fallback.
- 06 `codex`: Codex app-server adapter, sandbox, overrides, fail-closed checks, efforts as tiers.
- 07 `gemini`: Gemini ACP adapter, private directory, policy file, checks, price table.
- 08 `who-leads`: `--lead`, the primary vendor, the question screen, `choose_lead`.
- 09 `helpers`: workers hire helpers in their worktree, `--helpers`, salon folding.
- 10 `dogfooding-prep`: worktrees follow HEAD, repo notes in instructions, the `run` tool, vendor commands.
- 11 `change-review`: a worker's `done` → Δ in WAITING FOR YOU, diff view, apply (three-way when clean)/reject, the runs on exactly its tree.
- 12 `completion`: `complete.rs` menus for names, commands, `:` words and directories.
- 13 `own-shell`: `!cmd` in the person's shell, the terminal dance.
- 14 `journal`: when a closing text is journaled as `Replied`.
- 15 `themes`: `theme.rs`, terminal/dark/light/mono, base16 schemes, colour files, the probe.
- 16 `quiet-erase`: row-by-row erase and auto-wrap off (iTerm2 scrollback leaks).
- 17 `code-map`: one row per file, fixed lease columns, a legend of the glyphs shown.
- 18 `projects`: `:open`/`:close`/`:rename`, places per project, the demo's copies.
- 19 `persistence`: `store.rs`, one SQLite file, what is saved and how, the wake briefing.
- 20 `claude-resume`: `--resume`, transcript check, cost floor, `Live::restart`.
- 21 `codex-resume`: non-ephemeral threads, `thread/resume`.
- 22 `gemini-resume`: `session/load`, history replay handling.
- 23 `hygiene`: `Live::forget`, removing vendor sessions, `:forget`, `--fresh`.
- 24 `leftovers`: days on the clock and journal, retention and VACUUM, prefs in the store.
- 25 `left-off`: the "where you left off" window at start.
- 26 `posture-change`: the plan from restricted tools to full capability, (a)–(d) and what is done.
- 27 `ask-the-person`: `send(to: "you")`, questions in WAITING FOR YOU.
- 28 `confine-dial`: `--confine open|standard|strict` per vendor.
- 29 `legibility`: dimmed backdrop, plain rows, the coloured trace, a conversation that follows.
- 30 `approval-routing`: Claude's permission prompts to `approve`, Codex's `on-request` asks, Gemini's `ask_user` shell, routine commands, `⚑` asks in WAITING FOR YOU.
- 31 `demo-mirrors-live`: the demo shows what live does (Δ, `⚑`, questions, hiring, helpers, sessions); the merge queue left it.
- 32 `card-screen`: `#323` or `#word` opens a card's screen (who, story, its inbox and journal lines), `n p`, `inbox.card`, the `#N` heading fix.

`docs/neighbours.md`: voff, what was taken from it and the rules that come
from what it lacks. Read it before building a part it names.

## Security

Same standing rules as Kule: deny by default, nothing weakens silently, every
boundary has a test that fails without it. What imece guards is its own
boundaries: who may call which tool on the bus, what an agent's text may do
on the screen, what a session reports it runs with against what was asked
(a mismatch stops it), the pot, and the Δ gate between a worker's worktree
and the person's checkout. imece is not a sandbox and never calls itself
one: an agent with a shell can do what the person's account can, and the
vendor's own restrictions (`--restricted`, Codex's sandbox, Gemini's policy
file) are what imece switches on and verifies, not something it provides.
Isolation beyond that is a container's or Kule's job.
