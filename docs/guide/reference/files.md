---
title: Files
description: Everything imece writes or reads on disk - the SQLite store and its schema, the worktrees, Gemini's private directory, the bus's sockets, exports, the copy list, what the vendors keep of a session, the lock, and what --fresh and :forget remove.
---

# Files

imece keeps what it remembers in one directory, the state directory, and leaves the rest where the vendors keep it. You meet these paths when you want to know what a worktree holds, where an export went, what a second imece complains about, or what `--fresh` removes. Nothing here is written to the repository you work on; your checkout is touched only when you press `a` on a change.

## The state directory

`$XDG_STATE_HOME/imece` when the variable is set and absolute, else `~/.local/state/imece`, else (no `HOME`) `imece/` under the system's temporary directory. Everything below that is not marked otherwise lives there.

| Path | What it is |
| --- | --- |
| `imece.sqlite` | the store: everything imece remembers between runs (0600, WAL) |
| `imece.sqlite-wal`, `imece.sqlite-shm` | SQLite's write-ahead log and its index; move them with the file |
| `imece.sqlite.lock` | the lock one imece holds while it runs (0600, `flock`) |
| `screen` | the file imece kept its screen choices in before 2026-10-30; moved into the store and removed the first time it is seen |
| `worktrees/<dir>-<hash>/<agent>` | one git worktree per worker, `<dir>` the checkout's directory name and `<hash>` eight hex digits of its path (`worktrees/kule-3f2a9c01/claude-1`) |
| `worktrees/<dir>-<hash>-gemini/<agent>` | a Gemini agent's private directory (`lead` for the project's leader, whatever the project is called); holds `GEMINI.md` and `policy/imece.toml` |

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
| `meta` | key-value: `schema_version`, `started_at` (the world's first start, which the clock and the ticks count from), `budget` (the pot), `saved_at` (the last save, for "WHERE YOU LEFT OFF · 2 h ago"), `messages` (the last message number given), and your settings `side` (the side panel's width), `theme`, `lead` (the primary vendor) |
| `projects` | each project's name, leader, whether it is closed, its checkout path, whether it is pinned, and the paths `:copy` lists (`copies`, JSON) |
| `agents` | each agent's project, name, vendor, role, parent (a helper's worker), card, cost, taint, whether it is gone, its model and whether a tier chose it, the message-driven turns and posts since you last spoke, its own command list, and its last vendor session with what that session had spent |
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

A save runs after every step and every key and once more at quit, and writes only what moved: journal, story and trace lines are appended, a row of a growing table is written again only when its hash changed. A failure to save is flashed once: `not saved for the next run: …`.

### Retention and VACUUM

Once at every live start, after the load: journal lines older than **90 days** that lie past the newest **5000** (the ones a start loads, whatever their age) are deleted, except lines you marked; sent lines past the newest **500** go too; and when the file's free pages are a quarter of it or more, `VACUUM` writes it anew. Stories stay with their cards, inbox items stay (both are numbered by position). A tidy that fails is ignored and tried at the next start; its result is shown nowhere. There is no retention yet for the stories of long-closed projects or for closed inbox items.

### The lock

`imece.sqlite.lock` is held with `flock` for the whole run, so one imece at a time has the file. A second one is told, at exit code 1: `another imece is running on /Users/me/.local/state/imece/imece.sqlite; open the other checkout from it with :open`. A lock that is only being let go of (an imece that is quitting) is waited for a moment. Your settings (`side`, `theme`, `lead`) are read and written through a connection of their own without that lock, so the theme is read before the store is taken and a demo beside a live imece may change it.

## Worktrees

Each worker gets `worktrees/<dir>-<hash>/<name>`, a git worktree of your checkout, detached at your HEAD when it is made. Helpers work in their worker's worktree; the leader works in your checkout and has none. A worktree is kept between runs: one found clean at an older commit is moved to your HEAD when its agent next starts and between turns; one with uncommitted changes or commits of its own stays where it is, and the journal says so either way (`claude-1: worktree moved from a1b2c3d to HEAD e4f5a6b`, `… kept at a1b2c3d, it has uncommitted changes (HEAD is e4f5a6b)`). A directory that is there but is not a worktree of this checkout is refused, never reused or replaced; one removed by hand that git still lists is refused too, with `git worktree prune` as the way out, and the agent stops with that reason.

- An applied change is committed in the worktree as a mark, author `imece <imece@localhost>`, so the worker's next `done` shows only what came after, and the worktree still follows your HEAD once you commit. The row's `⎇ branch ↑own ↓checkout +N −M` says where it stands.
- Reading a change, where a worktree stands, and a run's before-and-after tree go through private index files in the worktree's git directory, named `imece-index`, `imece-tree-<pid>-N`, `imece-stand-<pid>-N`; the agent's own index is never touched. A three-way apply tries the patch first in a copy of your checkout's index, `<git dir>/imece-apply-index`, removed after.
- Each worktree builds in its own `target/`: a shared one would make the workers rebuild each other's changes in turn.
- `:close` leaves the worktrees on disk; nothing removes them. `:copy` puts the ignored paths you list (`.env`, `target`) into a worktree that lacks them at its worker's start, as an APFS clone or reflink where the volume can, never over what is there, at most 2 GiB per start. See [changes](../concepts/changes.md) and [projects](../concepts/projects.md).

## Gemini's private directory

Gemini CLI has nothing like Claude Code's `--restricted` and takes settings from `~/.gemini` and from the `.gemini` directory of the folder it runs in, so imece runs each Gemini agent in a directory of its own, `worktrees/<dir>-<hash>-gemini/<agent>` (0700, next to the worktrees, kept between runs so the session can be resumed), and gives it the checkout or worktree as an included directory. Written fresh at every start:

| File | What it is |
| --- | --- |
| `GEMINI.md` | the agent's instructions (its role, then a line naming its files' directory), the only way ACP has to give them; Gemini's `/memory list` must show it |
| `policy/imece.toml` | the policy that allows exactly the tools of the role and the `--confine` level and imece's own tools by name, and under `strict` and `standard` denies every other tool (a denied tool is not even shown to the model); `--policy` names the directory |

The repository's `.gemini` settings, hooks and `.env` never reach the session. The directory goes when the agent leaves whole (`:close`, a helper's `done`); `:forget` removes only the chat files. See [vendors](../concepts/vendors.md).

## The bus's sockets

At each start imece makes `$TMPDIR/imece-<pid>-<hex>/` (0700; an existing directory is refused, not reused) and binds one Unix socket per agent in it, `a0.sock`, `a1.sock`, …, in agent order, at hire time. A vendor's CLI connects through `imece mcp-relay <socket>`. The directory and its sockets are removed when imece ends, also on SIGTERM, SIGHUP and SIGINT. `:commit`'s one-off Claude call runs in this directory, where there is nothing to read.

## Exports

`:export [PATH]` writes the open conversation or the card's screen as Markdown: `imece-conversation-YYYYMMDD-HHMM.md` or `imece-card-N-YYYYMMDD-HHMM.md`, in the directory imece was started from when no `PATH`, in `PATH` when it is a directory, else as the file `PATH`. A file that is there is never written over. Every time in it has its day; every line went through the same cleaning as the screen (control characters removed, secret-looking tokens hidden). See [commands](commands.md).

## What imece reads from the repository

- `AGENTS.md`, else `CLAUDE.md`, at the checkout's top: the notes for coding agents, at most 16 KiB, passed to every agent with its instructions (Codex reads `AGENTS.md` itself). See [message types](message-types.md).
- Git: the top of the checkout, HEAD, `git check-ignore` for `:copy`, `git diff --cached` and the eight newest subjects for `:commit` (with `--no-ext-diff --no-textconv`, so no program of yours runs), and `git ls-files` (tracked, and untracked but not ignored; the first 20,000) for completing a file's name in a message: at the project's opening, after each change you apply, and every minute while one of its agents is mid-turn.
- A colour file named with `--theme FILE` or `:theme FILE`, in Ghostty's or Kitty's format; its absolute path is remembered as the theme.

Nothing else: no file of the repository is written by imece, and the vendors' CLIs are started so that nothing of the repository loads (under `standard` and `strict`).

## What the vendors keep

A session is resumed from what its vendor kept; imece only remembers the id (`agents.session`). These are the vendors' files, not imece's.

| Vendor | Where | Notes |
| --- | --- | --- |
| Claude Code | `$CLAUDE_CONFIG_DIR` or `~/.claude`, `projects/<slug>/<id>.jsonl`, the slug being the agent's directory with every character that is not a letter or a digit as `-` | the transcript; a session is resumable while the file is there, from the same directory. Other traces (`debug`, `file-history`, `todos`) are not looked for |
| Codex | `$CODEX_HOME` or `~/.codex`, `sessions/YYYY/MM/DD/rollout-<time>-<id>.jsonl` | the thread's rollout, kept because imece starts threads non-ephemeral; `history.persistence="none"` keeps imece's turns out of Codex's global message log. Codex loads your own `~/.codex` config (a private home was rejected: no login) |
| Gemini CLI | `$GEMINI_CLI_HOME` or `~/.gemini`, `tmp/<name>/chats/session-<time>-<id8>.jsonl` | `<name>` comes from Gemini's registry `projects.json`, keyed by the private directory; a second, empty `session-*.jsonl` per start is Gemini's own stray; at most 50 history messages per chat |

Your own `~/.claude/CLAUDE.md`, `~/.gemini/GEMINI.md` and user-level hooks still apply where the vendor loads them whatever the directory; `--confine open` loads your whole setup.

## What `--fresh` and `:forget` remove

Both make an agent forget its vendor session so the next message starts a new one, told where it left off; `--fresh` does it for every remembered agent at the start, `:forget @NAME` for one. What goes, on a thread of its own (the trace says how it went: `forgot session <id>: its transcript was removed`):

| Vendor | Removed |
| --- | --- |
| Claude Code | the transcript file above (`it had no transcript left` when it was gone already) |
| Codex | the thread, through an app server started for it (`thread/delete`, 30 s cap), which also takes it out of Codex's own index; deleting the rollout file by hand would not |
| Gemini CLI | the chat files of that session under `tmp/<name>/chats/` (`N chat files were removed`), and 0-byte strays; when the agent leaves whole, everything under that name plus the private directory |

Only an id shaped like a UUID ever reaches a path or a vendor. A session that was just asked to stop is given 4 s to be gone first. Nothing of imece's own is removed: the agent, its cards, the journal and the worktree stay. Also removed when a session is replaced: imece's `/clear` on a vendor without one of its own, a model fallback, a session renewed after it hit its cap. See [sessions](../concepts/sessions.md).

## What is not written

- No log file and no trace file: the raw stream is the `trace` table (200 lines per agent), and the journal is the `journal` table.
- `!cmd` output stays on your terminal's main screen; its end (the last 64 KiB per project, for `O`) is kept in memory only and gone when imece ends.
- `:commit`'s call runs with `--no-session-persistence`: nothing under `~/.claude/projects`, nothing to resume.
- The demo writes nothing but your screen settings (`side`, `theme`, `lead`) into the store.
- The vendors' rate-limit windows shown in the header are not kept; a session says them again on its first turn.

## See also

- [Sessions](../concepts/sessions.md): what is remembered and how an agent wakes
- [Command line](cli.md): `XDG_STATE_HOME`, `--fresh`, `--theme`
- [Commands](commands.md): `:export`, `:copy`, `:forget`, `:open`, `:close`
- [Security](../concepts/security.md): why the checkout is touched only by `a`
- [Troubleshooting](../troubleshooting.md)
