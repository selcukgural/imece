#!/usr/bin/env python3
"""Drives a program on a tmux pane of its own, for an acceptance run
(docs/acceptance/README.md): start it, type, press keys, look, wait for
words, resize, stop. Every action goes to $ACC/drive.log with its time, so
a run can be read back.

    drive.py start imece [--size 120x40] -- imece-acc
    drive.py type imece 'find() should ignore case'
    drive.py key imece Enter            # tmux key names: Escape C-o Tab BTab Up PPage …
    drive.py snap imece [--ansi]
    drive.py wait imece 'TO DECIDE' [--timeout 120]
    drive.py size imece 60 20
    drive.py stop imece

A tmux server of its own (`-L imece-acc`), so the person's tmux is never
touched. The program starts with the variables an acceptance run sets
(ACC, PATH, HOME, XDG_*, LONCA_*, IMECE_*, CLAUDE_*, EDITOR, VISUAL, LANG,
LC_*), nothing else of the driver's environment.

Standard library only; run with `python3 -I`.
"""

from __future__ import annotations

import argparse
import os
import shlex
import subprocess
import sys
import time

SERVER = "imece-acc"
KEEP = ("ACC", "PATH", "HOME", "USER", "LOGNAME", "SHELL", "TMPDIR", "EDITOR", "VISUAL", "LANG")
KEEP_PREFIX = ("XDG_", "LONCA_", "IMECE_", "CLAUDE_", "LC_")


def tmux(*args: str, check: bool = True) -> subprocess.CompletedProcess:
    done = subprocess.run(["tmux", "-L", SERVER, *args], capture_output=True, text=True)
    if check and done.returncode != 0:
        sys.exit(f"drive: tmux {' '.join(args[:1])}: {done.stderr.strip()}")
    return done


def log(what: str) -> None:
    acc = os.environ.get("ACC")
    if not acc:
        return
    stamp = time.strftime("%H:%M:%S")
    with open(os.path.join(acc, "drive.log"), "a", encoding="utf-8") as f:
        f.write(f"{stamp} {what}\n")


def kept_env() -> list[str]:
    pairs = [
        f"{k}={v}"
        for k, v in sorted(os.environ.items())
        if k in KEEP or k.startswith(KEEP_PREFIX)
    ]
    pairs.append("TERM=xterm-256color")
    pairs.append("COLORTERM=truecolor")
    return pairs


def snap(name: str, ansi: bool = False) -> str:
    args = ["capture-pane", "-p", "-t", name]
    if ansi:
        args.insert(1, "-e")
    return tmux(*args).stdout


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(prog="drive.py", description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="what", required=True)
    start = sub.add_parser("start")
    start.add_argument("name")
    start.add_argument("--size", default="120x40")
    typ = sub.add_parser("type")
    typ.add_argument("name")
    typ.add_argument("text")
    key = sub.add_parser("key")
    key.add_argument("name")
    key.add_argument("keys", nargs="+")
    sn = sub.add_parser("snap")
    sn.add_argument("name")
    sn.add_argument("--ansi", action="store_true")
    wait = sub.add_parser("wait")
    wait.add_argument("name")
    wait.add_argument("text")
    wait.add_argument("--timeout", type=float, default=60.0)
    size = sub.add_parser("size")
    size.add_argument("name")
    size.add_argument("cols", type=int)
    size.add_argument("rows", type=int)
    stop = sub.add_parser("stop")
    stop.add_argument("name")
    # The program's own words come after `--`, untouched by the parser.
    command: list[str] = []
    if "--" in argv:
        cut = argv.index("--")
        argv, command = argv[:cut], argv[cut + 1 :]
    args = ap.parse_args(argv)

    if args.what == "start":
        if not command:
            sys.exit("drive: start NAME [--size CxR] -- COMMAND…")
        cols, _, rows = args.size.partition("x")
        if not (cols.isdigit() and rows.isdigit()):
            sys.exit(f"drive: --size {args.size} is not COLSxROWS")
        line = shlex.join(["env", "-i", *kept_env(), *command])
        tmux("new-session", "-d", "-s", args.name, "-x", cols, "-y", rows, line)
        tmux("set-option", "-t", args.name, "remain-on-exit", "on")
        tmux("set-option", "-t", args.name, "allow-passthrough", "on")
        log(f"start {args.name} {args.size}: {shlex.join(command)}")
    elif args.what == "type":
        tmux("send-keys", "-t", args.name, "-l", "--", args.text)
        log(f"type {args.name}: {args.text}")
    elif args.what == "key":
        tmux("send-keys", "-t", args.name, *args.keys)
        log(f"key {args.name}: {' '.join(args.keys)}")
    elif args.what == "snap":
        sys.stdout.write(snap(args.name, args.ansi))
    elif args.what == "wait":
        deadline = time.monotonic() + args.timeout
        while True:
            seen = snap(args.name)
            if args.text in seen:
                log(f"wait {args.name}: {args.text!r} seen")
                return 0
            if time.monotonic() >= deadline:
                log(f"wait {args.name}: {args.text!r} not seen in {args.timeout:g} s")
                sys.stdout.write(seen)
                print(f"drive: {args.text!r} not seen in {args.timeout:g} s", file=sys.stderr)
                return 1
            time.sleep(0.5)
    elif args.what == "size":
        tmux("resize-window", "-t", args.name, "-x", str(args.cols), "-y", str(args.rows))
        log(f"size {args.name}: {args.cols}x{args.rows}")
    elif args.what == "stop":
        tmux("kill-session", "-t", args.name, check=False)
        log(f"stop {args.name}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
