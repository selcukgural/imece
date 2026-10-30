# Helpers

Helpers (2026-10-29, user's decision): a worker is to imece what an
agent is to Claude Code, so it splits its task the same way: the
worker's socket offers `hire` too, and a worker hires HELPERS (role
`Helper`, `parent` = the worker, named `<worker>.N`), `--helpers N` is
the cap per worker (0-8, default 2). A helper runs in its worker's
worktree (same files; the worker says which), with the edit tools and
send/post/board only (no claim, no hire: no helpers of helpers), is not
offered cards and is not counted against `--agents`. Its `done` to its
worker marks it leaving; it leaves when that turn ends (so the turn's
cost is still reported): session ended, mail dropped, `gone`, journaled
as `Left`. A worker that stops takes its helpers with it. The vendor's
own sub-agent tool stays off the `--tools` list on purpose: a sub-agent
spawned inside the CLI would skip the worktree, the budget, the bus and
the screen. Salon: helpers sit under their worker at depth 3 (`├`/`└`),
the worker line ends with `▾ 2 helpers` / `▸ 2 helpers`; `h`/`l`/`za`
on a worker with helpers (or on a helper) fold its helpers before the
project, `zM`/`zR` fold and unfold workers too.
