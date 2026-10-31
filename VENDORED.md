# Code copied from elsewhere

imece never changes Lonca or Kule and does not depend on them; what helps
is copied here and owned here from then on (`CLAUDE.md`, "Code from Lonca
and Kule"). Every copied file starts with `Copied from <repo>@<sha> <path>`.
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

The site's pages (`docs/guide`, `website/i18n`) and the video scenarios
(`website/video/scenarios`) are imece's own.
