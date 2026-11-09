# The tests a checkout calls for

A real session (2026-11-09) ran imece on a small Python project whose
tests are `python3 -m unittest` (stdlib only, its `CLAUDE.md` names the
command). Every test run of every worker stopped on a permission ask
(`run Bash: python3 -m unittest 2>&1 | tail -5?`): the routine rule
(record 30) and the run tool (record 10) knew cargo, git's reading
commands and `--run`'s programs only. `A` (record 89) gave a yes to that
whole line only, since a rule is never "an option first" and
`python3 -m unittest` starts with one. The run tool refused `python3`
("these are: cargo, git"), so the worker tested through Bash and the
change's verdict said `! look first · nothing ran` though its tests had
passed: every non-Rust project's first change said nothing ran. The
person decided (2026-11-09): imece learns a repository's test commands
from the repository, so a Python, JavaScript or Go user does not say yes
to every test run and their runs count on the Δ verdict as cargo's do.

What the files can unlock is a fixed list of test shapes per language,
never an interpreter or a package manager whole (`runners.rs`, pure):
`Runners { python, node: Option<Node>, go }` from the checkout's files
(`runners::of`): Python for `pyproject.toml`, `setup.py`, `setup.cfg`,
`tox.ini`, `pytest.ini` or a `requirements*.txt` at the top, or a
`test_*.py` / `*_test.py` anywhere (the stdlib-only project of the
session has none of the marks, only its tests); JavaScript for a
`package.json` at the top, the manager by lockfile (`pnpm-lock.yaml`
first, then `yarn.lock`, else npm); Go for a `go.mod` at the top. The
shapes (`Runners::check`, `None` when the program is none of them):
`python3|python -m pytest …`, `python3|python -m unittest …`, `pytest …`;
`MANAGER test` and `MANAGER run test`, nothing after them but `--` and
words for the test script; `go test …`. The words after a shape go
through the routine rule's checks as before (one line, `run::split`: no
pipe, redirection, `&&`, `$`, glob) and then an allow-list per program
(`Options`: flags, letters that go together, options with a value):
pytest takes `-k -m -n -r --tb --maxfail --durations --color` with a
value and `-q -v -x -s -l --lf --ff --nf --sw --co --no-header …`
without; unittest `-v -q -f -b -c --locals` and `-k -p -s -t --durations`
(`discover`'s too); `go test` `-v -race -short -failfast -cover -json
-benchmem` and `-run -skip -count -timeout -bench -benchtime -p
-parallel -cpu -tags -list -shuffle -covermode`, one dash or two, its
packages `.` or `./…` only. Everything else is refused, because it loads
or runs more than the repository's tests or writes outside: `pytest -p`
(a plugin by name), `-c` and `-o` (another configuration), `-W` (imports
a category), `--pyargs`, `--rootdir`, `--confcutdir`, `--basetemp` (a
directory pytest empties), `--junitxml` and every other writer, `@file`
(pytest ≥ 8.2 reads arguments from it, past these checks), `--pdb`;
`go test -exec`, `-toolexec`, `-ldflags`/`-gcflags` (`-extld` runs a
program), `-o`, `-c`, `-coverprofile` and the other profiles, `-args`,
`-C`, `-modfile`, `-overlay`, a package by import path; anything of
npm's, pnpm's or yarn's own before `--` (`--script-shell`, `--prefix`,
`--workspace`: npm reads its options anywhere before `--`). No word may
name a path outside the directory (`runners::inside`): one starting `/`,
`~`, `\` or `@`, a `..` part, also after `=`, `:` or `,`, and a value
joined to a one letter option (`-s../x`) is read as such. The npm words
after `--` are the test script's, which the repository writes anyway;
only the path check applies to them.

The trust is cargo's: `cargo test` runs a `build.rs` and the crate's
tests, these run a `conftest.py`, the test script of `package.json`, a
`.npmrc`, `TestMain`. An agent that can write the repository can make
any of them do anything, and could before this record through `cargo
test`; the person's yes that counts is on the change (the Δ gate). What
is new is only that a Python, JavaScript or Go checkout gets what a Rust
one had. The shapes are read from the person's checkout (`Place.repo`),
never from a worker's worktree or workspace, so a worker's own new
`package.json` or `test_x.py` unlocks nothing until the person applies
it. Python module names given to unittest (`tests.test_x`) import what
Python finds on its path, the directory first; an installed module's
import side effects are the one thing this reaches past the repository,
taken as cargo's `-p` of a dependency is. The run tool's environment is
cleared as for cargo (`child::BASE_ENV` + `run::BUILD_ENV`): `HOME` and
`PATH` pass (an activated venv's interpreter, nvm's node, Go's default
caches under `HOME`), `GOFLAGS`, `PYTHONPATH`, `NODE_OPTIONS` and the
like do not, so the environment cannot add an `-exec`; through Claude
Code's shell a routine test has that shell's environment, as `cargo
test` has.

Where it runs: the git thread lists the checkout's files at a project's
opening and every `FILES_EVERY` while agents work (record 67); the same
job computes the runners from that list (`GitDone::Files.runners`), and
`Live::filed` keeps them on `Project.runners` (memory only, read again
at every open). A change is journaled (`Event::TestsFound { leader,
commands }`, `ev_tests_found` / `ev_tests_none`), so what runs without
the person never grows unseen; an ask that came before stays the
person's, since its answer would say they gave it. `tools::allowed` takes
the project's runners beside `--run`'s programs: cargo and git as
before, then the person's program (whole, for workers and helpers), then
the shapes, for every role (a leader runs `cargo test` too); a refusal
lists them (`; and this repository's tests: …`). So `routine`, the run
tool (whose runs a worker's Δ counts, record 112's verdict included), a
standing yes's "routine" closure (`standing_yes`, `always`) all see
them. An agent is told: the run tool's description says "the tests this
repository's files call for, when it is Python, JavaScript or Go
(whoami names them)" (its socket is bound once, before files are read);
`whoami` ends with `tests that run at once, through the run tool too:
…`; the system prompt ends with `prompts::tests_note` (the commands, one
program with no pipe, prefer the run tool), from what `Project.runners`
holds when the session starts: the very first leader of an open may
start before the list is back and learns them from `whoami`.

Standing yes (record 89): a Python interpreter's `-m MODULE` is a rule
(`standing::rule_of`: program `python`, `python3`, `python3.12`, then
`-m`, then a dotted name of letters, digits and `_`; stored as `sub:
"-m pip"`, shown `python3 -m pip …`, `:allow python3 -m pip` names it).
It holds that module run by that interpreter with any words after it,
not another module, not `-c`, not `python3 x.py` and not an option before
`-m`. It is as wide as `npm run …`, which the person can already give,
and narrower than `--run python3`; the module is what the person reads.
Without it `A` on any `python3 -m mypy …` could only say yes to that one
line. A stored rule an older imece reads compares `-m pip` with the
first word and matches nothing: it fails closed.

Read cost: one pass over the listed files on the git thread per listing
(≤ `FILES_MAX` 20,000 names); a check is a few comparisons of a
command's words. Growth: nothing kept but three flags a project.
Failure: a listing that fails keeps the last runners; a list cut at
`FILES_MAX` may miss a top-level mark that sorts after the cut (the
test files usually still say Python). Demo (record 31): each kit has its
runners (web: npm, billing: go), the start journals them as live does,
a web or billing worker's ask for `npm test -- tests/cart.test.ts` or
`go test -run TestTotal ./internal/invoice` goes on with `allowed on its
own`, and the demo's yes from now on and its standing check count them
as live's do (`crate::live::routine`).

Not done: a folder without git reads no runners (record 73 has no file
list there); `uv run pytest`, `poetry run pytest`, `tox`, `.venv/bin/python`
(a path), `bun test`, `deno test`, `npx vitest`, `make test`: those still
need `--run` or a yes; a pipe into `tail` or `head` still asks, as it
does for cargo (the agents are told to run one program); the npm script's
own options after `--` are not judged (the script is the repository's);
a `package.json` or `go.mod` below the top (a monorepo's packages) is not
read.
