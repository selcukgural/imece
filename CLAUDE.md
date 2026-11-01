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

- The site (user, 2026-11-01): `https://selcukgural.github.io/imece/` is built
  from `docs/guide/` (English) and `website/i18n/tr/…/current/` (Turkish) by
  `.github/workflows/docs.yml`; every feature imece has is described there,
  and every PR that adds, changes or removes a feature updates the pages it
  touches in BOTH languages in the same PR (the keyboard page for a key, the
  commands reference for a `:` command, the scenario that shows it).
  `website/check.sh` fails the build when a page lacks its Turkish copy, or
  when a `:` command or a `?` help key is missing from the reference pages.
  The tour videos (`website/video/`, English only) are re-rendered when what
  they show changes; `website/video/README.md` says how.

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
- 13 `own-shell`: `!cmd` in the person's shell, the terminal dance, its exit code and time in the journal (its output: record 68).
- 14 `journal`: when a closing text is journaled as `Replied`.
- 15 `themes`: `theme.rs`, terminal/dark/light/mono, base16 schemes, colour files, the probe; `:theme NAME` previews while written; the probe again on focus (not mode 2031: crossterm swallows it).
- 16 `quiet-erase`: row-by-row erase and auto-wrap off (iTerm2 scrollback leaks); the title counts what waits.
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
- 33 `next-waiting`: `W` opens the next item waiting for the person from anywhere, `S` puts one off; order, at hand, run-only.
- 34 `quit-ask`: the quit question counts agents mid-turn and changes not reviewed; `q s` lists them.
- 35 `fleet-and-done-unseen`: `✓` done, not seen until opened (`App::unseen`); the header's fleet `● working ✓ done ✕ stuck`.
- 36 `worktree-stand`: a worker's row `⎇ branch ↑own ↓checkout +N −M`, `worktree::stand` on the git thread, when it is read.
- 37 `readline-keys`: ctrl-a/e/w/u/k/y, alt-b/f/d in the command line (`edit.rs`), the cursor as `App::after`, the menu only at the end.
- 38 `attention-order`: `App::needs` ranks waiting > stuck > done-unseen; workers sort by it, fold last; `f` shows only what needs you, the selection settles.
- 39 `multi-line`: shift-enter (kitty keyboard protocol, probed), `\` enter and alt-enter for a new line, the footer grows, bracketed paste.
- 40 `help-filter`: `/` in the `?` help filters keys and legend by every word, case and Turkish letters folded; enter keeps, esc drops; the window keeps its size.
- 41 `colon-names`: every key that acts has a `:` name (`:salon` `:lang` `:quit` …, demo-only ones left out live); the `:` menu shows values (`Menu::notes`) and `$` on what raises the next bill; `:q` and the other short names run on enter.
- 42 `export`: `:export [PATH]` writes the open conversation, else the card's screen, as Markdown; days on every time, cleaned, never over an existing file.
- 43 `pin`: `:pin [NAME]` pins a project to the top of the salon (`▴`, folds last), saved in the store (schema 7); a colour per project is not done.
- 44 `turns`: `[ ]` in a conversation go turn to turn (the lines one side says before the other answers), `ui::turn_starts`, `app::jump` shared with the diff's file jump; bookmarks are record 46.
- 45 `local-link`: the newest loopback URL a worker or helper wrote shows on its row as an OSC 8 link (`model::local_url`, strict; `ui::hyperlink`, each cell opens and closes its own link, `ForcedWidth(1)`); gone with the session.
- 46 `bookmarks`: `m`/`:mark` marks the line at hand (conversation, else journal), `'` goes to the next; `World.marks`, `journal.marked` + partial index (schema 8), a mark is never folded, tidied or left out of a load.
- 47 `quote-hunk`: `>` in a change opens `@worker ` with the hunk at the top quoted in a `diff` fence (`ui::quote`, its file's names, 40 lines, cleaned); the `!cmd` output half of C15 is C15b.
- 48 `limits`: Claude's `rate_limit_event` and Codex's `account/rateLimits/updated` as `Event::Limits`; `World.limits` by vendor and window, the header shows each vendor's tightest (`words::limit`, loud from 90%); the demo fills them from spending; not kept between runs.
- 49 `diff-notes`: `c` notes the top line of a change (`App::diff_notes`, by patch hash), `n` sends them in one message (`ui::notes_message`, `ui::place` file:line, `−N` removed) and the change goes back like a reject; the top row is the line at hand.
- 50 `partial-apply`: `x` chooses hunks (`App::chosen`), `a` applies only them: `worktree::subset` + `worktree::part` (the part's tree through an index of its own), the worktree marked with only that part, the rest read again as a new Δ; the demo mirrors it.
- 51 `back-key`: `^o` goes back through the places left (`App::been`, `Place` = view, zoom, pane, card; recorded in `on_key` when a key moved the person, 50 kept), passing over places gone; closes a conversation first.
- 52 `focus-brief`: back in the terminal after `FOCUS_AWAY` (3 min) in another window, the away digest shows at once (`App::focus`, from the journal since the blur, no model), not over writing or another window.
- 53 `awake`: `--awake`/`:awake` (opt-in) hold the machine's sleep while an agent is mid-turn: `awake.rs`, caffeinate or systemd-inhibit as a child in its own process group, let go when nothing is mid-turn or imece ends; a failure is said once.
- 54 `diff-files`: a change of several files lists them above its patch (`ui::diff::file_lines`, six and the rest counted); `w` wraps its long lines (`App::diff_wrap`); folding all files is D3b.
- 55 `diff-kinds`: a change's files by kind (source, tests, generated, by name: `ui::diff::kind`), its lines per kind above the file list when there is more than one; the commit-message suggestion (D6b) is record 56.
- 56 `commit-message`: `:commit` asks Claude's cheap tier (`claude::once`: no tools, own system prompt, not saved) for a message for what is staged, put on the command line as `!git commit -m '…'`; $0.05 a call, 20 calls and $1 a day, reserved in the store (`paid`, schema 9) and the pot before the call, settled after; the same staged tree again costs nothing.
- 57 `copy-list`: `:copy PATH` lists paths git ignores (`.env`, `target`) per project (`projects.copies`, schema 10); a worker's start copies the ones its worktree lacks (`copy::into`: literal, inside, ignored, never a link, never over what is there, 2 GiB, APFS clone / reflink); the journal says what was copied and what was skipped.
- 58 `reply-to`: every message on the bus has a number (`MsgId`, `meta.messages`); `send` takes `re` (checked: sent to the sender, by the one the reply goes to), mail is headed `[answer 8, re 7]`; `↩` and what it answers in a conversation, `%` goes there and back; `mail.number`/`mail.re` (schema 11); the demo's answers and dones reply.
- 59 `solo`: leaderless sessions the person picks vendor, model and effort for: `imece_agent::Choice` `[MODEL][@EFFORT]` for every vendor (`claude::EFFORTS` via `--effort`, `codex::EFFORTS`, Gemini none, refused fail closed); `Role::Solo`, a project whose `leader` is the solo agent (`solo`/`solo-NAME`, `World::head_name`), its own worktree `solo-<project>` (`Live::tree_of`), the worker's tools less the board, helpers, Δ at every answered turn end (`snapshot` in `step`), `:solo [PATH] [CHOICE] [NAME]` and `--solo V[:MODEL][@EFFORT]` (`Open::Solo`, `Live::solo_at`, any number of solos next to one team per checkout, names numbered), schema 12; a bare `:solo` opens a pick screen (`SoloPick`: vendor → model alias → effort), `:effort LEVEL`/`:model NAME` re-tune the solo at hand (`Live::rechoose`: the session ends and resumes on the new choice, `Event::Chose`), completion after `:solo `/`:effort `/`:model `.
- 60 `notify`: `:notify off|bell|text` (bare: the next round; `notify.rs`, saved in prefs as `notify`): in another window (focus lost), a new WAITING FOR YOU item or a stuck agent still there after `AFTER` (5 s) rings once (BEL) or sends one OSC 9 note in imece's words with the agent's name (`words::notice`, several at once as a count); `Notices::due` keeps what was on screen before leaving (`known`) and what waits (`pending`), back in the window drops the pending; a terminal without focus reports never rings.
- 61 `state-ladder`: `Rung` per agent (stuck > waiting > working > starting > done > idle; `App::rungs`, `Need` = its top, `App::loudest`), `rung_glyph` on every salon row (`!` whatever it does, `◌ starting…` from `Agent.starting`: live's `Slot::Starting`, the demo's first card for `STARTING` ticks), a folded project / helpers / calm-workers line ends with its loudest member, `■N` stopped on the header; the fleet is unchanged.
- 62 `row-words`: one line per agent (the `⇄` talk line is gone); the status carries its own text (`? → you "…"`, `answered you "…"`, `⚑ … "cmd"`, `BLOCKED "topic" ← r`, `■ stopped: why` from `Agent.topic`), else the newest exchange `→`/`← ` name `"…"` (`Talk.mine`, kept until the next, `until` only for folding), then `· 3 min` since `App::since` (oldest open item, else `changed_at`), from a minute on; `STATUS_WIDTH` 40 for status+quote, both yield to the row's fixed parts (`agent_line` takes the width).
- 63 `todo-list`: the agent's own to-do list as `Event::Plan(Vec<Step>)` (Claude's `TaskCreate`/`TaskUpdate` folded by task number, `TASK_TOOLS` asked for at every level and tolerated missing with a note; Codex's `turn/plan/updated` with `tools.update_plan.enabled=true` set, not echoed so not checked; Gemini's `write_todos` allowed, its steps not carried by ACP: a gap); `Agent.plan`, `≡ 3/7` on the row (yields after the quote and the time), the steps on the focus (`PLAN_ROWS` 8, done folded, the rest counted), gone with the session; the demo's five steps per card follow the progress bar.
- 64 `context-fill`: `Event::Context { used, size }` (Claude's message `usage` + the result's `modelUsage.*.contextWindow`, Codex's `thread/tokenUsage/updated` `last` less reasoning + `modelContextWindow`, Gemini's `usage_update`); `Agent.context: Option<Fill>`, `◔ 72%` on the row from 70% (`CONTEXT_SHOWN`), loud from 90% (`CONTEXT_LOUD`), yields last of the yielding parts; the focus has the figure (`words::tokens`); traced by the tenth; gone with the session; the demo fills 1,600 a tick in 200k and auto-compacts at 90%.
- 65 `goto`: `&` opens a jump line on the completion menu (`App::jumps` frozen at open, `App::jump_list`: agents of open projects by Need, Rung, salon order, id; then projects; notes = glyph + status words cut to 30), `complete::jumps` filters by number / `!` needs-only / folded name, a digit 1-9 right after `&` goes at once, `&#` turns to the cards, a number past the list says `no_jump`; `App::goto` (`:goto [NAME]`, folded names; `go_agent` unfolds the project and the worker's helpers, drops `f` if it would hide the row); the `#N #word n p` help row merged so the help stays 55 rows; `@` and `>` prefixes not done.
- 66 `history`: every line sent (message, `:` command, `!cmd`; not `#` `&` `:goto` `:history`: `app::recalled`) kept in `World.sent` (`recall_line`, no repeat of the last), saved in `sent` (schema 13), the newest 500 loaded, the rest tidied; `↑` `↓` at the text's first / last line recall them (`App::recall`, the draft back past the newest, writing on one drops the recall); `^r` / `:history [WORDS]` search them on the completion menu (`App::searching`, `complete::history`, every word folded, newest first, one line each, `words::ago` notes; `enter` takes the line whole, tab steps); help row `tab ↑ ↓ ^r`.
- 67 `file-completion`: the last word of a message or `!cmd` line completes to a file of the project (`App::file_menu` → `complete::files`: by itself with a `/` in the word, any word on `tab` via `suggest_with(true)`; name-start first, then path, shorter, by name; `FILES_SHOWN` 30) and to a leased symbol as `file::name` (holder as the note); `World::files_of` (demo: `Project.repo`; live: `Project.files` from `worktree::files` = `git ls-files` tracked + untracked-not-ignored on the git thread: `GitJob::Files`, `files_due` at open / after apply / every `FILES_EVERY` 60 s while mid-turn, `FILES_MAX` 20,000 → `Event::FilesCut` once); live has no symbols (no leases yet).
- 68 `own-shell-output`: `!cmd` runs on imece's own pty (`shell::run_in`: `openpty` + `login_tty`, close-on-exec, the person's termios less suspend, the screen's size followed, keys through with the real terminal raw so ctrl-c is the command's), `pump` is one poll loop ending on the command's end + `DRAIN` 300 ms or the pty's end; the last `KEPT` 64 KiB → `screen_lines` (escapes dropped, `\r`/backspace/erase-line, cleaned) → `Output` (last 40, `above`), `App.outputs` per project, memory only; `O`/`:output` opens `@agent-at-hand-or-head ` with `words::output_quote` (`I ran `cmd` → exit N · time`, a `text` fence past the backticks); the demo runs the same path.
- 69 `release-and-install`: `release.yml` on a `vX.Y.Z` tag (= Cargo.toml's version): 4 targets (macOS arm64/x86_64, Linux x86_64/arm64 static musl), `imece-<target>.tar.gz` (no version in the name, `releases/latest/download`), `SHA256SUMS`, provenance attestation, write and token only in the publish job; `imece --version`/`-V`; `website/static/install.sh` (POSIX sh, all in `main`, https only, sum checked or nothing installed, `~/.local/bin`, one PATH line under `# imece` per shell rc, `IMECE_VERSION`/`_INSTALL_DIR`/`_NO_MODIFY_PATH`/`_DOWNLOAD_URL`), tested by `website/test-install.sh` in CI's `installer` job; no Windows (WSL); the first tag is the person's call.
- 70 `copy-mode`: `v` in a conversation or a change chooses lines (`Copying { anchor, at }`; `Thread::copy`, `App::diff_copy` by worker and patch hash, dropped off the change); `App::copy_key` first: `j k`, pages, `gg G` move, `y` copies (conversation: `ui::thread_lines` = drawn lines before wrap, the source kept by `markdown_src`, fences never lines; `ui::copy_lines`, a blank line between messages; change: patch lines as written), `Y` in a change the new side (`ui::copy_patch`: hunk state from the patch start, marks off, removed/heads/`@@` out), `v`/esc leave, other keys nothing but `?` `q`; all through `clean` and `clip::copy`; titles `COPY · N chosen`, the cursor `▶`/bold; `v` in a change no longer opens `:talk`.
- 71 `talk-folds-and-notes`: a conversation's messages fold to one row (`h l ← →`, `za`; `zM` `zR` and `:fold` `:unfold` with one open, `Thread::folds: Folds { all, except }`, `App::fold_talk` for the next ones this run): the journal's line cut by `ui::cut`, `▸ N lines` when more is there, `✎ N` its notes; choosing opens them all. Choosing goes word by word (`Thread::words`, `CopyLine::words` from `ui::line_words`, `Thread::part`, `w l → b h ←` through `ui::word_step`, `s` starts again at the cursor, `ui::paint_words`, `y` via `ui::chosen_text`); `c` while choosing notes the words (`Noting::Words(TalkNote)`, `App::talk_notes`, memory only, a dim `✎ "words" note` row, `✎ N` in the title), `n` sends them through `tell` (now `-> bool`), one message per `Event::agent()` of the line the words start in (`ui::talk_notes_message`: `On message N:`, `> ` quote, note); a refused message keeps its notes.
- 72 `peek-and-settle`: `K`/`:peek [@NAME]` opens the agent at hand's focus in a window over the view (`App::peek`, `ui::overlay::peek` = `focus::draw` 8 columns and 6 rows in, shaded); `peek_key` takes every key: `j k` step through `salon::rows` agents (`peek_step`), `enter` the whole focus, `@` writes, `?` help, `esc`/`K` close, the rest nothing; a peek sees (`opened`). `s`/`:settle` puts the selected row out of the salon (`App::settled`; a header or a leader: `settled_projects`), `row_group` = the agent + its helpers + its researcher, refused above `Need::Done`, its `unseen` cleared, the selection to the nearest row left; `wake_settled` after `watch_turns` lets back what needs the person, is gone or closed; `&`/`:goto` and `:unsettle` bring back; the title counts them; run only.
- 73 `solo-without-git`: `live::checkout_of` (git top + `worktree::born`, else the folder itself); `Place::git` found at open and start (`adopt`, canonical); without it a solo and its helpers work straight in the folder (`cwd_of`, `start` skips `ensure`/copies), `Live::tree_at` none (no Δ, stand, follow), no `files`, `:commit` refused; `Project::no_git` (not saved) → `⌀ no git` on the header, `Event::NoGit` at open and each start; prompts say so; a team is refused (`no_git_team`, `ProjectRefusal::NoGit`), a stored one closes; demo marks a real folder without git; teams without git: #162.
- 74 `hint-letters`: `F`/`:hints` puts a label on every row of the salon's screen that can be selected (`app::Target` = `Row(Sel)`, `Waiting(inbox index)`, `Line(journal index)`; the drawing records where each starts in `App::hintable` through `ui::hintable`, emptied by `ui::draw`, a row past its area's foot left out); `app::hint_labels` (`HINT_LETTERS` home row first, one letter while they go round, the last letters start labels of two, never a prefix, at most 26 × 26); `App::hints` frozen at open, painted where the target is now (`ui::hint_labels`, only those the typed letters start); `hint_key` takes every key (letter narrows / acts, none does nothing, backspace, `?`, esc); `hint_go` selects in its panel, the last letter in capitals also `deeper`, a target gone flashes `hint_gone`; off the salon's screen `no_hints`; help row `f  s  F`.
- 75 `welcome-and-news`: `:welcome` (first start ever, or the leader's vendor not signed in at a start: `App::start_welcome`, `lead_check`) lists each vendor (`live::look` on threads → `Seen` Checking/Missing/Here{version, login}; `imece_agent::version`, `claude::login` = `auth status --json`, `codex::login` = `login status`, over `child::ask`: 5 s, own group, 64 KiB, offline; Gemini `Unknown`), how to sign in, nine keys; `:news` / once at a new version's first start (`welcome::news_since`, `seen` in prefs, `Text::news` EN+TR, a test needs Cargo's version); order news > welcome > digest > left off; demo `sim::vendors`.
- 76 `keys`: a key file (`$XDG_CONFIG_HOME/imece/keys`, `KEY = KEY`, `#` comments, own parser) makes a key act as one of imece's, once (`Keymap::alone`/`pair` at the top of `App::key`; a side is one key or two, `gt = G`, `x = gg`: `keys::Keys`, the first of two held up to `keys::WAIT` 1 s in `App::keys_held`, let through by `App::keys_due`, never held while imece's own chord waits, `Why::Starts`/`Why::Long`; `[place]` lines (`keys::Scope`: salon board code talk change choose peek help, `App::key_scope`, `Keymap::at`, the help at `App::scope_below`); not while writing, in the help's `/` filter or hint letters; `^c` never; the lead question too); read again when its `keys::stamp` changes (`App::watch_keys` every 2 s, at once back in the window); refused whole (`keys::Why`: shape, no key, target not `Key::used`, twice, `^c`, 200 keys, 64 KiB, not text) keeps the keys in use, the last good text in meta `keys` (`prefs::keep_keys`, `keys::Start`); `:keys` reads again (taken N, `nothing does j now` from `Keymap::lost`, no file = imece's own); `?` lists the file's keys first and writes moved keys in place (`Keymap::shown`, `overlay::help_rows`, `TYPED` rows as they are).
- 77 `quiet-blur`: without the focus (`App::blurred`) the screen is drawn once per `BLURRED_DRAW` (1 s) unless something came from the terminal (`App::stirred`, any event; `App::draw_due`/`drawn` in the main loop), the rest of the loop as before; `tools/measure.py` (stdlib, `python3 -I`, `--fast`, `--focused`/`--blurred`) measures CPU, peak RSS and bytes drawn of a demo run on a pty: blurred 0.55 → 0.10 s CPU in 15 s fast; not in CI.
- 78 `undo-grace`: `:close` and `:forget` wait `UNDO_GRACE` (10 s) in `App::grace` (`Undoable::Close`/`Forget`, `grace_due` from the main loop, `end_grace` before another and at quit, `close_now`/`forget_now` as before); the bottom line counts down (`◷ … in N s · u keeps it`, `grace_left`), `u`/`:undo` drops it; refusals at once (`Live::can_forget`, `Sim::can_forget`, fewer than two open).

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
