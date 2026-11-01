#!/bin/sh
# Copied from kule@c09ec13545c331fd56d74ab76cb5f8557594a16c website/video/serve.sh
# and rewritten for a terminal program (see VENDORED.md).
#
# Serves imece's real screen, running `imece --demo`, in a browser page for
# PromoVideo to record: ttyd puts the terminal on a web page, and serve.mjs
# puts the title cards on the same origin. Usage, from anywhere:
#
#   website/video/serve.sh [port] [seed]   # default 8791 (the scenarios' baseUrl), 127.0.0.1 only
#
# Each page load is a fresh `imece --demo` (ttyd starts the command per
# connection), so a scenario opens the terminal page once and shows its cards
# with PromoVideo's own `card` step, not with `goto`. The demo keeps its
# state in a directory of its own, so the person's imece.sqlite is never
# read or written.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
port=${1:-8791}
seed=${2:-7}
ttyd_port=$((port - 1))
bin=${IMECE_BIN:-$repo/target/release/imece}
[ -x "$bin" ] || { echo "build it first: cargo build --release -p imece-tui" >&2; exit 1; }
state=${TMPDIR:-/tmp}/imece-tour-state
rm -rf "$state"; mkdir -p "$state"

cleanup() { kill 0 2>/dev/null; }
trap cleanup INT TERM EXIT

# At 1920x1080 (device scale 1: xterm.js draws twice too big under an emulated
# scale of 2) a 24 px Menlo gives 133 columns by 38 rows (xterm.js measures the font,
# about 28 px a row); serve.mjs refits the terminal once the options are in.
XDG_STATE_HOME="$state" COLORTERM=truecolor TERM=xterm-256color \
ttyd -i 127.0.0.1 -p "$ttyd_port" -W \
  -t fontSize=24 -t fontFamily=Menlo \
  -t lineHeight=1.0 -t cursorBlink=false -t disableLeaveAlert=true -t disableResizeOverlay=true \
  -t 'theme={"background":"#15161b","foreground":"#e6e6ea","cursor":"#e3a93f"}' \
  "$bin" --demo --theme dark --lang en --seed "$seed" --projects 2 --agents 12 &
sleep 0.5
node "$here/serve.mjs" "$port" "$ttyd_port" &
wait
