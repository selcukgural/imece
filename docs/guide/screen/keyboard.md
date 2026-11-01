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
| `tab` `⇧tab` `↑` `↓` | while writing: complete names and commands (`^n` `^p` step too) |
| `^a` `^e` `⌥b` `⌥f` | writing: to the start / end, a word back / on |
| `^w` `^u` `^k` `^y` | writing: cut a word / before / after the cursor; paste the cut back |
| `⇧enter` `⌥enter` | writing: a new line; `↑` `↓` between lines (where the terminal speaks the kitty keyboard protocol) |
| `\enter` `⌥enter` | writing: a new line (here `⇧enter` is `enter`); `\` then `enter` works in every terminal |
| `enter` | send; `esc` cancels |

## Moving

| Key | What it does |
| --- | --- |
| `#N`  `#word` | a card by number or title: all of it on one screen |
| `n`  `p` | on a card's screen: next / previous card |
| `1` `2` `3` | `:salon` · `:board` · `:code` map |
| `↑` `↓`  `j` `k` | select |
| `gg`  `G` | first / last line |
| `^d` `^u` `^f` `^b` | half a page / a page |
| `^o` | back where you were: view, panel, focus, card (vim's jump list, fifty places; an open conversation closes first) |
| `enter` | look deeper: focus, then the raw stream |
| `t`  `:trace` | the raw stream of the agent at hand |
| `v`  `:talk` | its conversation with you |
| `esc` | go back |
| `tab` `⇧tab` `[` `]` | next / previous panel |
| `,`  `.`  `<`  `>` | wider / narrower side panel |

## The salon

| Key | What it does |
| --- | --- |
| `←` `→`  `h` `l` | fold / unfold a project or a worker's helpers |
| `za`  `zM`  `zR` | fold: toggle · all (`:fold`) · none (`:unfold`) |
| `f`  `:needs` | salon: only what needs you · everyone |

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
| `m`  `:mark` | mark the line at hand to come back to; again: unmark |
| `'` | the next marked line: conversation, else journal |
| `space`, `pgup` `pgdn` | a page |
| `esc` | close |

## A change

| Key | What it does |
| --- | --- |
| `[`  `]` | in a change: previous / next file · in a conversation: turn |
| `>` | in a change: the hunk at the top, quoted in a message to its worker |
| `c`  `n` | in a change: a note on the top line · send the notes, it goes back |
| `x` | in a change: choose the hunk at the top; `a` applies only the chosen |
| `w` | in a change: wrap long lines · cut them again |
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
| `?`  `:help` | this help; `/` in it filters the keys |
| `W` | the next item waiting, from anywhere |
| `^o` | back where you were |

## The legend

| Glyph | Meaning |
| --- | --- |
| `◆` lead  `●` working  `◐` asking  `✕` blocked  `!` waiting for you | an agent's state |
| `◇` researcher  `○` idle  `✓` done, not seen  `■` stopped  `⇄` conversation  `▾` `▸` project / helpers  `▴` pinned | |
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
| `:fold` / `:unfold` | `zM` / `zR` |
| `:trace` / `:talk` | `t` / `v` |
| `:mark` | `m` |
| `:pause`, `:faster`, `:slower` | `space`, `+`, `-` (demo) |
| `:budget` (`:b`), `:lead`, `:open` (`:o`), `:close` (`:c`), `:rename`, `:solo`, `:effort`, `:model`, `:forget` (`:f`), `:export`, `:commit`, `:copy`, `:pin`, `:awake`, `:notify` | no key: typed after `:` |

The full list of what each command takes is in the [commands reference](../reference/commands.md).

## See also

- [The command line](command-line.md)
- [The salon](salon.md)
- [WAITING FOR YOU](waiting.md)
- [The change view](changes.md)
- [Commands reference](../reference/commands.md)
