---
title: Cheat sheet
description: "The keys and commands of a first week with imece on one printable page - start, write, decide, read a change, move, the useful : commands, your shell, quit and come back."
hide_table_of_contents: true
---

# Cheat sheet

The keys and commands you need once imece is installed and you have seen [the tour](videos.mdx), on one page you can print. On the screen, `?` lists the keys of the screen you are on and `?` again every key; [Keyboard](screen/keyboard.md) and [Commands](reference/commands.md) have the whole list.

## Start

| Command | What it does |
| --- | --- |
| `imece` | in a git checkout: the leader alone, idle until you write to it |
| `imece --model sonnet` | the model the leader and standard-tier workers run on; `opus@xhigh` adds an effort |
| `imece --budget 10` | the pot, in dollars, for the whole team; kept between runs ($50 the first time) |
| `imece --agents 8` | the most workers the leader may hire (0-16, default 4) |
| `imece --run npm,make` | programs workers may run with their `run` tool beside `cargo`; under the default confine, also without asking you |
| `imece --demo` | the whole screen at work with fake agents, at no cost |

## Write

| Key | What it does |
| --- | --- |
| `/`  `i` | write to the leader of the project at hand, your conversation with it above the line; `enter` sends and the line stays open, `esc` leaves the line, `esc` again the conversation |
| `@name text` | write to one agent; `tab` completes the name |
| `⇧enter`  `\` `enter` | a new line (the footer says which works in your terminal) |
| `↑` `↓`  `^r` | the lines sent before · search them |
| `^x` `^e` | the line in your editor (`$VISUAL`, `$EDITOR`, else `vi`) |
| `/cmd`  `@name /cmd` | the agent's own command (`/compact`, `/cost`, `/clear`) |

## Decide

What waits for you is on top of Home, under TO DECIDE: permissions first, then questions, then changes. The row at hand ends with the keys that decide it, and a decision moves you to the next row.

| Key | What it does |
| --- | --- |
| `W` | the next thing to decide, opened, from anywhere |
| `S` | put this one off till later; the next opens |
| `enter` | open the row: a change on its diff, a question whole, the leader's plan in a window |
| `a` | yes: allow a command once, approve a change (applied to your checkout, staged; on Lonca, the proposal approved), yes to a plan |
| `r` | no: refuse a command, send a change back (you write why), no to a plan |
| `e`  `:yes` | on a permission: yes to everything that agent asks until its turn ends |
| `A`  `:allow` | on a permission: yes from now on to that command, for the project |
| `1`-`9` | a question's answers, as the row numbers them |
| `/` | on a question: answer it in your own words |
| `n` | in the plan window: write to the leader instead of answering |

## Read a change

| Key | What it does |
| --- | --- |
| `tab`  `⇧tab` | the tabs of a piece of work: `diff · talk · log` |
| `j` `k`  `^d` `^u`  `gg` `G` | scroll: a line, half a page, the ends |
| `[`  `]` | the previous / next file |
| `h`  `l` | fold the file at hand to its header · open it |
| `c` | a note on the line at the top |
| `n` | send the notes; the change goes back to its worker |
| `x` | choose the hunk at the top; `a` then applies only the chosen |
| `>` | quote the hunk at the top in a message to its worker |
| `b` | when git refused it: send back what clashes |
| `w` | wrap long lines · cut them again |

## Move

| Key | What it does |
| --- | --- |
| `esc` | go back |
| `^o` | back where you were, like vim's jump list |
| `&`  `:goto` | to an agent or a project by name; `1`-`9` the nth, `!` only what needs you |
| `#N` | a card by its number |
| `gd`  `gw` | on Home: the first row to decide · the first at work |
| `[`  `]` | on Home: one project at a time |
| `t`  `v` | the agent at hand's raw stream · its conversation with you |
| `?` | this screen's keys; `?` again every key; `/` in it searches |

## Useful `:` commands

| Command | What it does |
| --- | --- |
| `:commit` | a cheap model suggests a message for what is staged, as `!git commit -m '…'` on the line for you to edit and run (at most $0.05) |
| `:lonca import` | make a [Lonca](concepts/projects.md#on-lonca) repository of the checkout (`a` in its window imports) |
| `:sync` | bring Lonca's trunk to your checkout, by fast-forward only |
| `:code` | the code map: on Lonca, who holds a lease on what |
| `:journal`  `J` | the journal in a window; `J` there tells the story by card |
| `:budget 80` | raise or lower the pot while agents run |
| `:yes` | `e`: yes to an agent's asks until its turn ends |
| `:allow npm run` | a program and one word the project's agents run without asking; bare, the list |
| `:plan on\|off` | the leader puts its plan before you before it hires a second worker (on by default) |
| `:closes on\|off` | a card whose change is in closes the issue its leader named (on by default) |

## Your own shell

| Key | What it does |
| --- | --- |
| `!cmd` | run `cmd` in your own shell, the screen stepped aside |
| `!git commit` | commit what you approved: it lands staged |
| `O` | quote the last `!cmd` (how it ran, its last 40 lines) in a message to the agent at hand |

## Quit and come back

| Key | What it does |
| --- | --- |
| `q` `q` | quit; the first `q` asks, and says what is mid-turn or not reviewed |
| `q` `s` | list what quitting would leave: agents mid-turn, changes not reviewed |
| `^c` | quit at once |
| `imece` | again in the same checkout: the same screen, every agent asleep at no cost until you write to it, then on in its old session ([sessions](concepts/sessions.md)) |
