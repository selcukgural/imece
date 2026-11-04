---
title: Files
description: Everything imece writes or reads on disk - the SQLite store and its schema, the worktrees, the bus's sockets, exports, the copy list, what Claude Code keeps of a session, the lock, and what --fresh and :forget remove.
---

# Files

imece keeps what it remembers in one directory, the state directory, and leaves the rest where Claude Code keeps it. You meet these paths when you want to know what a worktree holds, where an export went, what a second imece complains about, or what `--fresh` removes. Nothing here is written to the repository you work on; your checkout is touched only when you press `a` on a change.

## The state directory

`$XDG_STATE_HOME/imece` when the variable is set and absolute, else `~/.local/state/imece`, else (no `HOME`) `imece/` under the system's temporary directory. Everything below that is not marked otherwise lives there.

| Path | What it is |
| --- | --- |
| `imece.sqlite` | the store: everything imece remembers between runs (0600, WAL) |
| `imece.sqlite-wal`, `imece.sqlite-shm` | SQLite's write-ahead log and its index; move them with the file |
| `imece.sqlite.lock` | the lock one imece holds while it runs (0600, `flock`) |
| `screen` | the file imece kept its screen choices in before 2026-10-30; moved into the store and removed the first time it is seen |
| `worktrees/<dir>-<hash>/<agent>` | one git worktree per worker, `<dir>` the checkout's directory name and `<hash>` eight hex digits of its path (`worktrees/kule-3f2a9c01/claude-1`) |
| `worktrees/<dir>-<hash>-notes/<agent>` | `CLAUDE.md` (0600, the directory 0700): what the agent's Claude Code reads besides the file of its own directory, rewritten at every start. See [below](#what-an-agent-reads) |
| `worktrees/<dir>-<hash>-scratch/<agent>` | a writing agent's scratch directory (0700; a solo's is `solo-<project number>`): for notes, logs and downloads that are no part of its change. Its file tools may write there (`--add-dir`), its instructions name it, and its focus shows it. Kept between runs; removed by `:forget`, `--fresh` and when the agent leaves (`:close`, a helper's `done`), not when a new session goes on with the same work (a model fallback, a renewed cap, `/clear`) |
| `scripts/<sha256>.sh` | the private copy of a worktree setup script you said yes to, the bytes you read (0700 directory, 0600 file): what runs. See [a worktree setup script](../concepts/projects.md#a-worktree-setup-script) |

## The store

One SQLite file, yours alone (made 0600 before SQLite opens it), for every project. Schema version **13**; the version is in `meta`, and a file from a newer imece is refused. A file imece cannot open or load (not a database, a newer schema, a row it does not know) is refused and left as it was; the message names the file and says how to start afresh:

```text
/Users/me/.local/state/imece/imece.sqlite: <why>
imece left the file as it was. To start afresh, move it somewhere else with its
/Users/me/.local/state/imece/imece.sqlite-wal and /Users/me/.local/state/imece/imece.sqlite-shm
files, if they are there; the next start makes a new one.
```

imece never moves, mends or writes over a file it cannot read: a newer imece may still read it.

### Tables

| Table | Holds |
| --- | --- |
| `meta` | key-value: `schema_version`, `started_at` (the world's first start, which the clock and the ticks count from), `budget` (the pot), `saved_at` (the last save, for "WHERE YOU LEFT OFF · 2 h ago"), `messages` (the last message number given), `setup_approved` (the worktree setup scripts you said yes to: how each runs, its bytes' SHA-256 and its real path), and your settings `side` (the side panel's width), `theme`, `notify`, `motion`, `seen` (the version that started last, for what is new) |
| `projects` | each project's name, leader, whether it is closed, its checkout path, whether it is pinned, and the paths `:copy` lists (`copies`, JSON) |
| `agents` | each agent's project, name, role, parent (a helper's worker), card, cost, taint, whether it is gone, its model and whether a tier chose it, the message-driven turns and posts since you last spoke, its own command list, its last session with what that session had spent, its start point, and for a fork not yet started the worker it goes on from (`fork_of`) |
| `cards` | each card's project, title, owner, column and when it started and finished |
| `story` | a card's story lines, as JSON events with their kind |
| `journal` | every journal line (tick, project, agent, kind, the event as JSON) and whether you marked it (`marked`, with a partial index so marked lines are found whatever their age) |
| `inbox` | every item that waited for you, open or closed, with the agent and the card it came on |
| `trace` | each agent's raw stream, the newest 200 lines |
| `mail` | messages that wait unread for an agent, with their number and the one they reply to |
| `notes` | imece's notes to an agent ahead of its next prompt |
| `inflight` | the prompts an agent had in hand when imece closed; journaled as cut short at the next start, not run again |
| `posts` | the body of each board card and who posted it |
| `offers` | which workers each card was offered to |
| `changes` | each worker's change waiting for you: its worktree path, tree, files, patch and the runs on that tree |
| `paid` | imece's own paid calls by day (UTC): how many and what they cost (`:commit`) |
| `sent` | the lines you sent, with their tick, for `↑` `↓` and `^r` (`:history`); the newest 500 load |
| `recall`, `recall_rows` | the full-text index `H` and `:recall` search: the folded words of every journal and trace line (no copy of the text), and where each line is and when it was written; a line deleted takes its rows with it, and a file from before it is indexed once at its first open |

A save runs after every step and every key and once more at quit, and writes only what moved: journal, story and trace lines are appended, a row of a growing table is written again only when its hash changed. A failure to save is flashed once: `not saved for the next run: …`.

### Retention and VACUUM

Once at every live start, after the load: journal lines older than **90 days** that lie past the newest **5000** (the ones a start loads, whatever their age) are deleted, except lines you marked; sent lines past the newest **500** go too; and when the file's free pages are a quarter of it or more, `VACUUM` writes it anew. Stories stay with their cards, inbox items stay (both are numbered by position). A tidy that fails is ignored and tried at the next start; its result is shown nowhere. There is no retention yet for the stories of long-closed projects or for closed inbox items.

### The lock

`imece.sqlite.lock` is held with `flock` for the whole run, so one imece at a time has the file. A second one is told, at exit code 1: `another imece is running on /Users/me/.local/state/imece/imece.sqlite; open the other checkout from it with :open`. A lock that is only being let go of (an imece that is quitting) is waited for a moment. Your settings (`side`, `theme`, `lead`) are read and written through a connection of their own without that lock, so the theme is read before the store is taken and a demo beside a live imece may change it.

## Worktrees

Each worker gets `worktrees/<dir>-<hash>/<name>`, a git worktree of your checkout, detached at your HEAD when it is made. Helpers work in their worker's worktree; the leader works in your checkout and has none. A worktree is kept between runs: one found clean at an older commit is moved to your HEAD when its agent next starts and between turns; one with uncommitted changes or commits of its own stays where it is, and the journal says so either way (`claude-1: worktree moved from a1b2c3d to HEAD e4f5a6b`, `… kept at a1b2c3d, it has uncommitted changes (HEAD is e4f5a6b)`). A directory that is there but is not a worktree of this checkout is refused, never reused or replaced; one removed by hand that git still lists is refused too, with `git worktree prune` as the way out, and the agent stops with that reason.

- An applied change is committed in the worktree as a mark, author `imece <imece@localhost>`, so the worker's next `done` shows only what came after, and the worktree still follows your HEAD once you commit. The row's `⎇ branch ↑own ↓checkout +N −M` says where it stands.
- Reading a change, where a worktree stands, and a run's before-and-after tree go through private index files in the worktree's git directory, named `imece-index`, `imece-tree-<pid>-N`, `imece-stand-<pid>-N`; the agent's own index is never touched. A three-way apply tries the patch first in a copy of your checkout's index, `<git dir>/imece-apply-index`, removed after; finding what clashes after a refused apply does the same in `<git dir>/imece-clash-<pid>-N`.
- Each worktree builds in its own `target/`: a shared one would make the workers rebuild each other's changes in turn.
- `:close` leaves the worktrees on disk; nothing removes them. `:copy` puts the ignored paths you list (`.env`, `target`) into a worktree that lacks them at its worker's start, as an APFS clone or reflink where the volume can, never over what is there, at most 2 GiB per start; so does what the checkout's `.worktreeinclude` names among the paths git ignores (read with `O_NOFOLLOW`, at most 16 KiB). See [changes](../concepts/changes.md) and [projects](../concepts/projects.md).

## The bus's sockets

At each start imece makes `$TMPDIR/imece-<pid>-<hex>/` (0700; an existing directory is refused, not reused) and binds one Unix socket per agent in it, `a0.sock`, `a1.sock`, …, in agent order, at hire time. Claude Code connects through `imece mcp-relay <socket>`. The directory and its sockets are removed when imece ends, also on SIGTERM, SIGHUP and SIGINT. `:commit`'s one-off Claude call runs in this directory, where there is nothing to read.

## Exports

`:export [PATH]` writes the open conversation or the card's screen as Markdown: `imece-conversation-YYYYMMDD-HHMM.md` or `imece-card-N-YYYYMMDD-HHMM.md`, in the directory imece was started from when no `PATH`, in `PATH` when it is a directory, else as the file `PATH`. A file that is there is never written over. Every time in it has its day; every line went through the same cleaning as the screen (control characters removed, secret-looking tokens hidden). See [commands](commands.md).

## What imece reads from your configuration

In `$XDG_CONFIG_HOME/imece/`, or `~/.config/imece/` (a relative `XDG_CONFIG_HOME` is ignored):

- `keys`: your own keys, read at start and again when it changes. See [keyboard](../screen/keyboard.md).
- `push`: where `:notify push` sends what waits (ntfy's server, topic and the file of a token), read when push is chosen and at each start. See [`:notify`](commands.md#notify-offbelltextpush).
- `instructions.md`: your own instructions for every project, whole, for imece's agents alone; `~/.claude/CLAUDE.md` is for every Claude Code session. Read at each agent's start, a change at its next one; only a plain file: a pipe or a device is passed over. See [below](#what-an-agent-reads).

## What imece reads from the repository

- `CLAUDE.md`, else `AGENTS.md`, at the checkout's top, when the agent's own directory has no `CLAUDE.md`. See [below](#what-an-agent-reads).
- Git: the top of the checkout, HEAD, `git check-ignore` for `:copy`, `git ls-files --others --ignored --directory --exclude=…` for what `.worktreeinclude` matches (at a worker's start), `git diff --cached` and the eight newest subjects for `:commit` (with `--no-ext-diff --no-textconv`, so no program of yours runs), and `git ls-files` (tracked, and untracked but not ignored; the first 20,000) for completing a file's name in a message: at the project's opening, after each change you apply, and every minute while one of its agents is mid-turn.
- A colour file named with `--theme FILE` or `:theme FILE`, in Ghostty's or Kitty's format; its absolute path is remembered as the theme.

Nothing else: no file of the repository is written by imece.

## What an agent reads

Under `standard` and `strict`, `--restricted` would load no `CLAUDE.md` at all. imece turns Claude Code's own loader back on for two directories (`CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1`, `--add-dir`):

- **The agent's own directory**, its worktree or, for a leader, the checkout: `CLAUDE.md`, `.claude/CLAUDE.md`, `.claude/rules`, and their `@imports` inside the directory. A worker reads them as its worktree has them, so a change to `CLAUDE.md` reaches it once committed and its worktree moved. Hooks, settings, skills and commands there stay out.
- **Its notes directory** above, a file imece writes at every start, whole and in this order: your `~/.claude/CLAUDE.md` (`$CLAUDE_CONFIG_DIR` first), `instructions.md`, and the checkout's `CLAUDE.md` or `AGENTS.md` when the loader would find nothing in the agent's own directory (git ignores the file, or the worker started where it is not). A file there is a copy, so an `@import` in your `~/.claude/CLAUDE.md` is not followed. A link out of the checkout is not read.

An `@import` leaving those directories is not loaded. `CLAUDE.local.md` is not read at any of these levels. Under `--confine open` Claude Code reads your whole setup and the agent's directory itself, and the notes directory holds only what it would not find: `instructions.md` and the repository's fallback. `:commit` and `:review` read no notes: they answer about one diff, by the token.

## What Claude Code keeps

A session is resumed from what Claude Code kept; imece only remembers the id (`agents.session`). These are Claude Code's files, not imece's.

| Where | Notes |
| --- | --- |
| `$CLAUDE_CONFIG_DIR` or `~/.claude`, `projects/<slug>/<id>.jsonl`, the slug being the agent's directory with every character that is not a letter or a digit as `-` | the transcript; a session is resumable while the file is there, from the same directory. Other traces (`debug`, `file-history`, `todos`) are not looked for |

## What `--fresh` and `:forget` remove

Both make an agent forget its session so the next message starts a new one, told where it left off; `--fresh` does it for every remembered agent at the start, `:forget @NAME` for one. What goes, on a thread of its own (the trace says how it went: `forgot session <id>: its transcript was removed`):

What goes: the transcript file above (`it had no transcript left` when it was gone already).

Only an id shaped like a UUID ever reaches a path or Claude Code. A session that was just asked to stop is given 4 s to be gone first. The agent's notes directory goes, and its scratch directory too, moved aside at once so a session started right after gets a new one (`its scratch directory was removed`). Nothing else of imece's own is removed: the agent, its cards, the journal and the worktree stay. Also removed when a session is replaced: a model fallback, a session renewed after it hit its cap. See [sessions](../concepts/sessions.md).

## What is not written

- No log file and no trace file: the raw stream is the `trace` table (200 lines per agent), and the journal is the `journal` table.
- `!cmd` output stays on your terminal's main screen; its end (the last 64 KiB per project, for `O`) is kept in memory only and gone when imece ends.
- `:commit`'s call runs with `--no-session-persistence`: nothing under `~/.claude/projects`, nothing to resume.
- The demo writes nothing but your screen settings (`side`, `theme`, `lead`) into the store.
- Claude Code's rate-limit windows shown in the header are not kept; a session says them again on its first turn.

## See also

- [Sessions](../concepts/sessions.md): what is remembered and how an agent wakes
- [Command line](cli.md): `XDG_STATE_HOME`, `--fresh`, `--theme`
- [Commands](commands.md): `:export`, `:copy`, `:forget`, `:open`, `:close`
- [Security](../concepts/security.md): why the checkout is touched only by `a`
- [Troubleshooting](../troubleshooting.md)
