# Reply-to numbers

Backlog F3 in #59 (2026-11-01, from cmux and herdr; the person decided
on 2026-10-31 to build it): mail already says it was written by an
agent, not by the person (record 03); what it lacked was a way to say
which message a reply answers. With several questions open between two
agents, or a done that closes one of two assigns, neither the agent
reading it nor the person reading the conversation could tell.

Every message one agent sends another gets a number on the bus
(`MsgId`, `World::number`): in order from 1 across the world, kept
between runs (`meta.messages`). `send` answers with it ("delivered as
message 7; …"), and the mail is headed with it when it is delivered:
`[ask 7] from lead:`. `send` takes an optional `re`, the number of the
message this one replies to; the tool's description asks for it on an
answer to an ask and on a done for an assign, and the heading carries
it: `[answer 8, re 7] from claude-1:`. It stays optional: a message
that answers nothing in particular is still a message, and refusing one
for lacking `re` would cost a turn for nothing.

`re` is checked, deny by default (`bus::reply_to`, `bus::replies`): a
whole number from 1 that fits a `MsgId`, of a message the journal has,
that was sent to the sender, by the one the reply goes to. A reply goes
back to whoever wrote what it answers; anything else is refused with
the reason, takes no number and leaves no trace. Messages to the person
(`to: you`) take no number and no `re`: the person's own messages have
none, and their questions wait in WAITING FOR YOU (record 27), which
already ties the answer to the question.

Only the envelope drives routing (record 03), and `re` drives none of
it: it is not used to wake, route or cost anything, only to link. The
journal and the card's story keep both numbers on `Sent` (JSON, with
defaults: a line from before numbers reads as number 0, replying to
nothing); unread mail keeps them in `mail.number` and `mail.re`
(schema 11). The trace says `id=8 re=7`.

On the screen a reply has `↩` after its type, on its journal line, in
a card's story and in a conversation. In a conversation it also shows,
dim under its head, the first line of what it answers (`↩ re 7: Which
parser?`, cleaned through `i18n::fill`), or that the journal no longer
has it. `%` (vim's matching-pair key) goes from a reply to what it
answers, and from a message to its first reply (`ui::reply_jump`); the
`?` help names it on the conversation's line and the conversation's
hint row lists it, in both languages.

The demo mirrors it: its messages are numbered the same way, an answer
replies to the ask it answers, and a worker's or helper's done to the
assign that gave it its card or task (a card it claimed came with no
message: its done replies to none).

Known gaps: the lookup reads the journal in memory, so a reply to a
message older than the journal a load takes (record 24) is refused as
not there; the person cannot reply by number (their messages go by
name); `re` is not checked against the type (an assign may name one).
