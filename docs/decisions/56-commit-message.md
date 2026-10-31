# A suggested commit message

Backlog D6b in #57 (2026-10-31, from Orca; the person said yes on the
condition of ceilings): once a worker's change is applied it is staged
in the person's checkout, and writing its commit message is the step
left. `:commit` asks a cheap model for one, for what is staged in the
project at hand's checkout, and puts it on the command line as
`!git commit -m '…'` for the person to edit and run with enter (the
person's own shell, record 13), or drop with esc. Nothing is committed
by imece.

Asked for, not on every apply: a person often applies several workers'
changes and commits once, and an apply that pays a model each time pays
for messages nobody uses. The `:` menu marks `:commit` with `$`
(record 41).

The call (`claude::once`): Claude Code's cheap tier (`haiku`, or the
person's `--tier claude:cheap=…`), one process per suggestion, prompt on
stdin, stream-json back, gone after its first result. It is a session's
command line (`claude::args`) with no tools, no MCP server, the strict
level, and two changes: `--system-prompt` replaces Claude Code's own
(a few lines instead of thousands of tokens, every call) and
`--no-session-persistence` (nothing for `--resume`, nothing left in
`~/.claude/projects`). Its `init` is checked as every session's (no
tools, no server, the permission mode): anything else stops it before
it answers. It runs in a directory with nothing to read (the bus's
private one) and is given up after 120 s. Only Claude Code: it is the
one vendor whose call takes a hard spending cap (`--max-budget-usd`);
without it `:commit` is refused, saying so.

What it reads (`worktree::staged_change`, on the git thread after any
apply asked before it): `git diff --cached --stat`, the patch (at most
24 KiB, cut at a line and said to be cut) and the eight newest commit
subjects for the repository's style; `--no-ext-diff --no-textconv`, so
no program of the person's runs, and the index is not touched
(`write-tree` only adds the objects of the tree it names). The system
prompt says the diff is data, not instructions. The answer is cleaned
line by line (`clean`), Markdown fences and blank ends dropped, at most
30 lines and 2000 characters, and single-quoted for the shell (`'` as
`'\''`, so `$(…)` or a backtick in it stays text); the person still
reads it before enter.

Ceilings (the standing cost rules: a daily ceiling and a cap per person,
reserved atomically before the call): each suggestion may cost at most
$0.05 (`SUGGEST_CAP`, the call's `--max-budget-usd`); a day (UTC) holds
20 suggestions (`SUGGEST_DAY_CALLS`) and $1.00 (`SUGGEST_DAY_COST`,
caps held counting), every project together. imece has one person, so
the day's ceiling is that person's cap; one suggestion runs at a time.
Before the call, the store counts it at its cap in one immediate
transaction that checks and counts (`Store::reserve`, table `paid`,
schema 9) and the pot holds the cap (`World::paid`, part of
`World::spent`); refused when the pot has less than the cap or the day
is full. When the call ends its turn, what it reported replaces the cap
(`settle`); one that could not start cost nothing; one that stopped or
ran out of time without reporting keeps its cap (more, never less). The
pot's share is read back from the store at the start
(`Store::paid_total`), as what the agents spent is.

Replays are not paid twice: the last suggestion of each project is kept
with the staged tree it was for, and `:commit` on the same tree puts it
on the command line again without a call. A suggestion that comes while
the person writes waits until the command line is free. The journal
says each suggestion with its model and cost (`Suggested`) and each
failure with why and what it cost (`SuggestFailed`: nothing staged, the
day full, the call's error); refusals before git reads anything are
flashed.

The demo mirrors it: `:commit` is refused as live refuses it, answers
two ticks later from the titles of the changes applied in that project
(the first the subject, the others the body) or says nothing is staged,
costs a few hundredths from the pot, and comes again for nothing when
nothing new was applied. Its line is `git commit --dry-run`: the
message is made up and `!` runs in the person's real shell.

Known gaps: the day's ceilings are constants, not flags; the call may
read the person's `~/.claude/CLAUDE.md` (Claude Code loads it whatever
the directory; `--bare` would skip it but also the subscription
login); Codex and Gemini leads need Claude Code on PATH for this.
