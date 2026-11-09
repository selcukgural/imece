# Prompts taken

Found in the first real run of 2026-11-09 (`docs/field/2026-11-09-first-real-run.md`): the
person refused a worker's ask and at once wrote to it ("use your Edit tool"). imece counted
two prompts given (`Live.turns` 2); Claude Code took the second into the turn that was waiting
on the refused tool and ended both with one `result`. The count stayed at 1 for good: the
worker read `● working` forever (with the demo's progress label, `░░░░░ plan`, since a live
agent has no phases), its Lonca proposal was never offered (`offered` waits for no turn left),
its `done` never reached the leader (held until its turn ends, record 113), and the next start
called the finished turn cut short. Any message to a worker mid-turn did it.

**Probe** (Claude Code 2.1.319, `-p --input-format stream-json`, haiku). A prompt written while
the turn waits on a tool is taken into that turn: one `result` for both. A prompt written
while the model is generating (no tool) is queued and runs as a turn of its own: a `result`
each. With `--replay-user-messages` Claude Code echoes each prompt as it takes it, a `user`
line with the text and `"isReplay": true`, before the turn's `result`; a queued one is echoed
when its own turn begins (after the next `init`), never before the earlier `result`. Its own
commands (`/clear`) are not echoed (their `result` comes all the same). A `--resume` echoes no
history.

**What was decided.** Every session runs with `--replay-user-messages`; the parser turns an
echo into `Event::Took` (nothing of it is read: the text is imece's own) and `Live::step`
counts them in `Live.took`. A `TurnDone` ends `took.max(1)` prompts: the turns count and the
cut-prompt list (`inflight`) lose that many, and `took` starts again from nothing; a helper's
failed turn is its last when it ends every prompt left (`turns <= ends`). A session that ends
forgets what it took (`Live::end`). Without echoes (an older Claude Code, a command) a result
ends one prompt, as before. The stand-in (record 130) echoes too when given the flag, its
commands never, so `--demo-live` and the end-to-end test run the same path. A writer with a
shell is also told to change files with its Edit and Write tools, never through the shell (two
workers of the same run wrote whole files with `cat > file <<EOF` and `python3 - <<E`, asks of
a hundred lines the person cannot read).

**Concurrency.** The echo and the `result` come on the session's one stdout, in order, through
the one channel `step` reads; a prompt given between them is counted in `turns` at once and
echoed only when taken. **Failure.** An echo lost (a format change) leaves the old behaviour:
one prompt a result, a turn that took two left counted, as before this record; an echo with no
prompt behind it cannot take `turns` below nothing (`saturating_sub`). **Growth, read cost.**
One counter per agent. **Migration.** Nothing saved.

The demo (`--demo`, `sim.rs`) has no turns to count: what it shows did not change, and a
message to a working demo agent never stuck. Not done: the label `░░░░░ plan` for a live agent
with nothing to say is another PR's (the screen fixes of the same day).
