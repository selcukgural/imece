---
title: The salon
description: The default screen of imece - the header, the guild tree grouped by project, each agent's row and glyph, folding, and the panels beside it.
---

# The salon

The salon is the screen imece opens on and the one you come back to with `1` or `:salon`. It shows the whole team at a glance: a header with the pot and the fleet, the guild tree on the left (every project, its leader, its workers and their helpers), and on the right the two side panels, WAITING FOR YOU and the [story](journal.md#the-story) (what happened, by card; `J` shows the journal line by line). The board and the code map are one key away (`2`, `3`); everything else opens over the salon and `esc` brings it back.

```text
 imece   ! 2 waiting for you  │ ● 2 working  ✓ 1 done │ 10 min: 1 cards done · 1 Δ · 6 messages │ 3/9 cards │ spent $3.10 of $20.00 ($1.80/h, ~9 h left) │ claude 5h: 48% left, resets in 1 h 59 min  14:02
┌ SALON ──────────────────────────────────────────┐┌ WAITING FOR YOU (2) ───────────────────┐
│▾ ▴ shop-api  ▓▓▓░░░░░ 3/8  2 working  $2.40  ! 2││Δ 13:58 claude-1 finished: parser (3 files, +41 −6) · ✓ ran clean on this tree: cargo test
│  ◆ ▎lead      coordinating                       ││⚑ 14:01 claude-5 waits for your yes: Bash: npm install left-pad
│  ├ ● ▎claude-1 #3   Parse the manifest  writing tests · 2 min  $1.10  ⎇ fix-parser ↑2 +12
│  │ └ ● ▎claude-1.1                      thinking… ← claude-1 "add the tests"   ⎇          │
│  ├ ✓ ▎claude-5 #5   Money rounding      done, not seen yet → you "rounded half up" · 5 min│
│  └ … 2 agents doing fine                         │└────────────────────────────────────────┘
│▸ web  1 working  $0.70                           │┌ JOURNAL ───────────────────────────────┐
│                                                  ││13:58 ! claude-1 finished: 3 files, +41 −6 → waits for you
│                                                  ││13:55 ⇄ lead → claude-1 [assign] #3 Parse the manifest
│QUEUE  empty  │ latest: #2 ✓ #1 ✓                 ││13:40 › you: fix the parser crash on empty files
└──────────────────────────────────────────────────┘└────────────────────────────────────────┘
  / write to the lead  @ to an agent  # card  tab panel  ← → fold  enter focus  ? help   1 salon  2 board  3 code
```

## The header

One line, parts separated by `│`, the most wanted first. When the screen is narrower than the line, the least wanted part goes first, so its edge never cuts off what waits for you: the researchers and the usage windows (one nearly full stays), then the pot's rate, the clock, the pot, the cards, the last ten minutes and the fleet. What waits for you, the pause and a pot running low always stay.

| Part | What it says |
| --- | --- |
| `! 2 waiting for you` | open items in WAITING FOR YOU, in the attention colour. The terminal's title carries the same count, `! imece · 2 waiting`, so a tab in the background shows it, after the top of the ladder of the open projects: `✕` something is stuck, `!` something waits for you, `●` something works or starts, imece's mark `⁙` when all is calm (a turn done, not seen, is calm). Windows Terminal, kitty 0.39+, Ghostty 1.2+ and iTerm2 3.6.6+ also show it as the tab's progress bar (OSC 9;4): red full when stuck, yellow full when waiting, moving while working, none when calm. imece asks the terminal its name at start (XTVERSION) and goes by its answer, not by `TERM_PROGRAM`, which a terminal started from another inherits; other terminals are not sent it, since some take that sequence for a notice. Inside tmux it is sent: tmux 3.8 passes it on to Ghostty and iTerm2, an older tmux drops it. `IMECE_PROGRESS=on` or `off` decides it yourself. Your shell's title comes back, and the bar goes, when imece ends or hands the terminal to a `!` command. |
| `● 2 working  ✓ 1 done  ✕ 1 stuck` | the fleet: agents mid-turn (spending tokens), agents that finished a turn you have not opened since, agents blocked or whose session stopped. A count of zero is left out; with none at all it says `all idle`. What waits for you is not in the fleet: it has its own count. |
| `10 min: 3 cards done · 2 Δ · 14 messages` | what happened in the last ten minutes: cards done, changes that came for your review, messages the agents sent each other on the bus; a count of none is left out, and nothing shows when nothing happened. Read from the journal's newest lines back to ten minutes ago only. |
| `3/9 cards` | cards done out of all cards; left out while there are no cards (a live start has none until the leader posts). |
| `spent $3.10 of $20.00 ($1.80/h, ~9 h left)` | the pot: what all agents together have spent of the budget, what the last ten minutes cost per hour, and how long the pot lasts at that rate. With under half an hour left the whole part turns the attention colour and the journal says so once. |
| `claude 5h: 48% left, resets in 1 h 59 min` | your plan's usage window nearest to full, as sessions report it; loud from 90% used. A subscription's limit can stop the team before the pot does. Not kept between runs: a session says them again on its first turn. |
| `researchers 1/4` | researchers running out of the cap, only when there is a cap (`--demo --researchers N`; researchers are not built live). |
| `14:02` | the clock. |

See [budget and limits](../concepts/budget-and-limits.md) for how the pot and the windows are counted.

## The guild tree

Agents are grouped by project. Each project is a header row, then its leader, then its workers, each worker's helpers under it. Projects come in the order they were opened, pinned ones first.

While no project has a worker yet (a team that has only its leader, a solo), imece's mark and its words stand in the middle of the free rows: `imece v…`, `one terminal, many agents, one leader`, the project at hand and who heads it. Its dots come one a beat as it appears (see [first run](../getting-started/first-run.md#the-welcome-window)); the first worker's row takes its place, and `f` leaves it out.

A project header: `▾ shop-api  ▓▓▓░░░░░ 3/8  2 working  $2.40  ✕1  ■1   ! 2 waiting`. `▾` is open, `▸` folded. After the name: how far its cards are (eight cells, the done share filled down to the cell, then done of all; none before its leader posts a card), how many workers and helpers are not idle, what the project's agents have cost, `✕N` blocked and `■N` stopped agents when there are any, and, in the attention colour, how many items of this project wait for you. A folded project is one line, that line still shows what waits (quiet, not deaf) and ends with its loudest member, `▸ shop-api  2 working  $2.40   ! 1 waiting  ! claude-5`: the agent highest on the state ladder below, the first in order among equals. A pinned project (`:pin`) wears `▴` before its name and keeps the top of the salon; the pin is saved between runs.

An agent's row:

```text
  ├ ● ▎claude-1 #3   Parse the manifest  ▸ Write the tests  Edit tests/parse.rs · 2 min  $1.10  ⎇ fix-parser ↑2 ↓1 +12 −3  ▾ 1 helpers  ↗ localhost:5173/
```

- The tree lines `├ └ │` show who is under whom; a helper is under its worker, named `claude-1.1`.
- The state glyph: where the agent stands on the state ladder (see the legend below). Stuck shows as its status says (`✕` blocked, `■` stopped); `!` when something of its waits for you, whatever it does meanwhile; `✓` for a worker that finished a turn you have not opened since, with `done, not seen yet` as its status; `◌` while its session starts; else its status's own mark (`●` `◐` `◇` at work, `○` `◦` idle). Every state is a shape of its own, so the mono theme tells them apart. A leader always keeps `◆`. While motion moves ([`:motion`](../reference/commands.md#motion-offcalmfull), calm by default), a working row's glyph breathes in its colour (never its shape), a row that comes to wait for you blinks three times, one whose turn is done or whose session started settles from a green background, and `starting` counts its dots; in another window nothing moves.
- `▎` in the agent's speaker colour, the same colour its lines have in the journal and in a conversation, so a row and its talk read as one agent.
- Its name, then `~` or `≈` when it is tainted (research taint, demo only).
- A worker's card: `#3` and the card's title.
- Its status: `starting…` while its session comes up (the process, a first worktree's checkout), `thinking…`, what it says it is doing, `? → claude-2` while it waits for a teammate's answer, `? → you` while a question of its waits for you, `⚑ waits for your yes` while a tool call of its waits in WAITING FOR YOU, `answered you` while its answer waits to be read, `↻ 2/3 in 25 s` while imece waits to try a turn that failed on a passing API error again (the try, the tries, the time left), `✕ API error` while an API error waits for you, `coordinating` for a leader whose workers work, `BLOCKED`, `■ stopped, write to it to start again`, `◦ asleep since the last run, write to it to wake it`. A working agent whose to-do list has a step in progress says that step first and what it does for it after, dim: `▸ Write the tests  Edit tests/parse.rs`, `▸ Run the tests  thinking…`, so the row says what the agent is after rather than only its last tool call; the step takes at most 24 columns and never more than the row's fixed parts leave it, the tool call what is left after the step (a tool call waiting for your yes, or a session still starting, says that instead). Where the status has a text of its own, the row carries it: `? → you "which index?"`, `answered you "use the enum"`, `⚑ waits for your yes "cargo test"`, `✕ API error "Invalid API key"`, `BLOCKED "JWT clock skew" ← researcher-1`, `■ stopped: the process exited with code 1`. Otherwise the status ends with the agent's newest exchange, the arrow saying who said it: `thinking… ← lead "fix the parser"`, `idle → you "parser fixed, tests green"`, `? → claude-2 "which parser?"` (its own question to the one it waits for, quoted alone). The exchange stays until the next one replaces it, so a row says what the agent last did, and the status with its text keeps to forty columns. Messages that wait with an agent while it waits on you come first on the row: `✉ 2 held ? → you "which index?"`.
- `▁▃▅▂▁`: what the agent did lately, one column for each of the last five ticks (5 seconds each, 25 in all), the oldest first: how many lines its trace got in that tick (a text, a tool call, a message, a note), on one scale for every row (`▁` none, `▂` one, `▃` two, `▄` up to four, `▅` six, `▆` nine, `▇` fourteen, `█` more), so a busy row and a quiet one read apart at a glance. It moves only as the agent does something and as the ticks pass, never on a timer of its own; once nothing has been written for five ticks it is gone. It gives way first on a narrow row, even before the quote, and [`:motion off`](../reference/commands.md#motion-offcalmfull) leaves it out. The focus (`enter`) has a longer one, two minutes, 24 columns on the same scale, with how many lines they count: `activity: ▁▁▂▄▁…█▃ · trace lines in 2 min: 21`. Not kept between runs.
- How long it has been in that state, from a minute on: `· 3 min`, `· 1 h 5 min`. For something waiting for you, since the item came.
- What it has cost, when anything.
- `≡ 3/7`: how far its own to-do list is, the steps done and all of them, when its planning tool keeps one (Claude Code's tasks). The step in progress is the row's status (`▸`); the focus lists them all. The mark gives way after the quote and the time when the row is narrow. Gone with the session.
- `◔ 72%`: how full its context window is, from 70% on, in the attention colour from 90%, as Claude Code last said (its message usage and the window its result names). The focus has the figure whatever it is, `context: 22% · 45k of 200k`, or the tokens alone when the window's size is not said. It stays on a narrow row longer than the to-do mark. Gone with the session.
- A worker's worktree stand, `⎇ fix-parser ↑2 ↓1 +12 −3`: the branch the agent made (none while the worktree is detached, as imece makes it; a worker hired from a start point shows that point instead, `⎇ #41`), its own commits the checkout does not have (imece's marks of applied changes are not counted), the checkout's commits it does not have, and the lines added and removed against its own HEAD, untracked files included: what its next `done` would hand you. Each part shows only when there is something to say. Read on imece's git thread as the session starts, when a turn ends, after an apply and every 15 seconds while the worker or a helper is mid-turn; not kept between runs.
- `▾ 1 helpers` or `▸ 1 helpers`: its helpers are listed under it, or folded away.
- `↗ localhost:5173/`: the newest loopback address the worker or helper wrote in its own text, as a link the terminal opens (OSC 8). Only `http`/`https` on `localhost`, `127.0.0.1` or `[::1]` counts, the query and fragment are dropped, and an address over 60 characters is not shown. Gone with the session.
- A worker with helpers ends with `▾ 2 helpers`, or `▸ 2 helpers  ● claude-1.1` when they are folded away: the count and the loudest of them.

Every agent is one row, whatever it says: rows never change height, and the parts of a row stand in columns: every status starts at one column, whatever the depth, and each part after it is as wide as its widest on a row, blank on a row without it. On a narrow screen the blanks go first. The time, the quote and the activity bar take only what the row has left once its cost, stand, helpers and link are on it, so a narrow pane drops the bar first, the quote next and then the time (`.` narrows the side panel). A row that just changed stands out for a moment, then calms down. The selected row is reversed.

When some workers of an open project need you and it shows more than one, they sit under a header for what they need, in the ladder's order, and the rest under one header that counts them:

```text
▾ shop-api      7 working  $0.45   ! 2 waiting
  ◆ ▎lead-shop-api     coordinating
  ✕ stuck · 1
  └ ■ ▎claude-6          #17  Order tests           ■ stopped: the session ended
  ! waits for you · 2
  ├ ! ▎claude-4          #1   Payment retry policy  ⚑ waits for your yes "WebFetch: …"
  └ ! ▎claude-3          #7   Payment retry policy  ▸ Write it up  thinking…
  ● 4 working  ○ 1 idle
  ├ ● ▎claude-5          #2   Price list cache      ▸ Run the tests  Bash: dotnet test
  └ …
```

The headers are `✕ stuck`, `! waits for you`, `✓ done, not seen` and, for the rest, `● N working  ○ N idle` (a count of none left out); each closes its branch with `└`. They say where what needs you ends and the calm part starts. When all of a project's workers are calm, or it shows only one, there are none. They give way first: when the rows with them do not fit the pane, none is shown and the rows fold as below.

When the rows do not fit the pane, calm workers (working or idle, nothing said lately, not selected) fold from the bottom of the salon up into one line, `… 2 agents folded, loudest  ● claude-5`, which names the loudest of them; a pinned project folds last and what needs you is never folded away. The bottom line of the pane, `QUEUE`, lists cards being verified and the last four done (`#2 ✓`); live, nothing fills the verifying part, so it reads `empty`.

## The state ladder

Every agent stands on one rung, loudest first: stuck (blocked, or its session stopped: it goes nowhere until you write to it) > something of its waits for you > working (mid-turn, asking a teammate, researching) > starting > done, not seen > idle (or asleep, or gone). The rung decides the row's glyph, which worker comes first in its project (stuck, then waiting for you, then done and not seen; the calm rungs keep their order so the screen does not jump as agents go from idle to working and back), what `f` shows, and which member a folded group (a project, a worker's helpers, the folded calm workers) shows for all of them: the highest rung, the first in order among equals.

## The legend

Every glyph the `?` help lists, as the screen draws them:

| Glyph | Meaning |
| --- | --- |
| `◆` | the leader (it keeps this glyph whatever it does) |
| `◌` | starting: its session is coming up and has not answered yet |
| `●` | working: mid-turn |
| `◐` | asking a teammate and waiting for the answer |
| `✕` | blocked (waiting for research), or a count of blocked agents on a project header |
| `!` | something of its waits for you, whatever it does meanwhile; also the header's count and the inbox's own mark |
| `◇` | a researcher (demo only; the researcher layer is not built live) |
| `○` | idle |
| `✓` | done, not seen: finished a turn you have not opened since |
| `■` | stopped: the session ended; writing to it starts a new one; also a count of stopped agents on a project header |
| `◦` | asleep since an earlier run; writing to it wakes it |
| `⇄` | a conversation line |
| `▾` `▸` | a project or a worker's helpers, open or folded |
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

The fleet in the header uses the same shapes, so it reads in the mono theme too.

## Moving and folding

| Key | What it does |
| --- | --- |
| `↑` `↓` / `j` `k` | select the row above or below |
| `gg` / `G` | first / last row |
| `^d` `^u` / `^f` `^b` | half a page / a page |
| `←` `→` / `h` `l` | fold / unfold the selected project, or the selected worker's helpers (also the selected helper's worker). `enter` on a project header folds and unfolds too. |
| `za` | toggle the fold at hand |
| `zM` / `zR` (`:fold` / `:unfold`) | fold / unfold every project and every worker's helpers |
| `f` (`:needs`) | the salon shows only what needs you, then everyone again |
| `s` (`:settle`) | settle the selected worker's row, or the selected project (its header or its leader), until it needs you again; `:unsettle` brings everything back |
| `&` (`:goto`) | go to an agent or a project by name or number, what needs you first; its project and its worker's helpers unfold, and `f` lets go when it would hide the row |
| `enter` | look deeper: an agent's focus (its card, status, origin and story), `enter` again its raw stream (the trace) |
| `K` (`:peek`) | a peek: the agent at hand's focus in a window over the view, which stays as it was; `j` `k` the next / previous agent, `enter` its whole focus, `@` write to it, `esc` or `K` close |
| `F` (`:hints`) | hint letters on every row you can select; a label selects its row, its last letter in capitals opens it |
| `t` (`:trace`) / `v` (`:talk`) | the raw stream / the conversation of the agent at hand |
| `esc` | back up one depth |

Folding moves the selection to the worker or the header so it does not vanish.

## Only what needs you

What an agent needs of you is one rank, most first: something of it waits in WAITING FOR YOU, it is stuck (blocked, or its session stopped), it finished a turn you have not looked at, nothing. Within a project the workers sort by that rank, equals keeping their order, so the calm ones do not jump around. When rows do not fit, the calm ones fold first.

`f` keeps only the agents that need something, each under its project's header: a worker comes along for a helper of its that needs something, the leader only when it needs something itself. A project with nothing is left out, a folded one with something shows its header alone, and with nothing at all the salon says `Nothing needs you right now · f shows everyone`. The pane title says the filter is on, `SALON · only what needs you (f)`. While it is on, a selection whose row left moves to the first row of its project still shown (not while you write: the selection decides who `/` writes to). The filter lasts the run only.

## Settling what is finished

A worker that finished and that you have looked at still takes a row. `s` on its row settles it: the row goes, with its helpers and the researcher working for it, until it needs you again. On a project's header, or on its leader's row, `s` settles the whole project. The selection goes on to the next row still shown (else the one before it).

What waits for you or is stuck is never settled: `not settled: claude-2 needs you`. A finished turn not seen yet is fine; settling counts it as seen. A settled agent comes back by itself the moment something of its row needs you again: a question or a change in WAITING FOR YOU, blocked or stopped, or a new turn finished. A settled project comes back when any of its agents does. Nothing is gone silently: the pane title counts what is settled, `SALON · 2 settled (:unsettle)`, the header's fleet still counts every agent, and `&` still lists them; going to one with `&` or `:goto` brings its row (and its project) back. `:unsettle` brings everything back at once. Settling lasts the run only.

## A peek

`K` opens the focus of the agent at hand (selected in the salon, behind the inbox item or journal line, or the one you are zoomed in on) in a window over the screen, a little smaller than it; the view under it, the salon, the board, the code map or a card, does not move. In the window `j` `k` (or the arrows) step to the next / previous agent in the salon's order, `enter` opens its whole focus, `@` starts a message to it, `esc` or `K` close it; other keys do nothing. Looking at a finished agent this way counts as seeing it.

## Hint letters

`F` puts a label of one or two letters over the start of every row of the salon's screen you can select: the project headers and agents in the salon (on a card's screen, its people), the items of WAITING FOR YOU and the journal lines shown. The first rows drawn get one letter, home row first (`a s d f …`); when there are more rows than letters, the last letters start labels of two (`ma`, `ms`, …), as few as needed, and a letter that starts a label of two is never a label of its own. Typing a label selects its row and moves to its panel; typed with its last letter in capitals (`D`, `mS`), it also opens the row as `enter` does: an agent's focus, a change, a question's conversation, a journal line's conversation. The letters typed so far narrow the labels to those they start; a letter no label goes on does nothing, `backspace` takes one back, `?` shows the help over them, `esc` closes them. The labels are given when `F` opens them and stay with their rows: a row that moves follows its label, one that leaves the screen loses it, and typing its label says `that row has left the screen`. Where the screen has no such rows (the board, the code map, a focus, a conversation over the salon) `F` says so. Both the demo and live sessions have them.

## The side panels

To the right, WAITING FOR YOU on top and the JOURNAL under it. `tab` and `⇧tab` (or `]` and `[`) cycle salon → waiting → journal; `w` (`:waiting`) goes straight to the waiting panel. The waiting panel takes as many rows as its items need, up to half the side, and its border turns the warning colour when items wait and the panel is not focused. `:clear` takes both panels' contents off the screen without deleting anything; see [journal](journal.md#clearing-the-screen).

`,` and `.` (or `<` and `>`) make the side panel wider or narrower by four columns, between 20 and 70 percent of the screen; the flash says `side panel 38% (, . or < >)` and the width is saved for the next run. In a change's view `>` quotes a hunk instead, and `.` still narrows.

## Legibility

A window over the salon (a conversation, a peek, the help, the away digest, where you left off) dims everything around it, so the tree and the journal's clipped line ends read as background. The journal pane and a question in the inbox show a message as one row of plain words: cleaned, lines joined, heading and list marks off, a link by its label. The trace marks each line with a glyph for its kind, read from the line's first word: `›` the agent's own text, drawn as Markdown (lists, headings, code in the code colour, fences never shown); `▸` a tool call, its name in the info colour and its command or path in the code colour, a run's end by its exit code; `⇄` the bus (a message, a wake, a hire, the board) with its word in the accent; `•` a note of Claude Code's own (a retry, a refusal); `✕` what went wrong; `·` imece's own lines. The focus draws what agents wrote in its story the same way: a one-line message or a decision with its marks on its line, a longer message (or a list, a heading, code) under its head as Markdown, its first eight lines and how many more; imece's own story lines stay plain. In both, a web address (`http` or `https` only) shown whole on its row is a link your terminal opens on click (OSC 8): the text is the address, cleaned as it is drawn, and one cut by the row's end links nothing. An open conversation follows the talk: lines that arrive are appended, and the selection follows when it was on the last line.

Everything an agent wrote reaches the screen only through a cleaning that removes control characters and hides secret-looking tokens; nothing is drawn raw.

## See also

- [WAITING FOR YOU](waiting.md)
- [The journal](journal.md)
- [Conversations](conversations.md)
- [The board and the code map](board-and-code-map.md)
- [Keyboard](keyboard.md)
- [Budget and limits](../concepts/budget-and-limits.md)
- [Projects](../concepts/projects.md)
