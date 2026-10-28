# imece — notes for Claude

- One terminal for many coding agents from many vendors (ACP), led by a
  leader agent, talking to each other, sharing files through symbol leases.
- Stage: screen prototype on fake events (`crates/imece-tui`, `sim.rs`). The
  model in `model.rs` is what real agents will feed later.
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

- Projects: the salon groups agents by project, each with its own leader;
  groups fold to one line that still shows what waits for the person. The
  researcher cap is shared by all projects.

## Neighbours

- **voff** (github.com/orhun/voff, MIT/Apache-2.0, by ratatui's maintainer)
  is the closest competitor: lead + team, Codex/Claude/Ollama, ratatui TUI
  with a node-graph team map, SQLite state, team generated from a repo scan,
  disjoint `owned_paths` per worker, every patch and command approved.
  imece differs on purpose: symbol leases in the same file, AI-first
  approvals with graded taint, many projects at once, a calm screen (no
  spinners), ACP for any vendor.
- Taken from voff so far: cleaning agent text before drawing it. Worth
  taking later: generating the team from a repo scan, a per-agent
  token/cost table, compacting passing test output for agent context, a
  stop after repeated unchanged reads.

## Security

Same standing rules as Kule: deny by default, nothing weakens silently, every
boundary has a test that fails without it.
