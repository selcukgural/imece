# imece's tour videos

Three short tours of imece in English, recorded from imece's real screen
running the simulation (`imece --demo`), so the videos show what the
terminal shows, at an even pace, and cost nothing to make. Nothing here is
part of the docs site's build; the finished videos go to
`website/static/video/` and the page `docs/guide/videos.mdx` plays them.

| File | What it is |
| --- | --- |
| `serve.sh` | Starts `ttyd` running `imece --demo` (the release binary, a state directory of its own, so your `imece.sqlite` is never read) on `127.0.0.1`, and `serve.mjs` in front of it. |
| `serve.mjs` | One origin for the tour: the terminal page from ttyd (websocket included) and the title cards from this folder. It adds a `#corner` span inside the terminal's element, where the tour clicks to type so the pointer stays out of the way. |
| `card.html`, `card.css`, `card.js` | The opening card (`card.html?t=…&s=…&l=…`); the closing card is PromoVideo's own `card` step. |
| `scenarios/tour.json`, `review.json`, `team.json` | The scenes, narration and keys, in PromoVideo's format (the tool lives in the AfterApply repository, `tools/PromoVideo`; its README has the schema). |

## How the keys reach the screen

PromoVideo drives a Chrome page: ttyd puts the terminal on one (xterm.js),
and a `type` step's characters arrive as typed text, `press` sends enter,
escape, tab and the arrows. So a scenario can send every key imece reads
except the control and alt ones (`^o`, `⌥b`, `⇧enter`): those are told, not
shown. ttyd starts a fresh `imece --demo` per page load, so a scenario opens
`/` once and shows its closing card with the `card` step, not with `goto`.

The simulation steps on the wall clock (one step every 500 ms at ×1, `+`
doubles the speed up to ×8), and the scenario's keys arrive on the wall
clock too, so a run differs a little from the last one: the steps name what
to show, not a row the viewer must find. After 20 s without a key, the next
key opens the away digest instead of acting; the scenarios keep their keys
closer than that.

## Render

PromoVideo needs Google Chrome, `ffmpeg`, the .NET SDK, and `ttyd`
(`brew install ttyd`).

```sh
# 0. The binary the tour runs.
cargo build --release -p imece-tui

# 1. Serve the terminal and the cards (leave it running).
website/video/serve.sh 8791

# 2. From the AfterApply repository: check a scenario, then render a draft
#    with the macOS voice (drafts only: Apple's licence is personal).
cd ~/RiderProjects/AfterApply
dotnet run --project tools/PromoVideo -- validate ~/RiderProjects/imece/website/video/scenarios/tour.json
dotnet run --project tools/PromoVideo -- render   ~/RiderProjects/imece/website/video/scenarios/tour.json --provider say --voice Samantha
```

The videos to publish use the voice the scenarios name, Google Cloud
Text-to-Speech (Gemini-TTS), through the gcloud configuration whose project
has the Text-to-Speech API enabled:

```sh
for s in tour review team; do
  CLOUDSDK_ACTIVE_CONFIG_NAME=default dotnet run --project tools/PromoVideo -- render \
    ~/RiderProjects/imece/website/video/scenarios/$s.json
done
```

Output: `~/RiderProjects/AfterApply/artifacts/promo-video/<scenario>/`:
`video.mp4` (1920×1080, H.264 + AAC, a subtitle track inside),
`subtitles.srt`, `chapters.txt` and `narration.wav`.

## Publish

```sh
website/video/publish.sh    # copies the three videos, their captions (vtt) and posters (jpg) into website/static/video/
```

Then build the site (`cd website && npm run build`) and open the videos page.
A video is re-rendered when what it shows changes: a key it presses, a screen
it names, a word of the narration that is no longer true.

## Changing a tour

- Scenes keep their keys under 20 s apart (the away digest, above).
- A chapter must last at least 10.5 s, or YouTube drops the chapters;
  `minSeconds` holds the short ones.
- Only `type`, `press` (Enter, Escape, Tab, ArrowDown, ArrowUp, Backspace),
  `wait`, `hover`, `card` and `goto` are of use on a terminal page; the
  target of a `type` is `#corner`.
- Interaction changes what the simulation does next (a message, `a`, `r`
  and an allowed ask use random draws), so a scene that needs a change
  waiting speeds the simulation up first (`+++`, a few seconds, `---`).
