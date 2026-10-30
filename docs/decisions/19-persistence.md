# Persistence v0

Persistence v0 (2026-10-30, user's decisions: one pot that persists,
one SQLite file for every project, an interrupted turn is NOT run
again, resume must not depend on three vendor implementations):
`store.rs` keeps the world and the engine's part beside it in
`<state>/imece.sqlite` (0600, WAL, rusqlite bundled and pinned; a
`.lock` next to it, flock'd, so a second imece is refused with "use
:open from the first"; a file from a newer imece is refused;
`schema_version` in `meta`). Tables: meta, projects (with the
checkout), agents (with model, tiered, woken, posted, traced,
commands), cards, story, journal, inbox, trace, mail, notes, inflight,
posts, offers, changes; events (`Event`, `StoryEvent`) are JSON with
the variant name in a `kind` column (serde derives, pinned). A save
runs after every step and every key (`App::persist`, failures flashed
once) and once more at quit, and writes only what moved: new journal,
story and trace lines are appended (trace pruned to 200 per agent), a
row of a growing table is written again only when its hash moved
(`Store::plan`), so growth costs nothing per save; the growing tables
are read by their primary keys (`plans` test: no temp b-tree). Load
(`world()`): every agent comes back `Status::Asleep` (gone ones
idle), no process and no cost until the person writes to it, a
teammate's waking message arrives (`send` sets it idle) or a card
posted THIS run is offered (`loaded_cards`); mail and cards that
waited through the restart do not wake anyone. Ticks count from
`World.started_at` (the world's first start, so they stay monotonic
across runs; `run_spent` keeps earlier runs out of the rate), the clock
starts at the local time of that first start. A turn that was mid-way
(`inflight`) is journaled `Interrupted` and marked on the card; its
prompt is not run again but told at the wake. The wake briefing
(`Live::brief`, pushed as an imece note ahead of the first prompt,
every line through `clean`, 300 chars a line, 6000 in all): the card
and its last 12 story lines, the cut prompt, how many messages wait,
the worktree's uncommitted changes (`worktree::change` on
`<root>/<tree>` if it exists), the board's open cards. A worker told
again goes on with an unfinished card (was: a new card for a stopped
one). The checkout imece starts in is `Live::home` (opened as a new
project if the store does not have it; the salon selects its leader);
a remembered project whose checkout is gone loads closed. `--budget`
is `Option`: unset keeps the pot as it was left (`DEFAULT_BUDGET`
$50 the first time); `World.daily_budget` is `budget`. `Options.state`
carries the state dir (tests no longer leak worktrees into
`~/.local/state`). The gaps it left (Codex and Gemini resume, the day
in the journal, retention, the prefs file) closed in the entries below.
