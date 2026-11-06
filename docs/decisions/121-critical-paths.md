# A critical path waits for the person

The person's decision (2026-11-07). In cooperative mode every session is
`unobserved`, and Lonca's default refuses such a session's change to a
`[paths] critical` path (`Cargo.toml`, lockfiles, `package.json`, `.github/**`,
`build.rs`, the agents' instruction files), with nobody to ask: record 119's worker
says `blocked` and the person makes the change. imece's person reviews every
proposal anyway, so a repository `:lonca import` makes says what lonca#103 lets it
say: the change waits like any other, `critical_path:<path>` among its reasons.

**Two lines, not one.** Lonca's guide names `[observation] unobserved_critical =
"escalate"`. On a real daemon (`9bf19e0`) that alone is not enough for a worker of
imece's: a session that has called any tool of a server not in `[taint] mcp_servers`
is tainted, `[taint] critical` is a separate word, and the same change is denied
`tainted_critical_path`. Every worker calls the bus. So `Place::critical_waits` sets
both: `unobserved_critical = "escalate"` and `mcp_servers = ["imece"]`, the bus being
imece's own, not content from outside. Probed with each tool imece gives a worker
(`mcp__imece__send`, `Read`, `Glob`, `Grep`, `Edit`, `Write`, `Bash cargo test`, `Bash
git diff`, the task tools, `TodoWrite`): the change is pending, `escalate`.

**What stays refused, on purpose.** `[taint] critical` stays `deny`: a worker that has
used `WebSearch` or `WebFetch`, or a shell command that reaches the network (`curl`),
is tainted, and its change to a critical path is `tainted_critical_path`. That worker
says `blocked`, as before. Lonca's reason is imece's: content from outside entered the
session, and it may have been told what to write in `Cargo.toml`. The taint goes on
across a continued session. Opening this too is the person's word, not a default.
`unobserved_is_tainted` stays on: off, changes would integrate unseen, and imece
closes such a repository (record 120).

**How.** After `lonca init --import` and the login, on the import's own thread: the
client reads `lonca.toml` and `policy.cedar` of `status.policy` through `tree`,
`critical_opened` changes the two lines, and `lonca policy set --config --policy
--repo` signs it. The rules go back byte for byte. `lonca policy set` takes files, so
both stand for the call in a directory of their own (`with_files`: 0700, made there and
then, never one that was there, gone afterwards). Only Lonca's default text is touched:
each line must stand as Lonca writes it, or nothing is signed. A new repository has
that text; one named with `:lonca NAME` keeps its own policy, whatever it says.

**The person's yes.** The import window says it, a line of its own: a change to a
critical path waits instead of being refused, one more signature now. A Secure
Enclave key wants that one more finger. `a` covers both.

**Failure.** The import stands whether or not this goes: the repository exists and
imece can not take it back. `Event::StoreCriticalKept` says in the journal that a
worker's change to a critical path stays refused, with Lonca's word when it had one;
what stands is Lonca's default, the safe side. A crash between the two steps leaves the
same. To set it later the person runs `lonca policy set`; imece has no key for it.

**Read cost, growth.** Two `tree` reads and one child at an import, nothing after. A
policy commit in the store per import. **Concurrency.** The daemon is the sequencer;
a policy someone else signs meanwhile is read as it stands, and only Lonca's default
lines are changed. **Migration.** A repository imported before this keeps its policy.
`Event::StoreCriticalKept` is a new journal line, as ever an older build can not
read a store that has one.

**The demo** (record 31): the window's line is the same one. Its import can not fail
here, and it has no real policy: what can not be faked is a signature, a refusal by a
daemon and the taint itself.
