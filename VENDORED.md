# Code copied from elsewhere

imece never changes Lonca or Kule; it depends on Lonca's daemon as a client
(record 115) and on Kule not at all; what helps is copied here and owned
here from then on (`CLAUDE.md`, "Lonca and Kule"). Every copied file starts with `Copied from <repo>@<sha> <path>`.
A scheduled check will list upstream commits that touched these paths since
the recorded sha, so a fix upstream is not missed.

| Here | From | Commit | Date | Local changes |
| --- | --- | --- | --- | --- |
| `website/docusaurus.config.js` | kule `website/docusaurus.config.js` | `c09ec13` | 2026-11-01 | title, colours, URLs, navbar and footer for imece; docs path `../docs/guide`; `diff` added to Prism |
| `website/sidebars.js` | kule `website/sidebars.js` | `c09ec13` | 2026-11-01 | imece's pages |
| `website/src/css/custom.css` | kule `website/src/css/custom.css` | `c09ec13` | 2026-11-01 | amber instead of coral, `imece-` class names, `kbd`, the home video |
| `website/src/pages/index.js` | kule `website/src/pages/index.js` | `c09ec13` | 2026-11-01 | imece's hero, loop, features, the tour video, the start block |
| `.github/workflows/docs.yml` | kule `.github/workflows/docs.yml` | `c09ec13` | 2026-11-01 | paths `docs/guide`, Pages URL, `website/check.sh` before the build |
| `website/video/serve.sh` | kule `website/video/serve.sh` | `c09ec13` | 2026-11-01 | rewritten: ttyd runs `imece --demo`, `serve.mjs` adds the cards on the same origin |
| `website/video/card.html`, `card.js`, `card.css` | kule `website/video/card.*` | `c09ec13` | 2026-11-01 | imece's mark and colours |
| `website/package.json`, `package-lock.json` | kule `website/package.json`, `package-lock.json` | `c09ec13` | 2026-11-01 | name and description |

`.github/workflows/fuzz.yml` is lonca's (`d1d761b`, 2026-11-09): every
evening at 18:37 UTC instead of 03:37, imece's targets in its `plan` job.
The layout of `fuzz/` (its `Cargo.toml`, the two-line files in
`fuzz_targets/`, `seeds/` and `regressions/`) and of `crates/imece-fuzz`
(`TARGETS`, `choice`, `tests/inputs.rs`) follows lonca's `fuzz/` and
`crates/lonca-fuzz` at the same commit; the targets and their checks are
imece's own.
`fuzz/crash-recipient.txt` is the same age key as Lonca's: one key opens
what either fuzzer found.

The `crates-check` and `crates` jobs of `.github/workflows/release.yml`
are lonca's `.github/workflows/registries.yml` (`9fbbeb7`, 2026-11-10),
moved into the release workflow (a release made with the workflow's own
token starts no other workflow) and without Lonca's excluded crates. The
Homebrew tap `selcukgural/homebrew-imece` is a repository of its own,
copied from `selcukgural/homebrew-lonca`.

Built against, not copied: `crates/imece-agent/src/lonca.rs` speaks Lonca's
daemon protocol **number 1** as `docs/protocol.md` of lonca `v0.2.1`
(`d802fd6`, 2026-11-09; a daemon older than 0.2.1 is refused; with `session start --continue` and `--harness`, `change.diff`, `reject`, `git` in a workspace, a clean end kept in `status.sessions[].unproposed` and the reasons `unhooked_session`, `unhooked_critical_path` and `verification_missing`, a newer proposal's `replaces:` and `status.policy_checks`) writes it; a daemon of another
number is refused. The protocol
says a client copies nothing from its crates, so none of them is here: the
socket, its peer, the framing and the session file come from Lonca's
published `lonca-client` crate (`=0.2.3` since 2026-11-11, a dependency of `imece-agent`,
record 116), not a copy.

The site's pages (`docs/guide`, `website/i18n`) and the video scenarios
(`website/video/scenarios`) are imece's own.
