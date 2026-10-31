#!/bin/sh
# Copies the rendered tours into the site: the video, its captions as WebVTT
# and a poster frame. Usage, from anywhere, after the renders (README.md):
#
#   website/video/publish.sh [artifacts dir]   # default ~/RiderProjects/AfterApply/artifacts/promo-video
set -eu
here=$(cd "$(dirname "$0")" && pwd)
out="$here/../static/video"
from=${1:-$HOME/RiderProjects/AfterApply/artifacts/promo-video}
mkdir -p "$out"
for s in tour review team; do
  d="$from/$s"
  [ -f "$d/video.mp4" ] || { echo "no render at $d/video.mp4" >&2; exit 1; }
  cp "$d/video.mp4" "$out/imece-$s.mp4"
  # SRT → WebVTT: the header, and commas to dots in the times.
  { printf 'WEBVTT\n\n'; sed -E 's/^([0-9:]+),([0-9]{3}) --> ([0-9:]+),([0-9]{3})/\1.\2 --> \3.\4/' "$d/subtitles.srt"; } > "$out/imece-$s.vtt"
  # The poster: a frame from the first scene on the screen, not the card.
  ffmpeg -loglevel error -y -ss 9 -i "$d/video.mp4" -frames:v 1 -q:v 4 "$out/imece-$s.jpg"
  echo "imece-$s: $(du -h "$out/imece-$s.mp4" | cut -f1)"
done
