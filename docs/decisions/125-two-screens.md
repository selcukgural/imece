# Two screens

The person's word (2026-11-07, evening, after record 124's first three
steps were built): the screen is still what they like least in imece; they
are not sure of its ease of use or a developer's comfort in it, and they
want nvim's philosophy, not its looks: a plain surface, depth behind `:`
and `?`, a screen that shows the way where one is stuck. A second review
from a developer's chair, on `--demo` at 150×44, 110×32, 100×40 and 80×24,
with the help, the `:` menu and the keyboard page beside it, found one
cause under every finding: the screens are built to watch agents, and a
developer's work in imece is to decide. The row is an agent, the detail is
an agent's focus, the board is agents on cards, the timeline is agents in
lanes; the decisions (a diff, a question, a permission) sit in a 55-column
side panel or in a window over all that. The second cause is volume: six
views, about 120 keys, 68 `:` commands, 25 glyphs in the legend, a salon
row of up to 18 parts, a footer of 8 hints and a header of 7 figures, on
screen at once. nvim is simple because its visible surface is a buffer and
a status line. The person accepted four rules and this order; the review
with its captures and mockups is in `docs/field/2026-11-07-screens-2.md`.

The four rules, each a decision:

1. **Two screens.** Home is the list: what is to decide, grouped by what
   deciding costs (permissions, then questions, then changes; among the
   changes, the ones whose checks passed and that stayed inside their
   paths first), then who is at work, stuck or idle, folded by project.
   Detail is one piece of work with three tabs: diff, talk, log. The
   focus, the raw stream, the peek, the card's screen, the conversation,
   the story and the journal pane are the detail's tabs or its rows; the
   board, the code map and the timeline are `:` windows, off the number
   keys; the salon is Home's lower half. A decision is never a modal: the
   row says the whole ask, wrapped, the keys that decide stand at its end
   and in the status line, and a yes moves the cursor down. The script's
   bytes, a question's context and a change's patch are one `enter` away.
2. **The task is the row.** Card number and title first, everywhere: every
   list, every title, every message. The agent is a fact inside the detail
   (`claude-9 · 6 min · $0.06`), never the name of a row or a screen. With
   Claude Code alone (record 109) `claude-18` says nothing to anyone.
3. **A status line, not a hint bar.** The bottom line says where you are,
   what is selected, the pot and a pending chord, then at most three keys
   for the selected thing. A chord (`g…`, `z…`, `q…`) shows its
   completions on that line after 300 ms (which-key). Everything else is
   in `?`, which opens filtered to the screen you are on. Movement keys
   are vim's and are never shown. The header has four things: the
   projects, what is to decide, the pot, the time.
4. **The developer's words, five glyphs.** `!` needs you, `●` working, `○`
   idle, `✓` done, `■` stuck. A kind is a word in a group heading, not a
   glyph on a row; folding keeps `▸ ▾`. What the screen says changes in
   both languages; the names in the code and in these records stay:
   salon → team, waiting for you → to decide, Δ waits for you → change
   ready · 2 files +4 −2 · tests ✓, apply · reject → approve · send back,
   waits for your yes "X" → asks to run X, ✉ 2 held → 2 messages queued
   until you answer, lease → editing, pot → the dollars alone, story ·
   journal · trace · focus · peek → log, settle → hide until it needs you,
   the stand → branch · N ahead · M behind on the detail only, stopped:
   the session ended → session ended N min ago · r restart; the pulse, the
   cards count, rung, need and loudest never on screen; soft-tainted says
   nothing (its diff goes through the person already).

A change's detail starts with a verdict line, the Δ gate in one line:
`✓ ready to approve · build ✓ · tests ✓ · 2 files, both inside #6 · no
overlap with other changes · after your yes Lonca runs: build, test`, or
`! look first · tests ✗ (dotnet test: 1 failed) · 1 file outside #18 ·
overlaps #51 on Money.cs`. Record 124's step 2 has the pieces as six
lines; the verdict assembles them. A question's detail is the ask whole,
its options as digits `1`-`9`, `/` for an answer in words, and what the
worker did so far under it.

Probe facts: Claude Code 2.1.319 under `-p` has no `AskUserQuestion`
(`ToolSearch select:AskUserQuestion` finds nothing; the model writes the
question as text), so a question's options come from imece's bus: `send`
to `you` with type `ask` takes an `options` list (at most four, short),
the prompts tell agents to ask with options, and an ask without them has
only `/`. Lonca's `status` (read from `lonca-policy/src/state.rs` and a
running daemon) gives `changes[].paths`, `base`, `intent`, `provenance`,
`status`, `queue`, `trunk`, `leases[].pinned` and `sessions[].unproposed`:
enough for the verdict line; an overlap is the intersection of pending
changes' `paths`, computed by imece, since Lonca has no field for it. The
demo's `--projects 1 --agents 3` is the real scale, and the design is
judged at it, at 100×40, before 150×44.

Order of work, each step worth having alone, each taking things away:

1. Words, glyphs, header, status line, which-key (record 124's step 5,
   first): `i18n.rs`, `words.rs`, the header and footer; no new state.
2. Home as the triage list: groups by kind, rows that say the whole ask,
   keys at the row, a yes that moves down, no modal for a permission or a
   script; one column under 120 columns, the preview from 120. Record
   124's step 1 keeps its model (`home_rows_in`, `home_settle`).
3. The detail screen with the verdict line and the question screen; the
   log tab in the place of focus, trace, peek, card and conversation
   (record 124's step 4, before its step 3).
4. Lonca on the verdict and in the changes' heading, a `:lonca` window
   for trunk, pending and leases (record 124's step 3, shrunk).
5. Board, code map and timeline to `:` windows; the salon into Home; the
   help filtered by screen; then the guide, the reference and the videos.

Growth: nothing grows; the legend, the help and the footer shrink. Read
cost: the status line reads the selection and one timer; which-key is one
`WAIT` of the chord held, as the key file's `keys::WAIT` already is.
Migration: a key that leaves the surface keeps its `:` name where it has
one, so a key file (record 76) that names it still works; one with no
`:` name (`K`, `F`, `S`, `,` `.` `<` `>`) is reported by `Keymap::lost`
as now; the prefs `story`, `review`, `notify`, `motion` keep their
meaning. Ten times the scale: a Home of 400 rows is still one list with
group headings and `gd`/`gw`; the preview reads one item. Failure: the
status line and which-key draw from state already held; a verdict line
missing a part (Lonca down, no check) says so in words, never a blank.

Gaps: the vendor's own `AskUserQuestion` would carry options if a later
Claude Code offered it under `-p`; the bus's `options` stays the path
either way. The tour videos and the guide follow step 5.

Step 1's status line as built: `ui::footer` draws `place_spans` (the
view's name or the screen's, from `Text::views` and `Text::places`; the
project at hand; the card of the open screen's agent or Home's row, else
its name; `cut_title` to `PLACE_TITLE` 30 cells), then at most three keys
for the selected thing, `esc` where there is a way back and `?`;
`fit_hints` drops keys from the end, never `esc` or `?`, and the place
goes only when its tail does not fit beside it. The views' names and the
demo's `space` left the footer; the header says PAUSED. `which_key` lists
a chord's second keys (`Text::chord_g`/`chord_z`/`chord_q`, then the key
file's pairs that start with the key held, `Keys::keys`) while `pending`
or `keys_held` waits. A window with a title of its own and copy mode keep
their keys and show no place. Words the footer no longer says left
`i18n.rs`.

Step 1's glyphs as built: `ui::glyph` gives the five of rule 4 and no
other: `●` for working, asking a teammate and researching, `○` for idle
and asleep, `■` for blocked and stopped, `!` and `✓` as before; the colour
still tells asking from working and blocked from stopped, the status
words do without it. A leader's row has its status's glyph like any row
(`◆` is gone; its role is in its name), a starting session is `●`
(`rung_glyph`, the timeline's lane, the check that runs, a card under
verification), a soft taint says nothing on a row (`taint_span`), the
`?` legend's first lines are the five and the folds, the code map's
`GLYPHS` are the four that can hold a file, a project's header counts
blocked and stopped as one `■N`, and the journal's Blocked and Failed
kinds are `■` while the leader's acts are `→` like an assignment. A
result that failed stays `✗` (a check, a run's tail, the queue's test,
Claude Code not signed in), a kind apart from a state; the raw stream's
`tool_result … ✕` is the adapter's own line and keeps its mark. The
archives browser puts the head first in bold instead of marking it.

Step 1's words as built, in `i18n.rs` for both languages: the screen
says team (`TEAM`, the status line's place, the `:` notes; `:salon` keeps
its name), to decide (`TO DECIDE`, the header's `! N to decide`, the
title, Home's first group, the farewell, the export), needs you (`!` on a
row, the rung, the legend, a group heading), change ready (the story's
and the journal's `finished: change ready · 2 files +4 −2`), approve ·
send back (`a` `r` on a change and in the help; the journal's `sent
back:`; a refused apply's `b` sends the clash back; a proposal's keys end
`· Lonca`, since `r` asks why first there), asks to run (`⚑ asks to run
"cmd"` on a row and in TO DECIDE, the notices), queued (`✉ 2 queued`,
`:why`), editing (the code map's title and legend, the focus and card
label), hidden until it needs you (`s`, `:settle`'s note, the title's
count), session ended (`■ session ended · write to it to restart`, the
journal, `:why`). The Turkish: ekip, karar bekliyor / KARAR BEKLEYEN,
seni bekliyor, değişiklik hazır, onayla · geri gönder, çalıştırmak
istiyor, kuyrukta, düzenliyor, gizle, oturum bitti. Not yet: log for
focus · trace · peek · story · journal and the stand on the detail only,
which come with step 3's screen; the pot's words are step 2's header.

Step 1's header as built: `ui::header_parts` has what waits (`KEEP`), the
fleet (8), the pot (`KEEP`; in the attention colour with `~N left` once
`pot_low`), a usage window only from `LIMIT_LOUD` (`KEEP`), the clock
glued with the pause or the demo's speed. The last ten minutes
(`World::pulse`, `Pulse`, `pulse_spans`), the cards count
(`World::cards_done`) and the researchers' cap left; the rate, how long
the pot lasts and a window short of loud are the `:budget` note's
(`App::command_note`). The projects are not in the header yet: Home's
project bar has them, and step 2 folds the two.

Step 2's Home as built: `App::home_rows_in` lists what is to decide by
`HomeGroup::of` the item's kind (permissions: an approval, the setup
script, the check; questions: a question, a report, an answer, a fault,
the plan; changes: a change, a sensitive one), what `s` put off last as
`HomeGroup::Later`, then stuck, at work and quiet under one AT WORK
heading that counts them; `App::waiting_rank` (group, not easy, age) is
`W`'s order too, a change easy when `Change::easy` (proved clean by the
check or the runs, `Change::proof`, and inside its card's paths,
`Change::outside`). `ui::home::row_lines` says the whole ask in the
person's words (`row_*` in `i18n.rs`; a script's commands by
`commands_of`, the check's ask tied to the change's card in `sim::check`
and `setup::ask`), wrapped under its column, a change's parts under each
other when short, an agent's status cut; the row at hand ends with its
two keys (`row_keys`) when the ask keeps `ASK_MIN` cells. A decision on
Home settles the row in its place at once (`App::home_decided` from
`dispatch::act`), so the next key decides it; `gd` `gw`
(`home_go_heading`, which-key's `chord_home`), `W` and `S` in a script's
window (`script_key`). The preview from `WIDE` 120 reads a script's lines
numbered. The header has the projects (`ui::projects_spans`, the narrowed
one lit) in the fleet's place and the pot as `$3.10 of $20.00`; the
project bar and `/ writes to` left Home. The status line names the card
an item came on, as the screen numbers it.

Step 3's verdict line as built (the first of its three pieces): a
change's view and its Home preview start with `World::verdict` (model.rs:
`Verdict { ready, proof: Vec<Proof>, files, card, outside, others,
overlaps: Vec<Overlap>, clashes, partial, verifies }`), ready when the
change is `easy` (proved clean, inside its card's paths), overlaps no
other change waiting in its project (`Change::shared_files`, the other's
card from `change_card`: the item's, else the worker's), fits the
checkout (`clashes_of`) and is shown whole (a proposal's `redacted` or
`cut`); `Change::proofs` is the check when it ran, then the runs.
`ui::diff::verdict_spans` words it (`verdict_*` in `i18n.rs`, both
languages; `✓ ready to approve` / `! look first` bold, each part in its
own colour, `VERDICT_NAMES` 3 names and the rest counted) and
`verdict_lines` wraps it to the window; the runs' lines, the check's
`✓`/`✗` line, the outside line and the proposal's verifies line left the
change view (`check_lines` keeps a failure's output and a check not
ended), the words `ran_none_long`, `check_ok`, `row_outside`,
`diff_verifies*` left `i18n.rs`. Home's change row is the verdict without
its head, the lines added and removed on the files part
(`home::change_spans`), and the preview's first label is `verdict`;
`waiting_rank` and the changes heading's `N ready to approve` count
`Verdict::ready`, so an overlapping change is no longer easy. The
proposal's `Verdict` enum became `Decision`. The demo's workers already
overlap through their strays (`the_demo_shows_a_change_that_overlaps…`).
Left for the step: the question screen with options, and the detail's
tabs.

Step 3's question screen as built (its second piece): an `ask` to `you`
carries `options` (record 27 has the bus side: `OPTIONS_MAX` 4,
`OPTION_MAX` 60, `Event::ToYou.options`, `World::question_of`). On Home
the row says the question and, under it, each answer with its key
(`Tail::Asked`; `row_keys` `1-3 pick · a answer`, `1 pick` for one), the
preview reads the question whole (cleaned line by line), the answers,
then `so far:` and the agent's status, to-do list and newest trace lines
(`question_lines` + `agent_lines`); `1`-`9` answer (`App::option_at_hand`,
`answer_option` → `tell` → `home_decided`), `/` opens the composer to the
agent (`question_at_hand`, `answer_item`), both on Home only, and the
digits stay the views' otherwise. The demo's questions offer answers on
two of four (`Content.ask_options`). Left for the step: the detail's tabs
(diff · talk · log); the question's own detail screen comes with them.

Step 3's detail frame as built (its third piece): `App::detail` names the
piece of work on screen and its tab (`app/detail.rs`: `Tab` Diff/Talk/Log;
a conversation first (`thread_agent`: the first line's writer, else the
one written to), else `Zoom::Diff` → diff, `Zoom::Focus`/`Trace` → log);
`tab`/`⇧tab` (`next_tab`, before the conversation's and the zooms' keys)
go round to the next tab that has something (`tab_has`: a change, a line
said with the person else with anyone; the log always), `open_tab` (the
diff and the log close a conversation; the talk opens over what was
there); a digit and `/` work on a question's detail as on Home
(`question_at_hand` covers the detail). `ui::detail::frame` draws the
head (`head_line`: the card the open item came on else the agent's
(`detail_card`), the card's title else the change's on the diff else the
focus's first words, project, agent, `ago` since `App::since`, cost; on
the diff the files and lines and `place_among` at the right), a question's
block (`question_lines`: `{} asks you`, the ask whole cleaned line by
line, the answers with their key badges and `/ write your own answer`),
and the tab bar (`tab_line`: the open tab bold, one without content dim,
`tab switches` when another has); the bodies draw under it: `focus::draw`
framed skips its own title line (the peek keeps it), the change's block
lost its `CHANGE · …` title (`diff_title` left `i18n.rs`; the copy title
stays), `overlay::thread` takes the area it may use (the body under the
frame, so it no longer covers the composer). Help row `tab`, keyboard page.
Left: the question's own detail screen and the detail as the one place
of a card's screen, the story and the peek (step 5).

Step 4's verdict as built (its first piece): a look keeps every proposal
waiting in a repository that holds changes for a person
(`Looked.pending`, PROPOSE operations with their trunk; `pending_of` →
`model::StorePending { seq, change, name, holder, paths, on_trunk }`, the
holder through `Live::owner`, `on_trunk` = `base` is the trunk of that
look), `OnLonca.pending` through `World::store_pending`, gone with a store
that is down. `World::verdict` also overlaps the change's files with the
paths of the proposals no change of the project shows
(`pending_elsewhere`: anyone else's, and a worker's whose turn is not
over; its own and a worker's whose change is shown are judged by the
changes themselves), counted in `others`; `Overlap.with` is `Other::Change
{ agent, card }` or `Other::Proposal { seq, name }`, worded `overlaps
proposal 12 (ayse) on Money.cs`. `StoreProposal.on_trunk` (from `offers`,
followed by `offered`) → `Verdict.stale`, worded `based before trunk,
merged after your yes` as a dim fact: the queue merges a stale base
(record 115's probe), so it leaves the change ready. The demo's table
holds a stranger's proposal on each repository's first file
(`sim::STRANGER`, `demo_pending`), and each third demo proposal is based
before trunk.

Step 4's window as built (its second piece, the step's last): bare
`:lonca` on a project on Lonca opens `App.lonca_win: Option<LoncaWin {
at }>` (`open_lonca`; off Lonca the flash as before), drawn by
`overlay::lonca` over the screen: `lonca_state` as the head, `WAITING FOR
A HUMAN · N` with a row per `OnLonca.pending` (number, the holder's name
here else Lonca's, `diff::named` paths, `Δ shown` when `lonca_row_shown`:
its holder's `proposal_of` is this seq, `based before trunk`), the row at
hand marked and kept in view, `HELD · N` from `OnLonca.leases`. `lonca_key`
takes every key (`j k g G`, `enter` → `lonca_open_change`: the change's
`Zoom::Diff`, on Home its row at hand; a proposal that is no change shown
flashes `lonca_not_shown`; `esc` `q` close, `?` the help). Commands
reference EN+TR. The demo's second project shows it with the stranger's
proposal; the demo lists no held symbols (record 123: its map stays as it
was), which the page says.

Step 5's windows as built (its first piece): the board, the code map and
the timeline left `View` (`View::Salon` and `View::Home` remain) for
`App.window: Option<Window>` (`app/views.rs`): `:board`, `:code` and
`:timeline` alone open one over whatever is on the screen (`open_window`;
the timeline on its first lane), drawn by `overlay::view` on the shaded
screen (the board under a title that counts its cards, the map and the
timeline as they drew themselves); `window_key` takes every key: `esc`
and `q` close, `?` opens the help over it, the timeline's `j k g G`
(the arrows, Home, End), `[ ]`, `enter` (its focus in the window's
place) and `@` (the line to it); nothing under the window moves. A view
shown (`:home`, `:salon`, `:waiting` …) closes the window; a notice's
click too. The digits `1`-`5` open nothing: `1`-`9` pick a question's
answer on Home and the detail and do nothing else. The key file's places
`[board]`, `[code]`, `[timeline]` are the windows' (`Window::scope`,
`scope_below` first); the status line shows the window's keys and no
place; the board's window breathes as the view did, and nothing is
patched under a window; `at_rest` is false with one open. The help row
`1 2 3 4 5` left, the `:` notes say "in a window", `Text::views` has the
two views. Records 04, 17 and 100 say so; the keyboard, commands,
board-and-code-map, timeline, salon, Home, cards and command-line pages
in both languages.

Step 5's team as built (its second piece): Home's lower half is the
team. `App::home_rows_in` lists AT WORK project by project in the
salon's order (`World::salon_order`): a heading row `HomeRow::Project`
(group `HomeGroup::Team`) when more than one project is in view and none
is narrowed to (`home_headed`), drawn as the team's project line with the
cursor before it (`ui::home::project_row`, `salon::project_line`), a
folded project (`App.collapsed`, shared with the team) its heading alone,
a project with nobody on the list no heading; under it the workers and
solos stuck, at work and quiet, each with its helpers and researcher
hung under it (`App::row_group`, drawn `└`), then the helpers whose
worker is an item's row; an agent `s` hid, or one hung under a hidden
worker, is out (`App.settled`, `settled_projects`), counted in the AT
WORK heading (`· N hidden (:unsettle)`). `h` `l` (`home_fold`: the row
at hand's project, the heading coming to hand), `za` (`home_fold_toggle`),
`zM` `zR`, `:fold` and `:unfold` (`fold_all`) work on Home and stay
there; `s` on an agent's row or a heading is `:settle` (`home_hide`,
refused as the team refuses it), on an item `later` as before; `:settle`
and `:unsettle` stay on Home; `enter` on a heading opens the leader's
focus; the status line names the heading's keys (`enter · h l · s`),
an agent's (`enter · s · @`); the heading's preview is the folder, the
Lonca state and the leader's lines. The team view (`:salon`) stays as it
was for now: its panes go with the next piece.

Step 5's windows and the card as built (its third piece): the journal is a
window, `Window::Journal` (`:journal`, `J`; the key file's place
`[journal]`, `Scope::Journal`), drawn by `salon::journal_pane` on the
shaded screen, its keys the pane's (`App::journal_key`: `j k g G`, the
pages, `J` the story, `m` `'`, `c` `y`, `enter` the line's conversation in
the window's place or the story's item, `esc`/`q` close, `esc` first the
story back); `:story` opens it and turns the story; `'` from anywhere
opens it at the next mark. A card's screen is the log tab of its owner's
detail (`open_card` → `Zoom::Focus`, `card::draw` as the body under the
frame, `detail_card` and `card_at_hand` the card, `esc` leaves it). The
team's commands land on Home: `:salon` its AT WORK, `:waiting` its first
row to decide, `W` and a notice's click Home's item, `&`/`:goto` its row
or heading. The team view (`View::Salon`) is reached by nothing now and
goes with the next piece.

Step 5's salon gone (its last piece): `View` and `App.view` are no more,
Home is the screen and `App::go_home` is where every view-leaving path
lands; `ui/salon.rs` keeps the rows' pieces Home and the card's screen
use (`project_line`, `agent_line`, the journal pane) and lost its drawing
(`draw`, `salon`, `side`, `rows`, `Row`, the queue line). With the screen
went its keys and state: the panes' cycling (`tab` `⇧tab` `[` `]` `w` on
the salon, `pane_keys`, `cycle_pane`), the side panel's width (`, . < >`,
`App.side`, the `side` pref, `prefs::save` without it), the hint letters
(`F`, `:hints`, `Hints`, `Target`, `hintable`, record 74), only-what-needs-
you (`f`, `:needs`, `needs_only`, record 38), the salon's own folding of a
worker's helpers (`fold`, `worker_at_hand`), `Talk.until` (the salon's
folding of calm rows). `Pane` stays as the model under the keys (Home's
item or agent at hand, the journal window). The status line's place on
Home is `home` (`Text::home_place`); `:salon` is `go_team` (Home's AT
WORK), the `salon` place of the key file is Home's. Home shows what the
salon alone showed: what `:clear` hid (`· N hidden (:unclear)` on the TO
DECIDE heading, record 90), imece's mark in the free rows (record 101),
the breath on a working row (record 84).
