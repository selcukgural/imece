# The client of Lonca's daemon

The first step of record 115's plan, with Lonca at `01ba8a3` (protocol
number 1, `docs/protocol.md`): imece can read a repository through the
daemon, and a project on Lonca is closed to hires and Δ decisions while
it can not. Workers still work in worktrees and a Δ still lands in the
checkout: nothing here moves work onto Lonca.

**The client** is `imece_agent::lonca`: wire formats live in that crate,
the screen never sees one; the engine's half (who is asked, when, what it
closes) is `live/lonca.rs`. Written from the protocol document, which says a
client copies nothing from Lonca's crates, so `VENDORED.md` records the
number instead of a copy. Since 2026-11-09 (user, 2026-11-08: once it is on
crates.io, as a version dependency) the socket half is Lonca's own
`lonca-client` (`=0.2.2` since 2026-11-10, when Lonca 0.2.2 came out; the
oldest daemon taken stays 0.2.1, `LEAST`): connecting through `Trust` (the socket a socket,
not a link, of the expected uid; the peer that uid), the framing, the
timeout (`set_timeout`, 5 s, a held `log` longer), a request sent again once
on a new connection when the daemon closed a quiet one before reading it,
and the session file (`session::read_session`). What the answers mean stays
here (`Status::of` and the rest, `down_of` over `ErrorKind`: the client's
`io` and `timed_out` are `NoDaemon`). `ping`, `status`, `tree` (the manifest),
`change.get`, `log` (with `wait_ms`); `pending` is a field of `status`, as
the protocol has it. `tree`, `change.get` and `log` have no caller yet: the
boxes that read them come next, and they are tried against a real daemon.

**Who is talked to.** Lonca's own clients do not check the daemon yet
(lonca#52), and the epic says imece must. The socket is `LONCA_SOCKET`, else
`$LONCA_HOME/lonca.sock`, else `~/.lonca/lonca.sock`. It must be a socket
(no link) owned by the uid the machine's install names (`Trust::load()`:
root-owned `daemon.toml` of a hardened install, else the person), the peer
listening on it must be that uid, and `ping` must say protocol 1 and a `version`
of at least `LEAST`, 0.2.1 (`new_enough`: its three numbers, a suffix not
counted; none, or older, is `Down::Old`, worded `the daemon is older than Lonca
0.2.1, which imece needs`; 2026-11-09, record 120 says why): all of that
before the token leaves. So imece runs against cooperative mode. lonca#52
answered where a client reads a hardened daemon's uid, and `lonca-client`
reads it; imece still refuses a hardened install before connecting
(`Down::Hardened`, `Lonca is installed hardened on this machine; imece does
not work with a hardened daemon yet`): its sessions can count as observed and
a change integrate with no person's yes, which the Δ gate does not look at
yet (record 120). An install file that is not what the installer left
trusts nobody (`Place.trust` none: `NotTheDaemon`).

**The person's session.** Since lonca#51 a read needs a credential. The
token is read from `$LONCA_HUMANS/<name>.session`, the interface the
protocol documents (`LONCA_HUMANS` else `~/.config/lonca/humans`; `name` is
`LONCA_HUMAN` else the `default` file): the person's own plain file, 0600,
not a link, 64 KiB at most (`lonca-client`'s cap), not ended. imece does **not** run `lonca human
login` by itself, though the protocol allows it: with a Secure Enclave key
that is a Touch ID at a moment the person did not choose, from a thread that
gives calls five seconds. `⌀ no lonca` and the journal say to sign in; the
next look finds the session. Sessions last 24 h at most, so this comes back
daily: record 115's box 2 (`LONCA_HUMAN`, the person's yes) is where a login
at a moment of the person's choosing goes. The token is in no `Debug`, log or
word on the screen.

**A project on Lonca.** Box 2's `:open` is what makes one, but box 1 needs
projects to act on, or nothing here runs and nothing can be seen to fail
closed. So a word beyond the plan: `:lonca NAME` names a repository for the
project at hand, `:lonca off` leaves it, `:lonca` says how it stands. It is
kept with the project (`projects.lonca`, schema 27; an archive's fresh project
keeps it), and box 2 keeps it as the way to name a repository that exists.
A name is 1 to 64 letters, digits, `.`, `_`, `-`.

**The look.** `Live.lonca` (`Watch`) is a thread with one connection a look:
`status` for each open project on Lonca, at most every 5 s (`LOOK_EVERY`) and
at once after `:lonca NAME`, never a second while one is out, so the screen
never waits on a socket. What it finds goes to `World::store_seen`: an answer
from a repository the person has since replaced is dropped. `OnLonca.seen` is
`Asking`, `Up { trunk, pending }` or `Down(why)`, memory only: read anew each
run.

**Fail closed.** `World::store_closed` is `Some(why)` for a project on Lonca
unless its last look answered, so `Asking` counts as down. Refused then:
`hire` (`hire_room`, so `whoami`'s `may hire` too, and helpers), `:fork`,
`Live::apply_change` and `reject_change`, which also covers `b` and notes sent
back, since they reject first. Not refused: what a worker does in its
worktree, its `done`, the Δ being read; an answer to an `ask`, which is not a
decision on a change; the bus. The project's line says `⌀ no lonca` in the
warning colour, `lonca NAME 693d629 · 2 pending` when up, `lonca NAME …` while
asking. `Event::StoreDown { why }` and `StoreBack` say it once a change of
reason, not once a look.

**Asked to ask again** (lonca `010ba7e`). `busy`, `rate_limited`,
`verifying`, `closed` and `timeout` say nothing ran and the request may be
sent again (`closed` and `timeout` under id 0, then the connection closes):
`Down::Again`. A look that gets one keeps what the last look saw of that
repository, asks the next one on a new connection, and counts (`Kept.again`);
only `AGAIN_MOST` (3) looks in a row close the project, as `StoreDown::Busy`
(`the daemon is busy and asks to be asked again`), and an answer starts the
count again. A line under id 0 with any other code is still outside the
protocol, and every code imece does not know still closes (`Refused`).

**Frozen** (lonca `010ba7e`). `status.frozen` is why a human froze the repository
(`lonca freeze REASON`), or the auditor did on an integrity alarm; `null` when it is
not (probed). A frozen repository takes no proposal and no decision, so a look that
reads one says `StoreDown::Frozen` (`Lonca froze the repository (lonca status says
why); a person unfreezes it`) and the project closes as for any other reason: no
hire, no Δ decision. Anything but `null` is a freeze (fail closed); an older daemon
without the field is not frozen. Lonca's `security.events` are not read: a freeze
and an integrity alarm are already told on the desktop by Lonca at once
(`--notify-command`), and what they do to imece's work is this freeze.

A project not on Lonca changes in nothing: no socket
is touched, record 115's "the git path stays until the Lonca path carries a
team" holds.

**What a real daemon said** (`--ignored live_a_real_daemon_answers…`, Lonca
`01ba8a3`, a proposal pending; run by hand, CI has no daemon). Everything read
as the protocol says, with one thing the fake had hidden: a repository that
does not exist answers a human session `forbidden`, not `not_found`, because
the session is checked against the repository first. The word for it says both
(`you are not enrolled in a repository of that name`) and the demo answers the
same. Also found, for Lonca: `docs/protocol.md` writes `lease_held:<symbol>:
<session>:<expires_ms>`, the daemon writes the intent in the last place
(lonca#48).

**The demo** (record 31). `Extras.store` is the table: a row per demo project
with how many changes it integrated, the trunk a function of that, what is
pending the project's waiting Δs. The second project is on it. Its daemon stops
6 ticks in every 60 (`store_stopped`), in turn gone, busy and frozen
(`store_stop`), so `⌀ no lonca`, the journal's two
lines and a refused decision can be seen; `hire` and helpers wait those ticks
out. `:lonca NAME` takes any name; one not in the table answers as a real
daemon does. What can not be faked: a socket, a session file, a protocol
number.

**Growth.** One word per project in the store; the journal gains a line per
change of state, so a flapping daemon writes two lines a look at worst, within
the journal's retention. **Concurrency.** Only the watch thread talks to the
socket; two imeces read the same daemon, which is its design. **Read cost.**
Nothing in a frame or a key reads the socket; `status` is a millisecond call,
though its answer lists a repository's every active lease and change: at the
protocol's cap (96 MiB a line) the read stops, and a line cut short is no
answer. **Ten
times the scale.** Ten projects on Lonca are ten `status` calls on one
connection every 5 s. A long list of changes is what grows: `next_seq` is parsed
so a later box can skip a look that found nothing new. **Migration.** Schema 27
adds a nullable column; a project with none is not on Lonca. **Failure.** A
daemon gone mid-look is `NoDaemon` for what it had left; a hung one is five
seconds on a thread nobody waits for, and no look is queued behind it.
