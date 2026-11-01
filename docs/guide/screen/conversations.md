---
title: Conversations
description: The conversation window - everything said between two parties, read whole, with turns, replies, marks, folding, copying, notes on words and export.
---

# Conversations

A conversation is everything said between two parties, you and an agent or two agents, oldest first, each message whole. You open one with `enter` on a journal line or with `v` (`:talk`) on the agent at hand; it opens as a window over the salon, dimming what is around it, and `esc` closes it.

```text
┌ CONVERSATION · lead ⇄ claude-1 · 4/5 ──────────────────────────────────────┐
│ 13:55 lead → claude-1 [assign]                                            │
│   #3 Parse the manifest                                                   │
│   The manifest parser panics on an empty file; make it return an error.  │
│                                                                           │
│ 13:57 claude-1 → lead [ask] ↩                                             │
│   ↩ re 7: #3 Parse the manifest                                           │
│   Which parser? There are two: `manifest::read` and the legacy one.       │
│                                                                           │
│▌✱14:01 lead → claude-1 [answer] ↩                                         │
│▌  ↩ re 8: Which parser? There are two                                     │
│▌  `manifest::read`; the legacy one goes in the next card.                 │
│                                                                           │
│ ↑↓ j k message  [ ] turn  % reply  h l zM zR fold  c C copy  v choose  n send notes  esc close
└───────────────────────────────────────────────────────────────────────────┘
```

The title names the two parties and where the selection stands, `4/5`, then `✎ 2` when two notes on its words wait to be sent (below). A window opened on a line that is no conversation (an event of the journal) is titled EVENT and holds that line.

## Opening one

- `enter` on a journal line opens the conversation that line belongs to, with that line selected.
- `v` or `:talk` (`:talk @NAME` for a named agent) opens the conversation between you and the agent at hand: the selected or zoomed agent. With nothing said yet it says so.
- `enter` on a question (`?`) or a reply (`»`) in WAITING FOR YOU opens the conversation at that message; a reply is read, and closed, by it.
- `W` opens it the same way when the next item waiting is a question or a reply.

A message to an idle worker opens a card for it (the first line is the card's title), and its answer closes the card. So writing `@claude-2 text` from the window is a card as well as a message.

## Reading

| Key | What it does |
| --- | --- |
| `↑` `↓` / `j` `k` | the message above or below; the window scrolls to keep it in view |
| `[` `]` | turn to turn: a turn is the lines one side says before the other answers. `]` goes to the next turn's first line, `[` to the start of the turn at hand, or of the one before when at a start already |
| `%` | from a reply to the message it answers, and from a message to its first reply |
| `gg` `G` | first / last message |
| `pgup` `pgdn`, `^d` `^u` `^f` `^b`, `space` | scroll a page or half of one |
| `m` (`:mark`) / `'` | mark the selected message to come back to, again to unmark · the next marked message, round to the first |
| `h` `←` / `l` `→` | fold the selected message to one row / open it (below) |
| `za` / `zM` `zR` | fold or open the selected message / every message (`:fold` `:unfold`) |
| `c` `y` / `C` `Y` | copy the message / the whole conversation |
| `v` | choose lines, or words, to copy or to note (below) |
| `n` | send the notes on its words to their agents (below) |
| `:export [PATH]` | write the conversation as a Markdown file |
| `esc` | close |

### Replies

Every message one agent sends another has a number on the bus, in order across the world and kept between runs; the agents see it in the mail they get (`[answer 8, re 7] from claude-1:`) and the trace says `id=8 re=7`. On the screen a reply has `↩` after its type, on its journal line, in a card's story and in a conversation; in a conversation the first line of what it answers shows dim under its head (`↩ re 8: Which parser?`, or that the journal no longer has it). `%` goes between them. Messages to you take no number: your questions wait in WAITING FOR YOU, which already ties the answer to the question.

### Marks

`m` marks the selected message; `✱` shows in the gutter of its first row and the flash says `marked: ' comes back to it`. `'` selects the next marked message after the one at hand among the conversation's own lines. A mark is yours, so it outlives the run (kept in the store) and the journal never folds or tidies a marked line away. See [the journal](journal.md) for marks outside a conversation.

### Copying

`c` or `y` copies the selected message: its text alone. `C` or `Y` copies the whole conversation: every message with its day and time, who to whom and its type, `2026-10-31 14:01 lead → claude-1 [answer]`, then the text. The clipboard is the system's (`pbcopy`, `wl-copy`, `xclip`, `xsel`) or the terminal's over OSC 52, which works over ssh where the terminal allows it; the flash says which, `copied (pbcopy)`. The text is cleaned like everything drawn: no control characters, secret-looking tokens hidden.

### Folding

A long conversation reads faster with its messages on one row each. `h` (or `←`) folds the selected message, `l` (or `→`) opens it, `za` does one or the other; `zM` folds every message, `zR` opens them all (`:fold` and `:unfold` do the same with a conversation open). A folded message is one row, its head and its text on one line, the way the journal shows it:

```text
│  14:00 you → lead: Please split the parser work between two workers.      │
│                                                                           │
│▌ 14:00 lead → you: Plan I will split it in two: codex-1 takes… ▸ 6 lines   │
│                                                                           │
│  14:00 you → lead: ok                                                     │
```

A message longer than its row says so at the end: `…` where its text was cut, and `▸ 6 lines`, how many lines opening it shows. A message that fits whole on its row has no mark: nothing more is there. Notes on its words are counted after it, `✎ 2`. A message that comes while all are folded comes folded too; `zM` and `zR` also say how the conversations you open after them in this run start. While you choose lines (`v`) every message is open, so the cursor never steps over a hidden line; the folds are back when you leave.

### Copying lines

Your terminal's own selection still works (imece never takes the mouse), but it takes the window's border and gutter with the text, stops at the edge of what is shown, and cuts a wrapped line in two. `v` copies with the keyboard instead: a cursor opens on the first line of the selected message, and the lines between where you started and the cursor are chosen.

```text
┌ COPY · 3 chosen ───────────────────────────────────────────────────────────┐
│ 13:57 claude-1 → lead [ask]                                               │
│   Which parser? There are two: `manifest::read` and the legacy one.       │
│                                                                           │
│▌ 14:01 lead → claude-1 [answer]                                           │
│▌   `manifest::read`; the legacy one goes in the next card.                │
│▶   Leave the old tests as they are.                                       │
│                                                                           │
│ ↑↓ j k line  w b h l word  s start here  gg G first · last  y copy  c note  v esc leave
└───────────────────────────────────────────────────────────────────────────┘
```

| Key | What it does |
| --- | --- |
| `j` `k` / `↑` `↓` | the cursor a line down / up, across messages; the window keeps it in view and the title's count follows |
| `pgup` `pgdn`, `space`, `^d` `^u` `^f` `^b` | a page or half of one |
| `gg` `G` | the first / last line |
| `w` `b`, `l` `h`, `→` `←` | the cursor a word on / back, over the end of a line to the next one; the choice turns word by word |
| `s` | start the choice again where the cursor is |
| `y` | copy what is chosen and leave |
| `c` | write a note on what is chosen (below) |
| `v` `esc` | leave without copying; a second `esc` closes the window |

A line is a line as drawn before it wraps: a message's head, the `↩` line under a reply, and each line of its text. What is copied is the line as it was written, so a code block copies as its code without its fences (they are never drawn, so the cursor never lands on them), and a table copies as its Markdown rows. Lines of different messages come with a blank line between them. Other keys do nothing while you choose, so `[` or `r` is not taken by mistake; `?` and `q` still work. The copy goes the same way as `c`, cleaned the same.

### Choosing words

`v` chooses whole lines. The first word key turns the choice word by word, keeping the end it had: chosen downward, it starts at its first line's start and the cursor stands on its last line's last word; chosen upward, the other way round; a single line starts at its first word. A word is what stands between blanks as the screen draws it (`**beta**` is the word `beta`; the indent, a list's bullet, a code block's rule are no words). `w` steps a word on and past a line's last word to the next line's first, `b` a word back; `j` `k` keep the cursor on a word of the line they go to. The words chosen carry the selection background, the cursor's word is reversed, and a line chosen whole is chosen as a row.

To choose words in the middle of a line, move the cursor to the first one and press `s`: the choice starts again there, the cursor's word alone; then `w` takes more. For example `v` `j` `s` `w` `w` chooses the first three words of the message's first line.

`y` copies a line chosen whole as it was written, as above, and a line chosen in part as its words chosen, one blank between them.

### Notes on words

`c` while choosing writes a note on what is chosen. The command line opens with `note on "the words…" ▸`; `enter` keeps the note, `esc` drops it. The note shows under its message, dim, `✎ "the words" your note`, and the title counts the notes waiting, `✎ 2`. Choosing the same words again and `c` writes the note over (its text is on the line already); kept empty, it is taken away.

`n` in the conversation sends its notes: one message to each agent they are for, every note under the words it is about, quoted, and the number of the message they came from when it has one:

```text
Notes on words from our conversation, each under the words it is about.

On message 8:
> the legacy one goes in the next card
Which card? There is no card for it yet.
```

A note goes to the agent who wrote the message its words start in; on a message of yours, to the agent it went to. So in a conversation between the lead and a worker, a note on the lead's words goes to the lead and a note on the worker's to the worker. The message is yours like any other: it lands in your conversation with that agent, wakes it, costs what a message costs, and a message that could not go (the budget is spent) leaves its notes waiting. Notes live as long as imece runs, not between runs, and a conversation's notes wait while it is closed. The demo does all of it the same way; its agents get the message as they get any of yours.

### Export

`:export` with a conversation open writes it as Markdown: `# Conversation: lead ⇄ claude-1`, then each message as a heading with its day and time, who to whom and its kind, over its whole text. PATH is a file or a directory to name one in; none is the directory imece started from, the name `imece-conversation-20261031-1401.md`. A file that is there already is never written over. See [the journal](journal.md) for a card's export.

## Files and symbols in a message

While you write to an agent (or a `!cmd` line), the last word completes to a file of the project at hand: a word with a `/` in it opens the menu by itself (`src/au` → `src/auth.rs`), any other word on `tab` (`auth` → `src/auth.rs`, `tests/auth_test.rs`). The files whose name starts with the word come first, then the ones that have it anywhere in their path, the shorter path first; case and Turkish letters do not matter; thirty at most. `tab`, `↑` and `↓` step, `enter` puts the file in the word's place with a space after it, `enter` again sends. A file an agent holds on the code map shows its holder after it (`● claude-1`), and the symbols leased there come after the files as `file::name` (`Session.` → `src/Auth/Session.cs::Session.Start`; `auth.rs::` lists that file's). Not for a name (`@`), a command (`/`), the `:` `#` `&` lines, nor over several lines.

Live, the files are what git lists in the checkout: tracked, and untracked but not ignored, read at the project's opening, again after each change you apply, and every minute while one of its agents is mid-turn; a checkout of more than 20,000 files completes the first 20,000 and the journal says so once. The demo's files are its code map's. Live has no leased symbols yet (see [files](../reference/files.md)).

## The conversation follows

An open window keeps up with the talk: journal lines of its pair that arrive are appended. Your own line, written from the window with `@`, is selected, and so is a line that comes while the last one was selected; a selection further up stays where you are reading. Before this you had to close and reopen to see an answer.

## The trace

`enter` on an agent in the salon opens its focus (FOCUS · name: its card, status, origin, leases, how full its context window is when its vendor says, its own to-do list when it keeps one, and story). The to-do list is TO DO 3/7 and a step a line, `✓` done, `▸` in hand, `○` not begun, in the agent's order; a list longer than eight rows folds the done steps at its top into `✓ 4 done` and ends with `… 2 more`, so what is in hand stays in view; `enter` again, or `t` (`:trace`), opens its raw stream, TRACE · name. The trace is the agent's session as imece sees it, line by line, coloured by the line's first word: the agent's own text in its vendor colour, `tool_use` with the tool's name in the info colour, a `run:` by its exit code, `stopped` and `run refused` in the bad colour, the bus words in the accent. A tool call that runs over rows keeps them, indented, at most six, then `… +N`. `esc` goes back to the focus, then to the salon. `@` in a focus or a trace starts a message to that agent.

## See also

- [The journal](journal.md)
- [Messages and the board](../concepts/messages-and-board.md)
- [Message types](../reference/message-types.md)
- [The command line](command-line.md)
- [Keyboard](keyboard.md)
