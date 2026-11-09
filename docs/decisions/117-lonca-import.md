# A project on Lonca: sign in and import

The first half of record 115's step 3, with Lonca at `832c161` (protocol
number 1): a person can get from a git checkout to a project on Lonca without
leaving imece. How far the checkout is behind trunk and `:sync` are the second
half (record 118). Workers still work in worktrees.

**What changed from record 115's text.** Three things.

- *A word of its own, not `:open`.* `:open` takes a folder and makes a project;
  whether that project goes on Lonca is another act, with a yes of its own, and
  it may come long after. Record 116 left `:lonca NAME` as the way to name a
  repository that exists, so `:lonca import [NAME]` sits next to it. `import`,
  `login` and `off` can not name a repository from `:lonca`; `:lonca import
  login` still makes one of that name, and the project goes on it.
- *No human workspace for the leader.* The step asked for one so the leader
  could read trunk. Lonca's #83 came after it: the person's git checkout is a
  client now (`lonca fetch` into `refs/lonca/trunk`). A workspace has no `.git`,
  so the leader's `git log` and `git diff` would stop working, and there would
  be a third copy of the files. The leader stays in the checkout, and record
  118 keeps the checkout near trunk.
- *`LONCA_HUMAN` needed nothing new*: `imece_agent::lonca::Place` read it
  already, and the `lonca` child is given it.

**Who signs.** Reads go over the socket (record 116). What must be signed with
the person's key is done by the `lonca` program, as a child of imece, with
argv only, a cleared environment plus `LONCA_HOME`, `LONCA_SOCKET`,
`LONCA_HUMANS`, `LONCA_HUMAN`, `LONCA_SE`, and stdin closed
(`Place::run`). imece holds no key and signs nothing. A `Place` names the
program (`lonca`, on `PATH`) so a test can put a fake there.

**`:lonca login`** runs `lonca human login --json` at the moment the person
asks: record 116's reason for not doing it alone. With a Secure Enclave key that
is a finger on a sensor, so it gets 60 s and its own thread. The token stays in
the session file; imece reads only the name out of the answer, and a login that
prints none still went through. The journal says `signed in to Lonca as NAME`.
A session lasts 24 h at most, and the project's word for it is now
`you are not signed in (:lonca login)`.

**`:lonca import [NAME]`** makes a repository of the checkout, as NAME or the
project's name. Refused at once, with no window: a project already on a
repository, a folder without git (record 73), a bad name, a `lonca` command
under way, a detached HEAD (nothing says what goes then). Otherwise a window
says the checkout, the branch and its commit, the repository it becomes, that
the history is copied and what is not committed stays, and that imece can not
take it back. `a` is the person's yes, `esc` or `q` closes it and nothing goes,
`?` the help, every other key nothing. Only then `lonca init --import PATH
--branch BRANCH -- NAME` runs (the `--` because a valid name may start with a
dash), for up to 10 minutes on its own thread: the bottom line spins as for
`:commit` (`App::asking`), and the screen goes on. It takes the branch the
checkout is on, not Lonca's default `main`.

On success the project is on the repository (`World::store_imported`), unless
the person named another while it ran: the repository exists either way and
the line says so. The daemon is asked at once. What Lonca warned of comes as
`warning: ` lines on stderr (a file above the change cap: a workspace gets it, a
proposal can not change it), the first in the journal with the count of the
rest; lonca#97 asks for them as data. The same yes then signs the repository's policy
so a critical path waits for the person (record 121).

**The profile** (lonca `010ba7e`). `lonca init` starts a repository's policy from a
profile: the install's own unless named, which is hardened on a hardened install
(presence for every human signature, verification required, the network denied,
agents given a list). imece names none: choosing cooperative there would weaken
what the person installed, and nothing weakens silently. It reads the `profile
hardened` line `init` prints (`lonca::Imported.hardened`) and the journal says it
(`…, from Lonca's hardened profile (lonca doctor says what it asks for)`,
`Event::StoreImported.hardened`, `serde(default)` for older lines). A hardened
repository refuses a key that can not prove presence; Lonca's own words say what to
do (a Secure Enclave key, or `lonca init --without-presence` by hand, then
`:lonca NAME`), and they reach the journal as any refusal does (probed: `invalid: a
hardened repository requires presence and this key can not prove it: …`).

**What a real daemon did** (`832c161`, by hand; CI has no daemon).

- A daemon that serves no repository can sign nobody in: `human login` and
  `status` say `no enrolled key of that human verifies this signature`, while
  `init` passes without a credential (bootstrap). So on a new daemon the first
  import comes before any session, and it would leave the project closed with
  `you are not signed in` right after the person said yes. The same yes
  therefore signs in, when `Place::open` says `SignedOut` (`run` in
  `live/lonca.rs`); what fails there is the project's `⌀ no lonca`, with its
  word, and `:lonca login` is one command away. `:lonca login` before any
  import shows the daemon's refusal in the journal.
- A session opened before an import reaches the repository made after it: no
  login per import.
- An import of a name that exists: `repo.prepare_init: conflict: repository demo
  exists`, shown as it is, the project left off Lonca.
- `lonca` says a refusal on stderr with exit 1, its last line kept (300
  characters, through `clean` on the screen, no token in it: Lonca's rule).

**Fail closed.** No program or out of time is `Failed::NoAnswer` and the journal
says so. A job whose thread died is a failed job. Exit 0 with odd output is
accepted: the store is asked at once and says how it really stands, so nothing
rests on parsing a line. A job does not hold the project open: `store_closed` is
as it was, and a project only goes on Lonca through a look that answers.

**The demo** (record 31). `Sim::store_job` takes the same `StoreJob`, three ticks
long. An import adds a row to the store's table, warns of a large file, every
second one says it is hardened, and is
refused as a real daemon refuses a name already in the table; a login says
`signed in to Lonca as demo`. The window is the same window. What can not be
faked: a real key, a finger, the daemon's bootstrap.

**Concurrency.** One job at a time (`Watch.job`), with its own thread beside the
looks, so a slow import never stops them; a second is refused in words, also
when it was asked for while its window stood open. Two imeces importing the same
name: the daemon refuses the second. **Growth.** A journal line a command.
Lonca's store grows by the history imported; that is Lonca's. **Read cost.**
`:lonca import` reads the branch and commit from git on the key's thread, as
`:open` reads its folder: two short calls, once. **Ten times the scale.** A
repository too large for 10 minutes fails with `lonca did not answer` while the
daemon may finish alone, and the next `:lonca NAME` finds it. **Migration.** New
journal events; the schema is unchanged.

Field fix (2026-11-09): the import window showed the checkout's whole
path (`/Users/<name>/…`); it is shown as every folder on the screen is,
`~` for the home (`App::shown_path`, record 103).

**The workers already hired (2026-11-09).** The import window has a line on
them when the project has workers: they move onto a workspace at their next
turn, and one whose worktree holds work stays on it, off Lonca, until its change
is in (record 119).
