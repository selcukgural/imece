# Lonca tells, imece listens

Record 116's look asked the daemon for every repository's `status` every
5 s (`LOOK_EVERY`), whether anything happened or not. Lonca 0.2.3 has
`events.watch` (lonca#262, `lonca-client` `Client::watch`): with the human
session the person started the agents with, a request the daemon holds up
to 30 s until something happens to one of those agents' sessions (a change
integrated, conflicted or failed, an approval, a rejection, a message, a
lease handed over, an interest reserved). It marks nothing as read, so the
agents still hear it through their own tools; it never carries what was
written, only the kind, the session and the change. imece now listens with
it and asks less often when nothing happens.

**An ear per repository.** `live::lonca::hear` is a thread for each
repository a project on Lonca names (`Watch.ears`, made and stopped by
`Watch::listen` at every `lonca_due` as `World::stores` changes, each with
its number, so what a stopped ear says late is nobody's). It opens its own
connection (`Place::open`, the same checks of the socket, the peer and the
protocol as a look), sends `events.watch` with `wait_ms` 30 000
(`HEAR_WAIT` = `LOG_WAIT_MOST`; the client's timeout is the wait and the
usual 5 s on top, as a held `log`'s), and goes on from each answer
(`Watch::after`: its `next` and `reserved`), never sooner than
`HEAR_SOONEST` 250 ms after its last watch began: a burst of news, or a
daemon that answers at once though nothing happened, is never a loop. It tells the screen's thread
three things (`Heard`): `Up` at its first answer (the repository is
heard), `News` when an answer holds anything but messages between agents
(`news`: a message is the agents' to read and no look shows it; an `ended`
session counts), and `Down` when the daemon stops answering one that was
up. It never touches `World`: the screen's thread reads what it said.

**The pace.** `Live::lonca_pace` is `LOOK_CALM` 30 s while no worker in a
Lonca workspace is mid-turn and every repository on Lonca is heard, else
`LOOK_EVERY` 5 s as before; also while the last look met a daemon that
asked to be asked again (`Looked.again`), so record 116's three looks in a
row before a busy daemon closes its project stay about 15 s. Mid-turn, a look still comes every 5 s because
what Lonca says of a working session is not all in the events: whether its
hooks were heard (record 119's `stop_unheard`, judged by a look), the
leases it takes (the code map, record 123), the proposal it makes (a
proposal is no event: its look comes at the worker's turn end, as before).
The pace is from the last look (`Watch.asked`), so a worker that starts
work during a calm wait brings the next look to 5 s after the last one,
not 30. News, or an ear going down, brings the next look to
`LOOK_SOONEST` 1 s after the last (`Watch::soon`): a burst of events is one
look, and a look due sooner is never put off. `:lonca NAME`, a decision of
the person's and a worker's turn end still ask at once.

What the person sees: a proposal that lands, fails its checks or is
decided from a shell (`lonca approve`) shows within a second or two
instead of up to five, and between turns imece asks the daemon twelve
times less often. Nothing on the screen changes, so the demo has nothing
new to show: its store answers in the world's own ticks.

**Old daemons.** The daemon's floor stays Lonca 0.2.1 (`LEAST`). A daemon
before 0.2.3 answers `unknown_method` (`Unheard::Old`): its ear says
nothing, the repository is never heard, the pace stays 5 s, and the ear
asks again after `HEAR_OLD` 5 min, in case the daemon was upgraded. Any
other failure (no daemon, signed out, a line outside the protocol) is
`Unheard::Down`: a `Down` if it was up, a new connection after
`HEAR_AGAIN` 30 s, the watch starting from that moment (`from` none),
since the looks have read the state meanwhile.

**Growth.** One thread and one connection per repository on Lonca, held
while the project names it; nothing is kept but the last answer's `next`
and `reserved` (the reservations the agents hold now, a short list).
**Concurrency.** The ears share nothing with the look's thread but the
`Place`; each holds its own connection, which counts against the daemon's
connection limits like any other. Only the screen's thread reads what they
say, through one channel. **Read cost.** A frame reads nothing; a step
drains the channel. **Ten times the scale.** Ten repositories are ten held
connections and ten threads asleep in a read; between turns ten `status`
calls every 30 s instead of every 5. A daemon that limits connections per
person would refuse the eleventh: the ear goes down, its repository goes
back to the 5 s pace, and nothing closes. **Migration.** Nothing is
stored. **Failure.** An ear that fails is a slower pace, never a closed
project: what closes a project is still a look's answer (record 116). An
ear blocked in a held read when its repository leaves Lonca, or when imece
quits, ends after that read (at most 30 s; `Watch`'s `Drop` sets every
ear's stop); one whose channel is gone ends at its next word.

**Proved.** The fake daemon (`lonca_fake`) answers `events.watch` from
version 0.2.3 with what a test tells it (`Daemon::tell`, held at most
`WATCH_HOLD` 200 ms) and `unknown_method` before; the ear, the pace and
the soon look are tested against it. `live_a_watch_hears_a_proposal_decided`
(ignored, a real daemon) ran on Lonca 0.2.3: a watch held while the person
approved heard `approved` in 0.4 s.
