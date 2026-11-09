# imece's tour videos

Three short tours in English, each recorded from a **real** imece session:
real Claude Code agents (a sonnet leader, haiku and sonnet workers) on a
small Python repository, with real tests, a real Lonca daemon and the keys
a person presses. Nothing is simulated, so what the narration says is what
happened on the screen. The recordings are kept here (`casts/`), so the
videos can be cut, narrated and rendered again without paying for the
agents twice. Nothing here is part of the docs site's build; the finished
videos go to `website/static/video/` and the page `docs/guide/videos.mdx`
plays them.

| Tour | What it shows |
| --- | --- |
| `first-change` | `imece` in a checkout, the welcome, a task to the leader, a worker, the change's verdict and diff, `a`, `:commit`, quitting |
| `team-on-lonca` | the next day: `:lonca import`, a team task, the plan, two workers in Lonca workspaces, `:code`'s leases, Lonca's tests after the yes, `:sync` |
| `when-they-ask` | a question with options answered by a digit, notes on a change sent back, a permission given with `A` |

| File | What it is |
| --- | --- |
| `record.sh` | Drives a recording: a clean shell in tmux at 120×40 under `asciinema rec`, keys typed one at a time, and a log of every key (with the words its badge shows), every scene and every wait. |
| `plan.py` | The log → an edit file: badges, scene markers, and each wait sped up (the corner says `4× speed`). |
| `cut.py` | The edit file + the raw recording → the cast the tour plays. Nothing the program wrote is dropped (the screen is the sum of every byte); a span made faster or instant only loses time. |
| `casts/*.cast` | The cut recordings the scenarios play, and their edit files (the raw recordings stay with whoever recorded them). |
| `cast.html` | Plays a cast with asciinema-player (3.10.0, from jsDelivr) in JetBrains Mono, pauses at each scene, draws a key's badge below the terminal and the speed in the corner. ArrowDown plays the next scene. |
| `card.html`, `card.css`, `card.js` | The opening and closing cards (`card.html?t=…&s=…&l=…`). |
| `scenarios/*.json` | The scenes and narration, in PromoVideo's format (the tool lives in the AfterApply repository, `tools/PromoVideo`; its README has the schema). |
| `publish.sh` | Copies the renders into the site, compressed, with WebVTT captions and a poster. |

## Record

Needs `tmux`, `asciinema` (3.x), Claude Code signed in, and `lonca` for the
second tour. Use a repository of its own (the tours used a fresh `shelf`:
a catalogue, JSON storage and a CLI, `python3 -m unittest`, stdlib only) and
a state directory of its own, so your own `imece.sqlite` is never touched:

```sh
mkdir -p /tmp/imece-rec/bin /tmp/imece-rec/zdot
cat > /tmp/imece-rec/bin/imece <<'EOF'
#!/bin/sh
export XDG_STATE_HOME=/tmp/imece-rec/state XDG_CONFIG_HOME=/tmp/imece-rec/config
export LONCA_HOME=/tmp/imece-rec/lonca LONCA_HUMANS=/tmp/imece-rec/humans LONCA_SOCKET=/tmp/imece-rec/l.sock LONCA_HUMAN=dev
PATH=$HOME/.local/bin:$PATH exec /path/to/imece --theme dark --model sonnet --budget 10 "$@"
EOF
chmod +x /tmp/imece-rec/bin/imece
printf "PS1='%%F{green}%%~%%f \$ '\nunsetopt PROMPT_SP\n" > /tmp/imece-rec/zdot/.zshrc
export IMECE_REC=/tmp/imece-rec IMECE_PATH=/tmp/imece-rec/bin

r=website/video/record.sh
$r start first-change
$r mark intro; $r type "" -- "cd ~/code/shelf"; $r quiet Enter
$r mark start; $r type "" -- "imece"; $r quiet Enter
$r key Escape esc "close the welcome"
$r mark task; $r key / / "write to the leader"; $r type "" -- "Make find ignore case …"; $r key Enter enter "send it"
$r wait   # … the agents work; `$r snap` shows the screen …
$r go; $r mark change
…
$r quiet C-d    # ends the shell, and the recording
```

A scene starts at each `mark`; `wait` … `go` spans are sped up. Keep the
keys a real person would press, and let things take the time they take: the
cut decides what the viewer waits for.

## Cut

```sh
cd /tmp/imece-rec
python3 -I website/video/plan.py first-change.cast first-change.log first-change.edit.json --wait-seconds 4
python3 -I website/video/cut.py first-change.edit.json website/video/casts/first-change.cast
```

The edit file can be changed by hand: a span's factor (`0` makes it
instant, with no badge; used for anything done off camera, such as signing
Lonca's policy), a badge's words, a marker dropped. `cut.py` prints each
scene's length; a scenario's `minSeconds` for a scene is its length plus a
little, so the narration never runs over the next scene.

## Render

PromoVideo needs Google Chrome, `ffmpeg` and the .NET SDK. Serve `website/`
(the cards use the site's mark from `static/img`):

```sh
python3 -I -m http.server 8792 --bind 127.0.0.1 -d website &

cd ~/RiderProjects/AfterApply
dotnet run --project tools/PromoVideo -- validate ~/RiderProjects/imece/website/video/scenarios/first-change.json
# A draft with the macOS voice (drafts only: Apple's licence is personal):
dotnet run --project tools/PromoVideo -- render ~/RiderProjects/imece/website/video/scenarios/first-change.json --provider say --voice Samantha
# The videos to publish: the voice the scenarios name, Google Cloud TTS (Gemini-TTS):
for s in first-change team-on-lonca when-they-ask; do
  CLOUDSDK_ACTIVE_CONFIG_NAME=default dotnet run --project tools/PromoVideo -- render \
    ~/RiderProjects/imece/website/video/scenarios/$s.json
done
```

The viewport is 1920×1080 at a device scale of 2, so the terminal's text is
drawn at twice the pixels and stays sharp after compression.

## Publish

```sh
website/video/publish.sh            # all three, from AfterApply's artifacts
website/video/publish.sh "" when-they-ask
```

Then build the site (`cd website && npm run build`) and open the videos page.

## When to record again

When what a tour shows changes (a key it presses, a screen it shows, a word
of the narration that is no longer true), record that tour again on the
imece of the day: the old cast shows the old screen, and the narration is
written against what the recording shows, never the other way round.
