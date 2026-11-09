# Vendor errors: tried again, or the person's

Backlog E2 in #58 (from cmux; 2026-11-02, the person accepted the
proposal). A turn that failed at the vendor's API left the agent idle with
a journal line, `claude-1 stopped: success` on Claude (the result's
subtype stood in for the reason). Nothing tried it again and nothing
waited for the person, so an overloaded hour stopped a team silently, and
a signed-out account looked the same as a passing blip.

**Probe (2026-11-02, stub servers on the loopback answering a fixed
status).** Claude Code 2.1.316 (`ANTHROPIC_BASE_URL`): each of its own
retries is a `system`/`api_retry` line (`attempt`, `max_retries` 10,
`retry_delay_ms`, `error_status`, `error`: `overloaded`, `server_error`,
`rate_limit`, `authentication_failed`, `unknown`); 529 gives up after 3
tries (~2 s), 500, 429, 401 and a refused connection after 10 (~3 min),
`Credit balance is too low` (400) at once. The final `result` is
`subtype: success`, `is_error: true`, `terminal_reason: api_error`,
`api_error_status` (absent without an answer) and the message in
`result`. Codex 0.165.0's app-server (a `model_provider` on the stub):
`error` notifications with `willRetry: true` (`Reconnecting... 2/5`),
then `turn/completed` with status `failed` and
`turn.error.codexErrorInfo`: a kind (`internalServerError`,
`usageLimitExceeded`, the schema's `serverOverloaded`,
`rateLimitExceeded`, `unauthorized`, `contextWindowExceeded`, ...) or an
object naming a connection's failure with its `httpStatusCode`
(`httpConnectionFailed` 401, `responseTooManyFailedAttempts` 429,
`responseStreamDisconnected` null). Gemini CLI 0.65.0 over ACP
(`GOOGLE_GEMINI_BASE_URL`): `session/prompt` answers a JSON-RPC error
whose `code` is the HTTP status (503 and 429 after 10 tries, ~4 min, with
no word on the wire meanwhile; 401 and a daily quota at once). All three
sessions live on after a failed turn, take the next prompt, and keep the
failed prompt in their history (the next request carried it).

**Two kinds, decided in the adapter.** `imece_agent::Fault` is `Passing`
(overloaded, 5xx, 408, a rate limit, no answer at all) or `Account`
(401-403; a 429 or 400 whose words say quota, billing, credit, usage
limit, limit reached); `Fault::of_status` is the one table, each adapter
finds the status (Claude's `api_error_status` when `terminal_reason` is
`api_error`; Codex's kind or its object's `httpStatusCode`; Gemini's
error code between 100 and 599). Anything else (a refused request, a full
context, a budget cap, an interrupted turn) has no fault and is said as
before. `Event::Failed(fault)` comes just before the `TurnDone { ok:
false }` it explains; adding a field to `TurnDone` would have touched
every adapter's and test's literal for one fact. The vendors' own retries
become trace notes (`api error 529 overloaded: retry 2/10 in 1163 ms`,
`api error: Reconnecting... 2/5`); Gemini says nothing of its own.
Claude's failed turn now carries the message, not `success`.

**imece tries a passing fault three more times: 30 s, 2 min, 8 min
after each failure** (`RETRY_TICKS` 6, 24, 96 ticks), after the vendor's
own retries gave up, so a minute's outage costs no one anything and an
hour's ends with the person. `World::turn_failed` (shared by live and the
demo) counts the tries on the agent (`tries`), sets `retry: Retry {
attempt, at }`, and journals `Retrying` (`claude-1 hit a vendor error: …
· imece tries again in 30 s (1/3)`); the row says `↻ 1/3 in 25 s` while
the agent is idle. `Live::step` takes `World::retries_due` (idle, here,
the tick reached) when the agent has no turn queued and the pot is not
spent, and prompts `RETRY_PROMPT`: a short "carry on from where you
stopped", never the prompt again, since every vendor kept it. A good
turn, the person's word (`World::said`) and a session's end set the
count to nothing; any prompt that goes in (mail, the person) drops the
waiting retry, being the next try itself. A tiered agent's first turn
that fails with a fault is not taken for a missing model (no fallback).

**The person's: `InboxKind::Fault` (`✕`).** An account's fault, or a
passing one past its third try, opens a WAITING FOR YOU item with the
error's first line (`claude-1 vendor error, write to it once mended:
Invalid API key`), the row says `✕ vendor error "…"`, the agent stands on
the waiting rung, and `:notify` rings it as `imece · claude-1 hit a
vendor error that needs you` (never the vendor's words). `a` and `r`
open the composer `@agent ` as a question's do; the item closes when the
person writes to the agent, and that message is the next try. A newer
fault of the same agent takes the older's place.

**Demo.** Of the messages the person sends that a demo agent answers
with a turn, every third fails on an overloaded vendor (Claude's words)
and is tried again at its tick, then answers; every seventh fails on a
refused key and waits in the inbox (`Sim::told`, `fault_of`,
`Sim::retry`). Tests that count answers switch it off (`Extras::calm`).

**Growth.** One `Retrying` line per failed try (at most three per failing
streak) and at most one open `Fault` item per agent; both kept and tidied
as the journal and inbox are. Retry state is two small fields per agent,
memory only. **Read cost.** `retries_due` walks the agents once a step;
the row reads its own agent's `retry`. **Concurrency.** None new: the
vendor's thread sends `Failed` and `TurnDone` in order on the agent's
channel; the main thread alone touches the retry. **Ten times the
scale.** An outage at a vendor fails every agent of it at once: each
retries on its own clock from its own failure, so the tries spread with
the failures; at ten times the agents, an hour's outage puts as many
`✕` items in the inbox, one per agent, which `W` walks. **Migration.**
Schema 14: the `Fault` kind and the `Retrying` event are new tags, no
table changed; an older imece refuses the file (as with 3 and 4).
**Failure.** A retry whose prompt cannot go in stops the session as any
prompt does; a quit or crash drops waiting retries (memory only), and the
woken agent hears of its interrupted turn as before.

**Gaps.** Claude's subscription limit (OAuth) and Codex's ChatGPT plan
limits were not probed (real accounts are needed); they are classed by
their words (`usage limit`, `limit reached`, `hit your limit`) and by
Codex's `usageLimitExceeded`, and a reset time a vendor gives is not used
to time the try. The tries and their spacing are constants, not
settings. A failure while the person's message is the turn (not mail)
retries just the same; the answer goes to the person when it comes.

**Signed out** (found in the acceptance run of 2026-11-09; Claude Code 2.1.319): a turn while not signed in ends with a result of `is_error` true, `terminal_reason` `api_error`, `api_error_status` null and the text `Not logged in · Please run /login`, after an assistant message of its own (`model` `<synthetic>`) carrying `"error": "authentication_failed"`. With no status it was taken for a passing fault and tried again for minutes while Home said nobody was at work. Now the parser keeps an assistant `error` of `authentication_failed` or `billing_error` for its turn (`Parser.account`) and such a turn is the account's; with no status the words `not logged in` / `please run /login` are the account's too (`ACCOUNT_WORDS`). A status still decides first.
