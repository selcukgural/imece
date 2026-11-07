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

Step 1's header as built: `ui::header_parts` has what waits (`KEEP`), the
fleet (8), the pot (7; `KEEP` in the attention colour with `~N left` once
`pot_low`), a usage window only from `LIMIT_LOUD` (`KEEP`), the clock
glued with the pause or the demo's speed. The last ten minutes
(`World::pulse`, `Pulse`, `pulse_spans`), the cards count
(`World::cards_done`) and the researchers' cap left; the rate, how long
the pot lasts and a window short of loud are the `:budget` note's
(`App::command_note`). The projects are not in the header yet: Home's
project bar has them, and step 2 folds the two.
