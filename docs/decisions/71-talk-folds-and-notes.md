# Conversation folds and notes on words

The person asked for three things in one PR (2026-11-01, issue #158): a
message of a conversation longer than one row folds to one row and opens
again; every message folds or opens at once; and a note can be written
on chosen words of a conversation and reach the agents. They chose that
the notes go to the agents (not that agents write notes), and that the
choice is word by word, not line by line.

**Folds.** `Thread::folds: Folds { all, except }` (`app.rs`): every
message folded or none (`zM` `zR`, `:fold` `:unfold` with a conversation
open), less or more the ones folded or opened one by one since (`h l ←
→`, `za` on the selected message), by their place in the conversation.
So a message that arrives while all are folded comes folded (the window
follows the talk, record 29). `App::fold_talk` remembers the last `zM`
or `zR` for the conversations opened after it in this run; memory only,
like the rest of a conversation's state. The keys are the salon's own
fold keys (`h l`, `za zM zR`), so nothing new to learn; the chord arms
for a conversation come before the salon's. `:fold` with a conversation
open folds it and stays (it used to go to the salon).

**A folded row.** `ui::thread_layout` draws a folded message as the
journal draws its line: the clock and `said_spans` (head and text on one
line, Markdown made plain), cut to the window by `ui::cut` (`…` where cut,
the blanks before it dropped). What tells the person more is there is
its end: `▸ N lines` (`folded_lines`, `folded_line` for one), the lines
opening it shows, when it has more than one line or its one line was
cut; a message that fits whole has no mark. A folded message with notes
ends with `✎ N`. Folded rows have no line ranges: choosing (`v`) opens
every message while it lasts, so the cursor never walks over a hidden
line, and the folds are back after it.

**Words.** The choice of copy mode (record 70) gets a word-wise form in a
conversation: `Thread::words: Option<(anchor word, cursor word)>`, each
counted in its own line of `ui::thread_lines`; a `CopyLine` now has
`words` too, the drawn line's words after its lead spans (`ui::line_words`:
indent, bullet, code rule, quote bar are no words, `**beta**` is `beta`).
`Thread::part(i)` says which words of line `i` are chosen, `(first, last)`
with `last` past the end meaning to the end. `w l →` and `b h ←`
(`ui::word_step`) step a word on or back over line ends; the first one
turns a choice of whole lines word-wise from the ends it had (downward:
its first line's start, the cursor on its last line's last word; upward
the other way; one line: its first word). `j k` keep the cursor's word,
clamped to the line. `s` starts the choice again at the cursor (its word
alone), which is how a choice starts mid-line; vim starts the choice
where the cursor already is, but `v` here opens on the message and its
line-wise meaning was kept. Chosen words carry the selection background
(`ui::paint_words`, the blanks between two chosen words too), the
cursor's word is reversed, and a line chosen whole keeps the row's
background. `y` copies through `ui::chosen_text`: a line chosen whole as
written (`markdown_src`'s source, as record 70), a part as its words
chosen, one blank between them.

**Notes.** `c` while choosing in a conversation (it did nothing there
before) opens the command line on `Noting::Words(TalkNote)`: `noting`
became `enum Noting { Line(agent, line), Words(TalkNote) }` beside the
change's line notes (record 49). `TalkNote { to, entry, quote, text }`:
the journal line the choice starts in, the words as `chosen_text` gives
them (screen text, cleaned), the person's note. It goes to that line's
`Event::agent()`: the agent who wrote it, or for the person's own line
the agent it went to; a talk between the lead and a worker sends each
side its own. Kept in `App::talk_notes`, memory only, until sent; the
same words of the same line again write over (their text on the line),
an empty note takes it away, blank words take none. Under an open
message each note is a dim row `✎ "words" note` (cut to the window);
the title counts the conversation's notes, `✎ 2`.

**Sending.** `n` in a conversation (`send_talk_notes`): its notes, by
agent, oldest line first, one message each through `tell`, the same path
as a message the person writes, so it is journaled as theirs, wakes the
agent and is refused the same way (the budget spent: the notes stay;
`tell` now says whether the message went). The message (English, as
every word to an agent) is `ui::talk_notes_message`: a line saying what
follows, then per note `On message N:` when the message has a bus number,
the words quoted with `> `, the note. Unlike the change's notes nothing
goes back: a conversation has no gate.

**Screen.** The conversation's key bar became `↑↓ j k message · [ ] turn
· % reply · h l zM zR fold · c C copy · v choose · n send notes · esc
close` (it fits 116 columns in both languages; `%` lost `↔ question`,
`c C` its `message · all`, both still in the help). Choosing in a
conversation has its own bar (`choose_hints`: line, word, start here,
first · last, copy, note, leave; `pgup pgdn` still work, as in the
help); a change keeps `copy_hints`. The `?` help keeps 55 rows: `m` and
`'` share a row, the copy row split into `c y  C Y` and `v  w b  s`, the
`c  n` and `h l` rows name the conversation too. The legend's `▾ ▸` row
names a folded message. While the note is written the command line says
`note on "the words…" ▸` (`brief`, 24 characters).

**Demo.** All of it is the screen's, and the notes go through `tell`, so
the demo's conversations fold, choose words and send notes the same way,
and its agents get the message as any of the person's (record 31).

Not done: notes kept between runs (the store); a note on words of the
journal or of a card's screen; an agent answering a note with `re` to
the message it quoted (the person's messages carry no number); folds
kept per conversation after it closes.
