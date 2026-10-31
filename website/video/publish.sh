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
  # PromoVideo's render is near-lossless (hundreds of MB); a terminal's
  # frames are mostly still, so H.264 at CRF 26 keeps them sharp at a few
  # MB a minute. The subtitle track stays inside; faststart for streaming.
  ffmpeg -loglevel error -y -i "$d/video.mp4" \
    -c:v libx264 -crf 26 -preset slow -tune stillimage -pix_fmt yuv420p \
    -c:a aac -b:a 96k -c:s mov_text -movflags +faststart "$out/imece-$s.mp4"
  # SRT → WebVTT: the header, and commas to dots in the times.
  { printf 'WEBVTT\n\n'; sed -E 's/^([0-9:]+),([0-9]{3}) --> ([0-9:]+),([0-9]{3})/\1.\2 --> \3.\4/' "$d/subtitles.srt"; } > "$out/imece-$s.vtt"
  # The poster: a frame from the first scene on the screen, not the card.
  ffmpeg -loglevel error -y -ss 9 -i "$d/video.mp4" -frames:v 1 -q:v 6 "$out/imece-$s.jpg"
  echo "imece-$s: $(du -h "$out/imece-$s.mp4" | cut -f1)"
done
