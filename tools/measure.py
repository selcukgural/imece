#!/usr/bin/env python3
"""Measures a demo run of imece: CPU time, peak memory and bytes drawn.

Runs `imece --demo` on a pseudo-terminal of a fixed size with a fresh
state and config folder, closes the first-start window, and after
`--seconds` quits with `q q` (four of them: past 20 s without a key the
first opens the away digest and the next closes it). A focused run and
a blurred one by default (`--focused`, `--blurred` for one): in the
blurred one the terminal reports the focus lost (`CSI O`) a moment after
the start, as a terminal does when another window is chosen. `--fast` runs the demo at its top
speed (`+` three times: a tick of 62 ms, closer to live's 100 ms than the
demo's 500). `--pause` pauses the demo (space) 8 s after the start,
when its first sessions have started: its rows go on working and nothing new comes, like a live team
between events. `--motion off calm full` runs each of those `:motion`
levels in turn (typed after the start; a fresh state is calm). Prints one
line per run:

    focused fast  cpu 0.52 s (3.5%)  rss 5.8 MiB  drawn 0.64 MiB
    blurred fast  cpu 0.10 s (0.7%)  rss 5.8 MiB  drawn 0.17 MiB

Stdlib only; run it as `python3 -I tools/measure.py`, after
`cargo build --release`. Every process it starts is ended before it
returns. See docs/decisions/77-quiet-blur.md and 84-motion.md.
"""

import argparse
import fcntl
import os
import pty
import select
import signal
import struct
import sys
import tempfile
import termios
import time

COLUMNS, ROWS = 160, 48
# What a terminal sends when its window loses the focus (focus reports on).
BLUR = b"\x1b[O"


def run(binary, seconds, blurred, fast, pause, motion, extra):
    """One run; returns (cpu seconds, peak rss bytes, bytes drawn)."""
    with tempfile.TemporaryDirectory(prefix="imece-measure-") as home:
        env = {
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "HOME": home,
            "XDG_STATE_HOME": os.path.join(home, "state"),
            "XDG_CONFIG_HOME": os.path.join(home, "config"),
            "TERM": "xterm-256color",
            "LANG": "en_US.UTF-8",
        }
        pid, fd = pty.fork()
        if pid == 0:
            try:
                os.execve(binary, [binary, "--demo", *extra], env)
            finally:
                os._exit(127)
        try:
            fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLUMNS, 0, 0))
            os.kill(pid, signal.SIGWINCH)
            drawn = 0
            start = time.monotonic()
            # (seconds after the start, bytes): close the welcome window,
            # maybe lose the focus, and quit at the end.
            # Apart, so no key reads as alt with the escape before it.
            plan = [(1.0, b"\x1b")]
            if motion:
                plan.append((2.0, b":motion " + motion.encode() + b"\r"))
            if fast:
                plan += [(1.5, b"+"), (1.7, b"+"), (1.9, b"+")]
            if blurred:
                plan.append((2.2, BLUR))
            if pause:
                # After the first sessions started (their dots would move).
                plan.append((8.0, b" "))
            # Two more: past a while without keys the first key opens the
            # away digest (record 52) and the next closes it.
            plan += [(seconds + 0.3 * i, b"q") for i in range(4)]
            ended = None
            while ended is None:
                now = time.monotonic() - start
                while plan and plan[0][0] <= now:
                    os.write(fd, plan.pop(0)[1])
                if now > seconds + 5:
                    break
                ready, _, _ = select.select([fd], [], [], 0.05)
                if ready:
                    try:
                        chunk = os.read(fd, 65536)
                    except OSError:
                        chunk = b""
                    drawn += len(chunk)
                got, status, usage = os.wait4(pid, os.WNOHANG)
                if got == pid:
                    ended = (status, usage)
            if ended is None:
                ended = stop(pid, fd)
            _, usage = ended
        finally:
            os.close(fd)
        rss = usage.ru_maxrss if sys.platform == "darwin" else usage.ru_maxrss * 1024
        return usage.ru_utime + usage.ru_stime, rss, drawn


def stop(pid, fd):
    """Ends a run that did not quit: TERM, then KILL; returns its wait4.

    The terminal is read on while it ends: macOS keeps a process that
    exits with output not yet read from its terminal from ending (it
    hung there for half an hour once)."""
    for sig in (signal.SIGTERM, signal.SIGKILL):
        try:
            os.kill(pid, sig)
        except ProcessLookupError:
            pass
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            got, status, usage = os.wait4(pid, os.WNOHANG)
            if got == pid:
                print(f"warning: the run did not quit by itself ({sig.name})", file=sys.stderr)
                return status, usage
            drain(fd)
    while True:
        got, status, usage = os.wait4(pid, os.WNOHANG)
        if got == pid:
            return status, usage
        drain(fd)


def drain(fd):
    """Reads what the terminal has, waiting at most 50 ms."""
    ready, _, _ = select.select([fd], [], [], 0.05)
    if ready:
        try:
            os.read(fd, 65536)
        except OSError:
            time.sleep(0.05)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--binary", default="target/release/imece")
    ap.add_argument("--seconds", type=float, default=20.0)
    ap.add_argument("--fast", action="store_true", help="the demo at its top speed")
    ap.add_argument(
        "--pause",
        action="store_true",
        help="the demo paused after the start: rows that work, nothing new",
    )
    which = ap.add_mutually_exclusive_group()
    which.add_argument("--blurred", action="store_true", help="only a blurred run")
    which.add_argument("--focused", action="store_true", help="only a focused run")
    ap.add_argument(
        "--motion",
        nargs="+",
        choices=["off", "calm", "full"],
        default=[None],
        help="each :motion level in turn",
    )
    ap.add_argument("extra", nargs="*", help="more arguments for imece, after --")
    args = ap.parse_args()
    if not os.access(args.binary, os.X_OK):
        sys.exit(f"{args.binary}: not built (cargo build --release)")
    runs = [False] if args.focused else [True] if args.blurred else [False, True]
    for motion in args.motion:
        for blurred in runs:
            measure(args, blurred, motion)


def measure(args, blurred, motion):
    """One run, said on one line."""
    cpu, rss, drawn = run(
        args.binary, args.seconds, blurred, args.fast, args.pause, motion, args.extra
    )
    name = ("blurred" if blurred else "focused") + (" fast" if args.fast else "")
    name += " paused" if args.pause else ""
    if motion:
        name += f" {motion}"
    share = 100 * cpu / args.seconds
    print(
        f"{name}  cpu {cpu:.2f} s ({share:.1f}%)  rss {rss / 2**20:.1f} MiB"
        f"  drawn {drawn / 2**20:.2f} MiB"
    )


if __name__ == "__main__":
    main()
