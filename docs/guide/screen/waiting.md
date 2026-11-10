---
title: TO DECIDE
description: The upper half of Home - every decision only you can take, changes to review, tool calls to allow, questions to answer - grouped by what deciding costs, and the keys that work through it.
---

# TO DECIDE

TO DECIDE is the upper half of [Home](home.md), grouped by what deciding costs you: permissions (one key each), questions (read and answer), changes (a diff to read), then what you put off. Only decisions that need you land there; everything else stays in the journal. You meet it the first time a worker sends `done`: its change (`Δ`) appears, the header counts `! 1 to decide`, and the terminal's title says `! imece · 1 to decide`. In another window, `:notify bell` rings the terminal's bell and `:notify text` sends a desktop notification (`imece · claude-1 finished: a change waits for you`) when something new has waited five seconds; away from the machine, `:notify push` sends it to your phone through ntfy; off unless you ask, see [`:notify`](../reference/commands.md#notify-offbelltextpush).

```text
 TO DECIDE 1 permission · 1 question · 1 change
 permissions                                                                 one key each
▸! #2   Money rounding rules      run Bash: npm install left-pad                  a  yes   r  no
 questions
 ! #6   Shipping zones            Half-up rounding for Money, or banker's rounding?   1-2  pick   a  answer
 changes                                                                 1 ready to approve
 ! #4   Order validation rules    1 file +2 −1 · ✓ dotnet build, dotnet test
 later
 ? #3   Parse the manifest        later · Keep the old CLI flag or drop it?
```

## What lands there

| Mark | Item | Where it comes from | `a` | `r` |
| --- | --- | --- | --- | --- |
| `Δ` | a change | a worker's `done`: its worktree's whole change against your HEAD, read by imece. The row says the done's first line, the files and lines, and what imece's `run` tool ran on exactly that tree (`✓ ran clean`, `✗ failed`, or `no run on this tree`). | apply it to your checkout, staged | reject it; the composer opens `@worker ` so you say why |
| `⚑` | an ask | a tool call Claude Code would ask permission for. Routine commands (the build, the tests, git's reading commands) run on their own and never get here. The selected ask is shown whole under its row, its lines joined with ` ↵ `, never read as Markdown: what you allow is what runs. | allow | refuse; the agent is told it was your decision |
| `?` | a question | an agent's `send(to: "you", type: ask)`. A newer question of the same agent takes the older's place. | both open the composer `@agent `; the item closes when your answer goes | |
| `»` | a reply | an agent answered you and waits for you to read it | `enter` reads it (the conversation opens) and closes it; `r` dismisses it | |
| `■` | an API error | a turn failed on an error only you can mend: the account (signed out, a refused key, a quota or a bill), or a passing one (overloaded, a server error, a lost connection) after imece's three tries ran out (see [when the API fails](../concepts/sessions.md#when-the-api-fails)). The row has Claude Code's words. | both open the composer `@agent `; the item closes when your message goes, and that message is the agent's next try | |
| `▶` | a setup script | a new worktree found `.imece/worktree-setup.sh` in your checkout with bytes you have not said yes to; the worker started without it. See [a worktree setup script](../concepts/projects.md#a-worktree-setup-script). | yes: it runs now where it was missed, then in each new worktree; `enter` shows its bytes first | no, for this run |
| `▤` | a plan | the leader's `plan` tool before it hires a second worker: the cards it means to assign or post, each with a tier and the paths it owns, and what it expects them to cost. `enter` shows it whole, with the pot beside the cost; the leader's turn is over meanwhile and its mail waits ([the plan](../concepts/messages-and-board.md#the-plan); `:plan off` for a project that trusts its leader). | yes: the leader hires and assigns as planned, until its next plan | no: the plan is dropped and the leader asks you or plans again; `n` writes to the leader instead, and what you write closes the plan unanswered |
| `⊘` | a card at its breaker | a card failed its third try and its leader wanted to hand it again, or (on Lonca) its change came back a third time and its worker was not woken; the row says `failed 3 tries: one more, or it fails for good` or `its change came back 3 times: …`. The leader's mail goes on; the worker's waits ([a card's tries](../concepts/messages-and-board.md#a-cards-tries)). | one more try (the leader may hand it again) or round (its worker goes on) | the card fails for good: nobody hands it again, its leader is told |
| `!` | an issue to close | a card that names an issue landed on Lonca, in a project where [`:closes`](../reference/commands.md#closes-onoff) is off; the row says `close #58? “its title” · card #6 landed`. Nobody waits on it ([the issue a card closes](../concepts/messages-and-board.md#the-issue-a-card-closes)). | imece closes the issue with your `gh` | it stays open, and is not asked about again |
| `▶` | the repository's check | a worker's change was read and your checkout has `.imece/check` with bytes you have not said yes to; the change waits unchecked meanwhile. See [the repository's check](../concepts/changes.md#the-repositorys-check). | yes: it runs now on the changes that wait for it, then on every change read; `enter` shows its bytes first | no, for this run |

The row carries the time, `later ·` when you put it off, and the project's name when several are open. An item of another day carries its date.

A change (`Δ`) can also go back to its worker with notes (`n` in the diff), with what clashes after a refused apply (`b`), or be applied in part (`x` then `a`); see [the change view](changes.md). An item keeps the card its agent worked on when it came, so it shows on that [card's detail](cards.md) too.

## Keys

Home's keys; see [Home](home.md#keys) for all of them.

| Key | What it does |
| --- | --- |
| `↑` `↓` / `j` `k`, `gg` `G`, `gd` | move; the preview follows; `gd` the first row to decide |
| `enter` | open the item: a change's diff, a question's or reply's conversation, a setup script's bytes, an agent's focus for the rest |
| `a` / `r` | approve / allow · send back / refuse the row at hand, as the table above says; a yes moves the cursor down |
| `1`-`9` | on a question that offers answers: pick one |
| `e` / `A` | on an ask: yes for the rest of its agent's turn · yes from now on for the project |
| `s` (`:settle`) | later: the item goes to the `later` group at the end |
| `W` (`:next`) | from anywhere: open the next item to decide |
| `S` (`:defer`) | put this item off and open the next |
| `esc` | from what opened back to Home |

Every row says the whole ask, and the row at hand ends with its keys (`a yes  r no` on a permission or a script, `a approve  r send back` on a change, `1-2 pick  a answer` on a question, `enter open  r dismiss` on a reply); a permission's command stands whole under its row, a question's answers with their digits.

## `W` and `S`: one after the other

`W` works on every screen: the board, the code map, a card's screen, a diff, a conversation, a raw stream, a script's window. It closes what is open over Home, puts the cursor on the next open row to decide and opens it as `enter` would, so `a` and `r` answer it at once. Pressed again it goes on to the item after, and round to the first after the last. Its order is [Home](home.md)'s: permissions first, then questions, then changes (the ones proved clean and inside their card's paths first), then what was put off; within a group, the oldest first. With nothing waiting it says `Nothing needs you right now` and nothing moves.

`S` puts the item at hand off and opens the next one. Put-off items come after all the others, in the order they were put off, and their rows say `later ·` up front. Putting off one already put off moves it to the very end. The list forgets items that close, and it lasts the run only: a fresh start shows everything in age order again.

"At hand" is the item `W` last opened; when you move Home's cursor to a row to decide yourself, that is what is at hand, and `W` goes on from there. "Next" is by the item's place before it was put off, so `S` moves on to what came after it rather than back to the top.

## The `!` on a row

An agent whose item waits shows it on its own row in AT WORK: `? → you` while a question waits, `⚑ asks to run` while an ask does, `answered you` while a reply does, `■ API error` while an API error does; an agent with the status "waiting for you" wears `!` as its glyph. The project's heading in AT WORK counts them in the attention colour, `! 2 waiting`, even when the project is folded.

## Quitting with items open

`q` asks before quitting and counts what quitting now would leave: `quit imece? agents mid-turn: 2, changes not reviewed: 1 · q again quits · s shows them · any other key stays`. Agents mid-turn are the ones spending tokens; changes not reviewed are the open `Δ` items. Asks and questions are not counted here: they are in the title and the header already, and quitting cuts none of them. `q` `s` (`:pending`) opens BEFORE YOU GO with the agents mid-turn, each with what its row says it does, and the changes as Home lists them; there `q` quits at once, `W` opens the next item waiting, any other key goes back. With nothing in flight the question is the plain one. A `:close`, `:forget`, `:archive` or `:clear` still counting down is said too (`:close kule happens now`): quitting lets it act, as its time running out would.

## See also

- [Approvals and questions](../concepts/approvals-and-questions.md)
- [Changes](../concepts/changes.md)
- [The change view](changes.md)
- [The team](salon.md)
- [Keyboard](keyboard.md)
