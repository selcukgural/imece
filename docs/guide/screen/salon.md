---
title: The team
description: The team on Home - the header, AT WORK project by project, each agent's row and glyph, folding, settling, the windows.
---

# The team

The team is the lower half of [Home](home.md), AT WORK (record 125): `:salon` goes there, as `gw`. What this page describes (every project, its workers and their helpers, folding, `s`) stands there, project by project; what waited beside it is Home's TO DECIDE, and the [journal](journal.md) (told as the story) is a window, `J` or `:journal`. The board, the code map and the timeline open as windows too (`:board`, `:code`, `:timeline`); everything else opens over Home and `esc` brings it back.

```text
 imece  shop-api · mobile-app │ $0.13 of $200.00  14:05
  lonca mobile-app 9d3f1c2 · 1 pending
 AT WORK 3
▸▾ shop-api      ░░░░░░░░ 0/1  1 working  $0.04  ~/src/shop-api
 ● shop-api    #1   Shipping zones            Run the tests · 4 min
 ▾ mobile-app    ░░░░░░░░ 0/3  2 working  $0.08  lonca mobile-app 9d3f1c2 · 1 pending  ~/src/mobile-app
 ● mobile-app  #2   Shipping cost             Run the tests · 2 min
 ● mobile-app  #3   Paging in product search  Run the tests · 4 min
 home · shop-api    enter  open   h l  fold · unfold   s  hide   ?  help
```

## The header

One line, four things, parts separated by `│`: the projects, what waits for you, the pot, the clock; a usage window joins them only when it is nearly full. When the screen is narrower than the line, the least wanted part goes first, so its edge never cuts off what waits for you: the clock, then the projects. What waits for you, the pot, the pause and a window nearly full always stay.

| Part | What it says |
| --- | --- |
| `shop-api · mobile-app` | the open projects. On [Home](home.md) the one the list is narrowed to is lit. |
| `! 2 to decide` | open items in TO DECIDE that ask you to decide, in the attention colour; replies to read are counted apart, dim, `1 to read`, and go first on a narrow screen. The terminal's title carries the count to decide, `! imece · 2 to decide`, so a tab in the background shows it, after the top of the ladder of the open projects: `■` something is stuck, `!` something waits for you, `●` something works or starts, imece's mark `⁙` when all is calm (a turn done, not seen, is calm), and `✓ imece · all done` when every open project's handed-out work is through and nothing waits for you ([when the work is done](home.md#when-the-work-is-done)). Windows Terminal, kitty 0.39+, Ghostty 1.2+ and iTerm2 3.6.6+ also show it as the tab's progress bar (OSC 9;4): red full when stuck, yellow full when waiting, moving while working, none when calm. imece asks the terminal its name at start (XTVERSION) and goes by its answer, not by `TERM_PROGRAM`, which a terminal started from another inherits; other terminals are not sent it, since some take that sequence for a notice. Inside tmux it is sent: tmux 3.8 passes it on to Ghostty and iTerm2, an older tmux drops it. `IMECE_PROGRESS=on` or `off` decides it yourself. Your shell's title comes back, and the bar goes, when imece ends or hands the terminal to a `!` command. |
| `$3.10 of $20.00` | the pot: what all agents together have spent of the budget. With under half an hour left at the current rate the part turns the attention colour and says how long is left, `$19.60 of $20.00, ~25 min left`, and the journal says so once. The rate and the time left at any other time are in the `:budget` menu's note. |
| `claude 5h: 10% left, resets in 1 h 59 min` | your plan's usage window nearest to full, only from 90% used, in the attention colour: a subscription's limit can stop the team before the pot does. Below that it is in the `:budget` note. Not kept between runs: a session says it again on its first turn. |
| `14:02` | the clock; the demo's `‖ PAUSED` or `×4` glued to it. |

What happened in the last ten minutes, the count of cards and the researchers' cap left the header with record 125: the board has the cards, the journal has what happened. The fleet (`● 2 working  ✓ 1 done  ■ 1 stuck`) left it with step 2: Home's AT WORK heading counts who works, who is stuck and who is idle, and each row says what its agent does.

See [budget and limits](../concepts/budget-and-limits.md) for how the pot and the windows are counted.

## Project by project

AT WORK lists the open projects in the order they were opened, pinned ones first. When more than one is in view (none narrowed to with `[` `]`), each heads its agents with a line of its own; narrowed to one, or with one project open, the rows stand alone. Under a project come its workers and solos: the stuck first, then those at work, then the quiet ones, equals keeping their order so the list does not jump as agents go from idle to working and back; each has its helpers and the researcher working for it hung under it (`└`). A leader is listed only while it works or is stuck, and an agent waiting on a yes or an answer of yours is that item in TO DECIDE instead. The AT WORK heading counts them: `AT WORK 12 · stuck 2 · finished 1 · idle 1` (finished: idle, the work it last had done; [Home](home.md) has the words of an idle row). When the rows do not fit the screen, the list scrolls with the row at hand; nothing folds by itself.

While no project has a worker yet (a team that has only its leader, a solo), imece's mark and its words stand in the middle of the free rows: `imece v…`, `one terminal, many agents, one leader`, the project at hand and who heads it. Its dots come one a beat as it appears (see [first run](../getting-started/first-run.md#the-welcome-window)); the first worker's row takes its place.

A project's line: `▾ shop-api  ▓▓▓░░░░░ 3/8  ! 2 waiting  ■2  2 working  $2.40  ~/src/shop-api`. `▾` is open, `▸` folded. After the name: how far its cards are (eight cells, the done share filled down to the cell, then done of all; none before its leader posts a card), in the attention colour how many items of this project wait for you, `■N` stuck agents (blocked or stopped) when there are any, how many workers and helpers are not idle, what the project's agents have cost, how its Lonca repository stands, and last, while it is open, the folder it works in. What wants you comes first, so a line too long for the screen loses the rest. A folded project is one line, that line still shows what waits (quiet, not deaf) and ends with its loudest member, `▸ shop-api  ! 1 waiting  2 working  $2.40  ! claude-5`: the agent highest on the state ladder below, the first in order among equals. A pinned project (`:pin`) wears `▴` before its name and keeps the top of the list; the pin is saved between runs.

## An agent's row

On Home an agent's row is short and says the piece of work first:

```text
 ● #16  Search result ranking     Make the change · 3 min
 ■ #8   Shipping cost             session ended: the process exited with code 1 · 9 min
```

- Its status's glyph: `●` at work (mid-turn, starting, asking a teammate, researching), `○` idle or asleep, `■` stuck (blocked, or its session ended: the words say which), `!` waiting for you. Every state is a shape of its own, so the mono theme tells them apart; a leader's row carries its status's glyph like any other. While motion moves ([`:motion`](../reference/commands.md#motion-offcalmfull), calm by default), a working row's glyph breathes in its colour (never its shape), a row that comes to wait for you blinks three times, one whose turn is done or whose session started hides from a green background; in another window nothing moves.
- Its project, dim, when several are open and the list is not narrowed to one.
- The card's number and title; an agent with no card shows its name instead. A helper or a researcher hangs under its worker, `└`, without a number.
- What it does: the step of its to-do list in progress (`Make the change`), else its status in the words below, cut to what the row has left.
- How long it has been in that state, from a minute on: `· 3 min`, `· 1 h 5 min`. For something waiting for you, since the item came.

The row at hand has `▸` before it and the selection colour. `enter` opens the agent's focus, where everything else about it is: its folder, worktree stand, to-do list, context window, activity and story.

### Status words

`starting…` while its session comes up (the process, a first worktree's checkout), `thinking…`, what it says it is doing, `? → claude-2` while it waits for a teammate's answer, `? → you` while a question of its waits for you, `⚑ asks to run` while a tool call of its waits in TO DECIDE, `answered you` while its answer waits to be read, `↻ 2/3 in 25 s` while imece waits to try a turn that failed on a passing API error again (the try, the tries, the time left), `■ API error` while an API error waits for you, `coordinating` for a leader whose workers work, `BLOCKED`, `■ session ended · write to it to restart`, `◦ asleep since the last run, write to it to wake it`. Where the status has a text of its own, the row carries it: `? → you "which index?"`, `answered you "use the enum"`, `⚑ asks to run "cargo test"`, `■ API error "Invalid API key"`, `BLOCKED "JWT clock skew" ← researcher-1`, `■ session ended: the process exited with code 1`. Messages that wait with an agent while it waits on you come first, `✉ 2 queued ? → you "which index?"`, and `✓ turn` first says you said yes to everything it asks this turn.

### The long row

A [card's screen](cards.md) lists who had a part in the card with the long row, every figure of an agent on one line:

```text
owner     ● ▎claude-1  #3   Parse the manifest  ▸ Write the tests  Edit tests/parse.rs · 2 min  $1.10  ⎇ fix-parser ↑2 ↓1 +12 −3  1 helpers  ↗ localhost:5173/
```

- The state glyph: where the agent stands on the [state ladder](#the-state-ladder). The five glyphs again, with two more meanings: `!` when something of its waits for you, whatever it does meanwhile, and `✓` for a worker that finished a turn you have not opened since.
- `▎` in the agent's speaker colour, the same colour its lines have in the journal and in a conversation, so a row and its talk read as one agent.
- Its name, then `~` or `≈` when it is tainted (research taint, demo only).
- A worker's card: `#3` and the card's title.
- Its status, in the words above. A working agent whose to-do list has a step in progress says that step first and what it does for it after, dim: `▸ Write the tests  Edit tests/parse.rs`, `▸ Run the tests  thinking…`, so the row says what the agent is after rather than only its last tool call; the step takes at most 24 columns and never more than the row's fixed parts leave it, the tool call what is left after the step (a tool call waiting for your yes, or a session still starting, says that instead). Otherwise the status ends with the agent's newest exchange, the arrow saying who said it: `thinking… ← lead "fix the parser"`, `idle → you "parser fixed, tests green"`, `? → claude-2 "which parser?"` (its own question to the one it waits for, quoted alone). The exchange stays until the next one replaces it, so a row says what the agent last did, and the status with its text keeps to forty columns.
- `▁▃▅▂▁`: what the agent did lately, one column for each of the last five ticks (5 seconds each, 25 in all), the oldest first: how many lines its trace got in that tick (a text, a tool call, a message, a note), on one scale for every row (`▁` none, `▂` one, `▃` two, `▄` up to four, `▅` six, `▆` nine, `▇` fourteen, `█` more), so a busy agent and a quiet one read apart at a glance. It moves only as the agent does something and as the ticks pass, never on a timer of its own; once nothing has been written for five ticks it is gone. It gives way first on a narrow row, even before the quote, and [`:motion off`](../reference/commands.md#motion-offcalmfull) leaves it out. The focus (`enter`) has a longer one, two minutes, 24 columns on the same scale, with how many lines they count: `activity: ▁▁▂▄▁…█▃ · trace lines in 2 min: 21`. Not kept between runs.
- How long it has been in that state, from a minute on, as on Home.
- What it has cost, when anything.
- `≡ 3/7`: how far its own to-do list is, the steps done and all of them, when its planning tool keeps one (Claude Code's tasks). The step in progress is the row's status (`▸`); the focus lists them all. The mark gives way after the quote and the time when the row is narrow. Gone with the session.
- `◔ 72%`: how full its context window is, from 70% on, in the attention colour from 90%, as Claude Code last said (its message usage and the window its result names). The focus has the figure whatever it is, `context: 22% · 45k of 200k`, or the tokens alone when the window's size is not said. It stays on a narrow row longer than the to-do mark. Gone with the session.
- A worker's worktree stand, `⎇ fix-parser ↑2 ↓1 +12 −3`: the branch the agent made (none while the worktree is detached, as imece makes it; a worker hired from a start point shows that point instead, `⎇ #41`), its own commits the checkout does not have (imece's marks of applied changes are not counted), the checkout's commits it does not have, and the lines added and removed against its own HEAD, untracked files included: what its next `done` would hand you. Each part shows only when there is something to say. Read on imece's git thread as the session starts, when a turn ends, after an apply and every 15 seconds while the worker or a helper is mid-turn; not kept between runs.
- `1 helpers`: how many helpers the worker has; Home lists them under it (`└`).
- `↗ localhost:5173/`: the newest loopback address the worker or helper wrote in its own text, as a link the terminal opens (OSC 8). Only `http`/`https` on `localhost`, `127.0.0.1` or `[::1]` counts, the query and fragment are dropped, and an address over 60 characters is not shown. Gone with the session.

The time, the quote and the activity bar take only what the row has left once its cost, stand, helpers and link are on it, so a narrow screen drops the bar first, the quote next and then the time.

## The state ladder

Every agent stands on one rung, loudest first: stuck (blocked, or its session stopped: it goes nowhere until you write to it) > something of its waits for you > working (mid-turn, asking a teammate, researching) > starting > done, not seen > idle (or asleep, or gone). The rung decides the long row's glyph, the mark the terminal's title and tab carry for the open projects, and which member a folded project shows for all of them: the highest rung, the first in order among equals. Home orders its rows by its own groups (stuck, at work, quiet), not by the rung.

## The legend

Every glyph the `?` help lists, as the screen draws them:

| Glyph | Meaning |
| --- | --- |
| `!` | something of its waits for you, whatever it does meanwhile; also the header's count and the inbox's own mark |
| `●` | at work: mid-turn, its session starting (`starting…` as its status), asking a teammate (`? → claude-2`), researching |
| `○` | idle, or asleep since an earlier run (writing to it wakes it) |
| `✓` | done, not seen: finished a turn you have not opened since |
| `■` | stuck: blocked (`BLOCKED` as its status), or its session ended (`■ stopped`; writing to it starts a new one); also a count of stuck agents on a project header. In the attention colour: a worker that told its leader it is blocked, while the leader has it (`blocked "…" · lead is on it`; not counted as stuck: [a worker that is blocked](../concepts/messages-and-board.md#a-worker-that-is-blocked)) |
| `⇄` | a conversation line |
| `▾` `▸` | a project, open or folded |
| `▴` | a pinned project |
| `~` | soft-tainted: read a research report (demo only) |
| `≈` | tainted: reads the outside world (demo only) |
| `⎇` `↑` `↓` `+N −M` | a worker's worktree: branch, own commits, the checkout's new commits, lines not yet handed over |
| `↗` | a local address the worker wrote, a link the terminal opens |
| `≡ 3/7` | its own to-do list: steps done, and all of them |
| `▸ Run the tests` | on a working row: the step of its to-do list in progress |
| `◔ 72%` | its context window, this full; shown from 70%, loud from 90% |
| `✱` | a line you marked with `m`; `'` goes to the next |
| `▁▃▅` | what it did lately: its trace lines in each of the last five ticks, 5 s a column |

Home's AT WORK rows use the same shapes, so they read in the mono theme too.

## Moving and folding

| Key | What it does |
| --- | --- |
| `↑` `↓` / `j` `k` | the row above or below; the preview follows |
| `gg` / `G`, `gd` / `gw` | first / last row · the first row to decide / the first at work |
| `^d` `^u` / `^f` `^b` | half a page / a page |
| `[` / `]` | all projects, then each one, round: the header lights the one shown |
| `←` `→` / `h` `l` | fold the row at hand's project to its heading / open it again |
| `za` | toggle the fold at hand |
| `zM` / `zR` (`:fold` / `:unfold`) | fold / unfold every project |
| `s` (`:settle`) | hide the row at hand's agent, or on a heading the project, until it needs you again; `:unsettle` brings everything back |
| `&` (`:goto`) | go to an agent or a project by name or number, what needs you first; its project unfolds and its row comes back if it was hidden |
| `enter` | look deeper: an agent's focus (its card, status, origin and story), `enter` again its raw stream (the trace); on a heading, the leader's focus |
| `K` (`:peek`) | a peek: the agent at hand's focus in a window over the screen, which stays as it was; `j` `k` the next / previous agent, `enter` its whole focus, `@` write to it, `esc` or `K` close |
| `t` (`:trace`) / `v` (`:talk`) | the raw stream / the conversation of the agent at hand |
| `esc` | back up one depth |

Folding moves the cursor to the heading so it does not vanish.

## Settling what is finished

A worker that finished and that you have looked at still takes a row. `s` on its row hides it: the row goes, with its helpers and the researcher working for it, until it needs you again. On a project's heading `s` hides the whole project. The selection goes on to the next row still shown (else the one before it).

What waits for you or is stuck is never hidden: `not hidden: claude-2 needs you`. A finished turn not seen yet is fine; hiding counts it as seen. A hidden agent comes back by itself the moment something of its row needs you again: a question or a change in TO DECIDE, blocked or stopped, or a new turn finished. A hidden project comes back when any of its agents does. Nothing is gone silently: the AT WORK heading counts what is hidden, `AT WORK 12 · 2 hidden (:unsettle)`, and `&` still lists them; going to one with `&` or `:goto` brings its row (and its project) back. `:unsettle` brings everything back at once. Settling lasts the run only.

## A peek

`K` opens the focus of the agent at hand (the row at hand on Home, the agent of the item at hand, or the one you are zoomed in on) in a window over the screen, a little smaller than it; what is under it does not move. In the window `j` `k` (or the arrows) step to the next / previous agent in AT WORK's order, `enter` opens its whole focus, `@` starts a message to it, `esc` or `K` close it; other keys do nothing. Looking at a finished agent this way counts as seeing it.

## Windows

What once stood beside the team is Home's upper half and a few windows. TO DECIDE is the upper half ([TO DECIDE](waiting.md)). The [journal](journal.md) is a window, `J` or `:journal`: its lines newest first, or the story by card (`J` in it turns between them); the board, the code map and the timeline open as windows too (`:board`, `:code`, `:timeline`). `esc` or `q` closes a window, a key that opens another closes the first, and `W` closes any to open the next item to decide. `:clear` takes the decided items and the journal's lines off the screen without deleting anything; see [journal](journal.md#clearing-the-screen).

## Legibility

A window over Home (a conversation, a peek, the help, the away digest, where you left off) dims everything around it, so the list reads as background. The journal window and a question's row show a message as one row of plain words: cleaned, lines joined, heading and list marks off, a link by its label. The trace marks each line with a glyph for its kind, read from the line's first word: `›` the agent's own text, drawn as Markdown (lists, headings, code in the code colour, fences never shown); `▸` a tool call, its name in the info colour and its command or path in the code colour, a run's end by its exit code; `⇄` the bus (a message, a wake, a hire, the board) with its word in the accent; `•` a note of Claude Code's own (a retry, a refusal); `✕` what went wrong; `·` imece's own lines. The focus draws what agents wrote in its story the same way: a one-line message or a decision with its marks on its line, a longer message (or a list, a heading, code) under its head as Markdown, its first eight lines and how many more; imece's own story lines stay plain. In both, a web address (`http` or `https` only) shown whole on its row is a link your terminal opens on click (OSC 8): the text is the address, cleaned as it is drawn, and one cut by the row's end links nothing. An open conversation follows the talk: lines that arrive are appended, and the selection follows when it was on the last line.

Everything an agent wrote reaches the screen only through a cleaning that removes control characters and hides secret-looking tokens; nothing is drawn raw.

## See also

- [TO DECIDE](waiting.md)
- [The journal](journal.md)
- [Conversations](conversations.md)
- [The board and the code map](board-and-code-map.md)
- [Keyboard](keyboard.md)
- [Budget and limits](../concepts/budget-and-limits.md)
- [Projects](../concepts/projects.md)
