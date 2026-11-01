# Neighbours

- **voff** (github.com/orhun/voff, MIT/Apache-2.0, by ratatui's maintainer)
  is the closest competitor: lead + team, Codex/Claude/Ollama, ratatui TUI
  with a node-graph team map, SQLite state, team generated from a repo scan.
  It drives Claude Code and Codex as stateless completion engines (fresh
  process per turn, built-in tools off, history re-sent, one tool call per
  turn), splits work by disjoint paths (overlap means serial), and a model
  reviews every patch and command. imece differs on purpose: real agent
  sessions over ACP with a message bus between peers, symbol leases in the
  same file, the person's review of each worker's diff as the last gate
  (no model reviews patches or commands, and imece sandboxes nothing: the
  vendor's own restrictions, checked fail closed, and a container when
  isolation is wanted), many projects at once, a calm screen.
- Taken from voff (deep review 2026-10-28, be322a2), to build when the
  matching part exists:
  - Done needs evidence: one entry per acceptance criterion citing what
    proves it, and a passing check after the last change; a later change
    voids the check.
  - Rejections say whether new evidence or a different action is needed;
    an identical action after a rejection is refused without a model call.
  - Repeated unchanged reads (path + range + file digest) are caught; the
    second repeat stops the agent and asks its leader.
  - Writes carry the base digest they were made against and are refused if
    the file moved on.
  - Passing test lines are dropped and repeats collapsed before test output
    reaches an agent.
  - An interrupted review or decision is denied on restart (fail closed);
    a state change and its event are written in one transaction.
  - Child processes: argv only, cleared environment plus an allowlist,
    prompt on stdin (never argv), byte caps on every pipe, process-group
    kill. A tool call is acted on only after the vendor's final event;
    streamed text is display only.
  - Before driving a vendor CLI, check its effective config as the vendor
    reports it, not the flags we passed; fail closed.
  - Onboarding: find vendors on PATH, scan the repo without sensitive
    files, propose a team through a forced tool call (here per-agent vendor
    and model, edited in place before it starts).
  - Screen: headline glyphs per event kind, speaker-coloured "A → B"
    threads, display-only folding with nothing lost, a compact approval
    line that opens into route / action / request / preview, a per-agent
    token and cost table, composer history per workspace and drafts per
    view.
- Rules that come from what voff lacks: hard caps on turns, revisions and
  money per agent and per project, reserved before each call; an empty
  scope is nothing, never everything; no decision of a model widens a
  grant; redraw on change, never on a timer; history kept per agent, no
  global cap; Esc never quits and nothing modal blocks typing; colours
  fixed per agent id.

## The study of 2026-10-31

Six more neighbours were read on 2026-10-31 for what the screen and the
team could take; each idea became a line of the backlog (#54–#61, A–H),
named after where it came from. What each is, as its own page says, and
what of it is built (the decision records say how):

- **openharness** (github.com/autonomous-ai/openharness, MIT): a command
  center for running and watching many coding agents (Claude Code, Codex,
  OpenCode) across machines, with a desktop app and a CLI. Taken: the
  waiting count in the terminal title (A1), the away digest (A3), the
  next-waiting key (A4), the state ladder (B1), the fleet count (B3), each
  row saying what its agent does (B4), the to-do list (B7), readline keys
  (C2), the jump line (C1), the key file (C9, C9b, C9d), the limits line
  (G1).
- **Warp** (warp.dev): a terminal with AI agents, workflows and blocks
  built in. Taken: notifications outside the window (A2), the back key
  (A5), the quit question (A7), branch and ahead/behind on a row (B5),
  attention order (B6), the to-do list (B7), context fill (B8), the jump
  line (C1), `:` names and their values (C4), sent-line history (C5),
  turns and bookmarks (C6, C6b), multi-line writing (C10), file completion
  (C11), `:export` (C14), quoting a hunk and a command's output (C15,
  C15b), the welcome and the news (C19), the theme following the terminal
  (C21), a command's exit and time (C22), notes on diff lines, parts of a
  change, the file list (D1, D2, D3), its file folds (D3b), the undo grace
  (C23), the key file (C9, C9b, C9d).
- **Orca** (onorca.dev): an open-source desktop IDE around coding agents,
  each in its own git worktree, with terminals, diffs and a browser.
  Taken: notifications outside the window (A2), keeping the machine awake
  (A6), notes on diff lines (D1), the file list and wrap of a change (D3),
  its file folds (D3b), the kinds of its files and the commit message (D6,
  D6b), ignored files copied into worktrees (E13), the jump line (C1),
  pins (C17), the key file and its places (C9, C9b, C9d), the limits line
  (G1).
- **Terax** (terax.app): an open-source desktop workspace around a
  terminal, with an editor, git tools, agents that propose reviewable
  edits and a live preview of the dev server. Taken: a worker's local URL
  as a link (B9), notifications outside the window (A2), file completion
  (C11), parts of a change (D2), drawing less in another window (C18).
- **herdr** (herdr.dev): an open-source app for running coding agents,
  whose agents run in a background server and keep working when it is
  closed. Taken: the waiting count in the title (A1), notifications
  outside the window (A2), the state ladder (B1), done-not-seen (B2),
  branch on a row and attention order (B5, B6), the jump line (C1),
  readline keys (C2), the help's filter (C3), copy mode (C7), peek and
  settle (C16), the welcome (C19), the theme following the terminal (C21),
  reply ids on mail (F3).
- **cmux** (cmux.com): an open-source macOS terminal on Ghostty's engine,
  with vertical tabs, splits, a scriptable browser and notifications when
  an agent needs attention. Taken: the next-waiting key and the back key
  (A4, A5), notifications outside the window (A2), keeping the machine
  awake (A6), the quit question (A7), the state ladder (B1), each row
  saying what its agent does (B4), `:` names (C4), turns and bookmarks
  (C6, C6b), hint letters (C8), multi-line writing (C10), pins (C17),
  reply ids on mail (F3).

What the backlog left out on purpose is crossed out there with the
person's reason (E3, E9).
