# A worker's own to-do list on its row and focus

Backlog item B7 of #55 (2026-11-01, from Warp and openharness; issue
#140): a worker's own to-do list (what its vendor's planning tool holds)
on its focus, with `3/7` on its row.

What the vendors give, probed on 2026-11-01. Claude Code 2.1.316 keeps
tasks: `TaskCreate {subject, description}` answers `Task #N created
successfully: subject`, `TaskUpdate {taskId, status, subject}` answers
`Updated task #N status`, statuses `pending`, `in_progress`, `completed`,
`deleted`; `TodoWrite` is gone (asked for with `--tools`, `init` lists
nothing), and the task tools are not in Claude Code's default `-p` list
either, so they are asked for by name. Codex app-server 0.165 sends
`turn/plan/updated` `{plan: [{step, status: pending | inProgress |
completed}], explanation}` from its `update_plan` tool, the whole list
each time (`item/plan/delta` is the proposed-plan text of plan mode, not
this; it stays opted out); the tool is OFF in an app-server thread unless
the config asks for it (a live probe without it answered without a plan),
`tools.update_plan` is a struct, `-c tools.update_plan.enabled=true` turns
it on (probed live: the plan came), and `config/read` does not echo it
(`tools` echoes `web_search` alone). Gemini CLI 0.65 has `write_todos` (on by
default, `useWriteTodos`, and its own prompt asks for it), kind `other`;
over ACP its `tool_call` carries the title alone, `Set 3 todo(s)` or
`Cleared todo list`, no `rawInput`, and the result's `{todos}` display
is dropped by `toToolCallContent`, so its steps never reach imece.

One event for all: `Event::Plan(Vec<Step>)`, `Step { text, state: Todo |
Doing | Done }`, the whole list in the agent's order, an empty list a
cleared one. Claude's adapter folds its per-task calls into the list
(`Parser::tasks` by Claude Code's number, in flight calls in `pending`
by tool-use id): a task counts once the answer comes, by the number the
answer gives, a refused call and an answer that names no created task
count nothing, `deleted` removes, a status it does not know leaves the
step; a new session id (`/clear`) clears the list before the new session
is said; a resumed session starts with an empty list until the agent
touches it (Claude Code's own list is not read back). The task tools are
in `claude::tools` at every level and every role (they change no file);
a Claude Code whose `init` lacks them is not stopped, the check tolerates
exactly those (`TASK_TOOLS`), and the trace says once "no task tools in
this Claude Code: its to-do list stays empty". Codex's adapter maps the
notification (a status it does not know is not begun); `OVERRIDES` sets
`tools.update_plan.enabled=true` for every session and level, the one
override `check_config` does not verify, since Codex does not echo it and
a thread without the tool only has no list to show (`PLAN_TOOL`; the
ignored `live_codex_reports_its_plan` drives the real Codex). Gemini's allowlist
and policy gain `write_todos` at every level, so the model's call is no
longer denied with a wasted turn, and its row says `write_todos Set 3
todo(s)`; the list itself is a known gap until Gemini's ACP sends `plan`
updates or `rawInput`.

The screen: `Agent.plan` (`model::Step`), set whole by `sessions::apply`,
which traces `plan 3/7 "the step in hand"`; kept past the turn's end,
cleared when the session ends (`Live::end`), when the agent leaves, and
by the demo's stop; not saved between runs (record 19). The row ends its
yielding part with `≡ 3/7` (`salon::plan_spans`, `Agent::plan_count`):
with the fixed parts on the row, the mark takes its seven columns first,
then the time its eight, then the quote what is left, so a narrow pane
drops the quote first, the time next and the mark last; the card's
screen shows it through the same row. The focus (`focus::plan_lines`)
lists TO DO 3/7 and a step a line, `✓` done, `▸` in hand, `○` not begun,
up to `PLAN_ROWS` (8): a longer list folds the done steps at its top
into `✓ 4 done` and ends with `… 2 more`, so the step in hand stays in
view. The `?` legend's fifth line names the mark (six lines still: the
help must fit forty rows).

Mirrors: the demo gives a worker five steps (`Content.steps`, English and
Turkish) when it takes a card, all not begun; `Sim::plan_at` marks a step
done per fifth of its progress bar and the next in hand, traces when a
step completes, keeps the list at its done and clears it at its stop.

Not done: Gemini's steps (above); a step's text on the row (the status
has the activity); the list in the exported Markdown (record 42) and in
the card's screen beyond the row; the context-window fill (B8) is the
next row word.
