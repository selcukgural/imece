#!/bin/sh
# Drives a recording of a real imece session and logs what it did, for
# plan.py. See README.md, "Record". Every action is a line of
# $IMECE_REC/<name>.log: epoch<TAB>kind<TAB>key<TAB>label.
#
#   record.sh start NAME            a 120x40 tmux session recording a clean
#                                   shell into $IMECE_REC/NAME.cast
#   record.sh type LABEL -- TEXT    type TEXT a character at a time (a badge
#                                   with LABEL once, none when it is "")
#   record.sh key KEY SHOWN LABEL   one tmux key; the badge shows SHOWN · LABEL
#   record.sh quiet KEY...          keys with no badge
#   record.sh mark NAME             a scene starts here (the tour pauses)
#   record.sh wait / go             the span between plays sped up
#   record.sh snap [N]              the screen's first N lines, as text
#
# The recorded shell starts with nothing of yours: HOME, USER and TMPDIR
# (Claude Code's sign-in needs them), PATH with $IMECE_PATH first (where an
# `imece` wrapper sets the state directories and flags the tour runs with),
# and $IMECE_ZDOTDIR's .zshrc for a plain prompt.
set -eu
rec=${IMECE_REC:-/tmp/imece-rec}
s=imece-tour
mkdir -p "$rec"
log() {
  name=$(cat "$rec/.current")
  printf '%s\t%s\t%s\t%s\n' "$(python3 -I -c 'import time; print(f"{time.time():.3f}")')" \
    "$1" "$2" "$3" >>"$rec/$name.log"
}
case "${1:-}" in
start)
  echo "$2" >"$rec/.current"
  rm -f "$rec/$2.log" "$rec/$2.cast"
  tmux kill-session -t $s 2>/dev/null || true
  tmux new-session -d -s $s -x 120 -y 40 -c "$HOME" \
    "env -i HOME=$HOME USER=$USER LOGNAME=$USER TMPDIR=${TMPDIR:-/tmp} \
TERM=xterm-256color COLORTERM=truecolor LANG=en_US.UTF-8 \
PATH=${IMECE_PATH:-}:/opt/homebrew/bin:/usr/bin:/bin ZDOTDIR=${IMECE_ZDOTDIR:-$rec/zdot} \
asciinema rec -I -q -c 'zsh -i' $rec/$2.cast"
  sleep 2
  ;;
type)
  label=$2
  shift 3
  log type "" "$label"
  # One key a character, 45 ms apart; tmux reads a lone ";" as its own
  # separator, so it goes escaped.
  python3 -I -c '
import subprocess, sys, time
for c in sys.argv[2]:
    subprocess.run(["tmux", "send-keys", "-t", sys.argv[1], "-l", "--", "\\;" if c == ";" else c])
    time.sleep(0.045)' "$s" "$*"
  ;;
key) log key "$3" "$4"; tmux send-keys -t $s "$2" ;;
quiet) shift; for k in "$@"; do tmux send-keys -t $s "$k"; sleep 0.2; done ;;
mark) log mark "" "$2" ;;
wait) log wait "" "" ;;
go) log go "" "" ;;
snap) tmux capture-pane -p -t $s | head -"${2:-40}" ;;
*) sed -n '2,17p' "$0"; exit 2 ;;
esac
