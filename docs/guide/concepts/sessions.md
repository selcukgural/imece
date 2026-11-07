---
title: Sessions and persistence
description: What imece remembers between runs in its SQLite file, how an agent sleeps and wakes, how a session is resumed or restarted, :forget and --fresh, retention, and the "where you left off" window.
---

# Sessions and persistence

Quit imece and start it again: the same screen comes back, every agent asleep, and the first message to an agent wakes it, in the session it had when Claude Code still has it. This page says what is kept, where, and what can be resumed.

## The store

Everything imece knows is in one SQLite file, `$XDG_STATE_HOME/imece/imece.sqlite` (`~/.local/state/imece/imece.sqlite` by default), mode `0600`, WAL, yours alone:

- projects (with their checkout, pin and `:copy` list), agents (with model, tier, remembered session), cards and their stories, the journal, the inbox, the trace (the newest 200 lines per agent), mail that waits, the board and its offers, the changes waiting for you (with the runs on their trees), the pot and imece's own paid calls, and preferences (theme, notify, motion, story, review).
- A save runs after every step and every key and once more at quit, and writes only what moved: new journal, story and trace lines are appended; a row is written again only when its hash changed. A failed save is flashed once (`not saved for the next run: …`).
- **One imece at a time** has the file (a `.lock` beside it): a second one is refused and told to use `:open` from the first.
- A file imece cannot open or load (not a database, a newer imece's schema, a row it does not know) is refused and left as it was; the message names the file and says how to start afresh (move it away with its `-wal` and `-shm`). imece never backs it up or starts over on its own, since a newer imece may still read it.

## Asleep and awake

At the next start every agent comes back `◦ asleep since the last run, write to it to wake it`: no process and no cost until you write to it, a teammate's waking message arrives, or a card posted in this run is offered to it. Mail and cards that waited through the restart wake nobody.

A turn that was mid-way when imece closed is journaled as `interrupted` and marked on the card. Its prompt is **not run again**; the agent is told about it when it wakes.

## Resume

Whether a woken agent goes on in its old session or starts a new one depends on what Claude Code kept. The journal says which: `goes on in the session it had, with all it knew` or `starts a new session, told where it left off`, with why when it had one: `its last one was not found` or `its last one could not be resumed (…)` with Claude Code's words.

| Resumed when | How |
| --- | --- |
| the remembered id is a UUID and the transcript file is where Claude Code keeps it for the agent's directory (`~/.claude/projects/<dir>/<id>.jsonl`) | `claude -p --resume ID` with the same flags. The session reports the whole session's cost, so imece starts counting at what was already booked (the cost floor). The briefing is short: only the cut turn and waiting mail. |

A resume that fails (Claude Code says the session is gone, or reports another id) is one restart with the full wake briefing ([messages](../concepts/messages-and-board.md)), never a loop.

## When the API fails

Claude Code tries a failed API call again by itself first, up to ten times (three for an overloaded API); each try is a line in the agent's trace (`api error 529 overloaded: retry 2/10 in 1163 ms`). Only when it gives up does the turn fail, and imece looks at why:

| Claude Code says | imece |
| --- | --- |
| overloaded, a server error (5xx), a lost connection, a rate limit | a passing error: the journal says `claude-1 hit a API error: … · imece tries again in 30 s (1/3)`, the row counts down `↻ 1/3 in 25 s`, and when the time comes the session is told to carry on where it stopped. The second try is 2 min after the next failure, the third 8 min after that. After the third, it waits for you. |
| signed out, a refused key, a used-up quota, a bill, a plan's usage limit | the account's: it waits for you at once, as `■` in [TO DECIDE](../screen/waiting.md). |
| anything else (a refused request, a full context) | the journal says the turn stopped, as before. |

The session lives on through a failed turn, and Claude Code keeps the failed prompt in it, so a try sends a short "carry on", never the prompt again. A good turn, your message, or mail to the agent starts the count again; a session that ends takes a waiting try with it. In the demo, every third thing you tell an agent fails on an overloaded API and is tried again, every seventh on a refused key.

## `:forget` and `--fresh`

A session imece forgets leaves nothing behind:

- `:forget [@NAME]` (`:f`): the running session ends (its run killed, unfinished prompts kept as interrupted and told at the wake), the remembered session goes, the agent sleeps until spoken to, and its next message starts a new session with the full briefing. Refused for an agent with nothing to forget or one that is gone. The journal: `you made claude-1 forget its session`. The `:` menu marks it `$`: the new session pays the whole briefing again.
- `--fresh` does it for every remembered agent when the store is read, journaled per agent.
- What is removed, on a thread of its own: Claude Code's transcript file. The agent's scratch directory goes too (see [files](../reference/files.md)). Only a UUID ever reaches a path.
- imece's own `/clear`, a model fallback and a renewed session (after `:budget`) forget the replaced session the same way.

## Archives

When an epic is done, `:archive [TITLE]` puts the project at hand away and opens a fresh one of the same name in its place, on the same checkout ([`:archive`](../reference/commands.md#archive-title)). The archive is kept in the store with everything it had:

- its agents, asleep, each with its remembered session, mail, notes, change and the prompts a turn was cut short in (an archive cuts a turn under way short, as a quit does);
- its cards, its items in TO DECIDE and its journal lines, out of sight: the screen shows only the fresh project.

Nothing of an archive wakes or spends: no card is offered to its workers, nobody reaches its agents by name (the fresh head may have the same one), and a tool call one of them made before the archive is answered `your project was put away; nothing was done`. `--fresh` leaves its sessions alone, and the tidy keeps its journal lines whatever their age. Claude Code removes old transcripts by itself after a while (30 days by default), and a session gone by then is started anew, told where it left off.

`:restore [NAME]` brings one back ([`:restore`](../reference/commands.md#restore-name)): it opens again as it was put away, its head selected, nothing awake. Each agent goes on in its session when you next write to it (resumed if Claude Code still has it), told first that the project was put away and of the turn the archive cut short. The team open on its checkout gives way: archived in its turn when it did anything, closed when it did not.

`A` ([`:archives`](../reference/commands.md#archives)) lists the archives as a tree: each checkout, the projects open on it now, its archives newest first with what they spent, their cards done and their agents; `/` filters them, `r` brings the one at hand back, `enter` opens it read-only without bringing it back: that archive alone, as it was put away, with all the store keeps of its journal, under an `ARCHIVE · …` banner; nothing in it wakes or spends, `esc` goes back.

## Leftovers: days, retention, preferences

- **Days:** the clock starts at the local time of the world's first start. The journal and a conversation draw a dim `─ 2026-11-01` row above the first line of another day; an item in TO DECIDE from another day carries its date; copied and exported text is always stamped.
- **Retention:** at every start journal lines older than 90 days that lie past the newest 5000 are deleted, and `VACUUM` runs when a quarter of the file or more is free. Stories and inbox rows stay. A tidy that fails is tried again next start.
- **Preferences** (theme, notify, motion) live in the store's `meta` table, readable before the store is taken, so a demo beside a live imece shares them.

## Where you left off

A start that remembers an earlier run opens one window before anything moves, WHERE YOU LEFT OFF · 3 h 20 min ago:

```text
 WHERE YOU LEFT OFF · 3 h 20 min ago
 $12.40 of $50.00 spent; every agent sleeps until spoken to
 imece: 4 agents, 2 open cards
 2 wait for you:
   Δ claude-1 finished: parser tokens (3 files, +34 −8)
   ? claude-2 asks you: keep the old CLI flag?
 turns cut short when imece closed: claude-3
 the last lines before:
   14:02 claude-1 finished: change ready · 3 files +34 −8
   14:05 claude-2 → you: ask keep the old CLI flag?
 any key: close
```

It shows the pot, one line per open project, the oldest open items (6 at most, then `… and N more`), the turns the earlier run cut short and the earlier run's last 6 journal lines. Any key closes it and does nothing else. Nothing to tell, no window; the demo never has one. The same ground is covered in [next morning](../scenarios/next-morning.md).

## See also

- [How imece works](../concepts/how-imece-works.md): the store and the git thread.
- [Files](../reference/files.md): every path imece writes.
- [Budget and limits](../concepts/budget-and-limits.md): the pot persists.
- [A new epic](../scenarios/new-epic.md): archiving a finished project.
- [Troubleshooting](../troubleshooting.md): a store that will not open, a second imece.
