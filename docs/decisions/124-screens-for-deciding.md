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
   beside an editor. The salon stays, one key away, until a real session
   has used both.
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
