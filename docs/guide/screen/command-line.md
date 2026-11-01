---
title: "The command line"
description: "The footer line where you write - to the leader or one agent, vendor commands, imece's own : commands and their menu, completion, readline keys, several lines, and your own shell."
---

# The command line

The footer is where you write. `/` (or `i`, as in vim) opens it to the leader of the selected project, `@` to one agent, `:` to imece, `!` to your own shell, `#` to a card. The hint that follows the line says what `enter` does and how to add a line; `esc` cancels.

```text
 › to lead: fix the parser crash on empty files█     enter send · ⇧enter new line · esc cancel
```

```text
 budget (spent $3.10 of $20.00)   board   close (shop-api)   code   commit   copy (shop-api)
 › command: :b█                                       tab next · enter take · esc hide
```

## Where a line goes

| Line | Goes to |
| --- | --- |
| plain text | the leader of the selected project, which splits the request into cards |
| `@name text` | one agent. A message to an idle worker opens a card for it; its answer closes it. `@` in a focus, a trace or a change fills the agent's name in |
| `/cmd`, `@name /cmd` | the agent's own command line, not a message: `/clear`, `/compact`, `/cost`, `/context`, whatever the vendor's CLI knows. It runs as a turn and its answer comes back like a reply. Codex's app server has no command line, so only `/compact` works there; `/clear` on a vendor without one of its own ends the session, and the next message starts a fresh one |
| `:command` | imece itself; see below |
| `!cmd` | your own shell |
| `#N`, `#word` | a card's screen; see [cards](cards.md) |

The label at the start says where it goes: `to lead:`, `to claude-1:`, `command:`, `shell:`, `card:`, or `note on file:line ▸` while a change's note is written. The agents' names are completed, so `@cl` `tab` is `@claude-1 `.

A message can run over lines; the first line of one that opens a card is the card's title. A spent pot refuses words and commands: nothing starts a turn until `:budget` raises it.

## `:` commands

`:` opens the line with `:` and the menu of every command. Each command has its key, when it has one, and does what the key does from wherever you are; movement keys have no name, since typing them would be slower than the key.

| Command | Key | What it does |
| --- | --- | --- |
| `:budget USD` (`:b`) | | raise or lower the pot while agents run; a turn under way finishes on its old cap |
| `:lead VENDOR` | | who leads from the next start on: `claude`, `codex`, `gemini`, or `ask` to be asked then |
| `:open PATH [NAME]` (`:o`) | | open the checkout at PATH as a project with its own leader |
| `:close NAME` (`:c`) | | close a project: its agents leave, their worktrees stay on disk |
| `:rename NAME` | | rename the project at hand; its leader becomes `lead-NAME` |
| `:forget @NAME` (`:f`) | | the agent forgets its session; a new one is told where it left off |
| `:export [PATH]` | | the open conversation, else the card's screen, as Markdown |
| `:commit` | | a cheap model suggests a commit message for what is staged, put on the line as `!git commit -m '…'` |
| `:copy PATH` | | copy a path git ignores (`.env`, `target`) into workers' worktrees at their start; again stops |
| `:pin [NAME]` | | pin a project to the top of the salon; again unpins |
| `:awake` | | keep the machine awake while an agent is mid-turn; again turns it off |
| `:notify [off\|bell\|text]` | | in another window, a bell or a desktop note when something comes to wait for you; bare, the next one round |
| `:salon` `:board` `:code` | `1` `2` `3` | the three views |
| `:lang [en\|tr]` | `L` | the other language, or the one named |
| `:theme [NAME\|FILE]` (`:t`) | `T` | the next built-in theme, or the one named |
| `:help` | `?` | the help |
| `:quit` (`:q`) | `q` | quit; it asks first, as `q` does |
| `:pending` | `q` `s` | before quitting: the agents mid-turn and the changes not reviewed |
| `:next` / `:defer` | `W` / `S` | open the next item waiting / put this one off |
| `:waiting` | `w` | the waiting panel |
| `:needs` | `f` | the salon shows only what needs you |
| `:fold` / `:unfold` | `zM` / `zR` | fold / unfold every project and every worker's helpers |
| `:trace [@NAME]` / `:talk [@NAME]` | `t` / `v` | the raw stream / the conversation of the named agent, else the one at hand |
| `:mark` | `m` | mark the line at hand |
| `:pause`, `:faster`, `:slower` | `space`, `+`, `-` | the demo only; live they say so and are left out of the menu |

The commands that work in a salon panel (`:waiting`, `:needs`, `:fold`, `:unfold`) go to that panel first. What each takes and answers is in the [commands reference](../reference/commands.md).

### The menu and its values

The menu shows each command's value after it, in brackets, drawn but never inserted: `budget (spent $3.10 of $20.00)`, `lead (claude)` or `(ask)`, `theme (dark)`, `lang (English)`, `close` and `rename` with the project at hand, `forget` `talk` `trace` with the agent at hand, `next` and `waiting` with how many items wait, `needs` and `pause` on or off, `faster` `slower` the speed, `pin` the project (with `▴` when pinned), `export` what it would write (`the conversation`, `#12`). `$` marks what raises the next bill: `:open` (a new leader session), `:forget` (the next message starts a new session that pays the whole briefing again) and `:commit` (it asks a model for the message). No other `:` command starts a paid turn by itself.

### Short names

`b` `c` `f` `o` `q` `t` stand for `budget` `close` `forget` `open` `quit` `theme`. A short name typed whole is a command already, so the menu offers nothing and `enter` runs it: `:q` `enter` asks at once, `:b 80` raises the budget.

### Theme preview

While `:theme NAME` is written with NAME a built-in theme, typed or stepped to with `tab`, the screen wears it; `enter` keeps it, `esc` gives it up, and the theme in use comes back when the line no longer names one. See [themes and language](themes-and-language.md).

## Completion

Writing completes as a shell does: after `@` the agents' names (the project at hand first, gone ones left out, prefix match, case aside), after `:` imece's commands and after `:lead` the vendors, after `/` or `@name /` that agent's own command line as its session reported it (never a static list, so a vendor's new command needs no release; empty until a session says, and the journal says when a session listed none so only `/clear` completes), after `#` the cards, and directories after `:open` and `:export`; `:pin` and `:close` complete the open projects' names. The matches show in one row above the footer as you type; one match equal to the word is no menu.

| Key | What it does |
| --- | --- |
| `tab` | put the first match in, then step on |
| `⇧tab`, `↑` `↓`, `^n` `^p` | step through the matches, the text following (zsh menu style) |
| `enter` | take the highlighted match when it is not in yet, else send |
| `esc` | hide the menu; again, cancel the writing |

The menu is offered only with the cursor at the end, since a completion replaces the word the line ends with; moving off the end hides it, `tab` there does nothing, coming back offers it again. Nothing completes after `!`.

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

There is one cut, not a ring: each cut that takes something replaces it. The character under the cursor is drawn reversed, the block `█` stands for the end. On macOS the alt keys need the terminal to send Option as Meta (iTerm2: Profiles → Keys → Left Option key: Esc+; Terminal.app: Use Option as Meta key); without it Option-b types `∫`. Not here: kill-ring cycling, undo, transposing, vim's normal mode.

## Several lines

`⇧enter` starts a new line, as in Claude Code, where the terminal can tell it from `enter`: it speaks the kitty keyboard protocol (kitty, Ghostty, WezTerm with `enable_kitty_keyboard`, foot, Alacritty, iTerm2), which imece asks for at start. Everywhere, `\` then `enter` turns the `\` into a new line instead of sending, and `⌥enter` does too where Option is Meta. The `?` help and the footer's hint name the one your terminal has: `⇧enter new line` or `\ enter new line`. A multiplexer (tmux, screen) that does not answer the query gets the fallback.

The footer takes one row per line, at most eight and a third of the screen; the first row carries who the text goes to, the others are indented under it. When not all lines fit, the cursor's line stays in view and the hint starts with where it is, `3/12`. `^a` `^e`, home/end and `^u` `^k` keep to the line the cursor is on; `↑` `↓` go between lines, keeping the column (while the menu is shown they step through it instead); backspace at a line's start joins it to the one above. `@name` ends at a space or a new line, so `@name` `⇧enter` text goes to that agent.

A paste goes into the line whole (bracketed paste): line ends become new lines, tabs are kept, other control characters are left out, and nothing is sent by it. Before this a paste's first line end sent half a request. A paste while not writing is left out, and the footer says to start writing first. The journal and the salon show a message of several lines on one line; the conversation shows its lines.

## Your own shell

`!cmd` runs `cmd` in your own shell (`$SHELL`, else `/bin/sh`) in the checkout, as `:!` does in vim: the screen steps aside and the command runs on a terminal of its own (a pty, as `script` does), which imece passes your keys to and copies to the screen as it comes. That terminal starts set like yours and follows your window's size, so `git log`, `less` and `vim` work, and ctrl-c stops the command, not imece; ctrl-z stops nothing, since a stopped command would hold the screen. A line says how it ended and how long it took (`[imece] exit 0 · 3 s · any key: back to the screen`), and any key brings the screen back. The journal keeps the command, its exit code (or the signal that stopped it) and its time: `` you ran `cargo test` → exit 0 · 12 s ``. The output stays on the terminal's main screen, where you see it again when imece quits. imece keeps the end of it (the last 64 KiB) in memory for `O`, never on disk, and it reaches an agent only when you send it. The terminal's title is written again after.

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

## See also

- [Commands reference](../reference/commands.md)
- [Conversations](conversations.md)
- [Themes and language](themes-and-language.md)
- [Keyboard](keyboard.md)
- [Budget and limits](../concepts/budget-and-limits.md)
