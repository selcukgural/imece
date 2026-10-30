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
Known gaps: no answer from the screen but a message (there is no
"answered without words"); a question of an agent that is gone or
stopped stays open until the person writes to it.
