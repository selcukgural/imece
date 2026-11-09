---
title: A card's screen
description: Everything of one card on one screen - where it stands, who worked on it, its tries and its story - opened with # and the card's number or a word of its title.
---

# A card's screen

A card is one unit of work: posted by the leader (or opened by your message to an idle worker), taken by a worker, finished with a `done`. Its number, `#3`, shows on its worker's row on Home, on the board and in the journal. `#` opens a card's screen: the log tab of its [detail](changes.md#the-detail) (the piece of work is its owner's, else the first who had a part in it), the card's standing, who had a part and its story under the detail's head; `tab` goes to the owner's diff and talk, `esc` comes back to Home. You meet it when you want to know what happened to one task without reading the whole journal.

```text
#1 Shipping cost · shop-api · claude-1 · 27 min · $0.30

diff · talk · log                                                                       tab switches
┌──────────────────────────────────────────────────────────────────────────────────────────────────┐
│shop-api · TO REVIEW · 33 min                                                                     │
│owns   src/Auth/TokenService.cs, src/Catalog/ProductQuery.cs, src/Orders/Money.cs                 │
│                                                                                                  │
│WHO                                                                                               │
│posted    ○ ▎lead                                         coordinating · 33 min                   │
│owner     ! ▎claude-1          #17  Price list cache      ▸ Read th…  $0.30  +6 −3  ↗ localhost:50│
│talked    ! ▎claude-2          #16  Search result ranking ▸ Make th…  $0.26  +2 −1  ↗ localhost:│
│                                                                                                  │
│ATTEMPTS                                                                                          │
│#1 try 1  claude-1  claude-opus-5-5  4 Δ  $0.05  33 min  at work  2026-11-09                      │
│  by model: claude-opus-5-5 $0.04 · claude-haiku-5-5 <$0.01                                       │
│                                                                                                  │
│STORY                                                                                             │
│14:00 ◎ intent     lead posted it: Shipping cost                                                  │
│14:00 ⇄ talk       lead → claude-1 [assign]: #1 Shipping cost                                     │
│14:00 Δ change     changed src/Auth/TokenService.cs                                               │
│14:04 ⇄ talk       claude-2 → claude-1 [ask]: Should zone codes be an enum?                       │
│14:05 ⇄ talk       claude-1 → claude-2 [answer] ↩: No, a separate queue                           │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

## Opening a card

`#` opens the command line with `#` and a menu of cards (at most 30, of open projects). Digits match the start of a number; anything else matches inside a title, case aside, newest first; nothing typed lists every card, newest first. Each item is `#N title` and replaces the whole line; `enter` takes the highlighted card and opens it at once, one key, not insert-then-send. A line that names no card is flashed as `no card #…`. `tab` `⇧tab` `↑` `↓` step through the menu as for every completion.

`#` already writes the card at hand: the zoomed or selected agent's (a helper's is its worker's), the one the selected item in TO DECIDE came on, the one the selected journal line names or first mentions. On a card's screen its own card is no news, so `#` starts empty.

`#N` and `#word` are the help's names for this: `#3` by number, `#manifest` by a word of the title.

## What the screen shows

Under the detail's head (the card and its title, its project, the agent whose piece of work it is, how long and what it cost) and its tab bar:

- The first line: its project, its column in the board's words (TO DO, IN PROGRESS, TO REVIEW, VERIFYING, DONE, or FAILED for one that failed, WITHDRAWN for one the leader withdrew), ` · try 2` from its second try, and its minutes, to its end when done or failed.
- What it edits (its leases), when it has any (demo only; symbol leases are not built live).
- The paths it owns (`owns   src/parser.rs, docs (shared)`), when its `assign` or `post` gave it any ([the paths a card owns](../concepts/messages-and-board.md#the-paths-a-card-owns)); a write outside them is a `change` line of its story, `changed src/other.rs, outside the card's paths`.
- The checks it asks for (`checks api, e2e`), on a project on Lonca ([the checks a card asks for](../concepts/messages-and-board.md#the-checks-a-card-asks-for)).
- The cards it waits on, each with how it stands (`after  #2 ✓, #5 ⧗`): `✓` done, `✕` failed or withdrawn, `⧗` not yet ([cards that wait on others](../concepts/messages-and-board.md#cards-that-wait-on-others)).
- **WHO**: who posted it, its owner, the owner's helpers while it is still the owner's card, then everyone its story says talked about it, each once with the first part it had. They are drawn as [the long row](salon.md#the-long-row), so their state is live.
- **ATTEMPTS**: each try at the card, oldest first, as [the ledger](../concepts/budget-and-limits.md#the-ledger) lists it: the try, the worker, the model, the changes and those sent back, what it cost, how long it took and how it ended, and under it what each model spent of it. Shown once a try spent anything.
- **STORY**: the card's story, one line per event, oldest first, told as the owner had it. Each line has a glyph and a label: `◎` intent, `⚑` assumption, `⚖` decision, `⇄` talk (a message, with its type), `•` evidence (a run and how it ended), `Δ` change, `›` you (what you said about it). A card nobody took yet says `No story yet.`

What of the card waits for you is on Home, where its rows name the card; the journal's lines about it go into `:export` (below).

## Keys on a card's screen

Every key does what it does on the owner's focus: `tab` `⇧tab` turn to its other tabs (its diff, its conversation), `enter` opens its raw stream, `a` `e` `A` `r` decide its open ask, `t` and `v` open its trace or conversation, `@` writes to it, `W` goes to the next item waiting anywhere.

| Key | What it does |
| --- | --- |
| `n` / `p` | the next / previous card of an open project, by number |
| `#` | another card (the menu, starting empty) |
| `esc` | from a trace or diff opened here, back to the card; on the card, back to Home |
| `^o` | back where you were, the card screen being one of the places |
| `:export [PATH]` | the card's screen as a Markdown file |
| `:home` `:salon` | leave the card for Home, or its AT WORK |

Home's fold keys (`h` `l`, `za` `zM` `zR`) do nothing here.

## Export

`:export` on a card's screen (with no conversation open over it) writes `# #3 Parse the manifest`, its project, column and minutes, its leases, `## WHO`, `## STORY` (one item per line), `## To decide` with the open items on it, and `## Journal`: every line of the card, oldest first and none folded. Every time has its day, since the file leaves the screen. The default name is `imece-card-3-20261031-1401.md` in the directory imece started from; a file that is there already is never written over.

## Not here yet

The `#` menu finds cards by number and title only; `H` (`:recall`) searches the words of every journal and trace line. A card's own `Δ` line in the journal (finished, applied) does not name the card, so the card's export has it only through its items to decide.

## See also

- [Messages and the board](../concepts/messages-and-board.md)
- [The board and the code map](board-and-code-map.md)
- [The journal](journal.md)
- [The command line](command-line.md)
- [Keyboard](keyboard.md)
