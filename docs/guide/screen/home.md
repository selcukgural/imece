---
title: Home
description: The screen imece opens on - the triage list - what is to decide, grouped by what deciding costs, then who is at work; every row says the whole ask and the keys that decide it.
---

# Home

Home is the screen imece opens on, the one for deciding; `:home` comes back to it. One list across the open projects: what is to decide first, grouped by what deciding costs you, then who is at work. Every row says the whole ask, the keys that decide the row at hand stand at its end, and a yes moves the cursor down. From 120 columns on, the row at hand is read beside the list.

```text
 imece  shop-api │  ! 4 to decide  │ $0.89 of $200.00  14:33
 TO DECIDE 1 permission · 3 changes
 permissions                                                                           one key each
▸! #1   Shipping cost             run .imece/check on its changes? set -e; cd      a  yes   r  no
                                  "$IMECE_WORKTREE"; npm run build; npm test -- --silent
 changes                                                                         3 ready to approve
 ! #11  Money rounding rules      ✓ dotnet build · ✓ dotnet test · 1 file +2 −1, inside #11 ·
                                  no overlap with other changes
 ! #12  Search result ranking     ✓ dotnet build · ✓ dotnet test · 1 file +2 −1, inside #12 ·
                                  no overlap with other changes
 ! #13  E-mail templates          ✓ dotnet build · ✓ dotnet test · 3 files +6 −3, all inside #13 ·
                                  no overlap with other changes

 AT WORK 3
 ● #17  Price list cache          Read the code around it · 27 min
 ● #16  Search result ranking     Make the change · 3 min
 ● #7   Payments docs             Write it up for the leader · 5 min

 ○ lead  Three cards are out; claude-2 has the parser. · 2 min        /  write to the lead
 home · shop-api · #1 Shipping cost    enter  open   a  yes   r  no   ?  help
```

The [header](salon.md#the-header) names the open projects; the one the list is narrowed to (`[` `]`) is lit. A project on Lonca has a line of its own above the list saying how it stands.

## The list

| Group | What is in it |
| --- | --- |
| TO DECIDE | counts each kind: `1 stopped · 1 permission · 2 questions · 3 changes · 1 reply · 1 put off` |
| stopped | what stops a whole team, first of all: [when a whole team stops](#when-a-whole-team-stops) |
| permissions | one key each: a command an agent asks to run, the worktree setup script, the repository's check. The row says what would run: `run .imece/check on its changes?` and then the script's commands (its lines that are not comments; `enter` shows every byte) |
| questions | read and answer: an agent's question whole, with the answers it offers under it (`1`-`9` picks one, `/` writes your own: [questions](../concepts/approvals-and-questions.md#questions-)), a researcher's report, an API error, the leader's plan; a worker's block its leader asks you about (the row is the blocked work's: `claude-2 is blocked · lead asks: …`) or let lie (`blocked: … · lead did not solve it`): [a worker that is blocked](../concepts/messages-and-board.md#a-worker-that-is-blocked) |
| changes | a diff to read: `✓ dotnet build · ✓ dotnet test · 2 files +4 −2 · 1 outside #51: Money.cs` — the [verdict](changes.md#what-is-above-the-patch) without its head: what was proved on its tree (the repository's check when it ran, then imece's runs, else `nothing ran`), the files and lines against the paths its card owns, what other waiting change it overlaps, what Lonca runs after your yes. The ready ones (proved clean, inside their paths, overlapping nothing, fitting, whole) come first; the group's line counts them, `2 ready to approve` |
| replies | an agent's closing words (`answered: …`): nothing to decide, so after every decision; `enter` reads it whole, `r` dismisses it. The header counts them apart, `1 to read`, never in `! N to decide` |
| later | what you put off with `s`, in the order put off |
| AT WORK | `AT WORK 12 · stuck 2 · finished 1 · idle 1 · 2 hidden (:unsettle)`: the team, project by project in the order they were opened, pinned ones first. When more than one project is in view (none narrowed to with `[` `]`), each heads its agents with its line from the [team](salon.md): `▾ shop-api  ▓▓░░ 3/8  ! 2 waiting  1 working  $4.10  ~/src/shop-api`; `h` folds it to that line, `l` opens it, `za` toggles, `zM` `zR` fold and open them all. Under it the workers and solos stuck first (`■`, blocked or their session ended), then mid-turn or asking a teammate (`●`, the step they are on or what they do, then how long from a minute on), then idle (`○`), each with its helpers and researcher hung under it (`└`). An idle worker or solo says what became of the work it last had, so one that finished reads apart from one never given anything: `idle · finished, applied` (its change is in), `idle · finished` (done, with no change to apply), `idle · #12 failed`, `idle · #12 withdrawn`, `idle · no work yet`; the heading counts the finished ones apart from the rest of the idle. A row too narrow for it loses how long first, then `idle`. A worker whose change waits and who has gone on to other work is here too: two pieces of work. One waiting on a yes or an answer of yours is that item instead, its helpers standing on their own after the workers; a leader with nothing to do is not listed. `s` on an agent or a heading hides it until it needs you; the heading counts what is hidden. A worker that told its leader it is blocked stays here among those at work, `■` in the attention colour, `blocked "…" · lead is on it`, and is not counted as stuck while its leader has it; so does one whose session ended or whose turn failed (`session ended "…" · lead is on it`, [a worker that stopped](../concepts/messages-and-board.md#a-worker-that-stopped)). A card that waits for what never comes is a row of its own while its leader is on it, after the stuck agents: `■ #21 Import from CSV  waits on #20 which failed · lead is on it` (`enter` opens the card; [a card that waits for what never comes](../concepts/messages-and-board.md#a-card-that-waits-for-what-never-comes)) |

A row is the piece of work: `!` (or the agent's glyph), its project when several are open and the list is not narrowed, the card and its title (a change's own title; an agent with no card, its name), then the ask or the status. An ask wraps under its own column, whole; a change's parts go under each other when the row is short; an agent's status is cut. A title longer than its column takes what a one-line row leaves free after its status (and, at hand, its keys). The agent's name stands in the preview. `W`'s order is this list's: permissions, questions, changes, replies, what was put off.

The list scrolls only when the row at hand would leave the screen: while it shows with a line under it, TO DECIDE and the group lines stay in view, even when new rows come above it. With nothing listed the list says so once, across the whole width, and the status line offers `/ write to the lead`; while no project has a worker, not even an idle one with a change waiting, imece's mark stands in the free rows.

## When the work is done

When every card the leader handed out is through, Home says so on its first line, so you do not work it out row by row. It comes in two steps.

```
 ◐ TEAM FINISHED  shop-api · 5 cards · 3 landed · 2 wait for you · $4.12 · 30 min
```

The team finished: no card waits or is at work, nobody of the project is in a turn, and what is left is yours, counted on the line (a change to read, a question, a card out of tries). TO DECIDE is right under it.

```
 ✓ ALL DONE  shop-api · 5 cards · 4 landed · ✕ 1 failed · $4.12 · 38 min
 Nothing is left for the team or for you. / tells the lead what is next.
```

All done: nothing waits for you either. The project's name in the header gets a `✓`, and when every open project is there the terminal's title reads `✓ imece · all done` (`· 1 failed` when a card failed). In another window, [`:notify`](../reference/commands.md#notify-offbelltextpush) says it once per round: `imece · shop-api: all the work is done, 4 of 5 landed, 1 failed`.

A card that failed or was withdrawn does not hold the line back, and is always counted on it, a failed one in red. The figures are the round's: the cards opened since the project last had none open, what their tries cost (the leader's own spending is not in it) and the time from the first card to the last one's end. A new card ends the state and, after all done, begins the next round. With several projects in view the line counts them together (`2 projects · 9 cards …`) and comes only when every team that has cards is finished; `[` `]` narrow to one. A line too long for the screen goes on under itself, whole. A solo has no such line: nobody hands it cards.

Nothing is said while a card still waits on another whose change waits for your yes: that card has not begun, so the team is not through.

## When a whole team stops

Some stops are no leader's to solve: the pot is spent, the team took its 12 turns on each other's messages since you last spoke, your Claude usage limit is reached, Lonca does not answer, the head itself (the leader, or a solo) stopped: its session ended, or its turn hangs or failed. Each comes straight to you as one stop of its project, on Home's first line and as a row first in TO DECIDE, with who waits behind it and imece's own choices as digits.

```
 ■ STOPPED  shop-api · the pot is spent · 3 wait behind it · 2 min

 TO DECIDE 1 stopped · 1 change
 stopped
▸■      shop-api                  the pot is spent · $200.00 of $200.00 · 3 wait behind it
                                  1  raise by $50.00   2  raise by $20.00   3  rest here
```

| Stop | When | Choices |
| --- | --- | --- |
| `the pot is spent` | the pot is spent and something waits on it: mail not delivered, a wake or a retry not taken, a message the bus refused | `1` raise it by $50, `2` by $20, `3` rest here (the row goes after everything else waiting, the line stays); `:budget USD` for another amount |
| `the team took 12 turns on messages since you last spoke` | an agent took its 12 turns on its teammates' messages and something waits on it: a wake, a message the bus refused | `1` let it go on (12 more); writing to anyone of the team does the same |
| `Lonca does not answer` | the project is on Lonca and its daemon does not answer, is signed out or the repository is frozen (the line says why) | `1` look again now, `2` open the [Lonca window](../concepts/projects.md#on-lonca) |
| `your Claude usage limit is reached` | a turn met the plan's usage limit; the line says when it `opens 17:00` (from Claude Code's own windows), else when `imece tries again at 17:15` (15 minutes on) | `1` try again now; at that time imece tries the turns it cut by itself. While it holds no mail is delivered, no wake or retry is taken and no card offered; your own word still goes. One line for the team, not an item per agent |
| `lead's session ended` | the head's session ended (a leader's, or a solo's); its workers can not reach it | `1` start it again where it left off |
| `lead's turn gives no sign of life` | the head is mid-turn and its newest trace line is as old as [`:health stall`](../reference/commands.md#health-stallidleloop-value) (10 minutes while it is off); the line says its last call (`last: Bash "cargo test"`) | `1` end the turn: its session ends, cutting the turn, and resumes where it left off, told not to run what hung the same way again |
| `lead's turn failed` | the head's turn failed with nothing imece tries again, its session alive; until its next turn | `1` tell it to go on, in the session it has |

The preview names who waits behind it and with what (`2 messages for it, not delivered`, `its retry after an API error`, `its turn, tried again when the limit opens`, `can not reach lead`) and what each choice does. `enter` opens what stopped (the Lonca window, else the leader's work), `s` lets it rest like `3`. The header marks the project with `■` and counts the stop in `! N to decide`; the tab shows stuck. In another window [`:notify`](../reference/commands.md#notify-offbelltextpush) says it once, loud: `imece · shop-api stopped: the pot is spent`. The journal says when a team stopped and when it goes on again (`shop-api: the team goes on after 4 min: the pot has room again`).

imece lifts none of these by itself: it never raises the pot, lets the team go on or starts the head unasked. The usage limit is the one it waits out: when it opens, the turns it cut are tried again, and a turn that goes through ends the stop. An agent whose turn the limit cut says so on its row, `■ usage limit · again in 12 min`. A leader whose session ended is the stop's row, not a stuck row among those at work.

## The leader's line

A leader has no row among those at work, so the leader of the project at hand has a line of its own under the list, always in the same place: its glyph and name, what it does right now while it is mid-turn, else the first line of its newest words to you and how long ago (`nothing said yet` before its first), and at the right the key that writes to it. `/` opens [your conversation with it](conversations.md#your-conversation-with-an-agent) with the line to write on under it. The row at hand decides whose leader it is; a solo has no such line (it has a row), nor has a project put away. Its words are looked for in the journal's newest 2,000 lines.

## The preview

From 120 columns the row at hand is read beside the list without a key: a question whole with its answers and what the asking agent did so far, or an ask's words; a script's lines, numbered; a change's verdict line, why (the worker's words as the Markdown they are, its lists as lists), cost, gate, files and the top of its patch without git's own lines; an agent's status, to-do list and its newest trace lines, the newest last, as the trace window follows them. Below 120 columns, beside an editor, the list takes the width and the rows say it all.

## A small terminal

imece works on a phone over ssh. Under 80 columns each row stands stacked: the card and its title on one line, the ask, the verdict or the status under it, whole; an agent at work keeps one line while it fits. The row has no keys at its end then: the status line has them, and on any screen it keeps every key before it says where you are. The help's words wrap to the screen, a piece of work's head line breaks after its title, and a conversation takes the whole width.

Under 40 columns or 12 rows imece shows only the terminal's size and its own least, and no key acts but `ctrl-c`: nothing is decided on a screen that cannot show it. Make the window larger (or the font smaller) and everything is back as it was.

## Keys

| Key | What it does |
| --- | --- |
| `j` `k` / `↑` `↓`, `gg` `G` | move; the preview follows |
| `gd` `gc` `gw` | the first row to decide · the first change, under the permissions and the questions (`no change waits for you` when none does) · the first at work |
| `[` `]` | all projects, then each one, round |
| `enter` | open it: a change's diff, a question's conversation, a script's bytes, an agent's focus, a project's leader. `esc` comes back here |
| `a` `r` | yes and no, as in [TO DECIDE](waiting.md); on a change approve and send back; on a question `a` writes back. The row at hand says them at its end |
| `e` `A` | on an ask: yes for this turn · yes from now on |
| `s`  `:settle` | later: the item goes to the `later` group at the end of TO DECIDE and you stay on the list; on an agent's row or a project's heading: hidden until it needs you ([the team](salon.md#settling-what-is-finished); `:unsettle` brings everything back) |
| `h` `l`  `za` `zM` `zR`  `:fold` `:unfold` | fold the row at hand's project to its heading · open it · toggle · every project · none; only while the projects head their rows |
| `/` `i` | write to the leader of the project at hand, your conversation with it above the line |
| `@` | write to one agent: its name first (`tab` completes it); once the name is whole your conversation with it shows above the line |
| `W` | open the next item, in the list's order |

Every other key works as everywhere; the status line's keys are the row at hand's.

A yes moves down: once `a`, `r`, `e` or `A` acted, the row now in that place is at hand, so the next key decides it. A decision never lands on what you did not read: when the row at hand went by itself before you pressed the key, the key does nothing, a line says so, and the row now in its place is at hand.

Nothing else shares the screen: the [journal](journal.md) (where a start's notes land) is a window, `J` or `:journal`, and so are the [board, the code map](board-and-code-map.md) and the [timeline](timeline.md), opened by name (`:board`, `:code`, `:timeline`); `esc` closes them. A [key file](keyboard.md#your-own-keys)'s `[salon]` place is Home's.
