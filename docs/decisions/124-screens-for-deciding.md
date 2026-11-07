# Screens for deciding

The person's word (2026-11-07): the screen is what they like least in
imece, after a review from a developer's chair on `--demo` at 150×44,
110×32 and 80×24. The finding: imece's screens watch agents, and a
developer's work is to decide. At 150 columns WAITING FOR YOU has 55, so a
question of 51 characters shows 19 while 26 of the salon's 40 rows stand
empty; a change is a raw `git diff` without its why; Lonca is a phrase in
a project's row; one piece of work has six screens; a salon row can hold 18
parts; `enter` has six meanings. They tried a keyboard prototype of the
answer and said to build it.

The direction, step by step, each worth having if the next never comes:

0. What a screen says is true. This record's first pull request, below.
1. Home: a "your turn" list across projects with a preview of the
   selection, a project bar saying whom `/` writes to, a layout that holds
   beside an editor. The salon stays, one key away.
2. The change: why, proof, gate and risk above the patch, a file tree, no
   `diff --git` lines, the next change after a decision.
3. Lonca as a surface: a line on top and a screen in the code map's place.
   `status` already gives `queue`, `changes[].paths`, conflicted symbols,
   `lease.pinned` and `session.unproposed`; the screen shows only trunk,
   the pending count, how far behind, and leases.
4. One detail screen with tabs (diff, talk, activity) in the place of
   focus, trace, peek, card and conversation; a board per project where
   done means in trunk.
5. Words and keys: one meaning a key, a help that starts from where you
   are, the guide and the videos.

Record 111 keeps new screen features to what a real session rubbed; this
review is the person's own friction, in `docs/field/2026-11-07-screens.md`,
and most of it takes things away.

Step 0 as built:

- A card finished whose change waits is not done: `card::board_column` puts
  it in TO REVIEW, between IN PROGRESS and VERIFYING, on the board and on
  the card's screen. The first column is TO DO: WAITING meant something
  else there than in WAITING FOR YOU. The owner's state glyph shows only
  while the card is at work, since it says what the agent does now.
  `cards_done` and the bars still count a `done`: they follow step 3.
- A project's row says what waits, then what is stuck, then the rest, Lonca
  last: Lonca's words had pushed `! N waiting` off the row at 150 columns.
- The row under a question names its keys (`a approve` was wrong there, and
  `enter` opens the conversation); an API error's too.
- An idle worker whose change waits says `Δ waits for you`, not `idle`.
- The footer starts with `W` while something waits; under a conversation
  it has only `esc` and `?`, the window has the rest. `ui::fit_hints` drops
  the view names, then keys from the end, never `esc` or `?`; `← →` left
  the salon's keys and a change's start with `a x`, `r`, `c n`.
- The pot outlives the last ten minutes when the header is short: what it
  costs matters more than what just happened.
- The story's first line counts agents, and says so: `! waiting on you (6)`
  beside a header of 9 items no longer reads as a contradiction.

The demo draws the same screens, so it mirrors all of this with no change
of its own. Nothing here grows or is kept.

Step 1 as built: `View::Home` (`5`, `:home`), `app/home.rs` and `ui/home.rs`,
beside the salon, which stays one key away. imece opens on Home (the person,
2026-11-07: `App::open_home` from `main`; a test's `App` starts on the salon). `App::home_rows_in` lists what waits in
`W`'s order, then what is stuck, at work and quiet; an agent waiting on an ask
or a question is that item, and a worker that went on from a waiting change is
both, for they are two pieces of work. Home keeps no selection of its own for
acting: `home_settle` puts the row at hand where every other key looks (the
pane, the waiting panel's selection, the salon's) before a key acts, so `enter`,
`a`, `r`, `e`, `@`, `K` and the rest are the ones the salon has. The row is held
by what it is, an `InboxItem`'s index or an agent, never by its place; when it is
gone as a key comes, `a r e A` do nothing and say so, for the row now in its
place was not read. The person's `[salon]` keys hold on Home (`Scope::Salon`):
a place of its own can follow if a key file wants one. Hints (`F`) are not on
Home, and a start's notes land in the journal, on the salon. The rows are drawn from the world each frame, so nothing is kept but the
row and the scope, for the run; the demo shows it through the same code.

Step 2 as built, the change: what the worker said under the first line of its
newest `done` heads the view (`World::done_why`, read from the journal, so
nothing new is stored and an older change has it too; four rows), and Home's
preview says the same. `ui::diff::why_it_waits` puts Lonca's reasons in words;
a reason it does not know stands as Lonca wrote it, so a new one is never
hidden. `Shown::of` leaves out git's `index`, `---` and `+++`, only above a
file's first hunk: below it `--- x` is a removed line `-- x`. The patch is
kept as git wrote it, so copying, notes, quotes and the scroll's numbers do not
change. From 120 columns a change of several files lists them beside the patch
(`tree_lines`, by path, for `Change.files` need not follow the patch's order).
`a` from Home opens the next change waiting; a failed apply stays (`decide_change
-> bool`), and from the salon it goes back to the panel as before. The demo's
`done` says why, so it shows all this through the same code. Risk beyond what
the view said before (overlaps with other changes) is step 3's: it needs
`status.changes`.
