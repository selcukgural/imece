# Ask the person v0

Ask the person v0 (2026-10-30, posture PR b): the bus reaches the
person as `you` (`live::YOU`, exact; `send(to: "you", type, body)`,
`Live::send_you`). An `ask` becomes `InboxKind::Question` in WAITING
FOR YOU (`?`, "X asks you: first line"; a newer question of the same
agent takes the older's place, `World::asked_you`), the agent's salon
line reads `? → you` while it is idle with one open
(`World::has_question`, like the `answered you` line: derived from the
inbox, no new `Status`), and it closes when the person writes to that
agent (`World::said` → `close_questions`), which starts the agent's
next turn as any message does; `enter` on the item opens the
conversation at the question (not an answer), `a` / `r` open the
composer `@name `. Every message to `you` is `Event::ToYou { from,
kind, text }` (kind Talk, drawn as a conversation line agent → you with
its type, `StoryEvent::ToYou` in the card's story, a talk bubble, a
trace line); inform/progress/notice/done/blocked/answer only inform (no
inbox item, nothing wakes), a worker's `done` to `you` shows its change
like any done; assign/handoff/cancel to `you` are refused. Nothing to
`you` counts against the mail caps or the budget. Prompts: the leader
may send a mid-task question (its closing text still reaches the person
as before), a worker asks its leader first and sends the person only
what only they can decide, a helper only what no teammate can answer;
the `send` tool's `to` says "or you for the person". Store schema 3 (no
table changed; the new tags). The demo asks the person too (decision
31).
While a question is open the agent's mail and board offers wait (record 107).
Known gaps: no answer from the screen but a message (there is no
"answered without words"); a question of an agent that is gone or
stopped stays open until the person writes to it.

Options (2026-11-07, record 125's question screen): `send` to `you` with
type `ask` takes `options` (`bus::options_of`: an array of at most
`OPTIONS_MAX` 4 strings, each trimmed, not empty, at most `OPTION_MAX`
60 characters, no control character; refused on any other message:
`options go with an ask to the person`), kept on `Event::ToYou.options`
(serde default, so an older journal reads back without them; no schema
change) and read by `World::question_of` (the newest ask of the agent in
the journal, with its text whole; the item's first line alone once the
journal no longer holds it). On Home a question's row lists the answers
under it with their keys (`ui::home::Tail::Asked`, keys `1-3 pick · a
answer`), the preview reads it whole with the answers and what the agent
did so far; `1`-`9` on Home (`App::option_at_hand`/`answer_option`) send
the answer as the person's message through `tell`, which closes the
question as any message does, and `/` on the row opens the composer to
the agent (`question_at_hand`); off Home, or with no answers to pick,
the digits are the views'. The tool's description tells agents to offer
answers; the demo offers them on two of its four questions
(`Content.ask_options`). The gap "no answer from the screen but a
message" is closed for a question that offers answers.

**Later (record 143).** A leader's ask about a worker's blocked carries
`re` (the blocked's number), `tried` and `risk`; its row is the blocked
work's and its preview reads the worker's words and what the leader tried
before the question.
