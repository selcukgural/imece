# Releases and the one-line install

The person's ask (2026-11-01): install imece as Claude Code and Codex
install, with `curl … | sh`: built for the system, on PATH, in use at
once, no Rust toolchain. Decided with the person: macOS and Linux only,
the script on the docs site, the first tag pushed only when the person
says imece may be tried by others.

What a release is. `.github/workflows/release.yml` runs on a tag `vX.Y.Z`,
which must be the version in `Cargo.toml` (the build stops otherwise). It
builds `imece-tui` in release mode, every build to its end (`fail-fast`
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
`IMECE_DOWNLOAD_URL` (a mirror; it alone also allows `file://`, which
the test uses). Running it again updates; uninstalling is removing the
file and the lines.

Tests. `website/test-install.sh BINARY` makes a release from the binary
it is given (the archive, `SHA256SUMS`) in a temporary directory and runs
the installer against it with a clean `HOME` and `PATH`: bash (the line
once after two runs, no `.imece.new` left), zsh, fish, any other shell,
`IMECE_INSTALL_DIR` with `IMECE_NO_MODIFY_PATH=1`, a directory already on
PATH, a wrong sum, a `SHA256SUMS` without the archive, a missing release;
nothing is installed in the last three. CI's `installer` job runs it on
ubuntu and macOS against that run's debug build, after shellcheck. The
release workflow itself only runs on a tag: its build steps were checked
by hand (the aarch64 musl build in a container, static, its tests green;
the x86_64 macOS cross build run under Rosetta).

A new version needs its "what's new" lines (`Text::news`, both
languages, record 75): the tests fail on a version without them.

Gaps. Releases are not signed with a key of the person's own; the
attestation ties an archive to this repository's workflow and tag. The
installer trusts the TLS of github.com and github.io, as every `curl |
sh` does; reading it first is said in the docs. No self-update inside
imece and no package managers (Homebrew, apt) yet. The first tag
(`v0.1.0`) is the person's call.
