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

Built against, not copied: `crates/imece-agent/src/lonca.rs` speaks Lonca's
daemon protocol **number 1** as `docs/protocol.md` of lonca `010ba7e`
(2026-11-08, with `session start --continue` and `--harness`, `change.diff`, `reject` and `git` in a workspace) writes it; a daemon of another
number is refused. The protocol
says a client copies nothing from its crates, so none of them is here.

The site's pages (`docs/guide`, `website/i18n`) and the video scenarios
(`website/video/scenarios`) are imece's own.
