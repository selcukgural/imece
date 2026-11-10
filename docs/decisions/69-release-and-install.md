# Releases and the one-line install

The person's ask (2026-11-01): install imece as Claude Code and Codex
install, with `curl … | sh`: built for the system, on PATH, in use at
once, no Rust toolchain. Decided with the person: macOS and Linux only,
the script on the docs site, the first tag pushed only when the person
says imece may be tried by others.

What a release is. `.github/workflows/release.yml` runs on a tag `vX.Y.Z`,
which must be the version in `Cargo.toml` (the build stops otherwise). It
builds the crate `imece` (`crates/imece-tui`) in release mode, every build to its end (`fail-fast`
off, so one failure shows all), for four targets: `aarch64-apple-darwin`
and `x86_64-apple-darwin` (on macOS arm64; the x86_64 build is not run
there, Apple silicon may lack Rosetta), `x86_64-unknown-linux-musl` and
`aarch64-unknown-linux-musl` (each on its own CPU, with musl-tools for the
bundled SQLite; checked statically linked, so any distribution runs it:
`file` says `statically linked`, or `static-pie linked` for x86_64, which
the first v0.1.0 run missed and stopped on; the tag, which had no
release yet, was moved to the fix with the person's consent).
Each build runs `imece --version` against the tag. The archives are
`imece-<target>.tar.gz`, with `imece` and `LICENSE` at their top and no
version in the name, so `releases/latest/download/…` always names the
newest. The publish job writes `SHA256SUMS`, a build provenance
attestation per archive (`gh attestation verify … --repo
selcukgural/imece`), and the release with generated notes. Only that job
may write (`contents: write`) or mint a token (`id-token`,
`attestations`); the builds, which run crates' build scripts, see no
credential. Actions are pinned to commit SHAs; event data goes through
`env:`. No Windows: imece needs a pty, Unix signals and `libc`'s terminal
calls; WSL runs the Linux build.

`imece --version` (or `-V`, first) prints `imece X.Y.Z` and leaves with 0;
the release and the installer check the binary by it.

The installer. `website/static/install.sh`, served at
`https://selcukgural.github.io/imece/install.sh` by the docs site
(`docs.yml`), POSIX `sh` (dash and macOS's sh), shellcheck clean.
Everything runs from `main` at the end, so a download cut short runs
nothing. It maps `uname` to a target (Rosetta on Apple silicon gets the
native build), downloads the archive and `SHA256SUMS` over HTTPS only
(`--proto =https`, redirects too, TLS 1.2; wget `--https-only` when there
is no curl), and installs nothing when the archive's sum is missing or
does not match. It extracts only the `imece` member, copies it next to
the old one and moves it over (a running imece keeps its file; no
half-written file on PATH), and runs `--version`. Default place
`~/.local/bin` (no sudo). When that is not on PATH it adds one line, once
(grep for the exact line first), under `# imece` in the shell's startup
file: zsh `${ZDOTDIR:-~}/.zshrc`, bash `~/.bashrc` (`~/.bash_profile` on
macOS, whose terminals open login shells), fish
`conf.d/imece.fish` with `fish_add_path`, else `~/.profile`; it says the
line to run now. When the directory is on PATH but another `imece`
comes first (a `cargo install` in `~/.cargo/bin`), it says so. It ends
with the coding agents it found on PATH, or how to get one. Settings:
`IMECE_VERSION`, `IMECE_INSTALL_DIR`, `IMECE_NO_MODIFY_PATH=1`,
`IMECE_NO_ICON=1` (record 101's tab icon), `IMECE_DOWNLOAD_URL` (a
mirror; it alone also allows `file://`, which the test uses). Running it again updates; uninstalling is removing the
file and the lines.

Tests. `website/test-install.sh BINARY` makes a release from the binary
it is given (the archive, `SHA256SUMS`) in a temporary directory and runs
the installer against it with a clean `HOME` and `PATH`: bash (the line
once after two runs, no `.imece.new` left), zsh, fish, any other shell,
`IMECE_INSTALL_DIR` with `IMECE_NO_MODIFY_PATH=1`, a directory already on
PATH, a wrong sum, a `SHA256SUMS` without the archive, a missing release;
nothing is installed in the last three; the tab icon (record 101): on
macOS iTerm2's tables, on Linux the icon theme and iTerm2's folder left
alone. CI's `installer` job runs it on
ubuntu and macOS against that run's debug build, after shellcheck. The
release workflow itself only runs on a tag: its build steps were checked
by hand (the aarch64 musl build in a container, static, its tests green;
the x86_64 macOS cross build run under Rosetta).

A new version needs its "what's new" lines (`Text::news`, both
languages, record 75): the tests fail on a version without them.

Version numbers (user, 2026-11-03): from `0.1.202611030053` on, a
release's patch number is the local date and time it was made,
`0.1.YYYYMMDDHHMM`; each is still a plain `MAJOR.MINOR.PATCH`, larger
than the one before, so the tag check, `releases/latest` and the news
order hold. `welcome::parse_version` reads the numbers as `u64` (a
stamp is past `u32`).

Gaps. Releases are not signed with a key of the person's own; the
attestation ties an archive to this repository's workflow and tag. The
installer trusts the TLS of github.com and github.io, as every `curl |
sh` does; reading it first is said in the docs. No self-update inside
imece and no package managers (Homebrew, apt) yet. The first tag
(`v0.1.0`) is the person's call.

Acceptance fixes (2026-11-09): `IMECE_DOWNLOAD_URL` said it "may also be
a local directory", but a bare path reached curl as is and failed (`URL
rejected: No host part`). `main` now takes `https://…` and `file://…` as
they are, turns a directory's absolute path into `file://PATH`, and
refuses anything else (a relative path, `http://`, another scheme) with
one line before downloading; `website/test-install.sh` case 7b installs
from `$release/` and checks the refusals.

## Homebrew and crates.io (2026-11-10)

The person's ask (2026-11-10): the release, a Rust crate and a Homebrew
package, and the three ways to install in the README and on the site as
Lonca has them. Done as Lonca does it, so the two read and work alike.

**The crate.** The package `imece-tui` is named `imece`, after its
program (`cargo install --locked imece`, Rust 1.99 or newer); its
directory keeps its name. `imece-agent` is published with it, since a
crate on crates.io can depend only on crates there: its description says
its API has no stability promise. The two name each other at the
workspace's version exactly (`[workspace.dependencies]`, `=X.Y.Z`): a
version changed without that line fails the build. `imece_core` on
crates.io is someone else's and has nothing to do with imece.

**Publishing.** The first version of each crate is published by hand
(`cargo publish --workspace --locked` from the tag): crates.io's trusted
publishing needs a crate to exist. From then on `release.yml`'s last two
jobs do it, Lonca's `registries.yml` moved into this workflow (a release
made with the workflow's own token starts no other workflow, and imece's
release is published by the workflow, not drafted): `crates-check`
packages and builds without a token, `crates` uploads with `--no-verify`
under a token crates.io hands out for that job in the `release`
environment. Both are off until the repository variable
`PUBLISH_CRATES` is `true`, which waits for the person to set trusted
publishing on crates.io for the two crates (workflow `release.yml`,
environment `release`). No registry token is kept anywhere.

**Homebrew.** The tap `selcukgural/homebrew-imece`
(`brew tap selcukgural/imece`, `brew install imece`), copied from
`selcukgural/homebrew-lonca`: its `update.yml` reads imece's latest
release every day and on demand, checks every archive against
`SHA256SUMS` and its attestation (signed by imece's `release.yml` on the
tag), writes `Formula/imece.rb` with `scripts/formula.sh`, installs and
tests it on macOS and Linux, and commits it with its own token. imece's
repository holds no token for the tap. The formula installs the same
archive the installer does; the tab's icon stays the installer's.

Homebrew 7 loads a formula of a tap that is not its own only after
`brew trust --formula selcukgural/imece/imece` (found on the person's
machine the day the tap was made, 2026-11-10); the README and the install
page say so. The tap's own test in CI passed without it.

Growth: one formula, two crates a release. Failure: a release whose
crates did not go up is still a release (the archives and the installer
do not wait for them); `cargo publish` again from the tag finishes it,
and a version on crates.io can only be yanked, never replaced, which is
why the upload is last. Migration: someone with `~/.cargo/bin/imece` from
`cargo install --path` keeps it; `cargo install imece` replaces it.

Since record 147 (2026-11-10) a tag's first job is the acceptance gate
(`tools/acceptance.py gate`): nothing is built or published without
`docs/acceptance/runs/vX.Y.Z.md` passed on the commit that is tagged, so
the version pull request's last commit is the run's report.
