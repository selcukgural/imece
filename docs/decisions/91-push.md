# Push to the phone

`:notify push` (2026-11-03, #239, from the study of agent-deck) sends
what `:notify` would say to an ntfy topic the person follows on their
phone, for when they are away from the machine. Only the notification:
deciding an ask from the phone is a separate decision (who may approve,
replay, phishing) and is not done.

When: `App::notices` as for the bell and the desktop note (record 60:
`Notices::due`, `AFTER` 5 s, once each, a count when several), with one
addition for a push only: away is the focus lost OR no key for
`:away`'s time (record 52, 30 min; `off` leaves the focus alone), since
a terminal left on the desk keeps its focus. `notify::Notify::Push` is
taken by name only (`Notify::next` never reaches it: it needs
settings); `ring` writes nothing for it; `App::push_word` sends.

Words (`words::pushed`): `notice`'s words and the project's name
(`imece · codex-2 asks you · shop`), several as `notify_many`; nothing
an agent wrote, no command, cleaned by `fill`. Loud (stuck, a fault) is
ntfy's `Priority: high`.

Settings (`push.rs`): `$XDG_CONFIG_HOME/imece/push` (`keys::config_file`),
`name = value` lines `server`, `topic`, `token` (a file's path, `~/`
allowed), `#` comments, each once, refused whole on the first fault
(`push::Why`, worded by `words::push_why`). Server: `https` with a host
starting alphanumeric or `[`, no user, query or fragment; `http` only for
`localhost`, `127.0.0.1`, `[::1]`; default `https://ntfy.sh`. Topic:
`[A-Za-z0-9_-]`, at most 64; on the public server at least 16 with a
digit (`Why::Guessable`), refused, not warned, since a topic is the only
lock there. Token file: opened `O_NOFOLLOW | O_NONBLOCK`, looked at as
opened: a plain file, the person's uid, `mode & 077 == 0`, at most
4 KiB, one word of visible ASCII without `"` or `\`. `Push`'s `Debug`
hides the token. Read at `:notify push` (a refusal keeps the way as it
was) and at start when `push` was kept in prefs (`App::start_push`: no
longer good → `off` and the flash says why). The demo reads nothing.

Sending: `curl -q --config - --silent --show-error --fail --max-redirs 0
--proto =https --max-time 10 --output /dev/null` (`=http` for a loopback
server), its config on stdin (`push::config`: `url`, `data-raw`, a
`Title` and `Priority` header, `Authorization: Bearer` when a token),
through `imece_agent::child::ask_fed` (own process group, a deadline of
12 s, capped pipes; the input written on a thread). No new dependency:
`curl` is on macOS and every Linux the release targets; missing, the
push fails and says so. One thread per push, its end on `App::pushes`,
taken by `App::pushed` from the main loop: `Event::Pushed { agent,
words, sent }` per push that went, `Event::PushFailed { agent, why }`
(curl's first line, cleaned, else its exit code) only when the one
before did not fail (`push_failing`), so a server that is down is said
once; no retry, the next thing that waits is pushed as its own. The
demo journals `Pushed { sent: false }` and sends nothing.

Growth: a journal line per push, as many as the notices (each thing
once). Concurrency: a thread per push, nothing shared but the channel.
Failure: a push that hangs is killed at 12 s with its group; one that
fails never blocks the screen. Migration: `push` in prefs' `notify`
reads as `Push` in this version; an earlier imece reads it as nothing
known and keeps `off`. Known gaps: no answer from the phone; the token
is in memory for the run; a proxy set in `.curlrc` is not used (`-q`),
one in the environment is.
