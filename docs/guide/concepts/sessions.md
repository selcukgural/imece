---
title: Sessions and persistence
description: What imece remembers between runs in its SQLite file, how an agent sleeps and wakes, how each vendor's session is resumed or restarted, :forget and --fresh, retention, and the "where you left off" window.
---

# Sessions and persistence

Quit imece and start it again: the same screen comes back, every agent asleep, and the first message to an agent wakes it, in the session it had when its vendor still has it. This page says what is kept, where, and what each vendor can resume.

## The store

Everything imece knows is in one SQLite file, `$XDG_STATE_HOME/imece/imece.sqlite` (`~/.local/state/imece/imece.sqlite` by default), mode `0600`, WAL, yours alone:

- projects (with their checkout, pin and `:copy` list), agents (with model, tier, remembered session), cards and their stories, the journal, the inbox, the trace (the newest 200 lines per agent), mail that waits, the board and its offers, the changes waiting for you (with the runs on their trees), the pot and imece's own paid calls, and preferences (theme, side panel width, the primary vendor for `:lead`).
- A save runs after every step and every key and once more at quit, and writes only what moved: new journal, story and trace lines are appended; a row is written again only when its hash changed. A failed save is flashed once (`not saved for the next run: …`).
- **One imece at a time** has the file (a `.lock` beside it): a second one is refused and told to use `:open` from the first.
- A file imece cannot open or load (not a database, a newer imece's schema, a row it does not know) is refused and left as it was; the message names the file and says how to start afresh (move it away with its `-wal` and `-shm`). imece never backs it up or starts over on its own, since a newer imece may still read it.

## Asleep and awake

At the next start every agent comes back `◦ asleep since the last run, write to it to wake it`: no process and no cost until you write to it, a teammate's waking message arrives, or a card posted in this run is offered to it. Mail and cards that waited through the restart wake nobody.

A turn that was mid-way when imece closed is journaled as `interrupted` and marked on the card. Its prompt is **not run again**; the agent is told about it when it wakes.

## Resume per vendor

Whether a woken agent goes on in its old session or starts a new one depends on what its vendor kept. The journal says which: `goes on in the session it had, with all it knew` or `starts a new session, told where it left off`.

| Vendor | Resumed when | How |
| --- | --- | --- |
| Claude Code | the remembered id is a UUID and the transcript file is where Claude Code keeps it for the agent's directory (`~/.claude/projects/<dir>/<id>.jsonl`) | `claude -p --resume ID` with the same flags. The session reports the whole session's cost, so imece starts counting at what was already booked (the cost floor). The briefing is short: only the cut turn and waiting mail. |
| Codex | the remembered thread id is a UUID | `thread/resume`; the answer must carry the id asked for. A thread whose rollout is gone is refused by Codex before any turn, and imece restarts with the full briefing. A resumed thread keeps the instructions it started with; a role prompt that changed since reaches it only through imece's notes. |
| Gemini CLI | the remembered session id is a UUID | `session/load`; the history is replayed around the answer and passes unread. An id Gemini has no chat of is refused before any turn, and imece restarts. Gemini keeps at most 50 messages per chat, so a long session resumes with its tail; `GEMINI.md` is read again. |

A resume that fails (the vendor says the session is gone, or reports another id) is one restart with the full wake briefing ([messages](../concepts/messages-and-board.md)), never a loop. Codex threads are kept for this (`ephemeral: false`); Codex's global message log stays off.

## `:forget` and `--fresh`

A session imece forgets leaves nothing behind at its vendor:

- `:forget [@NAME]` (`:f`): the running session ends (its run killed, unfinished prompts kept as interrupted and told at the wake), the remembered session goes, the agent sleeps until spoken to, and its next message starts a new session with the full briefing. Refused for an agent with nothing to forget or one that is gone. The journal: `you made claude-1 forget its session`. The `:` menu marks it `$`: the new session pays the whole briefing again.
- `--fresh` does it for every remembered agent when the store is read, journaled per agent.
- What is removed, on a thread of its own: Claude's transcript file; Codex's thread through an app server started for it (`thread/delete`, which also clears Codex's index); Gemini's chat files of that session under `~/.gemini/tmp/<name>/chats/`, and when the agent itself goes (`:close`, a helper leaving), everything under that name plus its private directory. Only a UUID ever reaches a path or a vendor.
- imece's own `/clear`, a model fallback and a renewed session (after `:budget`) forget the replaced session the same way.

## Leftovers: days, retention, preferences

- **Days:** the clock starts at the local time of the world's first start. The journal and a conversation draw a dim `─ 2026-11-01` row above the first line of another day; an item in WAITING FOR YOU from another day carries its date; copied and exported text is always stamped.
- **Retention:** at every start journal lines older than 90 days that lie past the newest 5000 are deleted, and `VACUUM` runs when a quarter of the file or more is free. Stories and inbox rows stay. A tidy that fails is tried again next start.
- **Preferences** (theme, side panel, `:lead`) live in the store's `meta` table, readable before the store is taken, so a demo beside a live imece shares them.

## Where you left off

A start that remembers an earlier run opens one window before anything moves, WHERE YOU LEFT OFF · 3 h 20 min ago:

```text
 WHERE YOU LEFT OFF · 3 h 20 min ago
 $12.40 of $50.00 spent; every agent sleeps until spoken to
 imece: 4 agents, 2 open cards
 2 wait for you:
   Δ claude-1 finished: parser tokens (3 files, +34 −8)
   ? codex-1 asks you: keep the old CLI flag?
 turns cut short when imece closed: gemini-1
 the last lines before:
   14:02 claude-1 finished: 3 files, +34 −8 → waits for you
   14:05 codex-1 → you: ask keep the old CLI flag?
 any key: close
```

It shows the pot, one line per open project, the oldest open items (6 at most, then `… and N more`), the turns the earlier run cut short and the earlier run's last 6 journal lines. Any key closes it and does nothing else. Nothing to tell, no window; the demo never has one. The same ground is covered in [next morning](../scenarios/next-morning.md).

## See also

- [How imece works](../concepts/how-imece-works.md): the store and the git thread.
- [Files](../reference/files.md): every path imece writes.
- [Budget and limits](../concepts/budget-and-limits.md): the pot persists.
- [Troubleshooting](../troubleshooting.md): a store that will not open, a second imece.
