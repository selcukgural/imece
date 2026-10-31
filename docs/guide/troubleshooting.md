---
title: Troubleshooting
description: What you see when something goes wrong, why imece does it, and what to do.
---

# Troubleshooting

imece fails closed: when a file, a session or a change is not what it expects, it refuses and says so, rather than mending or guessing. This page lists those refusals and what to do about each. The messages are quoted as the screen or the terminal prints them; `imece: …` on the terminal means imece did not start (exit code 1), a flash is the one-line message above the footer, and the journal keeps what happened.

## Start

### `another imece is running on …; open the other checkout from it with :open`

**What you see.** `imece` stops at once with that line.

**Why.** One imece at a time holds the store (`imece.sqlite.lock`, taken with `flock`). Two writers on one SQLite file and one pot would corrupt each other's view.

**What to do.** Go to the running imece and type `:open PATH` to add the other checkout as a project of its own, with its own leader. If no imece is running (the last one was killed), the lock is released by the OS when its process ended; a lock that stays means a process is still there (`pgrep imece`).

### `…/imece.sqlite: … imece left the file as it was. To start afresh, move it somewhere else …`

**What you see.** `imece: <path>: <reason>`, then `imece left the file as it was. To start afresh, move it somewhere else with its <path>-wal and <path>-shm files, if they are there; the next start makes a new one.` The reason may be `the file was written by a newer imece (schema 11, this one knows 9)`, `file is not a database`, or a row it does not know.

**Why.** imece never moves, mends or writes over a file it cannot read: what it holds may still be wanted, and a newer imece may still read it.

**What to do.** Either build the imece that wrote it (the schema number says it is newer), or move the file and its `-wal` and `-shm` siblings aside to start afresh. Nothing is deleted on its own.

### `no coding agent on PATH (looked for claude, codex, gemini); install Claude Code … and log in`

**Why.** imece drives the vendors' own CLIs and found none. Install one and make sure its program is on the PATH of the shell you start imece from.

### `codex is not on PATH (found claude); install it, or pick the leader with --lead`

**Why.** `--lead` (or your primary from an earlier `p`) names a vendor that is not there. A primary that is missing is also said on the WHO LEADS? screen (`codex is your primary, but not on PATH`), never replaced silently. `:lead ask` forgets a primary.

### `--projects is for --demo` / `--agents: a leader hires at most 16`

**Why.** `--projects`, `--researchers` and `--seed` belong to the simulation; live `--agents` is 0-16 (default 4), `--helpers` 0-8 (default 2). The usage text follows the message.

## Sessions

### `codex-1 could not start: is logged in with apikey, whose cost imece can not count yet; log in with ChatGPT (codex login)`

**Why.** Codex reports tokens, not dollars. On a ChatGPT plan a turn costs the pot nothing and imece says so; with an API key it would have to price the tokens, which it cannot yet, so it refuses rather than book zero for a paid call. `is not logged in; run codex login` is the same check with no account.

**What to do.** `codex login` with a ChatGPT account.

### `gemini-1 stopped: ran on gemini-x, whose price imece does not know; name a model it knows with --model or --tier`

**Why.** With an API key, Gemini turns are booked from a list-price table imece carries; a model not in it cannot be booked, so the session stops instead of spending unseen. With a Google login the turn costs the pot nothing and this does not happen.

**What to do.** `--model gemini:NAME` or `--tier gemini:strong=NAME` with a model the table knows (the `pro`, `flash` and `auto` aliases are known); or log in with Google.

### `claude-1 stopped: tools differ: extra ["WebFetch"], missing []`

**What you see.** The row shows `■ stopped, write to it to start again`; the journal has `claude-1 stopped: tools differ …`, or for Codex and Gemini a line naming what did not match (an MCP server that is not imece's, a hook, a plugin, a sandbox, a mode, a model, an extension).

**Why.** imece asks each vendor for exactly the tools of the role and the confine level, then checks what the session reports it runs with (Claude on every turn, Codex before the thread and its MCP servers before every turn, Gemini before the first turn). Anything else stops the session: a vendor update that adds a tool, a setting that pulls in a server, must be looked at, not worked around.

**What to do.** Read the trace (`t`) for the whole report. If your own vendor setup is what you want loaded, `--confine open` runs the vendor as you have it (the leader still never writes and the sub-agent tool stays off). Writing to the agent starts it again with the same checks. A session that stopped this way is still the one remembered; `:forget @claude-1` makes the next start a fresh one.

### `codex-1 stopped: refused …` with a Codex approval, elicitation or dynamic tool

**Why.** A Codex request imece has no answer for (an approval from a session that may not ask, such as a leader or anything under `--confine strict`; the old protocol's approvals; an elicitation; a dynamic tool call) stops the session rather than being answered blindly. Under `standard` and `open` a worker's approval requests come to WAITING FOR YOU (`⚑`) instead.

### `claude-1: model opus is not available here, using the default`

**Why.** The leader hired by tier and the tier's model is one your account lacks (Claude Code reports that as a failed first turn at zero cost). imece restarts the worker once on the standard model with the same prompts; a model that fails for another reason is not retried.

**What to do.** Nothing, or `--tier strong=NAME` with a model your account has.

### A session that seems stuck, or a wake that goes wrong

**What to do.** `:forget @NAME` (`:f`): the running session ends (its run killed, unfinished prompts kept as interrupted and told at the wake, not run again), what the vendor kept of it is removed (Claude's transcript, Codex's thread, Gemini's chat files), and the agent sleeps until spoken to; its next message starts a new session with the full briefing (`Restarted`). The `:` menu marks it `$`: the briefing is paid again. `--fresh` does it for every remembered agent at start. Refused for an agent with nothing to forget or one that is gone (`claude-1 keeps its session: …`).

### `lead stopped: …` right after `/clear`

**Why.** `/clear` on a vendor without a command line of its own (Codex) ends the session; the next message starts a fresh one. Codex's app server knows only `/compact`, and a second `/compact` with no turn between is refused as a failed turn, the session going on.

## Changes

### `claude-1's change did not apply: error: patch failed: src/csv.rs:41 … conflicts with the checkout in src/csv.rs`

**What you see.** The flash, and the same line in the journal; the `Δ` item stays in WAITING FOR YOU.

**Why.** `a` runs `git apply --index` in your checkout. When your checkout moved on under the patch, imece merges it three-way into a copy of the index first and refuses when that conflicts (`conflicts with the checkout in a, b`); when you have unstaged edits in the same file, git's own `does not match index` is returned as it is and no merge goes over your work. Nothing is half applied, ever: git's three-way on a conflict would leave markers in files, so it never runs unchecked.

**What to do.** Commit or stash your own edits to the file and press `a` again; or `r` with a note (`the checkout moved on in src/csv.rs; rebase your worktree`): a clean worktree follows your HEAD before the worker's next prompt, so a worker told to redo the part usually comes back with a patch that fits. `x` lets you apply the hunks that do fit and send the rest back.

### `a` refused with `reading` or `applying`

**Why.** Git runs on one thread, one job at a time: `a` while that worker's newer `done` is still being read, or `a`/`r` while an apply of it is under way, is refused rather than racing. Wait for the journal line.

### `no run on this tree` on every change

**Why.** Only imece's own `run` tool counts as evidence, tied to the exact tree; a vendor's own shell does not show (imece does not see which tree it ran on). Workers are told to check with `run` after their last edit. A run that changed the tree (a formatter) counts for none.

### A `done` that brings no change: the patch is over 4 MiB

**Why.** A patch over 4 MiB is refused when read and the worker is told in the answer to its `done`. Ask it to leave generated files out or split the work.

## Worktrees

### `claude-1 could not start: …/worktrees/…/claude-1 is gone, but git still has it as a worktree of …: whatever it held that was not committed is lost; \`git worktree prune\` there lets imece make it anew`

**Why.** The worktree directory was removed by hand but git's list still has it. Git's own advice is `add -f`, which would throw away what git knows of it; imece leaves that to you.

**What to do.** In the checkout, `git worktree prune`, then write to the agent again.

### `… is there but is not a worktree of …`

**Why.** Something else sits where the worker's worktree should be. imece never reuses or replaces a directory it did not make. Move it away.

### `claude-1: worktree kept at 3f9a1c2, it has uncommitted changes (HEAD is 8b20d44)`

**Why.** Worktrees are kept between runs so work is never thrown away. A worktree found clean at an older commit is moved to your HEAD when its agent starts and between turns; one with uncommitted changes or commits of its own stays where it is (`… it has commits of its own …`) and the row shows `↓N` for the commits it lacks. The worker's `done` is read against its own HEAD, and the apply is three-way, so it usually still fits.

**What to do.** Nothing, usually. If a worktree is a leftover you no longer want (a closed project's: `closed NAME: its agents left; their worktrees stay on disk`), remove it yourself with `git worktree remove <path>` from the checkout, or `git worktree prune` after deleting the directory; imece removes no worktree on its own. They live under `~/.local/state/imece/worktrees/` (or `$XDG_STATE_HOME/imece/worktrees/`), one directory per project. Each has its own `target/`, so they take space.

## Keys and the terminal

### `⇧enter` sends the message instead of starting a new line

**Why.** Most terminals send `⇧enter` as a plain `enter`; only one that speaks the kitty keyboard protocol tells them apart (kitty, Ghostty, WezTerm with `enable_kitty_keyboard`, foot, Alacritty, iTerm2). imece asks at start and the footer says what it found: `⇧enter new line` or `\ enter new line`.

**What to do.** Type `\` and then `enter`: the `\` turns into a new line. `⌥enter` works where Option is Meta. Under tmux or screen the query may not get through, so the fallback applies there too.

### `⌥b` types `∫` (macOS)

**Why.** The terminal sends Option as a character, not as Meta. iTerm2: Profiles → Keys → Left Option key: *Esc+*. Terminal.app: *Use Option as Meta key*. `^←` `^→` step by word too, without Meta.

### A paste did nothing, and the footer says `a paste goes into the line you write: / or @ first`

**Why.** Bracketed paste is on: a paste is one event and goes into the line whole, never sent by its line ends. While you are not writing, its characters would be read as keys, so it is dropped and the footer says so.

### The screen is dark text on a dark background, or the selection is hard to see

**Why.** The `terminal` theme asks your terminal for its colours at start (OSC 10/11, with a cursor-position report so a late answer never leaks into the keys). A terminal that does not answer is taken as dark, or as `COLORFGBG` says; one that answered light once is read again when the window regains focus (tmux needs `focus-events on`). Without `COLORTERM=truecolor` RGB colours are rounded to 256.

**What to do.** `T` cycles the built-in themes; `:theme dark`, `:theme light` or a scheme (`:theme nord`) assume instead of asking; `:theme FILE` takes a Ghostty or kitty colour file (`--theme`, `IMECE_THEME` too). The choice is remembered. Alacritty's TOML and `.itermcolors` are not read; convert them first.

### Copies of imece in iTerm2's scrollback after a resize

**Why.** iTerm2's *Save lines to scrollback in alternate screen mode* copies an erased alternate screen into the scrollback. imece erases row by row and turns auto-wrap off while it has the screen, so this should not happen on current builds; if it does, that option is where to look.

### `3` opened the code map and `/` the composer by themselves at start

**Why.** A terminal's late answer to the colour probe was once read as keys (Terminal.app answered after the probe's first deadline). The probe now waits for a cursor-position report, so this is fixed; a report of it on a current build is a bug.

## The pot and the machine

### Agents stop taking turns; the journal says `at $4.20/h the budget lasts about 25 min more; :budget USD raises it`

**Why.** The pot is one budget for the whole team, kept between runs; once spent, nobody takes a turn. The header's rate is the last ten minutes' cost. Under half an hour left, the budget turns the attention colour and the journal says so once.

**What to do.** `:budget 80` raises it while agents run; a turn under way finishes on its old cap, the next starts on the new remainder. A session that hit its own cap is started again with the same prompts, once.

### `could not keep the machine awake: …`

**Why.** `--awake`/`:awake` runs `caffeinate` (macOS) or `systemd-inhibit` (Linux, needs systemd) as a child; a program that does not start or ends by itself is said once in the flash and not tried again that run. Elsewhere there is nothing to run. A `kill -9` of imece can leave `systemd-inhibit`'s lock until its `sleep` is killed; `caffeinate -w` follows imece's pid by itself.

### `not saved for the next run: …`

**Why.** A save to the store failed (disk full, the file gone). It is flashed once; the screen goes on, and the next save tries again.

## See also

- [Install](getting-started/install.md)
- [Sessions](concepts/sessions.md)
- [Vendors](concepts/vendors.md)
- [Changes](concepts/changes.md)
- [Budget and limits](concepts/budget-and-limits.md)
- [Themes and language](screen/themes-and-language.md)
- [Security](concepts/security.md)
