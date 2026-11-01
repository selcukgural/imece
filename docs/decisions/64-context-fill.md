# How full each agent's context window is

Backlog item B8 of #55 (2026-11-01, from Warp; issue #142): the
context-window fill per agent, where the vendor reports it.

What the vendors give, probed on 2026-11-01. Claude Code 2.1.316: every
`assistant` message carries `usage` (`input_tokens`,
`cache_creation_input_tokens`, `cache_read_input_tokens`, `output_tokens`),
the request's prompt being the context so far, and the `result` carries
`modelUsage[model].contextWindow` (1,000,000 for haiku 5.5 here), so the
size is known only from the first turn's end. Codex app-server 0.165:
`thread/tokenUsage/updated` has `last` (the last request's breakdown,
`totalTokens`, `reasoningOutputTokens`) and `modelContextWindow`
(nullable). Gemini CLI 0.65 over ACP: `usage_update { used, size }` after
each model response, `size` from its own token-limit table (1,048,576 for
the current models).

One event for all: `Event::Context { used, size: Option<u64> }`, the
tokens in the context and the window's size when the vendor says. Claude's
adapter sums the message's usage (the prompt read from the cache or not,
and the answer) and keeps the window from the last result (the largest
named, in case a turn ran on more than one model), says the fill again
when the window becomes known, and says an empty context when the session
id changes (`/clear`). Codex's adapter takes the last request's total less
its reasoning (the parts summed when there is no total). Gemini's takes
the meter as it is, a size of nothing being no size.

The screen: `Agent.context: Option<Fill>` (`Fill::percent`: none without a
size, at most 100), set by `sessions::apply`, which traces `context 22% ·
45k of 200k` (or `45k tokens`) the first time and then only when the
figure moves a tenth, so a turn of many requests does not fill the trace;
kept past the turn's end, gone with the session (`Live::end`), on leave
and by the demo's stop; not saved between runs (record 19). The row shows
`◔ 72%` from `CONTEXT_SHOWN` (70%) on, in the attention colour from
`CONTEXT_LOUD` (90%), and nothing under that or without a size: a context
that is fine is not news; the warning takes its seven columns before the
to-do mark, the time and the quote, so it is the last of the yielding
parts a narrow row drops (`salon::context_spans`). The focus has the
figure whatever it is (`context: 22% · 45k of 200k`, `words::tokens` for
`980`, `45k`, `1.2M`), loud from 90% too. The `?` legend's third line
names the mark (six lines still).

Mirrors: a demo worker's context grows by `PER_TICK` (1,600 tokens) per
work tick from `FRESH_CONTEXT` (20,000) in a `WINDOW` of 200,000; past
`AUTO_COMPACT` (90%) the session compacts on its own, as the vendors'
sessions do near their limit, and the trace says so; the person's
`/compact` empties it too; a stop forgets it and the next session starts
fresh (`Sim::fill`, `Mind.filled`).

Not done: a note in WAITING FOR YOU or the journal when a context nears
full (the row's colour and the trace are all); the header has no fleet
figure for it; the fill in the exported Markdown (record 42).
