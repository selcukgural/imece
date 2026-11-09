# imece doctor

Issue #431 (the person, 2026-11-09, after the acceptance run on real
projects): a first start depends on things imece only found out
mid-task. Claude Code signed out was retried for minutes before it was
called the account's fault; the test commands a checkout has, Lonca's
daemon and policy, an editor that is not set, were each met one at a
time. The welcome window (the entry "welcome and news") checks one of
them. A person who just installed imece, or whose team does not start,
needs one command that says what is wrong and how to mend it before any
agent spends a cent, in the shape of `flutter doctor` and `lonca doctor`.

## What is decided

`imece doctor [--repo DIR] [--verbose] [--json] [--lang en|tr]`,
dispatched in `main` before the screen as `stand-in` and `mcp-relay`
are (`doctor::main`). It prints one line per category, `[✓]`, `[!]` or
`[✗]` by the category's worst finding; a category with an issue lists
every finding (`✓`, `•` a note, `!`, `✗`) and, under each one that is
not right, one line of what to do. The last line counts the categories
with issues. Exit code 0, 1 when a category is `✗`, 2 for a command line
refused. `--verbose` lists every finding of every category; `--json`
gives every finding with a name that does not change with the language
(`claude.login`), its status (`ok` `note` `warn` `fail`), its words and
its fix. Colour only on a terminal, not with `NO_COLOR` or `TERM=dumb`.

**Two halves** (`doctor.rs`). `gather(repo, state, own, terminal)`
looks at the machine and brings back `Facts`, plain data; `report(lang,
&Facts)` judges and words them and is pure. The tests give `report`
facts (a machine with nothing wrong, then one thing changed at a time),
and the window on the screen will give it the same.

**A note is no issue.** `Mark` is `Ok < Note < Warn < Fail`. A note
(`•`) says what is so and needs no mending: no key file, no kitty
keyboard protocol, a narrow terminal, Lonca installed and not used by
this checkout. Only `!` and `✗` make a category an issue, and only `✗`
makes the exit code 1: a doctor that calls every optional thing a
problem is one nobody reads twice.

**The categories.**

- *imece*: the version; the state directory (`dir_seen`: there, to be
  made, or not to be written to, judged at the nearest directory that is
  there); free space there (`statvfs`; `!` under `SPACE_LOW` 1 GiB,
  since a worktree is a copy of the checkout's files); the store
  (`store::peek`: none yet, its schema, `✗` for one a newer imece wrote
  or one that can not be read); another imece on the store
  (`store::held`, `!`); the worktrees under the state directory, as a
  count that points at `:tidy` (which of them no agent owns needs the
  world, so the command line does not say).
- *Claude Code*: `claude --version` (`✗` when it is not there: nothing
  more is asked); signed in through a plan or an API key
  (`claude::login`, `✗` when not, with `claude auth login`; `!` when it
  did not say); the options imece passes (`claude::FLAGS`, 26 and
  `--mcp-config`, looked for as whole words in `claude --help` by
  `claude::flags_missing`; `!` for one not named, since a session would
  fail to start on it). A test holds `FLAGS` to what `args`,
  `session_args` and `once_args` write. There is no oldest version
  imece names: the options are what it needs, and they are checked.
- *git*: its version (`!` under `GIT_LEAST` 2.35, the entry "worktree
  housekeeping"); the folder (`✗` not a folder; `!` not a checkout or no
  commit yet: a team does not start, a solo works in the folder; `!` a
  detached HEAD, which `:lonca import` refuses); `.worktreeinclude`
  when there is one (`copy::included`: what it names, `!` with the first
  line not followed).
- *This checkout's tests* (a checkout with a commit only): the commands
  the entry "test runners" finds from the files (`runners::of`), with
  `cargo test …` for a `Cargo.toml` at the top; `!` with the list of
  what imece knows and `--run PROG` when none.
- *Lonca* (only when `lonca` is on PATH or the store says this
  checkout's project is on Lonca): the program and its version against
  `lonca::LEAST`; cooperative mode; the daemon reached and the person
  signed in (`Place::open`, worded by the screen's own `store_down`);
  the project's repository (`status`: frozen, a policy that lets changes
  in unseen, not there), and how many checks its policy defines. With a
  project on Lonca what is wrong is `✗` (the project is closed to hires
  and decisions, the entry "lonca client"); with none on it, a daemon
  that is not running is a note, and the category's one line says
  `this checkout is not on Lonca`.
- *Terminal* (only on one; piped, it says nothing was asked): its name
  and version (XTVERSION), size (`✗` under the floor of the entry "a
  narrow screen", a note under the 100 by 40 the screens are designed
  at), 24-bit colour, the kitty keyboard protocol, the tab's progress,
  how a notice goes, tmux, SSH. Asked with the start's own probe.
- *Your files*: `instructions.md`, the key file (read as `:keys` reads
  it), the push file (as `:notify push` reads it; the server is said,
  the topic and the token never), the editor of `^x ^e` (`!` when it is
  not on PATH), the tool `:awake` runs.

**It reads, and makes nothing.** No model is called and nothing is
spent. The store is opened read-only (`SQLITE_OPEN_READ_ONLY`): no file
made, no schema brought up, no lock taken, so it is safe beside a
running imece and on a file of a newer one. The lock is asked with a
shared `flock` on the lock file as it is, let go at once; a lock that
is only being let go of gets the second a start gives it (`LOCK_TRIES`),
so beside a running imece the doctor takes that second longer. Claude Code
is asked `--version`, `auth status --json` and `--help`, each through
`child::ask` (5 s, its own process group, 64 KiB, nothing on stdin);
Lonca through its program's `--version` and the daemon's socket with
the checks the client always makes (the socket's owner, the protocol's
number before the token leaves). Everything a program said reaches the
terminal through `fill` or `clean`.

**The words** are in `i18n.rs`, in a group of their own (`Doctor`,
through the `words!` macro, which lists every word so one test holds
the two languages to the same placeholders without a list kept by
hand). `--lang` or `IMECE_LANG` chooses.

## The points

- **Growth**: nothing is kept. The report is as long as its findings,
  about thirty.
- **Concurrency**: beside a running imece the store is read through
  SQLite's own read lock, with a 1 s busy timeout; the lock file is
  held shared for the length of one call, and a start that meets it
  tries again (`LOCK_TRIES`).
- **Read cost**: one run asks three programs a handful of questions,
  under a second on a laptop; `git ls-files` is cut at 20,000 files.
- **Ten times the scale**: the count of worktrees reads two levels of
  directories; nothing else grows with the team.
- **Migration**: an older store is said to be brought up at the next
  start; a newer one is `✗`, read and left as it is.
- **Failure**: a program that does not answer in time is a finding
  (`claude --help did not answer`), never a hang; what can not be read
  is said with its reason; nothing is left behind halfway, since
  nothing is written.

## Gaps

- `:doctor` on the screen (the same report in a window, the welcome
  window pointing at it, the demo's own facts) is the next change.
- The worktrees no agent owns are counted only inside imece (`:tidy`).
- OSC 8 links are not probed: no terminal says whether it shows them.
- The programs `--run` names are a start's flags; the doctor does not
  take them.
- A hardened Lonca is said as the screen says it (refused); what the
  daemon would need is Lonca's to say (`lonca doctor`).
