# Writing to the leader and the workers as in Claude Code's own screen: the conversation above the line

Issue #453 (user, 2026-11-10): earlier exchanges with the leader and with
workers should be easy to see, and writing to them should feel like
Claude Code's own CLI, the conversation above and the line to write on
below. Until now `/` opened only the line; what was said before was a
window to open apart (`v`), and while the workers were at work the leader
was on no row of Home.

## What was decided (user, on the mock-ups: A · 1 · P)

- **Where the conversation stands while writing: a screen of its own**
  (A). `/` opens the person's conversation with the leader, the line to
  write on under it; `enter` sends and the line stays open. A strip above
  the line on Home (B) was dropped: a long answer does not read in a few
  rows.
- **What it shows: the words and, between them, what the agent did** (1),
  as Claude Code shows its tool calls. The words alone (2), today's
  conversation window, leave the doing to another tab.
- **The leader on Home: one line of its own above the status line** (P),
  always in the same place, where the line to write on opens. A first row
  of AT WORK (Q) scrolls out of sight with a long list.

## What is built

**A chat** is the open conversation when it is between the person and one
agent (`ui::chat_with`: by its first line, else, with nothing said yet,
the agent it was opened for, `Thread.about`; `App::chat_agent`). No new
state beside `Thread`: `App::open_chat(id)` opens it at its newest line
(`ui::chat_of`), empty when nothing was said.

- `/` `i` → `App::write_to(leader of the project at hand)`; in a chat open
  already, to its agent, as it stands. `@` in a chat writes to its agent
  with no name; elsewhere it is as before (a name first, filled in on a
  focus, a trace and a change).
- A line that names nobody goes to the chat's agent
  (`App::unnamed_target`, `typing_target`), else to the leader as before.
- In a chat the person opened, the line stays open after a message that
  went (`submit`); one refused (the pot) closes it, since a flash is not
  seen while writing. `esc` leaves the line, `esc` again the chat.
- **A passing chat** (`Thread.passing`, `App::chat_follows` after every
  key of the line and the `@` key): once the line names an agent, its
  name whole (`named_on_line`), the conversation with it stands above the
  line; it goes when the line is sent, dropped or names it no more, so a
  message from Home still ends on Home (a yes moves down, record 125).
  Never in place of a conversation the person opened, never over a
  change (what is written there is about the lines in view), not while a
  note or a search is written.
- An archive's view opens no chat for a line it will refuse.
- An empty chat takes the conversation's own keys and does nothing with
  them (`thread_key`), follows its first line whoever writes it
  (`follow_thread`).

**How it reads** (`ui/overlay.rs`). `message_lines`: between the person
and an agent the head is the time and the kind (`14:02 [ask]`), the text's
first line led by `›` (the person, bold) or `●` (the agent, its colour) in
the indent's place (`speak`; both are leads for `is_lead`, so no word of a
choice). The head's copied text keeps who to whom. `thread_layout`: under
each open message what the agent did before the next (`Deeds`): the
journal's lines `did` takes (what it told a teammate, whom it hired, its
plan, what became of its work; not what it said to the person, nor a
project's news kept under its leader's name: the pot, the store, the
person's shell) and the files it changed (`StoryEvent::Touched` of the
cards it owns, by tick), dim, `⎿`, cut to the row, its own name off the
front, at most `DID_ROWS` 5, the newest, the rest counted (`… 3 earlier ·
tab log`). At the foot, while the agent is mid-turn, what it does now
(the step of its list, its call). `thread`: a chat is the whole body
under a rule (top and bottom borders, no box, no margins), the hints row
given to the conversation while the line is open; the status line offers
`/`. A conversation between two agents is drawn as before.

**The leader's line** (`ui::home::lead_line`, the last row of Home's
body): the leader of the project at hand, its glyph and name, what it
does mid-turn (`status_spans`), else the first line of its newest words
to the person and how long ago from a minute on, else `nothing said yet`;
the key at the right, kept while the words give way. None for a solo (it
has a row), a leader gone, a project closed or put away.

## The points of the list

- **Growth**: nothing new is kept. `Thread.passing` is a flag; the chat
  is journal indexes as every conversation.
- **Read cost**: every frame of an open chat reads the journal from the
  chat's first line, the newest `DID_BACK` 2,000 lines at most, and the
  stories of the cards the agent owns; under older messages nothing is
  listed. The leader's line reads back at most `LEAD_BACK` 2,000 lines
  each frame of Home; past them it says nothing rather than that nothing
  was said.
- **Ten times the scale**: a chat of thousands of messages is laid out
  whole each frame, as every conversation was before this; that is the
  first to slow, and the caps above do not help it.
- **Migration**: none; nothing is saved.
- **Failure**: a message refused closes the line and says why; a name
  that is no agent's opens nothing.

## Demo and live

The screen is the same code for both: the demo's leader answers and posts
cards under the person's words, its workers' files show between the
lines. Nothing here is engine's.

## Gaps

- An agent's reads and commands are not between the lines: only what the
  journal and the cards' stories keep (messages, hires, plans, files
  changed, what became of a change). The trace has them, without a tick
  to place them by; the log tab shows them.
- Every message has its time on a row of its own, also when it follows
  at once; the mock-up left the time out there.
- The detail's head says `lead · shop-api · lead` for a leader (its
  title is its name): as on its focus before this.
- The leader's line does not breathe while it works (`:motion`).
- The tour videos show `/` opening only the line; they are not rendered
  again here.
