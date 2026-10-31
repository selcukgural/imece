---
title: The board and the code map
description: The two other views of the salon - the board with every card in its column, and the code map that shows who works where once symbol leases exist.
---

# The board and the code map

The salon shows the team; the board shows the work, and the code map shows the files. `2` (`:board`) and `3` (`:code`) open them in the salon's place; `1` (`:salon`) comes back. Both keep the header and the footer, and every key that works everywhere (`W`, `?`, `:` commands, writing) works on them.

## The board

```text
┌ WAITING (2) ──────────┐┌ IN PROGRESS (2) ──────┐┌ VERIFYING (0) ───────┐┌ DONE (3) ────────────┐
│#6  Legacy parser      ││#3  Parse the manifest ││                      ││#2  Money rounding    │
│#7  Changelog entry    ││    ● claude-1 ▰▰▰▱▱   ││                      ││    ○ codex-2         │
│                       ││#5  Retry on timeout   ││                      ││#1  CI cache          │
│                       ││    ● gemini-3 ▰▱▱▱▱   ││                      ││    ○ claude-1        │
└───────────────────────┘└───────────────────────┘└──────────────────────┘└──────────────────────┘
```

Four columns, each titled with its count:

| Column | Cards in it |
| --- | --- |
| WAITING | posted and not yet taken: the leader's `post`, or a card the leader wrote for a worker that is not free yet |
| IN PROGRESS | taken: assigned by the leader or claimed from the board by an idle worker. Under the title, its owner with the owner's state glyph and a progress bar |
| VERIFYING | nothing fills it live. The demo's merge queue used it; live, a finished card goes straight to DONE and its change waits for you in WAITING FOR YOU instead |
| DONE | finished with a `done`, newest first, with the owner that finished it |

Each card is `#N` and its title. When several projects are open, the project's name follows in dim. The cards themselves live in the model and in the store, so the board is the same after a restart.

What a card is, how the leader posts and assigns and how a worker claims, is in [messages and the board](../concepts/messages-and-board.md). The board has no keys of its own: it lists; to open a card use `#N` from anywhere, to write to its owner `@name text`.

## The code map

`3` opens CODE MAP · project · who works where (leases). It is meant to show one row per file of the project, and on that row one cell per symbol lease held in it:

```text
┌ CODE MAP · shop-api · who works where (leases) ─────────────────────────────┐
│src/billing/                                                                  │
│  Money.cs             ● codex-2     Round         ◐ claude-1   Parse   ⇆ 2  │
│  Invoice.cs           ● codex-2     Total                                   │
│src/parser/                                                                   │
│  Manifest.cs          ● claude-1    Read                                    │
│  Legacy.cs                                                                  │
│3 leases · 1 files with several agents at once   ● working  ◐ asking  ⇆ several agents in one file
└──────────────────────────────────────────────────────────────────────────────┘
```

- Files are grouped under their directory, a dim `dir/` row above each group.
- A lease cell is the holder's state glyph (the salon's), its name in its vendor colour and the symbol (its last part, `Round` for `Money.Round`). Every cell is padded to the widest name and symbol in the project, capped, so the columns do not jump when a lease comes or goes.
- A file nobody holds is dim with nothing after it. A file several agents hold ends with `⇆ N` in the warning colour.
- The bottom line counts the leases and the shared files, and words only the glyphs on the map right now: a fixed legend missed the glyphs the map had started to use. `⇆` is explained only when a file is shared.

### What the map shows today

Symbol leases are not built yet. A lease would let several agents work in one file, each on its own symbols, on a shared trunk; imece will copy the lease code from Lonca (`lonca-symbols`, `lonca-daemon`) when agents share files, and own it from then on. Live, every agent works in its own git worktree and hands its change over through the Δ review, so there is no shared trunk and no lease to show: the map says so, `no shared trunk yet: each agent works in its own worktree, so there are no leases to show`.

The map with rows and cells above is what `--demo` shows: its projects carry a file list and its cards lease symbols, so the demo is where the view can be seen at work. See [the demo](../getting-started/demo.md).

## See also

- [Messages and the board](../concepts/messages-and-board.md)
- [A card's screen](cards.md)
- [The salon](salon.md)
- [Changes](../concepts/changes.md)
- [Keyboard](keyboard.md)
