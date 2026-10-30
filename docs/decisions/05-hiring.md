# Hiring

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
