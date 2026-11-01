---
title: "Keyboard"
description: "Every key imece knows, grouped by where it works, with its : name - the same list the ? help shows, and how to filter that help."
---

# Keyboard

imece is keyboard first: everything is done with keys, vim style, next to the arrows, and a mouse is never needed or captured, so your terminal's own selection keeps working. This page is the complete list, the same one `?` shows on the screen in both languages. Every key that acts also has a `:` name, so it can be typed from anywhere; movement keys and the keys that act on the selected row have none.

The conventions are vim's: `h` `j` `k` `l` move and fold, `gg` and `G` go to the ends, `za` `zM` `zR` fold, `^d` `^u` `^f` `^b` page, `/` and `i` start writing, `esc` goes back, `q` quits (after asking), `m` and `'` mark and go to a mark, `%` jumps to the matching message, `^o` goes back through the places you left, `[` `]` go section to section. `^a` means ctrl-a, `⌥b` alt-b (Option on a Mac, sent as Meta), `⇧tab` shift-tab.

## The help window

`?` (`:help`) opens HELP: every key with what it does, in two columns on a wide screen, then the legend of glyphs, then the new-line key your terminal has. Any key closes it, except `/`: that starts a filter, typed on the window's last line. Every key typed goes into it (`?`, `q` and `/` too), so the help stays open while you type; backspace takes a letter back, `^u` empties it. A key line or a legend line stays when it has every word of the filter, in either column: `zM` finds the fold keys, `first last` finds `gg` `G`. Case and the Turkish letters fold to plain Latin (`calis` finds `çalış`, `BÜTÇE` finds `bütçe`). Nothing matching says so. `enter` keeps the filter and ends the typing (then `/` changes it, any other key closes); `esc` drops it and the whole help is back; backspace on an empty filter leaves it too. The window keeps its size while filtered, so it does not jump.

## Writing

| Key | What it does |
| --- | --- |
| `/`  `i` | write to the lead of the selected project |
| `@name text` | write to one agent: `@name` is the agent, `text` the message |
| `/cmd`  `@name /cmd` | the agent's own command line (`/clear`, `/compact`, `/cost` …) |
| `:` | a command to imece; `tab` opens a menu with the commands' values |
| `$` | in the `:` menu: this command raises the next bill |
| `:budget USD` | raise or lower the budget while agents run; `USD` is dollars like `80` or `2.50` |
| `:lead VENDOR` | who leads from the next start on; `VENDOR` is `claude`, `codex`, `gemini` or `ask` |
| `:open PATH` | open the checkout at `PATH` as a project; a `NAME` after it names it |
| `:solo CHOICE` | a solo session you drive yourself, no leader: `CHOICE` is `vendor[:model][@effort]`; a `PATH` and a `NAME` may come too; without a choice, a screen asks |
| `:effort LEVEL` | the solo at hand thinks at `LEVEL` from its next turn (`default`: the vendor's own) |
| `:model NAME` | the solo at hand runs on `NAME` from its next turn (`default`: the vendor's own) |
| `:close NAME` | close the project `NAME`: its agents leave, worktrees stay |
| `:rename NAME` | rename the project at hand; `lead-NAME` leads it |
| `:forget @NAME` | the agent `@NAME` forgets its session; a new one is told where it left off |
| `:export PATH` | the open conversation or card, as Markdown, to `PATH` (a file or a directory) |
| `:commit` | a cheap model suggests a commit message for what is staged (`$`) |
| `:copy PATH` | copy a path git ignores (`.env`) into worktrees; again: stop |
| `:pin NAME` | pin a project to the top of the salon; again: unpin |
| `!cmd` | run `cmd` in your own shell, screen aside |
| `O`  `:output` | quote the last `!cmd` of the project at hand (how it ran, its last 40 lines) in a message to the agent at hand, else the project's head |
| `tab` `⇧tab` `↑` `↓` | while writing: complete names and commands (`^n` `^p` step too); `tab` also completes any word of a message or a `!cmd` line to a file of the project (a word with a `/` opens the menu by itself), or to a symbol leased on the code map as `file::name` |
| `↑` `↓` | writing, at the text's first / last line: the lines sent before, newest first; past the newest, what you were writing comes back |
| `^r`  `:history` | search the lines sent: every word typed must be in the line, `enter` puts the one highlighted on the line, `tab` steps |
| `^a` `^e` `⌥b` `⌥f` | writing: to the start / end, a word back / on |
| `^w` `^u` `^k` `^y` | writing: cut a word / before / after the cursor; paste the cut back |
| `⇧enter` `⌥enter` | writing: a new line; `↑` `↓` between lines (where the terminal speaks the kitty keyboard protocol) |
| `\enter` `⌥enter` | writing: a new line (here `⇧enter` is `enter`); `\` then `enter` works in every terminal |
| `enter` | send; `esc` cancels |

## Moving

| Key | What it does |
| --- | --- |
| `#N`  `#word`  `n` `p` | a card by number or title, all of it on one screen; on it `n` `p`: next / previous card |
| `&`  `:goto` | go to an agent or a project: type a name, `1`-`9` the nth on the list, `!` keeps only what needs you; `&#` turns to the cards |
| `1` `2` `3` | `:salon` · `:board` · `:code` map |
| `↑` `↓`  `j` `k` | select |
| `gg`  `G` | first / last line |
| `^d` `^u` `^f` `^b` | half a page / a page |
| `^o` | back where you were: view, panel, focus, card (vim's jump list, fifty places; an open conversation closes first) |
| `enter` | look deeper: focus, then the raw stream |
| `K`  `:peek` | a peek: the focus of the agent at hand in a window over the view, which stays as it was; in it `j` `k` the next / previous agent of the salon, `enter` its whole focus, `@` write to it, `esc` or `K` close |
| `F`  `:hints` | hint letters: a label of one or two letters on every row of the salon's screen you can select (project headers, agents, waiting items, journal lines, the people on a card's screen); typing a label selects its row in its panel, its last letter in capitals opens it too (as `enter`); `backspace` takes a letter back, `esc` closes |
| `t`  `:trace` | the raw stream of the agent at hand |
| `v`  `:talk` | its conversation with you |
| `esc` | go back |
| `tab` `⇧tab` `[` `]` | next / previous panel |
| `,`  `.`  `<`  `>` | wider / narrower side panel |

## The salon

| Key | What it does |
| --- | --- |
| `←` `→`  `h` `l` | fold / unfold a project or a worker's helpers (in a conversation: a message) |
| `za`  `zM`  `zR` | fold: toggle · all (`:fold`) · none (`:unfold`); in a conversation its messages |
| `f`  `:needs` | salon: only what needs you · everyone |
| `s`  `:settle` | settle the selected worker's row, or the project (on its header or its leader), out of the salon until it needs you again; `:unsettle` brings everything back |

## WAITING FOR YOU

| Key | What it does |
| --- | --- |
| `w`  `:waiting` | straight to waiting for you |
| `W`  `S` | `:next` waiting, from anywhere · `:defer` this one and open the next |
| `a`  `r` | approve / apply / allow · reject / refuse (in waiting for you) |
| `enter` | open the item: a change's diff, a question's conversation, a reply (read by it) |

## A conversation

| Key | What it does |
| --- | --- |
| `enter`  `%` | the conversation (from a journal line) · `%` in it: reply ↔ question |
| `[`  `]` | turn to turn (the lines one side says before the other answers) |
| `c` `C`  `y` `Y` | copy the message · the conversation |
| `h` `l`  `←` `→` | fold the selected message to one row · open it |
| `za`  `zM`  `zR` | fold: the message toggled · all (`:fold`) · none (`:unfold`) |
| `v` | choose lines: `j` `k` and the page keys move, `y` copies, `v` `esc` leave |
| `v`  `w b`  `s` | while choosing: `w` `b` (`l` `h`, `→` `←`) a word on / back, the choice turns word by word · `s` starts it again where the cursor is |
| `c`  `n` | while choosing: a note on the words chosen · `n` sends the conversation's notes to their agents |
| `m '`  `:mark` | mark the line at hand to come back to (again: unmark) · `'` the next marked line: conversation, else journal |
| `space`, `pgup` `pgdn` | a page |
| `esc` | close |

## A change

| Key | What it does |
| --- | --- |
| `[`  `]` | in a change: previous / next file · in a conversation: turn |
| `>` | in a change: the hunk at the top, quoted in a message to its worker |
| `c`  `n` | in a change: a note on the top line · send the notes, it goes back (in a conversation: on the words chosen, above) |
| `x` | in a change: choose the hunk at the top; `a` applies only the chosen |
| `w` | in a change: wrap long lines · cut them again |
| `v` | in a change: choose lines to copy; `y` as written, `Y` the new side |
| `a`  `r` | apply (the chosen hunks, else all) · reject |
| `j` `k`, `gg` `G`, `^d` `^u` `^f` `^b`, `space` | move |

## The card screen

| Key | What it does |
| --- | --- |
| `#N`  `#word` | open a card |
| `n`  `p` | next / previous card |
| `esc` | back to the salon |
| `tab`, `enter`, `a` `r`, `c`, `t`, `v`, `m`, `W` | as on the main screen |

## Demo only

| Key | What it does |
| --- | --- |
| `space` | `:pause` / resume the simulation |
| `+`  `-` | `:faster` / `:slower` simulation (`=` is `+` too) |

## Everywhere

| Key | What it does |
| --- | --- |
| `L`  `:lang` | Türkçe / English; `:lang en\|tr` |
| `T`  `:theme` | next theme · `:theme NAME` or `FILE` |
| `:awake` | keep the machine awake mid-turn; again: off |
| `:notify` | in another window, a word when something comes to wait: `:notify off\|bell\|text`; bare, the next one round |
| `q` `q`  `:quit` | quit (`q` or `:quit` asks first; `^c` quits at once) |
| `q` `s`  `:pending` | before quitting: mid-turn, not reviewed |
| `?`  `:help` | this help; `/` in it filters the keys · `:keys` reads your key file again |
| `W` | the next item waiting, from anywhere |
| `^o` | back where you were |

## The legend

| Glyph | Meaning |
| --- | --- |
| `◆` lead  `◌` starting  `●` working  `◐` asking  `✕` blocked  `!` waiting for you | an agent's state |
| `◇` researcher  `○` idle  `✓` done, not seen  `■` stopped  `⇄` conversation  `▾` `▸` project / helpers / a folded message  `▴` pinned | |
| `~` soft-tainted (read a report)  `≈` tainted (reads the outside world) | demo only: researchers are not built live |
| `⎇` branch  `↑` own commits  `↓` the checkout's new commits  `+N −M` lines not yet handed over | a worker's worktree |
| `↗` a local address the worker wrote (localhost), a link the terminal opens | |
| `✱` a line you marked (`m`), kept between runs; `'` goes to the next | |

## Every `:` command and its key

| Command | Key |
| --- | --- |
| `:salon` `:board` `:code` | `1` `2` `3` |
| `:lang` | `L` |
| `:theme` | `T` |
| `:help` | `?` |
| `:quit` (`:q`) | `q` `q` |
| `:pending` | `q` `s` |
| `:next` / `:defer` | `W` / `S` |
| `:waiting` | `w` |
| `:needs` | `f` |
| `:settle` | `s` |
| `:peek` | `K` |
| `:hints` | `F` |
| `:fold` / `:unfold` | `zM` / `zR` |
| `:trace` / `:talk` | `t` / `v` |
| `:mark` | `m` |
| `:pause`, `:faster`, `:slower` | `space`, `+`, `-` (demo) |
| `:budget` (`:b`), `:lead`, `:open` (`:o`), `:close` (`:c`), `:rename`, `:solo`, `:effort`, `:model`, `:forget` (`:f`), `:export`, `:commit`, `:copy`, `:pin`, `:awake`, `:notify`, `:unsettle`, `:keys` | no key: typed after `:` |

The full list of what each command takes is in the [commands reference](../reference/commands.md).

## Your own keys

A key file gives you keys of your own: `~/.config/imece/keys` (or `$XDG_CONFIG_HOME/imece/keys`). Each line makes a key act as one of imece's, `KEY = KEY`; a line starting with `#` is a comment:

```text
# j and k the other way round, and ctrl-n / ctrl-p to move too
j = k
k = j
^n = j
^p = k
J = G
```

A key is written as the help writes it (`j`, `G`, `^n`, `⌥b`, `⇧tab`, `enter`, `esc`, `tab`, `space`, `↑ ↓ ← →`, `home`, `end`, `pgup`, `pgdn`) or in words (`ctrl-n`, `alt-b`, `shift-tab`, `up`, `escape`, `pageup`); `#` is `hash` on the left of the `=`, since a line starting with `#` is a comment. A line is applied once, as vim's `noremap`: `j = k` and `k = j` swap the two, and a key the file does not name stays itself. The file's keys work wherever imece reads keys (the salon, a conversation, a change, peek, the windows, the pickers, the lead question at start), except where keys are text: writing, the help's `/` filter and the hint letters. `^c` always quits and cannot be given.

A side of a line can be two keys, one after the other:

```text
# gt goes to the last line, x to the first, ctrl-t folds everything
gt = G
x = gg
^t = zM
g enter = ^o
```

Two plain keys go in one word (`gt`, `zM`); a word that is a key's name is that key (`f1`, `up`), so write `f 1` or `u p` for two. Keys with ctrl, alt or a name are written apart (`g enter`, `^x j`). The first key of two waits for the second, as in vim, for a second at most; another key, or none in time, lets it through as itself, so `g` then `g` is still imece's `gg`. A key given alone cannot also be the first of two (`g is given alone and as the first of two keys (line 2)`), and a side has two keys at most. The keys that come out are not turned again; the second goes where the first left the screen (`y = :q` opens the command line and writes `q`).

imece reads the file at start, on `:keys`, and again when you save it: it looks every two seconds, and at once when you come back to its window, and says what came of it as `:keys` does. A file it cannot take whole is refused, and the keys you had stay: a line that is not `KEY = KEY` (`line 3: write it as KEY = KEY`), a name that is no key (`` `nope` is not a key ``), a key imece does nothing with on the right (`imece does nothing with d`), a key given twice (`^n is given on line 1 already`), three keys on a side, `^c` on either side, more than 200 keys, a file over 64 KiB or not text. The last good file is kept in the store, so a start with a file refused still has your keys and says why. A bad edit is said once and leaves the keys you had. With no file, imece's own keys are back.

In the `?` help, your keys come first (`^n  your key: as j`), and every key line shows the keys that do it now: with the file above, `select` reads `↑ ↓  k/^n j/^p`; a key whose meaning went to no other key shows `–`; with the lines of two above, `first / last line` reads `x/gg  gt/G`. The writing keys are shown as they are, since the file does not turn them.

## See also

- [The command line](command-line.md)
- [The salon](salon.md)
- [WAITING FOR YOU](waiting.md)
- [The change view](changes.md)
- [Commands reference](../reference/commands.md)
