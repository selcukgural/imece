---
title: A card's screen
description: Everything of one card on one screen - who worked on it, its story, what of it waits for you and its journal - opened with # and the card's number or a word of its title.
---

# A card's screen

A card is one unit of work: posted by the leader (or opened by your message to an idle worker), taken by a worker, finished with a `done`. Its number, `#3`, shows on its worker's row in the salon, on the board and in the journal. `#` opens a card's screen: the main screen's three panes, but for one card only. You meet it when you want to know what happened to one task without reading the whole journal.

```text
┌ #3 Parse the manifest ──────────────────────────────┐┌ WAITING FOR YOU · #3 (1) ─────────────┐
│shop-api · IN PROGRESS · 23 min                       ││Δ 13:58 claude-1 finished: parser (3 files, +41 −6)
│                                                      │└───────────────────────────────────────┘
│WHO                                                   │┌ JOURNAL · #3 (4) ─────────────────────┐
│posted    ◆ ▎lead      claude            coordinating ││14:01 ⇄ lead → claude-1 [answer] ↩: use manifest::read for #3 …
│owner     ● ▎claude-1  #3   Parse the manifest  writing tests
│helper    ● ▎claude-1/1                  thinking…    ││13:57 ⇄ claude-1 → lead [ask]: Which parser for #3? …
│                                                      ││13:55 → lead → claude-1: #3 Parse the manifest
│STORY                                                 ││13:50 ◆ lead posted #3 Parse the manifest
│13:55 ◎ intent      Parse the manifest without panics ││                                       │
│13:57 ⇄ talk        → lead [ask] Which parser? …      ││                                       │
│14:01 ⚖ decision    use manifest::read, legacy later  ││                                       │
│14:10 ◌ evidence    ✓ cargo test                      ││                                       │
└──────────────────────────────────────────────────────┘└───────────────────────────────────────┘
  tab panel  enter focus  n p next / previous card  # another card  esc back  ? help
```

## Opening a card

`#` opens the command line with `#` and a menu of cards (at most 30, of open projects). Digits match the start of a number; anything else matches inside a title, case aside, newest first; nothing typed lists every card, newest first. Each item is `#N title` and replaces the whole line; `enter` takes the highlighted card and opens it at once, one key, not insert-then-send. A line that names no card is flashed as `no card #…`. `tab` `⇧tab` `↑` `↓` step through the menu as for every completion.

`#` already writes the card at hand: the zoomed or selected agent's (a helper's is its worker's), the one the selected item in WAITING FOR YOU came on, the one the selected journal line names or first mentions. On a card's screen its own card is no news, so `#` starts empty.

`#N` and `#word` are the help's names for this: `#3` by number, `#manifest` by a word of the title.

## What the screen shows

Left, in the salon's place:

- The title row: `#3` and the card's title; then its project, its column in the board's words (WAITING, IN PROGRESS, VERIFYING, DONE) and its minutes, to its end when done.
- Its leases, when it has any (demo only; symbol leases are not built live).
- **WHO**: who posted it, its owner, the owner's helpers while it is still the owner's card, then everyone its story says talked about it, each once with the first part it had. They are drawn as the salon's agent rows, so their state is live; `j` `k` select among them and `enter` opens the selected one's focus as in the salon.
- **STORY**: the card's story, one line per event, oldest first, told as the owner had it. Each line has a glyph and a label: `◎` intent, `⚑` assumption, `⚖` decision, `⇄` talk (a message, with its type), `◌` evidence (a run and how it ended), `Δ` change, `›` you (what you said about it). A card nobody took yet says `No story yet.`

Right, the side panels for this card alone:

- **WAITING FOR YOU · #3**: the open items that came on this card. An item keeps the card its agent worked on when it came, so a worker's `Δ` stays with the card it finished after the worker takes the next one.
- **JOURNAL · #3**: the lines that name the card (posted, claimed, assigned) and every said line whose text has `#3` in it (where no digit follows, so `#30` is not `#3`), folded like the journal, newest first; the number is marked in the accent wherever it is named.

## Keys on a card's screen

Every key does what it does on the main screen: `tab` cycles the panels, `enter` on a journal line opens the conversation, `a` `r` answer the selected item, `c` copies, `t` and `v` open the selected agent's trace or conversation, `m` marks, `W` goes to the next item waiting anywhere.

| Key | What it does |
| --- | --- |
| `n` / `p` | the next / previous card of an open project, by number |
| `#` | another card (the menu, starting empty) |
| `esc` | from a focus, trace or diff opened here, back to the card; on the card, back to the salon |
| `^o` | back where you were, the card screen being one of the places |
| `:export [PATH]` | the card's screen as a Markdown file |
| `1` `2` `3` | leave the card for the salon, the board or the code map |

The salon's fold keys (`h` `l`, `za` `zM` `zR`) and `f` do nothing here.

## Export

`:export` on a card's screen (with no conversation open over it) writes `# #3 Parse the manifest`, its project, column and minutes, its leases, `## WHO`, `## STORY` (one item per line), `## Waiting for you` with the open items on it, and `## Journal`: every line of the card, oldest first and none folded. Every time has its day, since the file leaves the screen. The default name is `imece-card-3-20261031-1401.md` in the directory imece started from; a file that is there already is never written over.

## Not here yet

There is no free-text search of the journal; the menu finds cards by number and title only. A card's own `Δ` line in the journal (finished, applied) does not name the card, so it is on the card's screen only through WAITING FOR YOU.

## See also

- [Messages and the board](../concepts/messages-and-board.md)
- [The board and the code map](board-and-code-map.md)
- [The journal](journal.md)
- [The command line](command-line.md)
- [Keyboard](keyboard.md)
