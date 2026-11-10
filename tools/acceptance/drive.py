#!/usr/bin/env python3
"""Drives a program on a tmux pane of its own, for an acceptance run
(docs/acceptance/README.md): start it, type, press keys, look, wait for
words, resize, stop; or play a scripted case whole. Every action goes to
$ACC/drive.log with its time, so a run can be read back.

    drive.py start imece [--size 120x40] -- imece-acc
    drive.py type imece 'find() should ignore case'
    drive.py key imece Enter            # tmux key names: Escape C-o Tab BTab Up PPage …
    drive.py snap imece [--ansi]
    drive.py wait imece 'TO DECIDE' [--timeout 120]
    drive.py size imece 60 20
    drive.py stop imece
    drive.py run [--jobs 4] [--report REPORT] [ID|FILE …]   # scripted cases

A tmux server of its own (`-L imece-acc`), so the person's tmux is never
touched. The program starts with the variables an acceptance run sets
(ACC, PATH, HOME, XDG_*, LONCA_*, IMECE_*, CLAUDE_*, ACC_*, EDITOR,
VISUAL, LANG, LC_*), nothing else of the driver's environment.

A scripted case is `tools/acceptance/scenarios/<ID>.scn`, a step a line
(`#` starts a comment):

    start [--size CxR] [--bare] [ARG …]
                                 imece-acc with ARGs on a screen of its own;
                                 --bare: imece itself, without imece-acc's flags
    type TEXT                    the text as typed (the rest of the line)
    key KEY …                    tmux key names
    see TEXT                     TEXT on the screen within 30 s
    wait SECONDS TEXT            TEXT on the screen within SECONDS
    gone TEXT                    TEXT off the screen within 30 s
    size COLS ROWS               the screen resized
    sh COMMAND                   `sh -c COMMAND` in the case's directory; exit 0
    fails COMMAND                the same, and it must exit non-zero
    says TEXT                    the last sh's or fails' output holds TEXT
    write FILE TEXT              TEXT and a newline into FILE (the case's directory)
    sleep SECONDS                a pause, where no word can be waited for
    stop                         the screen goes (its terminal closed)

Each case runs in a directory of its own under $ACC/s/<ID> (its state,
config, Claude Code home and copy of the checkouts), so cases do not see
each other and `--jobs` runs several at once. At its end the screen's
terminal goes, and a process still working in the case's directory five
seconds later fails it (imece quits when its terminal goes, its sessions
with it). A case that fails leaves its screen in $ACC/out/<ID>.txt.

Standard library only; run with `python3 -I`.
"""

from __future__ import annotations

import argparse
import os
import re
import shlex
import shutil
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path

SERVER = "imece-acc"
KEEP = ("ACC", "PATH", "HOME", "USER", "LOGNAME", "SHELL", "TMPDIR", "EDITOR", "VISUAL", "LANG")
KEEP_PREFIX = ("XDG_", "LONCA_", "IMECE_", "CLAUDE_", "LC_", "ACC_")
SCENARIOS = Path(__file__).resolve().parent / "scenarios"
SEE_WAIT = 30.0
LOOK_EVERY = 0.2
KEY_GAP = 0.15
ESCAPE_GAP = 0.5
LEFT_WAIT = 5.0

_log_lock = threading.Lock()


class Failed(Exception):
    """A scripted step that did not hold, in words."""


def tmux(*args: str, check: bool = True) -> subprocess.CompletedProcess:
    done = subprocess.run(["tmux", "-L", SERVER, *args], capture_output=True, text=True)
    if check and done.returncode != 0:
        raise Failed(f"tmux {args[0]}: {done.stderr.strip()}")
    return done


def log(what: str) -> None:
    acc = os.environ.get("ACC")
    if not acc:
        return
    stamp = time.strftime("%H:%M:%S")
    with _log_lock, open(os.path.join(acc, "drive.log"), "a", encoding="utf-8") as f:
        f.write(f"{stamp} {what}\n")


def kept_env(env: dict[str, str]) -> list[str]:
    pairs = [
        f"{k}={v}" for k, v in sorted(env.items()) if k in KEEP or k.startswith(KEEP_PREFIX)
    ]
    pairs.append("TERM=xterm-256color")
    pairs.append("COLORTERM=truecolor")
    return pairs


def start(name: str, size: str, command: list[str], env: dict[str, str], cwd: str | None = None) -> None:
    cols, _, rows = size.partition("x")
    if not (cols.isdigit() and rows.isdigit()):
        raise Failed(f"--size {size} is not COLSxROWS")
    line = shlex.join(["env", "-i", *kept_env(env), *command])
    args = ["new-session", "-d", "-s", name, "-x", cols, "-y", rows]
    if cwd:
        args += ["-c", cwd]
    tmux(*args, line)
    tmux("set-option", "-t", name, "remain-on-exit", "on")
    tmux("set-option", "-t", name, "allow-passthrough", "on")
    log(f"start {name} {size}: {shlex.join(command)}")


def snap(name: str, ansi: bool = False) -> str:
    args = ["capture-pane", "-p", "-t", name]
    if ansi:
        args.insert(1, "-e")
    return tmux(*args).stdout


def wait_for(name: str, text: str, timeout: float, present: bool = True) -> str:
    """The screen once `text` is on it (or off it); Failed when not in time."""
    deadline = time.monotonic() + timeout
    while True:
        seen = snap(name)
        if (text in seen) == present:
            log(f"wait {name}: {text!r} {'seen' if present else 'gone'}")
            return seen
        if time.monotonic() >= deadline:
            what = "not seen" if present else "still there"
            log(f"wait {name}: {text!r} {what} in {timeout:g} s")
            raise Failed(f"{text!r} {what} in {timeout:g} s")
        time.sleep(LOOK_EVERY)


@dataclass
class Step:
    line: int
    verb: str
    rest: str


VERBS = ("start", "type", "key", "see", "wait", "gone", "size", "sh", "fails", "says", "write", "sleep", "stop")


def parse(text: str) -> list[Step]:
    steps = []
    for n, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        verb, _, rest = line.partition(" ")
        if verb not in VERBS:
            raise Failed(f"line {n}: `{verb}` is not a step ({', '.join(VERBS)})")
        if verb in ("type", "see", "gone", "sh", "fails", "says") and not rest:
            raise Failed(f"line {n}: `{verb}` needs its text")
        if verb == "wait" and not re.fullmatch(r"[0-9.]+ \S.*", rest):
            raise Failed(f"line {n}: `wait SECONDS TEXT`")
        steps.append(Step(n, verb, raw.strip()[len(verb) + 1 :]))
    return steps


def case_dir(acc: Path, cid: str) -> Path:
    """A fresh directory for case `cid`: state, config, a Claude Code home
    and a copy of the run's checkouts."""
    d = acc / "s" / cid
    if d.exists():
        shutil.rmtree(d)
    (d / "state").mkdir(parents=True)
    (d / "claude").mkdir()
    shutil.copytree(acc / "config", d / "config", symlinks=True)
    shutil.copytree(acc / "repos", d / "repos", symlinks=True)
    return d


def play(acc: Path, path: Path) -> tuple[str, float, str]:
    """Plays one scripted case: (`pass` or `fail`, seconds, what failed)."""
    cid = path.stem
    began = time.monotonic()
    name = f"case-{cid.lower()}"
    d = case_dir(acc, cid)
    env = dict(os.environ)
    env.update(
        {
            "XDG_STATE_HOME": str(d / "state"),
            "XDG_CONFIG_HOME": str(d / "config"),
            "ACC_REPOS": str(d / "repos"),
            "ACC_CASE": str(d),
        }
    )
    if (acc / "stand-in").exists():
        env["CLAUDE_CONFIG_DIR"] = str(d / "claude")
    out = ""
    step = Step(0, "", "")
    try:
        for step in parse(path.read_text(encoding="utf-8")):
            v, rest = step.verb, step.rest
            if v == "start":
                words = shlex.split(rest)
                size, program = "120x40", "imece-acc"
                while words[:1] in (["--size"], ["--bare"]):
                    if words[0] == "--size":
                        size, words = words[1], words[2:]
                    else:
                        program, words = "imece", words[1:]
                start(name, size, [program, *words], env, str(d))
            elif v == "type":
                tmux("send-keys", "-t", name, "-l", "--", rest)
                log(f"type {name}: {rest}")
                time.sleep(KEY_GAP)
            elif v == "key":
                # One at a time: an Escape and the key after it in one
                # read are alt and that key, so an Escape waits longer (a
                # loaded machine reads late).
                for k in rest.split():
                    tmux("send-keys", "-t", name, k)
                    time.sleep(ESCAPE_GAP if k == "Escape" else KEY_GAP)
                log(f"key {name}: {rest}")
            elif v == "see":
                wait_for(name, rest, SEE_WAIT)
            elif v == "wait":
                seconds, _, text = rest.partition(" ")
                wait_for(name, text, float(seconds))
            elif v == "gone":
                wait_for(name, rest, SEE_WAIT, present=False)
            elif v == "size":
                cols, rows = rest.split()
                tmux("resize-window", "-t", name, "-x", cols, "-y", rows)
                log(f"size {name}: {cols}x{rows}")
            elif v in ("sh", "fails"):
                done = subprocess.run(
                    ["sh", "-c", rest], cwd=d, env=env, capture_output=True, text=True, timeout=120
                )
                out = done.stdout + done.stderr
                log(f"{v} {name}: {rest} → {done.returncode}")
                if (done.returncode == 0) != (v == "sh"):
                    raise Failed(f"`{rest}` exited {done.returncode}: {out.strip()[-400:]}")
            elif v == "says":
                if rest not in out:
                    raise Failed(f"the output does not say {rest!r}: {out.strip()[-400:]}")
            elif v == "write":
                file, _, text = rest.partition(" ")
                target = d / file
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(text + "\n", encoding="utf-8")
            elif v == "sleep":
                time.sleep(float(rest))
            elif v == "stop":
                tmux("kill-session", "-t", name, check=False)
        # Nothing of the case may outlive its terminal (record 86).
        tmux("kill-session", "-t", name, check=False)
        left = outlived(d)
        if left:
            raise Failed(f"still running {LEFT_WAIT:g} s after its terminal went: {left}")
        took = time.monotonic() - began
        return "pass", took, ""
    except (Failed, subprocess.TimeoutExpired, OSError) as why:
        took = time.monotonic() - began
        screen = ""
        try:
            screen = snap(name)
        except Failed:
            pass
        (acc / "out").mkdir(exist_ok=True)
        (acc / "out" / f"{cid}.txt").write_text(
            f"{cid} line {step.line}: {step.verb} {step.rest}\n{why}\n\n{screen}", encoding="utf-8"
        )
        return "fail", took, f"line {step.line}: {why}"
    finally:
        tmux("kill-session", "-t", name, check=False)
        # Nothing of the case is left running: imece quits when its
        # terminal goes; whatever outlived it goes now.
        time.sleep(0.5)
        leftovers(d)


def running_in(d: Path) -> list[str]:
    """The processes whose working directory is in `d`, as `pid name`."""
    found = []
    root = str(d.resolve())
    if Path("/proc/self/cwd").exists():
        for proc in Path("/proc").iterdir():
            if not proc.name.isdigit():
                continue
            try:
                cwd = os.readlink(proc / "cwd")
                name = (proc / "comm").read_text().strip()
            except OSError:
                continue
            if cwd == root or cwd.startswith(root + "/"):
                found.append(f"{proc.name} {name}")
        return found
    out = subprocess.run(["lsof", "-a", "-d", "cwd", "-Fpcn"], capture_output=True, text=True).stdout
    pid = name = ""
    for line in out.splitlines():
        if line.startswith("p"):
            pid, name = line[1:], ""
        elif line.startswith("c"):
            name = line[1:]
        elif line.startswith("n"):
            cwd = os.path.realpath(line[1:])
            if cwd == root or cwd.startswith(root + "/"):
                found.append(f"{pid} {name}")
    return found


def outlived(d: Path) -> str:
    """What still runs in `d` once it has had LEFT_WAIT to end; ended now."""
    deadline = time.monotonic() + LEFT_WAIT
    while True:
        left = running_in(d)
        if not left or time.monotonic() >= deadline:
            break
        time.sleep(0.5)
    for entry in left:
        subprocess.run(["kill", "-9", entry.split()[0]], capture_output=True)
    return ", ".join(left)


def leftovers(d: Path) -> None:
    """Ends what of case directory `d` still runs, never the tmux server
    (whose command line is its first session's, so it may name `d`)."""
    found = subprocess.run(["pgrep", "-f", str(d)], capture_output=True, text=True)
    for pid in found.stdout.split():
        name = subprocess.run(["ps", "-o", "comm=", "-p", pid], capture_output=True, text=True)
        if os.path.basename(name.stdout.strip()) != "tmux" and int(pid) != os.getpid():
            subprocess.run(["kill", pid], capture_output=True)


def fill_report(report: Path, results: dict[str, tuple[str, float, str]]) -> None:
    lines = report.read_text(encoding="utf-8").splitlines()
    for i, line in enumerate(lines):
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if line.startswith("| ") and len(cells) == 4 and cells[0] in results:
            result, took, why = results[cells[0]]
            note = f"scripted, {took:.0f} s" if result == "pass" else f"scripted: {why}"
            lines[i] = f"| {cells[0]} | {result} | | {note.replace('|', '/')} |"
    report.write_text("\n".join(lines) + "\n", encoding="utf-8")


def run(args: argparse.Namespace) -> int:
    acc = Path(os.environ.get("ACC", ""))
    if not os.environ.get("ACC") or not (acc / "repos").is_dir():
        print("drive: run needs $ACC, a world setup.sh made (. $ACC/env.sh)", file=sys.stderr)
        return 2
    if args.cases:
        paths = [Path(c) if c.endswith(".scn") else SCENARIOS / f"{c}.scn" for c in args.cases]
    else:
        paths = sorted(SCENARIOS.glob("*.scn"))
    for p in paths:
        if not p.is_file():
            print(f"drive: no scripted case {p}", file=sys.stderr)
            return 2
        parse(p.read_text(encoding="utf-8"))
    began = time.monotonic()
    results: dict[str, tuple[str, float, str]] = {}

    def one(p: Path) -> None:
        results[p.stem] = r = play(acc, p)
        print(f"{r[0].upper():4} {p.stem:9} {r[1]:6.1f} s  {r[2]}", flush=True)

    with ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        list(pool.map(one, paths))
    failed = sorted(c for c, r in results.items() if r[0] == "fail")
    print(
        f"drive: {len(results) - len(failed)} of {len(results)} passed in "
        f"{time.monotonic() - began:.0f} s" + (f"; failed: {', '.join(failed)}" if failed else "")
    )
    if args.report:
        fill_report(args.report, results)
    return 1 if failed else 0


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(prog="drive.py", description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="what", required=True)
    st = sub.add_parser("start")
    st.add_argument("name")
    st.add_argument("--size", default="120x40")
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
    rn = sub.add_parser("run")
    rn.add_argument("--jobs", type=int, default=1)
    rn.add_argument("--report", type=Path)
    rn.add_argument("cases", nargs="*")
    # The program's own words come after `--`, untouched by the parser.
    command: list[str] = []
    if "--" in argv:
        cut = argv.index("--")
        argv, command = argv[:cut], argv[cut + 1 :]
    args = ap.parse_args(argv)

    try:
        if args.what == "run":
            return run(args)
        if args.what == "start":
            if not command:
                sys.exit("drive: start NAME [--size CxR] -- COMMAND…")
            start(args.name, args.size, command, dict(os.environ))
        elif args.what == "type":
            tmux("send-keys", "-t", args.name, "-l", "--", args.text)
            log(f"type {args.name}: {args.text}")
        elif args.what == "key":
            tmux("send-keys", "-t", args.name, *args.keys)
            log(f"key {args.name}: {' '.join(args.keys)}")
        elif args.what == "snap":
            sys.stdout.write(snap(args.name, args.ansi))
        elif args.what == "wait":
            try:
                wait_for(args.name, args.text, args.timeout)
            except Failed as why:
                sys.stdout.write(snap(args.name))
                print(f"drive: {why}", file=sys.stderr)
                return 1
        elif args.what == "size":
            tmux("resize-window", "-t", args.name, "-x", str(args.cols), "-y", str(args.rows))
            log(f"size {args.name}: {args.cols}x{args.rows}")
        elif args.what == "stop":
            tmux("kill-session", "-t", args.name, check=False)
            log(f"stop {args.name}")
    except Failed as why:
        print(f"drive: {why}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
