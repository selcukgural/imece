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
    drive.py run --real [--jobs 3] [ID|FILE …]              # the real ones

A TEXT to wait for may be several, ` || ` between them: any one will do
(a real model may take either of two right ways).

A tmux server of its own (`-L imece-acc`), so the person's tmux is never
touched. The program starts with the variables an acceptance run sets
(ACC, PATH, HOME, XDG_*, LONCA_*, IMECE_*, CLAUDE_*, ACC_*, EDITOR,
VISUAL, LANG, LC_*), nothing else of the driver's environment.

A scripted case is `tools/acceptance/scenarios/<ID>.scn`, a step a line
(`#` starts a comment):

    env NAME VALUE               NAME=VALUE for what starts after it
    start [--size CxR] [--bare] [ARG …]
                                 imece-acc with ARGs on a screen of its own;
                                 --bare: imece itself, without imece-acc's flags
    type TEXT                    the text as typed (the rest of the line)
    key KEY …                    tmux key names
    see TEXT                     TEXT on the screen within 30 s
    wait SECONDS TEXT            TEXT on the screen within SECONDS
    allow SECONDS TEXT           the same, saying yes to every ask meanwhile
    until SECONDS COMMAND        `sh -c COMMAND` again until it exits 0, saying
                                 yes to every ask meanwhile
    asked-none TEXT              no ask `allow` said yes to held TEXT
    gone TEXT                    TEXT off the screen within 30 s
    size COLS ROWS               the screen resized
    sh COMMAND                   `sh -c COMMAND` in the case's directory; exit 0
    bg COMMAND                   COMMAND in the background (a daemon), ended
                                 with the case before what outlived it is looked for
    fails COMMAND                the same, and it must exit non-zero
    says TEXT                    the last sh's or fails' output holds TEXT
    write FILE TEXT              TEXT and a newline into FILE (the case's directory)
    sleep SECONDS                a pause, where no word can be waited for
    stop                         the screen goes (its terminal closed)

`$ACC_CASE` in a step's text is the case's directory (`sh` has it as a
variable, and `$ACC_SOURCE`, the checkout the tools come from).

Each case runs in a directory of its own under $ACC/s/<ID> (its state,
config, Claude Code home and copy of the checkouts), so cases do not see
each other and `--jobs` runs several at once. At its end the screen's
terminal goes, and a process still working in the case's directory ten
seconds later (fifteen for a real case) fails it (imece quits when its terminal goes, its sessions
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
# The real cases' scripts: real Claude Code sessions, which cost.
REAL = SCENARIOS / "real"
# What asks the person: a permission or a script's yes.
ASKS = ("run Bash:", "run .imece/")
SEE_WAIT = 30.0
LOOK_EVERY = 0.2
KEY_GAP = 0.15
ESCAPE_GAP = 0.5
LEFT_WAIT = float(os.environ.get("ACC_LEFT_WAIT", "10"))

_log_lock = threading.Lock()


class Failed(Exception):
    """A scripted step that did not hold, in words."""


def exact(name: str) -> str:
    """Session `name`'s window by the session's whole name: tmux takes
    `-t case-scr-1` for case-scr-14 when no session is named case-scr-1
    exactly (`=` asks for the exact name, `:` for its window)."""
    return "=" + name + ":"


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
    # One command to tmux: the options are set before the server sees the
    # program end, so a program that ends at once (`--repo /`) still
    # leaves its pane to be read.
    tmux(
        *args,
        line,
        ";",
        "set-option", "-t", exact(name), "remain-on-exit", "on",
        ";",
        "set-option", "-t", exact(name), "allow-passthrough", "on",
    )
    log(f"start {name} {size}: {shlex.join(command)}")


def snap(name: str, ansi: bool = False) -> str:
    args = ["capture-pane", "-p", "-t", exact(name)]
    if ansi:
        args.insert(1, "-e")
    return tmux(*args).stdout


def wait_for(
    name: str, text: str, timeout: float, present: bool = True, allow: list | None = None
) -> str:
    """The screen once `text` (one of its ` || ` parts) is on it, or off
    it; Failed when not in time. With `allow`, every ask that shows
    meanwhile is said yes to (`g d` `a`) and its line kept there."""
    deadline = time.monotonic() + timeout
    parts = text.split(" || ")
    while True:
        seen = snap(name)
        if any(p in seen for p in parts) == present:
            log(f"wait {name}: {text!r} {'seen' if present else 'gone'}")
            return seen
        if time.monotonic() >= deadline:
            what = "not seen" if present else "still there"
            log(f"wait {name}: {text!r} {what} in {timeout:g} s")
            raise Failed(f"{text!r} {what} in {timeout:g} s")
        if allow is not None and "permissions" in seen:
            asked = [l.strip() for l in seen.splitlines() if any(a in l for a in ASKS)]
            for line in asked:
                if line not in allow:
                    allow.append(line)
            log(f"allow {name}: {asked}")
            tmux("send-keys", "-t", exact(name), "g")
            time.sleep(KEY_GAP)
            tmux("send-keys", "-t", exact(name), "d")
            time.sleep(KEY_GAP)
            tmux("send-keys", "-t", exact(name), "a")
            time.sleep(1.0)
        time.sleep(LOOK_EVERY)


@dataclass
class Step:
    line: int
    verb: str
    rest: str


VERBS = ("start", "env", "bg", "allow", "until", "asked-none", "type", "key", "see", "wait", "gone", "size", "sh", "fails", "says", "write", "sleep", "stop")


def parse(text: str) -> list[Step]:
    steps = []
    for n, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        verb, _, rest = line.partition(" ")
        if verb not in VERBS:
            raise Failed(f"line {n}: `{verb}` is not a step ({', '.join(VERBS)})")
        if verb in ("type", "see", "gone", "sh", "fails", "says", "env", "bg") and not rest:
            raise Failed(f"line {n}: `{verb}` needs its text")
        if verb in ("wait", "allow", "until") and not re.fullmatch(r"[0-9.]+ \S.*", rest):
            raise Failed(f"line {n}: `{verb} SECONDS TEXT`")
        if verb == "asked-none" and not rest:
            raise Failed(f"line {n}: `asked-none` needs its text")
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
            # The checkout the run's tools and site come from.
            "ACC_SOURCE": str(SCENARIOS.parents[2]),
        }
    )
    if (acc / "stand-in").exists():
        env["CLAUDE_CONFIG_DIR"] = str(d / "claude")
    out = ""
    allowed: list[str] = []
    background: list[subprocess.Popen] = []
    step = Step(0, "", "")
    try:
        for step in parse(path.read_text(encoding="utf-8")):
            v, rest = step.verb, step.rest
            if v in ("type", "write", "see", "wait", "gone", "says", "env", "allow", "bg", "until", "sh", "fails"):
                # The case's own directory, wherever the run is.
                rest = rest.replace("$ACC_CASE", str(d))
            if v == "start":
                words = shlex.split(rest)
                size, program = "120x40", "imece-acc"
                while words[:1] in (["--size"], ["--bare"]):
                    if words[0] == "--size":
                        size, words = words[1], words[2:]
                    else:
                        program, words = "imece", words[1:]
                start(name, size, [program, *words], env, str(d))
            elif v == "env":
                key, _, value = rest.partition(" ")
                env[key] = value
            elif v == "type":
                tmux("send-keys", "-t", exact(name), "-l", "--", rest)
                log(f"type {name}: {rest}")
                time.sleep(KEY_GAP)
            elif v == "key":
                # One at a time: an Escape and the key after it in one
                # read are alt and that key, so an Escape waits longer (a
                # loaded machine reads late).
                for k in rest.split():
                    tmux("send-keys", "-t", exact(name), k)
                    time.sleep(ESCAPE_GAP if k == "Escape" else KEY_GAP)
                log(f"key {name}: {rest}")
            elif v == "see":
                wait_for(name, rest, SEE_WAIT)
            elif v == "wait":
                seconds, _, text = rest.partition(" ")
                wait_for(name, text, float(seconds))
            elif v == "allow":
                seconds, _, text = rest.partition(" ")
                wait_for(name, text, float(seconds), allow=allowed)
            elif v == "bg":
                log_file = open(d / f"bg-{len(background)}.log", "w")
                background.append(
                    subprocess.Popen(
                        ["sh", "-c", f"exec {rest}"], cwd=d, env=env, stdout=log_file, stderr=log_file,
                        start_new_session=True,
                    )
                )
                log(f"bg {name}: {rest}")
            elif v == "until":
                seconds, _, command = rest.partition(" ")
                deadline = time.monotonic() + float(seconds)
                while True:
                    done = subprocess.run(
                        ["sh", "-c", command], cwd=d, env=env, capture_output=True, text=True, timeout=60
                    )
                    if done.returncode == 0:
                        out = done.stdout + done.stderr
                        log(f"until {name}: {command} → 0")
                        break
                    if time.monotonic() >= deadline:
                        raise Failed(f"`{command}` did not succeed in {seconds} s")
                    # Asks meanwhile get their yes, as `allow` gives them.
                    try:
                        wait_for(name, "\0never on a screen\0", 1.0, allow=allowed)
                    except Failed:
                        pass
            elif v == "asked-none":
                hit = [a for a in allowed if rest in a]
                if hit:
                    raise Failed(f"asked for a yes to {rest!r}: {hit}")
            elif v == "gone":
                wait_for(name, rest, SEE_WAIT, present=False)
            elif v == "size":
                cols, rows = rest.split()
                tmux("resize-window", "-t", exact(name), "-x", cols, "-y", rows)
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
                tmux("kill-session", "-t", exact(name), check=False)
        # What the case spent, as the header says it (a real case's cost).
        spent = ""
        try:
            m = re.search(r"([≈<]*\$[0-9.]+|\?) of \$", snap(name))
            spent = m.group(1) if m else ""
        except Failed:
            pass
        # Nothing of the case may outlive its terminal (record 86); what
        # the case started in the background is its own to end first.
        tmux("kill-session", "-t", exact(name), check=False)
        end_background(background)
        left = outlived(d)
        if left:
            raise Failed(f"still running {LEFT_WAIT:g} s after its terminal went: {left}")
        took = time.monotonic() - began
        return "pass", took, spent
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
        tmux("kill-session", "-t", exact(name), check=False)
        end_background(background)
        # Nothing of the case is left running: imece quits when its
        # terminal goes; whatever outlived it goes now.
        time.sleep(0.5)
        leftovers(d)


def is_tmux(name: str) -> bool:
    """Whether a process named `name` is tmux: `tmux` on macOS, its server
    `tmux: server` on Linux."""
    return os.path.basename(name.strip()).startswith("tmux")


def end_background(procs: list) -> None:
    """Ends what a case's `bg` steps started, and what they started."""
    for proc in procs:
        if proc.poll() is None:
            try:
                os.killpg(proc.pid, 15)
                proc.wait(timeout=10)
            except (ProcessLookupError, subprocess.TimeoutExpired):
                try:
                    os.killpg(proc.pid, 9)
                except ProcessLookupError:
                    pass
    procs.clear()


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
            if (cwd == root or cwd.startswith(root + "/")) and not is_tmux(name):
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
            if (cwd == root or cwd.startswith(root + "/")) and not is_tmux(name):
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
    # With its slash: SCR-1's directory is a prefix of SCR-14's.
    found = subprocess.run(["pgrep", "-f", str(d) + "/"], capture_output=True, text=True)
    for pid in found.stdout.split():
        name = subprocess.run(["ps", "-o", "comm=", "-p", pid], capture_output=True, text=True)
        if not is_tmux(name.stdout) and int(pid) != os.getpid():
            subprocess.run(["kill", pid], capture_output=True)


def fill_report(report: Path, results: dict[str, tuple[str, float, str]]) -> None:
    lines = report.read_text(encoding="utf-8").splitlines()
    for i, line in enumerate(lines):
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if line.startswith("| ") and len(cells) == 4 and cells[0] in results:
            result, took, why = results[cells[0]]
            spent = f", {why}" if why else ""
            note = f"scripted, {took:.0f} s{spent}" if result == "pass" else f"scripted: {why}"
            lines[i] = f"| {cells[0]} | {result} | | {note.replace('|', '/')} |"
    report.write_text("\n".join(lines) + "\n", encoding="utf-8")


def run(args: argparse.Namespace) -> int:
    acc = Path(os.environ.get("ACC", ""))
    if not os.environ.get("ACC") or not (acc / "repos").is_dir():
        print("drive: run needs $ACC, a world setup.sh made (. $ACC/env.sh)", file=sys.stderr)
        return 2
    stand_in = (acc / "stand-in").exists()
    if args.real == stand_in:
        # Never a stand-in's script on Claude Code (it would spend for
        # nothing), never a real one on the stand-in (it proves nothing).
        want = "a world without --stand-in" if args.real else "setup.sh --stand-in's world"
        print(f"drive: these cases run in {want}", file=sys.stderr)
        return 2
    global LEFT_WAIT
    if args.real and "ACC_LEFT_WAIT" not in os.environ:
        # Real Claude Code sessions take longer to be gone than the
        # stand-in's (seen 5-15 s; record 147's findings).
        LEFT_WAIT = 15.0
    where = REAL if args.real else SCENARIOS
    if args.cases:
        paths = [Path(c) if c.endswith(".scn") else where / f"{c}.scn" for c in args.cases]
    else:
        paths = sorted(where.glob("*.scn"))
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
    dollars = [
        float(m.group(1))
        for r in results.values()
        if r[0] == "pass" and (m := re.search(r"\$([0-9.]+)", r[2]))
    ]
    if dollars and args.real:
        print(f"drive: what the passed cases' headers say they spent: ${sum(dollars):.2f}")
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
    rn.add_argument("--real", action="store_true", help="the real cases' scripts, on Claude Code")
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
            tmux("send-keys", "-t", exact(args.name), "-l", "--", args.text)
            log(f"type {args.name}: {args.text}")
        elif args.what == "key":
            tmux("send-keys", "-t", exact(args.name), *args.keys)
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
            tmux("resize-window", "-t", exact(args.name), "-x", str(args.cols), "-y", str(args.rows))
            log(f"size {args.name}: {args.cols}x{args.rows}")
        elif args.what == "stop":
            tmux("kill-session", "-t", exact(args.name), check=False)
            log(f"stop {args.name}")
    except Failed as why:
        print(f"drive: {why}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
