# Export

`:export [PATH]` (2026-10-31, backlog C14 in #56, from Warp): what the
person has open, as a Markdown file of their own, to keep, send or read
elsewhere. Copying (`C`/`Y`) already put a conversation on the
clipboard; a file holds a card's whole screen too and survives the
session.

What is written is what is open: the conversation when one is open
(`App::thread`, also over a card's screen), else the card's screen
(`App::card`); nothing open says so and writes nothing. A conversation
(`ui/export.rs`, `thread`) is `# Conversation: A ⇄ B`, then each
message as a heading with its day and time, who to whom and its kind
(`### 2026-10-31 14:00 lead → claude-1 [ask]`) over its whole text, and
each other line as a list item. A card (`card`) is `# #N title`, its
project, column and minutes, its leases, `## WHO` (each part and name),
`## STORY` (the screen's story lines, one item each), what waits for
the person on it, and `## Journal`: every line of the card, oldest
first and none folded (`salon::card_lines`; the screen folds a
conversation's older lines under its newest). List items follow each
other; a message stands apart by a blank line. Every time has its day,
since the file leaves the screen. All text goes through `clean` like
everything drawn (no control characters, secret-looking tokens hidden);
an agent's own Markdown stays as it wrote it.

PATH is a file, or a directory to name one in; none is the directory
imece was started from. The name says what and when
(`imece-card-12-20261031-1400.md`, `imece-conversation-…`). A file
that is there already is never written over (`create_new`): the flash
says it could not write, and where. The flash says where it wrote.
`:export ` completes directories as `:open` does; the `:` menu shows
what it would write (`the conversation`, `#12`). The demo writes the
same files from its own world.
