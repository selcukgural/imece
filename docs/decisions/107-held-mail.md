# Mail waits with an agent that waits on the person

Backlog F2 (#59, after cmux and herdr). A waking message to an idle
agent started a turn whatever the agent had open, so an agent that had
asked the person and ended its turn went on without the answer, on a
teammate's word. Checked before the change: a mid-turn approval already
held mail (`Live::turns` > 0), so that case changed nothing in practice;
a question, a vendor error and a board offer did not hold, and
`Status::WaitingHuman` is set by the demo only.

`World::waits_on_person` decides, one rule for live and the demo: the
status `WaitingHuman`, or an open item of the agent's whose kind
`InboxKind::holds_mail`: Question, Approval, Fault, Script, Sensitive,
Research. While it holds, `Live::post_office` (the loop that starts
mail's turns, moved into the bus) starts no turn, `Live::offer` offers no
card, and a lost helper's `GO_ON` waits. A failed turn's retry does not:
it finishes a turn already begun.

A Reply does not hold: it is only to be read, and a leader's report
would stop the team's mail until the person opened it.

A Change does not hold either. Its worker is finished and needs no
answer to go on, a new `done` already replaces the older change
(`World::change_ready`), and holding would make a team stop after its
first cards and move at the speed of the review. The demo's workers take
their next card with a Δ open, as live's do. A question is what stops a
worker between cards, not a change.

How it ends: nothing is stored or re-armed, the rule is read each step
from the inbox, so whatever closes the item frees the mail, and a run
that restarts keeps it (the mail and the inbox are saved already; no
schema change). If the person writes to the agent and the word ends the
wait (`World::word_frees`: everything holding it `closes_on_word`, and no
turn under way), `Live::told` puts the mail after the person's words in
that one prompt, under `mail_prompt`'s heading that says who wrote it, and
gives it back if the prompt fails. Otherwise the next step starts a turn
for it as usual.

A `cancel` is never held: it starts a turn and takes all the mail along
in order. Delivering it alone would be worse: an `assign` it cancels
would reach the agent after it. Known gap: mail from someone else rides
that cancel's turn too.

Not a silent hang: the sender's answer says `held as message N; …`
(`message`, for a waking kind; a cancel keeps its usual words, a kind
that does not wake keeps its own), the recipient's trace has the line,
`Agent.held_mail` (memory only, set each step by `post_office`, counted
by the demo) puts `✉ N held` first on the row (`Text::held_mail_mark`) and
ends `:why` (`Text::why_held`), and `send_you`'s answer to an `ask` warns
the asker that its mail waits too. A leader's turn is a report past mail
that waits on the person (`Live::reports`): nothing moves until the
person comes, so they should hear it.

The caps stand: held mail counts against `MAX_MAIL`, a send over
`MAX_WOKEN` is refused as before, and held mail adds to `woken` only when
it is delivered; mail that rides the person's word adds nothing.

Demo: `Sim::free` is false while the agent waits, `Sim::send` counts
`held_mail` (not a cancel) and a step clears it once nothing waits. It has
no mail queue, so what cannot be faked is the delivery: the demo's
held messages only show.

Cost: one pass over the inbox per agent with mail, each step; an agent
with no mail costs nothing for mail, and offers are read for idle workers
only. Failure: an item that never closes holds the mail until the person
looks, which is why it is shown; a stopped or gone agent's mail is
treated as before.
