# Hiring

**Changed by [record 109](109-claude-only.md): Claude Code only; what is below about choosing a vendor is history.**

Hiring (2026-10-29, user's decisions): bare `imece` is live and the team
starts as the leader alone; the leader hires workers with the `hire`
tool (leader's socket only) as the work needs them, `--agents` is the
cap (0-16, default 4), the sim sits behind `--demo`. The person never
picks a team size. Workers are named `<vendor>-N`; a socket is bound per
agent at hire time, before the agent exists. No budget share per agent
(dropped 2026-10-29: $5 / 5 places gave an Opus leader $1, less than one
task). Models: the leader runs on its vendor's default (or
`--model`); the leader picks a TIER for a worker (cheap / standard /
strong), never a model name; each vendor maps tiers to its models
(Claude: haiku / default / opus), `--tier T=NAME` overrides. A tier model
that fails its first turn at zero cost (that is how Claude Code reports
a model the account lacks: init still succeeds, the first result is an
`api_error`) is replaced once by the standard model with the same
prompts, journaled as `Fallback`. Vendors are probed on PATH at start
(`claude`, `codex`, `gemini`); only vendors imece has an adapter for are
hireable, others give an honest error.

**A worker's start point (backlog E12 in #58, user's decision
2026-11-03).** The leader's `hire` takes `from`: a branch, tag or commit
of the checkout, or `#N` for pull request N of `origin`
(`worktree::valid_start`: one word, no option, no range, at most
`START_MAX` 200; `#` and 1–9 digits). A branch, tag or commit is resolved
at once in the checkout (`worktree::resolve`: `rev-parse --verify
--end-of-options X^{commit}`), so a wrong name is refused before anyone
joins; `#N` is fetched when the worker starts (`git fetch --no-tags
origin refs/pull/N/head`, `GIT_TERMINAL_PROMPT=0`, then `FETCH_HEAD`),
and a failed fetch stops the worker with git's words. A helper's `hire`
has no `from`: it works in its worker's worktree. `Agent.start` keeps
the point as asked (schema 17, `agents.start`); `worktree::ensure` makes
such a worktree at that commit (`Placed::From`, traced `worktree made
at abc1234`) and on later starts leaves it where it is (`Placed::Own`);
`Live::follow` skips it. Its Δ is what it holds against its own HEAD, as
any worker's, and applying it stays the three-way apply. The journal
says `lead hired claude-2 (haiku), starting from #41` (`Event::Hired`'s
`from`), the row `⎇ #41` when the agent made no branch (`stand_spans`;
`↑ ↓` still count against the checkout's HEAD), and the worker's
instructions say where it started. The demo's second worker of a
project starts from `#41` and is never moved. Not done: a solo's start
point, choosing it from imece's command line, other forges' pull
request refs (GitLab's `merge-requests/N/head`).

The person makes a worker too, from a worker: `:fork` (record 94) adds
one named as `hire` names workers (`worker_name`, shared), counted in
`--agents`, its start point the forked worker's HEAD.
