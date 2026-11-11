---
title: "The command line"
description: "The footer line where you write - to the leader or one agent, Claude Code's commands, imece's own : commands and their menu, completion, readline keys, several lines, your own shell and your editor."
---

# The command line

The footer is where you write. `/` (or `i`, as in vim) opens it to the leader of the selected project with your conversation with it above the line ([your conversation with an agent](conversations.md#your-conversation-with-an-agent)), `@` to one agent, `:` to imece, `!` to your own shell, `#` to a card. Until you type, a hint says what `enter` does and how to add a line, and that `esc` cancels; it goes away with the first character and comes back when the line is empty again.

```text
 › to lead:    enter send · ⇧enter new line · esc cancel
```

```text
 allow  archive  archives  awake (off)  away (30m)  board  +65 more ›
 › command: :   tab next · enter take · esc hide · 1 of 71
```

```text
 budget ($3.10 of $20.00 · $2.40/h, ~6 h 25 min left)
 › command: :bu   tab next · enter take · esc hide
```

## Where a line goes

| Line | Goes to |
| --- | --- |
| plain text | the leader of the selected project, which splits the request into cards; in a conversation of yours with an agent, that agent, and the line stays open after `enter` |
| `@name text` | one agent. A message to an idle worker opens no card until its work comes to a change; then the card opens, titled by the message. `@` in a focus, a trace or a change fills the agent's name in. Once the line names an agent, your conversation with it shows above the line until the line is sent or dropped (not over a change) |
| `/cmd`, `@name /cmd` | the agent's own command line, not a message: `/clear`, `/compact`, `/cost`, `/context`, whatever Claude Code knows. It runs as a turn and its answer comes back like a reply |
| `:command` | imece itself; see below |
| `!cmd` | your own shell |
| `#N`, `#word` | a card's screen; see [cards](cards.md) |

The label at the start says where it goes: `to lead:`, `to claude-1:`, `command:`, `shell:`, `card:`, or `note on file:line ▸` while a change's note is written. The agents' names are completed, so `@cl` `tab` is `@claude-1 `.

A message can run over lines; the first line of one whose work opens a card is the card's title. A spent pot refuses words and commands: nothing starts a turn until `:budget` raises it.

## `:` commands

`:` opens the line with `:` and the menu of every command. Each command has its key, when it has one, and does what the key does from wherever you are; movement keys have no name, since typing them would be slower than the key.

| Command | Key | What it does |
| --- | --- | --- |
| `:budget USD` (`:b`) | | raise or lower the pot while agents run; a turn under way finishes on its old cap |
| `:open PATH [NAME]` (`:o`) | | open the checkout at PATH as a project with its own leader |
| `:close NAME` (`:c`) | | close a project: its agents leave, their worktrees stay on disk |
| `:rename NAME` | | rename the project at hand; its leader becomes `lead-NAME` |
| `:forget @NAME` (`:f`) | | the agent forgets its session; a new one is told where it left off |
| `:export [PATH]` | | the open conversation, else the card's screen, as Markdown |
| `:commit` | | a cheap model suggests a commit message for what is staged, put on the line as `!git commit -m '…'` |
| `:copy PATH` | | copy a path git ignores (`.env`, `target`) into workers' worktrees at their start; again stops |
| `:pin [NAME]` | | pin a project to the top of the list; again unpins |
| `:awake` | | keep the machine awake while an agent is mid-turn; again turns it off |
| `:notify [off\|bell\|text\|push]` | | in another window, a bell or a desktop note when something comes to wait for you; `push`: to your phone through ntfy when you are away; bare, the next one round |
| `:motion [off\|calm\|full]` | | what moves on the screen: a working row breathes, what comes to wait blinks; full adds spinners; bare, the next one round |
| `:home` `:salon` | | Home · Home's AT WORK |
| `:board` `:code` `:timeline` | | windows over the screen; `esc` closes them |
| `:lang [en\|tr]` | `L` | the other language, or the one named |
| `:theme [NAME\|FILE]` (`:t`) | `T` | the next built-in theme, or the one named |
| `:help` | `?` | the help |
| `:quit` (`:q`) | `q` | quit; it asks first, as `q` does |
| `:pending` | `q` `s` | before quitting: the agents mid-turn and the changes not reviewed |
| `:next` / `:defer` | `W` / `S` | open the next item waiting / put this one off |
| `:waiting` | | Home, its first row to decide |
| `:settle` / `:unsettle` | `s` / — | hide the selected row until it needs you again / bring everything hidden back |
| `:fold` / `:unfold` | `zM` / `zR` | fold / unfold every project on Home; in a conversation its messages, in a change its files |
| `:trace [@NAME]` / `:talk [@NAME]` | `t` / `v` | the raw stream / the conversation of the named agent, else the one at hand |
| `:peek [@NAME]` | `K` | the focus of the named agent, else the one at hand, in a window over the screen |
| `:journal` | `J` | the journal in a window |
| `:mark` | `m` | mark the line at hand |
| `:pause`, `:faster`, `:slower` | `space`, `+`, `-` | the demo only; live they say so and are left out of the menu |

The commands that work on Home's list (`:waiting`, `:fold`, `:unfold`, `:settle`, `:unsettle`) go to Home first. What each takes and answers is in the [commands reference](../reference/commands.md).

### The menu and its values

The menu shows each command's value after it, in brackets, drawn but never inserted: `budget ($3.10 of $20.00 · $2.40/h, ~6 h 25 min left)` (the pot, its rate, how long it lasts, and a usage window when there is one), `theme (dark)`, `lang (English)`, `close` and `rename` with the project at hand, `forget` `talk` `trace` with the agent at hand, `next` and `waiting` with how many items wait (`4 to decide`), `story` `review` `awake` `pause` on or off, `faster` `slower` the speed, `pin` the project (with `▴` when pinned), `export` what it would write (`the conversation`, `#12`). `$` marks what raises the next bill: `:open` and `:solo` (a new session), `:fork` (a new worker), `:forget` (the next message starts a new session that pays the whole briefing again), `:commit` and `:review` (each asks a model). No other `:` command starts a paid turn by itself.

### Short names

`b` `c` `f` `o` `q` `t` stand for `budget` `close` `forget` `open` `quit` `theme`. A short name typed whole is a command already, so the menu offers nothing and `enter` runs it: `:q` `enter` asks at once, `:b 80` raises the budget. So is a command's whole name: `:news` `enter` opens the news, and `:archive` `enter` runs `:archive` though the menu offers `archives` too (moved to it, `enter` takes it, as for a word only begun).

### Theme preview

While `:theme NAME` is written with NAME a built-in theme, typed or stepped to with `tab`, the screen wears it; `enter` keeps it, `esc` gives it up, and the theme in use comes back when the line no longer names one. See [themes and language](themes-and-language.md).

## Completion

Writing completes as a shell does: after `@` the agents' names (the project at hand first, gone ones left out, prefix match, case aside), after `:` imece's commands, after `/` or `@name /` that agent's own command line as its session reported it (never a static list, so a new command of Claude Code's needs no release; empty until a session says, and the journal says when a session listed none so only `/clear` completes), after `#` the cards, and directories after `:open` and `:export`; `:pin` and `:close` complete the open projects' names. The matches show in one row above the footer as you type; one match equal to the word is no menu.

| Key | What it does |
| --- | --- |
| `tab` | put the first match in, then step on |
| `⇧tab`, `↑` `↓`, `^n` `^p` | step through the matches, the text following (zsh menu style) |
| `enter` | take the highlighted match when it is not in yet, else send |
| `esc` | hide the menu; again, cancel the writing |

This menu is offered only on a single line with the cursor at the end, since a completion replaces the word the line ends with; moving off the end hides it, coming back offers it again. The word list below works anywhere in the text.

### Words and files

Any other word of a message or a `!cmd` line has a list of its own, as an IDE's: from the second letter it opens by itself, above the line at the word's column, drawn over the screen so nothing moves. It holds, in this order, the words said in the conversation at hand, then the rest said in the project, then the lines you sent (newest first), then the names in the project's code (the ones used most first), then the project's files whose name starts with the word; a word with a `/` lists the files by any part of their path, and so does a word with a `.` that a file's path holds (`shelf.p`). Each row says where it comes from (`said`, `code`, `file`, or who holds a leased file). The names are read from the files git lists in your checkout, on the side while you work: plain files inside it only (never through a link), not lock files, generated or minified code, prose or binaries, at most 1 MiB a file and 8 MiB in all, the 10,000 used most. Matching folds case and the Turkish letters; a word has four letters at least, the word you typed is never offered, and a word that looks like a secret never enters the list. No model is asked: it is local and free.

```text
                   retry_policy          said
                   retry_after_ms        said
                   src/net/retry.rs      file
 › to lead: keep the old retr   tab take · ↓ choose · esc close
```

| Key | What it does |
| --- | --- |
| `tab` | take the word at hand; nothing goes after it, so a suffix may follow (`retry_policy'yi`). With no list open, `tab` opens one from the first letter (the files by any part of their path when no word starts so) and takes a word that is the only one |
| `↓` `↑`, `^n` `^p`, `⇧tab` | move in the list; nothing goes in until it is taken |
| `enter` | sends, unless you moved in the list: then it takes the word at hand |
| `esc` | close the list; it stays closed on that word, the next word has one again. Again: cancel the writing |

While no list is open, `↑` `↓` recall the lines sent as before. Six rows show at most, `+N` counts the rest; the list is 24 to 48 cells wide, a long name cut with `…`, and slides left at the screen's edge.

The list works on any line of a text of several lines and with the cursor mid-line: the word is the one that ends at the cursor, and what follows it stays where it is (`fix the retr| later` → `fix the retry_policy later`, the cursor after the word). It stands above the whole text, so it never covers a line you wrote. A cursor inside a word has no list, but a space, a full stop or an apostrophe after it end the word, so a suffix you wrote already stays (`retry_pol|'yi` → `retry_policy'yi`). Moving the cursor alone opens no list; writing does, and so does `tab` where the cursor stands.

## Readline keys

The line edits as a shell's does, readline's emacs set:

| Key | What it does |
| --- | --- |
| `^a` / `^e` (home / end) | to the start / end of the line the cursor is on |
| `←` `→`, `^b` `^f` | a character back / on |
| `⌥b` / `⌥f` (or alt or ctrl with `←` `→`) | a word back / on; a word is letters and digits |
| `^w` | cut back to the space before (anything up to a space, as in bash) |
| `⌥backspace` / `⌥d` | cut a word back / on |
| `^u` / `^k` | cut what is before / after the cursor on this line |
| `^y` | put the last cut back at the cursor |
| `^d`, delete | the character under the cursor; `^d` does not leave an empty line (`esc` does) |
| `^x` `^e` | the line in your editor ([below](#your-editor)) |

There is one cut, not a ring: each cut that takes something replaces it. The cursor is your terminal's own, standing where the next character goes, and its shape says what you are writing: a blinking bar for a message or a `!` command, a steady bar in the theme's accent for a `:` command, a blinking underline for words that find something (`#`, `&`, `^r`, and the `/` filter of the help and the archives). The terminal blinks it, so it costs imece no drawing; a terminal that does not change cursor shapes keeps its own, and when imece hands the terminal back (at the end or to a `!` command) the cursor is as your terminal's settings have it. Each frame reaches the terminal as one synchronized update, so a terminal that supports it never shows half of one. On macOS the alt keys need the terminal to send Option as Meta (iTerm2: Profiles → Keys → Left Option key: Esc+; Terminal.app: Use Option as Meta key); without it Option-b types `∫`. Not here: kill-ring cycling, undo, transposing, vim's normal mode.

## Several lines

`⇧enter` starts a new line, as in Claude Code, where the terminal can tell it from `enter`: it speaks the kitty keyboard protocol (kitty, Ghostty, WezTerm with `enable_kitty_keyboard`, foot, Alacritty, iTerm2), which imece asks for at start. Everywhere, `\` then `enter` turns the `\` into a new line instead of sending, and `⌥enter` does too where Option is Meta. The `?` help and the footer's hint name the one your terminal has: `⇧enter new line` or `\ enter new line`. A multiplexer (tmux, screen) that does not answer the query gets the fallback.

The footer takes one row per row of text, at most eight and a third of the screen: a line too long for the screen goes on in the next row (cut cell by cell; a wide character that does not fit moves down whole), so what you type is always in sight. The first row carries who the text goes to, the others are indented under it. When not all rows fit, the cursor's row stays in view and the last row says where it is, `3/12`. `^a` `^e`, home/end and `^u` `^k` keep to the line the cursor is on; `↑` `↓` go between lines, keeping the column (while the menu is shown they step through it instead); backspace at a line's start joins it to the one above. `@name` ends at a space or a new line, so `@name` `⇧enter` text goes to that agent.

A paste goes into the line whole (bracketed paste): line ends become new lines, tabs are kept, other control characters are left out, and nothing is sent by it. Before this a paste's first line end sent half a request. A paste while not writing is left out, and the footer says to start writing first. The journal and Home show a message of several lines on one line; the conversation shows its lines.

## Your own shell

While the line starts with `!`, its label ` › shell: ` sits on the theme's shell colour (pink in the terminal's theme, reversed in mono), so a command for your shell does not pass for a message or a `:` command; with the `!` gone the label is plain again. `!cmd` runs `cmd` in your own shell (`$SHELL`, else `/bin/sh`) in the checkout, as `:!` does in vim: the screen steps aside and the command runs on a terminal of its own (a pty, as `script` does), which imece passes your keys to and copies to the screen as it comes. That terminal starts set like yours and follows your window's size, so `git log`, `less` and `vim` work, and ctrl-c stops the command, not imece; ctrl-z stops nothing, since a stopped command would hold the screen. A line says how it ended and how long it took (`[imece] exit 0 · 3 s · any key: back to the screen`), and any key brings the screen back. The journal keeps the command, its exit code (or the signal that stopped it) and its time: `` you ran `cargo test` → exit 0 · 12 s ``. The output stays on the terminal's main screen, where you see it again when imece quits. imece keeps the end of it (the last 64 KiB) in memory for `O`, never on disk, and it reaches an agent only when you send it. The terminal's title is written again after.

`O` (or `:output`) quotes the last `!cmd` of the project at hand in a message. The command line opens to the agent at hand (else the project's head) with how the command ran and the last 40 lines it showed under the line you write on. Colours and what a progress bar wrote over are gone and secret-looking words are hidden; the lines sit in a `text` fence longer than any run of backticks in them:

````text
@lead 
I ran `cargo test` → exit 101 · 12 s
```text
… lines above: 214
test parse::tests::empty ... FAILED
test result: FAILED. 41 passed; 1 failed
```
````

Nothing is sent until you press `enter`; `esc` drops it. Each project keeps only its newest output, and a command that could not start leaves the last one as it was. No command has run in the project yet: `no !command has run in this project yet`.

`:commit` puts `!git commit -m '…'` on the line for you to edit and run this way. See [own shell and commit](../scenarios/own-shell-and-commit.md).

## Your editor

`^x` `^e` (as in bash and zsh) or `:edit` opens the line you are writing in your editor: `$VISUAL`, else `$EDITOR`, else `vi`; a line such as `code --wait` or `emacsclient -t` works too, as your shell would read it. The screen steps aside as for a `!` command and the editor runs in the project's checkout, on your terminal. When you save and quit, the text comes back on the command line, the cursor at its end, the line ends at its end taken off; nothing is sent: `enter` sends it, or you change it first. After `^x`, a key other than `^e` does what it does alone. The draft is a file of its own, made new, under imece's state, that only you may read (0600; `.md`, so the editor colours it as Markdown), removed when the editor closes, whatever happened. An editor that exits with an error (in vim, `:cq` is the way to give up), is stopped, or leaves more than 1 MiB or what is not UTF-8 text leaves the draft as it was; a line under what it left on the terminal (its own complaint among it) says why, and any key brings the screen back. While the editor is open, ctrl-c is the editor's, not imece's. `:edit TEXT` opens the editor on `TEXT`. See [commands](../reference/commands.md).

## See also

- [Commands reference](../reference/commands.md)
- [Conversations](conversations.md)
- [Themes and language](themes-and-language.md)
- [Keyboard](keyboard.md)
- [Budget and limits](../concepts/budget-and-limits.md)
