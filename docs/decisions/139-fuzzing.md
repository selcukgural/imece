# Fuzzing

The person, 2026-11-10: imece's fuzz tests run every evening at a set
time, as Lonca's do. Lonca's way is taken whole (its `fuzz.yml` at
`d1d761b`, `docs/fuzzing.md`): libFuzzer through cargo-fuzz 0.13.2 with
`-s none` (the compiler of `rust-toolchain.toml`, no nightly: only
AddressSanitizer needs one, and imece's Rust has no `unsafe` to check),
`-max_len=65536 -timeout=30 -rss_limit_mb=2048`.

**Targets.** Each is `pub fn check(data: &[u8])` in `crates/imece-fuzz`
that panics when what it holds does not; one place where imece reads
bytes it did not write:

| Target | Input | Holds |
|---|---|---|
| `stream` | a session byte, then a Claude Code session's stdout (`claude::Parser`) | any line read or refused without a panic, the same way twice (two parsers stop at the same line with the same events) |
| `bus` | lines to imece's MCP server (`mcp::serve`) | each reply one line of JSON-RPC 2.0 with an id and a result or an error, never both; no more replies than lines; only offered tools called |
| `approve` | the permission prompt tool's arguments (`claude::ask`, `claude::decision`) | a shell command read as given; a yes gives the input back exactly; a no carries none |
| `prompt` | a prompt's text (`claude::prompt_line`) | one line whatever the text, read back as a user message of exactly it |
| `split` | the `run` tool's command line (`run::split`) | quoted again, its words split the same; nothing a shell reads let through outside quotes |
| `tidy` | a command's output (`run::tidy`) | no escape, bell, carriage return or other control but a line end and a tab |
| `patch` | a byte choosing hunks, then a patch (`worktree::subset`) | lines of the patch in order; no hunk gives nothing; the cut patch cut again whole is itself |
| `paths` | a card's `paths` and a written path (`paths::parse`, `strays`) | at most 20, each non-empty, ≤ 200 characters, no control, relative, no `..`, none twice; a card's own path never strays |
| `clean` | agent text (`i18n::clean`, `fill`) | no control character; cleaning again changes nothing; `fill` puts the cleaned text in place |

The screen is a program, not a library: `i18n.rs` and `paths.rs`, which
name nothing else of it, are taken in whole by `#[path]` (`imece-fuzz`'s
`lib.rs`), so the fuzzer runs the screen's own code; a change that makes
either depend on the rest of the screen breaks `cargo test --workspace`
at once. Their unit tests stay the screen's (`[lib] test = false`).

**Every crash becomes a test.** `fuzz/seeds/<target>/` are the starting
inputs, written by `cargo test -p imece-fuzz --test inputs -- --ignored
write_seeds`; `stream_seeds_get_past_init` keeps the session seeds past
`init`, so the fuzzer starts from a session that runs. A fixed crash's
input goes to `fuzz/regressions/<target>/`; `cargo test --workspace` runs
every seed and regression through its target (`tests/inputs.rs`), which
also checks that `TARGETS`, `fuzz/Cargo.toml`'s binaries, `fuzz_targets/`
and `fuzz.yml`'s list agree. Keep a crash out of issues and pull request
text until it is fixed: the input is the exploit.

**CI.** `.github/workflows/fuzz.yml`: every evening on main at 18:37 UTC
(21:37 in Türkiye), 30 minutes a target, each its own job; on demand
(`gh workflow run fuzz.yml --ref main`, 10 minutes, `-f seconds=N`,
`-f targets=stream,bus`; an unknown name fails the run); on a pushed `v*`
tag (10 minutes, from the seeds alone: no cache restored or saved, so a
release run can not be poisoned). Pull requests do not run it and nothing
requires it. Each target keeps its corpus in the Actions cache. The
repository is public, so the log says only which target found what kind
of failure (`crash`, `timeout`, `oom`); the input and the fuzzer's output
are uploaded as `fuzz-<target>`, encrypted with age to
`fuzz/crash-recipient.txt`, the same key as Lonca's:

```sh
gh run download <run> -n fuzz-<target>
age -d -i ~/.config/lonca/fuzz-crashes.age-key fuzz-<target>.tar.gz.age | tar xz
```

`fuzz/` is its own workspace with its own lock: `deny.yml` checks it and
Dependabot updates it; `imece-fuzz` is `publish = false` and left out of
the release's `cargo publish --workspace`. The lock names the workspace's
version (`imece-agent`, `imece-fuzz`), so a version bump updates it too
(`cargo update -p imece-agent -p imece-fuzz` in `fuzz/`): the
v0.1.202611111627 tag's run stopped every target at `--locked` without a
finding. CI's `test` job now fails a lock left behind
(`cargo metadata --locked --manifest-path fuzz/Cargo.toml`).

**First finds** (before the first run, two minutes a target on a laptop):
a yes gave a tool call's input back with a float off in its last place
(serde_json's quick float reading; `float_roundtrip` on in `imece-agent`);
`worktree::subset` cut every line at `str::lines`, so a CRLF file's
carriage returns were lost from a part the person applies (now
`split_inclusive`, each line's end kept). Both inputs are regressions.

**Gaps.** Found while the targets were written and fixed after: `strays`
looked at every directory a written path is under one by one, each
against every pattern, quadratic in the path's length (a path of 50,000
directories took hours; `paths::prefixes` answers for all of them in one
pass now, and `paths` tries a written path of any length); a card that
owned `././x` kept `./x`, and a write to `x` strayed from it (paths are
kept `paths::normal` now, every `.` and empty part dropped, on both
sides). Not targets
yet: the key file (`keys.rs`, the person's own), Lonca's replies (the
daemon is checked to be the person's), the pty output (`shell`), the
Markdown of a conversation, a standing yes's command split
(`live::standing`): those live in the screen with the rest of it, and
need it to become a library or the code to move into `imece-agent`.

- **Growth**: the corpus per target in the Actions cache (GitHub evicts
  the least used past 10 GB); an encrypted artifact kept 14 days.
- **Read cost**: none at run time; the nightly takes nine jobs of about
  35 minutes on GitHub's runners.
- **Failure**: a red run holds nothing; a build or run that fails without
  an input says so in the log and uploads nothing.
