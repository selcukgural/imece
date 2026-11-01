# Welcome and what's new

Backlog item C19 of #56 (from herdr and Warp): a first-run screen
(which coding agents are on PATH, signed in or not, the keys) and a
once-per-version "what's new". The person chose (2026-11-01): Gemini's
sign-in is said to be checked at its first start rather than guessed
from files; the window opens by itself on the first start ever and on
any start where the leader's vendor is not signed in; the news are a
list in the code, in both languages, that a release cannot forget.

**Asking a CLI about itself.** `imece_agent::child::ask` runs a command
with nothing on stdin, in its own process group, for at most
`ASK_WAIT` (5 s): its status and what it wrote (stdout, then stderr,
each cut at 64 KiB), read on a thread of its own so a big answer does
not stall the program and a pipe a leftover process holds does not
stall the caller; the group is killed at the end either way. On it:
`imece_agent::version` (`--version`'s first line, at most 60
characters), `claude::login` (`claude auth status --json`, its
`loggedIn`; an API key counts) and `codex::login` (`codex login
status`: `Not logged in` → no, `Logged in…` with success → yes, on
stderr). Both answer offline in 30–110 ms (probed with Claude Code
2.1.316 and codex-cli 0.165.0, signed in and with an empty config
directory). Each runs outside any checkout (a repo's settings never
load) with its vendor's variables (`live::env_of`). Gemini CLI 0.65.0
has no such command: its sign-in is `Login::Unknown`, said as
"checked when it first starts"; its `/about` and a refused
`initialize` already tell the rest (record 07).

**Asking all of them.** `live::look(known, which)` asks each vendor of
`which` on a thread and sends `(Vendor, Seen)` as the answers come;
`Seen` is `Checking`, `Missing` (not on PATH, or `--version` failed)
or `Here { version, login }`. `App::vendors` keeps one line per
vendor of `welcome::LISTED`; `App::look` is how a live run asks (main
sets `live::look` over `KNOWN`; tests a fake; the demo none) and
`App::looked`, at the top of every round of main's loop, takes what
came.

**When it opens.** main tells `App::start_welcome(first)` whether the
store remembered an earlier run (`Live::left_off()` none: the first
start ever). The first start opens the window and asks every vendor.
Any other start asks only the leader's vendor (`App::lead_check`): a
`Login::No` opens the window and asks the others, or, while the person
writes, flashes `not_signed_in` instead. `:welcome` opens it any time
and asks again. Any key closes it and does nothing else.

**What it shows.** `ui::overlay::welcome`: `welcome_about`, one line per
vendor (`✓` signed in, `✕` with `welcome::sign_in`'s command, `?`
Gemini, `·` not on PATH, `…` checking; the version through `clean`:
a CLI's words), and `welcome_keys` (nine keys, a subset of the help).

**What's new.** `Text::news` lists each version's lines, newest first,
in both languages. The store keeps `seen`, the version that started
last (`prefs::seen`, a setting, written by every live start; the demo
writes none). `welcome::news_since(seen, first, VERSION, versions)`:
nothing on the first start ever (the welcome says it) or when this
version was seen; the versions after `seen` up to this one, at most
`NEWS_SHOWN` (3), compared as numbers; only this one's when nothing was
kept (a run from before) or the kept value is not a version. `:news`
shows the latest three. A test fails when Cargo.toml's version has no
entry or the languages differ, so a version bump writes its news.

**Order.** One window at a time, each closed by any key: the help
first, then what's new, the welcome, the away digest, where you left
off (record 25). A paste or a refocus does nothing while one is open
(`App::window_open`).

**Demo.** `sim::vendors`: all three there, Claude Code and Codex signed
in, Gemini's sign-in not asked; `:welcome` and `:news` open the same
windows, nothing is asked. The demo's start opens neither, as it opens
no left-off window. A vendor that is missing or not signed in cannot be
shown there (the demo's workers of every vendor run); the unit tests
draw both.

Gaps. Codex signed in with an API key is "signed in" here, though imece
refuses such a session (it cannot price its tokens, record 06); the
refusal says so at the start. The 0.1.0 entry describes the first
release; the features merged after the v0.1.0 tag go into the next
version's entry when Cargo.toml is bumped. No link to the release notes
on GitHub.
