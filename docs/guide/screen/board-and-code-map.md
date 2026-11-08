---
title: The board and the code map
description: Two windows over the screen - the board with every card in its column, and the code map that shows who edits what once symbol leases exist.
---

# The board and the code map

The team shows the team; the board shows the work, and the code map shows the files. `:board` and `:code` open them in a window over whatever is on the screen; `esc` (or `q`) closes the window. While it is open it takes every key, so `W`, `/` and the rest wait for it to close; `?` opens the help over it. Neither has keys of its own.

## The board

```text
┌ TO DO (2) ──────────┐┌ IN PROGRESS (2) ────┐┌ TO REVIEW (1) ──────┐┌ VERIFYING (0) ──────┐┌ DONE (2) ───────────┐
│#6  Legacy parser    ││#3  Parse the manif… ││#4  Retry budget     ││                     ││#2  Money rounding   │
│#7  Changelog entry  ││    ● claude-1 ▰▰▰▱▱ ││    claude-2         ││                     ││    claude-2         │
│                     ││#5  Retry on timeout ││                     ││                     ││#1  CI cache         │
│                     ││    ● claude-3 ▰▱▱▱▱ ││                     ││                     ││    claude-1         │
└─────────────────────┘└─────────────────────┘└─────────────────────┘└─────────────────────┘└─────────────────────┘
```

Five columns, each titled with its count:

| Column | Cards in it |
| --- | --- |
| TO DO | posted and not yet taken: the leader's `post`, or a card the leader wrote for a worker that is not free yet |
| IN PROGRESS | taken: assigned by the leader or claimed from the board by an idle worker. Under the title, its owner with the owner's state glyph and a progress bar; a working owner's glyph breathes as on the team (`:motion`) |
| TO REVIEW | finished with a `done`, and its change (`Δ`) waits for you in TO DECIDE, newest first: the agent is through, the work is not in yet |
| VERIFYING | nothing fills it live. The demo's merge queue used it |
| DONE | finished, and no change of it waits for you, newest first, with the owner that finished it. Only a card at work shows its owner's state: in the last two columns the owner may be at work on something else |

Each card is `#N` and its title. When several projects are open, the project's name follows in dim. The cards themselves live in the model and in the store, so the board is the same after a restart.

What a card is, how the leader posts and assigns and how a worker claims, is in [messages and the board](../concepts/messages-and-board.md). The board has no keys of its own: it lists; to open a card use `#N` from anywhere, to write to its owner `@name text`.

## The code map

`:code` opens CODE MAP · project · who edits what. It is meant to show one row per file of the project, and on that row one cell per symbol lease held in it:

```text
┌ CODE MAP · shop-api · who edits what ───────────────────────────────────────┐
│src/billing/                                                                  │
│  Money.cs             ● claude-2    Round         ● claude-1   Parse   ⇆ 2  │
│  Invoice.cs           ● claude-2    Total                                   │
│src/parser/                                                                   │
│  Manifest.cs          ● claude-1    Read                                    │
│  Legacy.cs                                                                  │
│3 being edited · 1 files with several agents at once   ● working  ⇆ several agents in one file
└──────────────────────────────────────────────────────────────────────────────┘
```

- Files are grouped under their directory, a dim `dir/` row above each group.
- A lease cell is the holder's state glyph (the team's), its name and the symbol (its last part, `Round` for `Money.Round`). Every cell is padded to the widest name and symbol in the project, capped, so the columns do not jump when a lease comes or goes.
- A file nobody holds is dim with nothing after it. A file several agents hold ends with `⇆ N` in the warning colour.
- The bottom line counts what is being edited (the leases) and the shared files, and words only the glyphs on the map right now: a fixed legend missed the glyphs the map had started to use. `⇆` is explained only when a file is shared.

### What the map shows today

On a project on [Lonca](../concepts/projects.md#on-lonca) the map is what its daemon says is held, read every few seconds: only the files someone holds, each lease in the same cells, with a pending change's leases among them. Someone who is no agent of this project shows dim as `· name`. A lease held shared, which others may hold too where the repository allows it, ends with `(shared)`. With nothing held it says `nobody holds a lease in NAME now`, and a daemon that does not answer clears it. On a project in worktrees agents keep to their own files, so the map says there is nothing to show; `--demo` draws it from its cards.

The map with rows and cells above is what `--demo` shows: its projects carry a file list and its cards lease symbols, so the demo is where the view can be seen at work. See [the demo](../getting-started/demo.md).

## See also

- [Messages and the board](../concepts/messages-and-board.md)
- [A card's screen](cards.md)
- [The team](salon.md)
- [Changes](../concepts/changes.md)
- [Keyboard](keyboard.md)
