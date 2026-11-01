---
title: The salon
description: The default screen of imece - the header, the guild tree grouped by project, each agent's row and glyph, folding, and the panels beside it.
---

# The salon

The salon is the screen imece opens on and the one you come back to with `1` or `:salon`. It shows the whole team at a glance: a header with the pot and the fleet, the guild tree on the left (every project, its leader, its workers and their helpers), and on the right the two side panels, WAITING FOR YOU and the journal. The board and the code map are one key away (`2`, `3`); everything else opens over the salon and `esc` brings it back.

```text
 imece  3/9 cards │ ● 2 working  ✓ 1 done │ spent $3.10 of $20.00 ($1.80/h, ~9 h left) │ claude 5h: 48% left, resets in 1 h 59 min │  ! 2 waiting for you   14:02
┌ SALON ──────────────────────────────────────────┐┌ WAITING FOR YOU (2) ───────────────────┐
│▾ ▴ shop-api  2 working  $2.40   ! 2 waiting     ││Δ 13:58 claude-1 finished: parser (3 files, +41 −6) · ✓ ran clean on this tree: cargo test
│  ◆ ▎lead      claude                coordinating ││⚑ 14:01 codex-2 waits for your yes: Bash: npm install left-pad
│  ├ ● ▎claude-1 #3   Parse the manifest  writing tests  $1.10  ⎇ fix-parser ↑2 ↓1 +12 −3
│  │ └ ● ▎claude-1.1                      thinking…    ⎇                                    │
│  ├ ✓ ▎codex-2  #5   Money rounding      done, not seen yet  $0.80  ↗ localhost:5173/      │
│  └ … 2 agents doing fine                         │└────────────────────────────────────────┘
│▸ web  1 working  $0.70                           │┌ JOURNAL ───────────────────────────────┐
│                                                  ││13:58 ! claude-1 finished: 3 files, +41 −6 → waits for you
│                                                  ││13:55 ⇄ lead → claude-1 [assign] #3 Parse the manifest
│QUEUE  empty  │ latest: #2 ✓ #1 ✓                 ││13:40 › you: fix the parser crash on empty files
└──────────────────────────────────────────────────┘└────────────────────────────────────────┘
  / write to the lead  @ to an agent  # card  tab panel  ← → fold  enter focus  ? help   1 salon  2 board  3 code
```

## The header

One line, left to right, parts separated by `│`:

| Part | What it says |
| --- | --- |
| `3/9 cards` | cards done out of all cards; left out while there are no cards (a live start has none until the leader posts). |
| `● 2 working  ✓ 1 done  ✕ 1 stuck` | the fleet: agents mid-turn (spending tokens), agents that finished a turn you have not opened since, agents blocked or whose session stopped. A count of zero is left out; with none at all it says `all idle`. What waits for you is not in the fleet: it has its own count. |
| `spent $3.10 of $20.00 ($1.80/h, ~9 h left)` | the pot: what all agents together have spent of the budget, what the last ten minutes cost per hour, and how long the pot lasts at that rate. With under half an hour left the whole part turns the attention colour and the journal says so once. |
| `claude 5h: 48% left, resets in 1 h 59 min` | each vendor plan's usage window nearest to full, as Claude and Codex sessions report them (Gemini reports none); loud from 90% used. A subscription's limit can stop the team before the pot does. Not kept between runs: a session says them again on its first turn. |
| `researchers 1/4` | researchers running out of the cap, only when there is a cap (`--demo --researchers N`; researchers are not built live). |
| `! 2 waiting for you` | open items in WAITING FOR YOU, in the attention colour. The terminal's title carries the same count, `imece · 2 waiting`, so a tab in the background shows it; your shell's title comes back when imece ends. |
| `14:02` | the clock. |

See [budget and limits](../concepts/budget-and-limits.md) for how the pot and the windows are counted.

## The guild tree

Agents are grouped by project. Each project is a header row, then its leader, then its workers, each worker's helpers under it. Projects come in the order they were opened, pinned ones first.

A project header: `▾ shop-api  2 working  $2.40  ✕1  ■1   ! 2 waiting`. `▾` is open, `▸` folded. After the name: how many workers and helpers are not idle, what the project's agents have cost, `✕N` blocked and `■N` stopped agents when there are any, and, in the attention colour, how many items of this project wait for you. A folded project is one line, that line still shows what waits (quiet, not deaf) and ends with its loudest member, `▸ shop-api  2 working  $2.40   ! 1 waiting  ! codex-2`: the agent highest on the state ladder below, the first in order among equals. A pinned project (`:pin`) wears `▴` before its name and keeps the top of the salon; the pin is saved between runs.

An agent's row:

```text
  ├ ● ▎claude-1 #3   Parse the manifest  writing tests  $1.10  ⎇ fix-parser ↑2 ↓1 +12 −3  ▾ 1 helpers  ↗ localhost:5173/
```

- The tree lines `├ └ │` show who is under whom; a helper is under its worker, named `claude-1.1`.
- The state glyph: where the agent stands on the state ladder (see the legend below). Stuck shows as its status says (`✕` blocked, `■` stopped); `!` when something of its waits for you, whatever it does meanwhile; `✓` for a worker that finished a turn you have not opened since, with `done, not seen yet` as its status; `◌` while its session starts; else its status's own mark (`●` `◐` `◇` at work, `○` `◦` idle). Every state is a shape of its own, so the mono theme tells them apart. A leader always keeps `◆`.
- `▎` in the agent's speaker colour, the same colour its lines have in the journal and in a conversation, so a row and its talk read as one agent.
- Its name in its vendor's colour, then `~` or `≈` when it is tainted (research taint, demo only).
- A worker's card: `#3` and the card's title. A leader shows its vendor instead.
- Its status: `starting…` while its session comes up (the process, a first worktree's checkout), `thinking…`, what it says it is doing, `? → claude-2` while it waits for a teammate's answer, `? → you` while a question of its waits for you, `⚑ waits for your yes` while a tool call of its waits in WAITING FOR YOU, `answered you` while its answer waits to be read, `coordinating` for a leader whose workers work, `BLOCKED`, `■ stopped, write to it to start again`, `◦ asleep since the last run, write to it to wake it`.
- What it has cost, when anything.
- A worker's worktree stand, `⎇ fix-parser ↑2 ↓1 +12 −3`: the branch the agent made (none while the worktree is detached, as imece makes it), its own commits the checkout does not have (imece's marks of applied changes are not counted), the checkout's commits it does not have, and the lines added and removed against its own HEAD, untracked files included: what its next `done` would hand you. Each part shows only when there is something to say. Read on imece's git thread as the session starts, when a turn ends, after an apply and every 15 seconds while the worker or a helper is mid-turn; not kept between runs.
- `▾ 1 helpers` or `▸ 1 helpers`: its helpers are listed under it, or folded away.
- `↗ localhost:5173/`: the newest loopback address the worker or helper wrote in its own text, as a link the terminal opens (OSC 8). Only `http`/`https` on `localhost`, `127.0.0.1` or `[::1]` counts, the query and fragment are dropped, and an address over 60 characters is not shown. Gone with the session.
- A worker with helpers ends with `▾ 2 helpers`, or `▸ 2 helpers  ● claude-1.1` when they are folded away: the count and the loudest of them.
- Under a row that talks, a dim italic line `⇄ claude-2 "which parser?"` with the newest exchange.

A row that just changed stands out for a moment, then calms down. The selected row is reversed.

When the rows do not fit the pane, calm workers (working or idle, nothing said lately, not selected) fold from the bottom of the salon up into one line, `… 2 agents folded, loudest  ● codex-2`, which names the loudest of them; a pinned project folds last and what needs you is never folded away. The bottom line of the pane, `QUEUE`, lists cards being verified and the last four done (`#2 ✓`); live, nothing fills the verifying part, so it reads `empty`.

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
| `✱` | a line you marked with `m`; `'` goes to the next |

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
| `enter` | look deeper: an agent's focus (its card, status, origin and story), `enter` again its raw stream (the trace) |
| `t` (`:trace`) / `v` (`:talk`) | the raw stream / the conversation of the agent at hand |
| `esc` | back up one depth |

Folding moves the selection to the worker or the header so it does not vanish.

## Only what needs you

What an agent needs of you is one rank, most first: something of it waits in WAITING FOR YOU, it is stuck (blocked, or its session stopped), it finished a turn you have not looked at, nothing. Within a project the workers sort by that rank, equals keeping their order, so the calm ones do not jump around. When rows do not fit, the calm ones fold first.

`f` keeps only the agents that need something, each under its project's header: a worker comes along for a helper of its that needs something, the leader only when it needs something itself. A project with nothing is left out, a folded one with something shows its header alone, and with nothing at all the salon says `Nothing needs you right now · f shows everyone`. The pane title says the filter is on, `SALON · only what needs you (f)`. While it is on, a selection whose row left moves to the first row of its project still shown (not while you write: the selection decides who `/` writes to). The filter lasts the run only.

## The side panels

To the right, WAITING FOR YOU on top and the JOURNAL under it. `tab` and `⇧tab` (or `]` and `[`) cycle salon → waiting → journal; `w` (`:waiting`) goes straight to the waiting panel. The waiting panel takes as many rows as its items need, up to half the side, and its border turns the warning colour when items wait and the panel is not focused.

`,` and `.` (or `<` and `>`) make the side panel wider or narrower by four columns, between 20 and 70 percent of the screen; the flash says `side panel 38% (, . or < >)` and the width is saved for the next run. In a change's view `>` quotes a hunk instead, and `.` still narrows.

## Legibility

A window over the salon (a conversation, the help, the away digest, where you left off) dims everything around it, so the tree and the journal's clipped line ends read as background. The journal pane and a question in the inbox show a message as one row of plain words: cleaned, lines joined, heading and list marks off, a link by its label. The trace colours by the line's first word: the agent's own text in its vendor colour, tool calls in the info colour, a run by its exit code, bus words in the accent. An open conversation follows the talk: lines that arrive are appended, and the selection follows when it was on the last line.

Everything an agent wrote reaches the screen only through a cleaning that removes control characters and hides secret-looking tokens; nothing is drawn raw.

## See also

- [WAITING FOR YOU](waiting.md)
- [The journal](journal.md)
- [Conversations](conversations.md)
- [The board and the code map](board-and-code-map.md)
- [Keyboard](keyboard.md)
- [Budget and limits](../concepts/budget-and-limits.md)
- [Projects](../concepts/projects.md)
