# The leader names the issue a card closes; imece closes it

Issue #456 (user, 2026-11-10): work assigned from the outside board (a
GitHub issue) should be closed there when it is done, as part of the
leader driving the process; a setting, on by default.

## What was decided (user, on the issue)

- An item counts as done after the person's yes at the Δ gate, not at the
  worker's `done`.
- **imece closes it, the leader names it.** The leader says which issue a
  card answers (`closes: N` on an `assign`, a `post` or a plan card); the
  leader stays non-writing, no worker is asked to close anything, no
  model's turn is spent. Two other ways were weighed and dropped: a worker
  running `gh issue close` (a turn each time, depends on the model, and a
  standing yes for `gh issue` also covers `gh issue delete`), and the
  leader closing it under an exception to its role.
- **On Lonca** the issue closes when the change is integrated. Setting
  on: at once, a journal line says so. Setting off: an item in TO DECIDE,
  `a` closes, `r` leaves it open.
- **On the git path** a yes leaves the change staged, not committed or
  pushed, so imece closes nothing: `:commit`'s message gets `Closes #N`
  and GitHub closes the issue when that commit reaches the default
  branch. Setting off: no line at all.
- The number: digits only, the checkout's own repository only.
- A change applied in part, a card failed or withdrawn: nothing closes.
  `gh` missing or signed out: said once, the issue stays open.
- The setting: on from the first start, `:closes on|off`, saved per
  project, no start flag.
- The look (mock-ups, the person took B): a line of its own under the
  verdict with the issue's title, read once with `gh`, since the number
  is a model's word and the title lets the person check it before the
  yes.

## What is built

**The envelope and the card.** `closes.rs`: `parse` takes a whole number
from 1 of at most 9 digits, as a number or its digits; anything else
(`#58`, `owner/repo#58`, an address, a leading zero, a sign) is refused
with why. `Card.closes`, `Mail.closes` (an assign's until it is
delivered), `Planned.closes`, `Opening.closes` (`model::Item` = `u32`).
`closes` goes with an assign only, and not with `card:` (a card handed
again keeps its own). Tool schemas `closes_schema` on `send`, `post` and
the plan's cards; the leader's prompt says a card that answers an issue
names it and that no worker closes one. The worker's task says nothing of
it.

**Where the item stands.** `Card.item: ItemState`: `Waits` (the change is
not in, or the git path) → `Due` (to be closed) or `Asked` (closing off:
`InboxKind::Close`, the leader's, holds no mail) → `Closed` / `Left` /
`Failed`; `Named` once a commit that ran named it. `World::items_landed`
from `cards_landed` on `Landed::Integrated` only; `items_due`,
`item_settled` (once), `decide_item`, `items_unread`, `closes_of`,
`change_closes`, `commit_closes`, `commit_ran`. One event,
`Event::Item { agent: leader, card, item, title, news: ItemNews }`
(`Closed`, `Already`, `Asked`, `Left`, `Failed(ItemWhy)`), and
`StoryEvent::Item` on the card. Schema 36: `cards.closes`, `cards.item`,
`mail.closes`, `projects.closes` (default 1). A card `Due` at a restart is
closed then.

**The board.** `imece_agent::board`: `look` = `gh issue view N --json
title,state,url`, `close` = a look, then `gh issue close N` only for an
issue the look found open. `gh` runs through `child::command` (cleared
environment, the sign-in and proxy variables kept, never `GH_REPO`) in
the checkout, 20 s, no prompt. `Seen`: `Open(title)`, `Closed(title)`,
`Pull`, `NoSuch`, `Unread(Why)`; a title is cut at 120 characters.

**Live.** `live/items.rs`: one call at a time on a thread
(`Live::items_due` each step: a card due first, else an item unread;
`take_items`). `World.items: HashMap<(ProjectId, Item), Outside>`, memory
only. The leader gets a note with its next prompt (imece's words, never
what `gh` printed), no wake. `Items.program` is none under the stand-in
and in tests: nothing is looked at or closed there.

**The screen.** `ui::diff::closes_line` under the verdict (`closes #58
“title” · when it lands` / `you are asked once it lands` / `:commit's
message will say so` / `nothing closes it: :closes is off`; loud for an
issue closed already, a pull request's number, no such issue); the Home
change row ends ` · closes #58`; the card's screen `closes #58 “title” ·
open`; the plan window `closes #58`; the Home item `close #58? “title” ·
card #6 landed` with `a close` `r leave open`; `:closes [on|off]`
(`Project.closing`, the menu's note). `:commit`: `closes::with_trailers`
adds `Closes #N` lines in `App::show_suggestion`; `App::ran` marks the
cards named when a `git … commit` that names them ended with 0 (not a dry
run).

**The demo.** Every third card names an issue (`demo_closes`,
`FIRST_ISSUE` 40 + the card's number); `Sim::items` reads it at once,
titled as its card, closes one due at the next step and fails every
seventh (`gh did not answer`). Nothing leaves the machine.

## Probe facts (gh 2.x, 2026-11-10)

- `gh issue view N` answers for a pull request's number too (`url` ends
  `/pull/N`, `state` `MERGED`), and `gh issue close` would act on it. So
  nothing is closed that a look did not find at `/issues/N`, open.
- A number nobody has: exit 1, `Could not resolve to an issue or pull
  request with the number of N`.
- `gh` finds the repository from the directory's remotes; `-R` and
  `GH_REPO` would name another, and neither is ever passed.

## Security

- The number is a model's word and reaches a command line: it is a `u32`
  written as its digits in an argument of its own, never a shell line,
  and the store refuses a value that is no such number at load.
- A title is what anyone who can open an issue wrote: it is drawn only
  through `fill`/`clean`, and is never sent to an agent.
- What `gh` printed on a failure goes to the journal (cleaned), never
  into the leader's note.
- Closing happens only after the person's yes to a change that showed
  the number, and with `:closes off` after a second yes.

## Known gaps

- Only GitHub issues, through `gh`; no other board.
- One issue a card.
- The title is read once a run; an issue retitled or closed meanwhile
  shows as it was, and the close itself looks again.
- A failed close is not tried again; the person closes it by hand.
- A `:commit` for another project than the one at hand (`git -C`) does
  not mark its cards named, so the next message says the line again.
- The stand-in's leader names no issue, so `--demo-live` shows none.
- `imece doctor` does not look for `gh`.
- Off the git path and off Lonca (a folder without git) there are no
  cards with a leader, so nothing applies.
