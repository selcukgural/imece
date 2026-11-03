# The story beside the salon, and whom the person holds up

Part B of #257 (2026-11-03), with D, after records 97 and 98. The
journal pane lists events one line each, newest first, both projects
mixed: on a 160×48 demo screen most lines were cut at the panel's edge,
the same setup line came once per worker (node_modules copied, a
`.worktreeinclude` line refused: 24 lines), and what belonged together
(a lead's assignment, a teammate's question, its answer, the done) was
spread among the rest. Nothing said who waited on whom.

The story (`ui/story.rs`) tells the same lines by card in the journal
pane's place. `App.story` (on by default: `App::new` has it off, as
`motion`, and `main` sets the saved one, `prefs` key `story` `on|off`,
`prefs::keep_story` when a key changed it) and `App::story_on` (not on a
card's screen, which keeps its own journal). `J` and `:story` switch,
with a flash; `:story` is allowed in an archive's view (it only changes
how the screen looks) and its menu note is `(on)`/`(off)`.

What it shows, top down:

- Whom the person holds up (D), `story::held`: the agents of open
  inbox items, in the inbox's order, then those `WaitingHuman`, then,
  in rounds, every agent `Asking` one already held, with that one
  (`claude-1 via codex-1`); as many rounds as there are agents, so a
  chain of any length is reached and no round runs forever. Gone agents
  and projects closed or archived are left out.
- A block per card, the card with the newest line first, as many as
  `height / 2 + 1` (each block takes at least two rows): the header
  (`#N title · owner`, then `✓ done`, `Δ waits for you` when an open
  change item names the card, both with ` · ` between, else the owner's
  `≡ 3/7`), the card's `STORY_LINES` (3) newest lines oldest first, and
  `now` with the owner's status and since-when while the card is not
  done and the owner is on it.
- `elsewhere`: lines of no card, folded by event kind
  (`std::mem::discriminant`), `STORY_OTHER` (4) kinds at most, each its
  newest line and `×N`; routine `Assign` lines of no card are left out,
  as the journal leaves assignments out.

Which card a line belongs to (`story::card_for`): the card it names
(`Event::card`); for something said (`words::said`), the card of whoever
asked, that is the sender of a new line and the one answered for a reply
(`re`), else the other party's; for an agent's own news (blocked, merged
or applied, a change, a stop, a failure, an apply that failed) its card.
Setting up (copies, setup scripts, hires, retries, vendor notes) is no
card's. "Its card" is the card the agent was on when the line was
written, not the one it holds now: the first build used `World::card_of`
and a worker that had moved on carried its last card's done and Δ into
the new card's block. `story::Held` keeps each owner's cards (not the
waiting ones) by when they were finished, an open one last; the card at
a tick is the first finished then or after, a helper's its worker's
(`Agent.parent`). It holds because a worker takes one card after the
other; a line at the very tick one card ends and the next begins goes
to the one that ended.

An ask (`MsgKind::Ask` with a number; one from before numbers, 0, is
not one that waits) with no reply whose `re` is its number says `no
answer yet`, then `no answer · 2 min`: the pass goes newest first, so a
reply is seen before its ask.

Read cost: one pass back over at most `STORY_BACK` (2,000) journal lines
per frame, whatever the journal's length, and one pass over the cards for
`Held`; nothing else that grows. The first build stopped the pass once
the pane was full, which no test could tell from going on; going on is
simpler and still bounded. Ten times the agents: the blocks are capped by
the pane, the held line is cut at its edge.

Keys: in the pane (focused with `tab`), `j k gg G` select a card's
header or a line of elsewhere (`App.story_sel` among `App.story_items`,
which the drawing leaves); the selected row is reversed and kept in
view; `enter` on a card opens its screen (`App::open_card`), on a line
the journal at that line (the story goes off); `#` writes the selected
card (`card_at_hand`); the agent at hand is the card's owner or the
line's agent. Help row `,  .  <  >  J`.

Mirrors: pure screen logic over the journal, cards and inbox the demo
fills as live does, so `--demo` shows it the same; checked on a 170×48
demo screen.

Not done: hint letters (`F`) do not label the story's rows (the journal
pane's `Target::Line` would select a journal line the story does not
show); a card's lines older than the 2,000 newest are not told; the
held line is not selectable (`W` goes to what waits).
