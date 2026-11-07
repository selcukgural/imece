---
title: Home
description: View 5 - the triage list - what is to decide, grouped by what deciding costs, then who is at work; every row says the whole ask and the keys that decide it.
---

# Home

Home is the screen imece opens on, the one for deciding; `5` or `:home` comes back to it. One list across the open projects: what is to decide first, grouped by what deciding costs you, then who is at work. Every row says the whole ask, the keys that decide the row at hand stand at its end, and a yes moves the cursor down. From 120 columns on, the row at hand is read beside the list.

```text
 imece  shop-api │  ! 5 to decide  │ $0.49 of $200.00  14:20
 TO DECIDE 1 permission · 1 question · 3 changes
 permissions                                                                           one key each
▸! #2   Money rounding rules      run .imece/check on its changes?                     a  yes   r  no
                                  set -e; cd "$IMECE_WORKTREE"; npm run build; npm test -- --silent
 questions
 ! #6   Shipping zones            Half-up rounding for Money, or banker's rounding?
 changes                                                                         2 ready to approve
 ! #4   Order validation rules    1 file +2 −1 · ✓ dotnet build, dotnet test
 ! #5   Order tests               1 file +2 −1 · ✓ dotnet build, dotnet test
 ! #6   Shipping zones            4 files +8 −4 · ✓ dotnet build, dotnet test · 1 outside #6

 AT WORK 2
 ● #8   Notification settings     Write it up for the leader · 5 min
 ● #7   Bulk e-mail queue         run dotnet test · 12 min
 home · shop-api · #2 Money rounding rules    enter  open   a  yes   r  no   ?  help
```

The [header](salon.md#the-header) names the open projects; the one the list is narrowed to (`[` `]`) is lit. A project on Lonca has a line of its own above the list saying how it stands.

## The list

| Group | What is in it |
| --- | --- |
| TO DECIDE | counts each kind: `1 permission · 2 questions · 3 changes · 1 put off` |
| permissions | one key each: a command an agent asks to run, the worktree setup script, the repository's check. The row says what would run: `run .imece/check on its changes?` and then the script's commands (its lines that are not comments; `enter` shows every byte) |
| questions | read and answer: an agent's question whole, with the answers it offers under it (`1`-`9` picks one, `/` writes your own: [questions](../concepts/approvals-and-questions.md#questions-)), a researcher's report, an answer to read, an API error, the leader's plan |
| changes | a diff to read: `✓ dotnet build · ✓ dotnet test · 2 files +4 −2 · 1 outside #51: Money.cs` — the [verdict](changes.md#what-is-above-the-patch) without its head: what was proved on its tree (the repository's check when it ran, then imece's runs, else `nothing ran`), the files and lines against the paths its card owns, what other waiting change it overlaps, what Lonca runs after your yes. The ready ones (proved clean, inside their paths, overlapping nothing, fitting, whole) come first; the group's line counts them, `2 ready to approve` |
| later | what you put off with `s`, in the order put off |
| AT WORK | `AT WORK 12 · stuck 2 · idle 1`: agents stuck first (`■`, blocked or their session ended), then mid-turn or asking a teammate (`●`, the step they are on or what they do, then how long from a minute on), then idle (`○`). A worker whose change waits and who has gone on to other work is here too: two pieces of work. One waiting on a yes or an answer of yours is that item instead; a leader with nothing to do is not listed |

A row is the piece of work: `!` (or the agent's glyph), its project when several are open and the list is not narrowed, the card and its title, then the ask or the status. An ask wraps under its own column, whole; a change's parts go under each other when the row is short; an agent's status is cut. The agent's name stands in the preview. `W`'s order is this list's: permissions, questions, changes, what was put off.

## The preview

From 120 columns the row at hand is read beside the list without a key: a question whole with its answers and what the asking agent did so far, or an ask's words; a script's lines, numbered; a change's verdict line, why, cost, gate, files and the top of its patch without git's own lines; an agent's status, to-do list and newest trace lines. Below 120 columns, beside an editor, the list takes the width and the rows say it all.

## Keys

| Key | What it does |
| --- | --- |
| `j` `k` / `↑` `↓`, `gg` `G` | move; the preview follows |
| `gd` `gw` | the first row to decide · the first at work |
| `[` `]` | all projects, then each one, round |
| `enter` | open it: a change's diff, a question's conversation, a script's bytes, an agent's focus. `esc` comes back here |
| `a` `r` | yes and no, as in [TO DECIDE](waiting.md); on a change approve and send back; on a question `a` writes back. The row at hand says them at its end |
| `e` `A` | on an ask: yes for this turn · yes from now on |
| `s` | later: the item goes to the `later` group at the end of TO DECIDE and you stay on the list |
| `@` | write to the agent at hand |
| `W` | open the next item, in the list's order |

Every other key works as everywhere; the status line's keys are the row at hand's.

A yes moves down: once `a`, `r`, `e` or `A` acted, the row now in that place is at hand, so the next key decides it. A decision never lands on what you did not read: when the row at hand went by itself before you pressed the key, the key does nothing, a line says so, and the row now in its place is at hand.

The team (with the journal, where a start's notes land), the board, the code map and the timeline stay, one key away. A key file's `[team]` part holds on Home too.
