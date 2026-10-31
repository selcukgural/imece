# A card's screen

A card's screen v0 (2026-10-31, user's request: search by the number a
card shows in the salon and the journal, `#323`, and see everything of
it on one screen grouped like the main one; user's decisions: the `#`
key, the journal lines that name the card plus what was said with its
number in it (NOT every line of its owner while it held the card), and
the menu matches the number or the title). `#` opens the command line
with `#` and the menu of cards (`complete::cards`, at most
`CARDS_SHOWN` 30 of open projects): digits match the start of the
number, by number; anything else matches inside the title, case aside,
newest first; nothing typed offers every card, newest first. Each item
is `#N title` and replaces the whole input; `enter` takes the
highlighted card and opens it at once (one key, not insert-then-send);
a line that names no card (`complete::card_named`, the number it starts
with) is flashed as `no card #…`. `#` writes the card at hand already
(`App::card_at_hand`): the zoomed or selected agent's (a helper's is
its worker's, `World::card_of`), the one the selected inbox item came
on, the one the selected journal line names or first mentions; on a
card's screen its own card is no news and `#` starts empty.

The screen (`App::card`, drawn by `ui/card.rs` in the salon's place,
the side panel by the salon's own code) is the main screen's three
panes for one card. Left: its project, column (the board's words) and
minutes (to `finished` when done), its leases, WHO (`World::card_people`:
who posted it, its owner, the owner's helpers while it is still the
owner's card, then everyone its story says talked about it; each once,
with the first part it had, drawn as the salon's agent rows so their
state is live; `j k` and `enter` work on them as in the salon) and its
STORY (the focus window's lines, `focus::story_lines`, told as the
owner had them). Right: WAITING FOR YOU for the card
(`World::card_inbox`) and its JOURNAL (`ui::card_rows`: `Posted`,
`Claimed`, `Assign`, `Merged`, and every said line whose text has
`#N` where no digit follows, `model::mentions`; folded like the
journal; the number marked in the accent wherever it is named,
`salon::marked`). Every key does what it does on the main screen (tab,
`enter` on a line opens the conversation, `a r`, `c`, `t`, `v`);
`n p` step to the next or previous card of an open project; `esc`
from a focus, trace or diff opened there comes back to the card, `esc`
on it back to the salon; `1 2 3` and closing the card's project leave
it; the salon's fold keys do nothing there.

An inbox item keeps the card its agent worked on when it came
(`InboxItem::card`, `World::card_of` at the item's birth; store schema
6, `inbox.card`, added to an older file by `ALTER`): a worker's Δ
stays with the card it finished after the worker takes the next one
(live keeps `agent.card` after done until the next task's card). A Δ
is still the worktree's whole change, so one that holds two tasks'
work shows on the later card only.

Found on the way: `ui::plain` and `ui::markdown` took any line that
starts with `#` for a heading and dropped the hashes, so a message
starting `#5 Money …` lost its number on the screen; a heading is now
one to six `#` and a space or nothing after them (CommonMark's ATX
rule, `ui::heading`).

The demo shows all of it (its cards, inbox and journal are the model's
own); nothing here is live-only. Gaps: no free-text search of the
journal (the user chose number and title for now); the menu shows 30
cards at most; a card's own Δ line in the journal (`ChangeReady`,
`Applied`) does not name the card, so it is on the card's screen only
through the inbox.
