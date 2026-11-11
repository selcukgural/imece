#!/usr/bin/env python3
"""Runs a program on a pty of its own, between the terminal it was started
in and the program, for an acceptance case that must see what the program
writes to the terminal itself (docs/acceptance/README.md): the tab's title
and progress, a note, the screen left behind at the quit.

    relay.py --as 'kitty(0.40.0)' --log FILE -- PROGRAM ARG …

Every byte the program writes is kept in FILE as written, and goes on to
the terminal. The program's question for the terminal's name (XTVERSION,
`CSI > 0 q`) is answered here as the terminal `--as` names would answer it
and never passed on, so a program under tmux takes itself for running in
that terminal; everything else goes through both ways, and the window's
size follows the terminal's. The relay ends with the program, with its
exit code.

Standard library only; run with `python3 -I`.
"""

from __future__ import annotations

import argparse
import fcntl
import os
import pty
import select
import signal
import struct
import sys
import termios
import tty

# XTVERSION, as imece asks it (probe.rs), and its shorter form.
ASKS = (b"\x1b[>0q", b"\x1b[>q")


def answer(name: str) -> bytes:
    """What a terminal called `name` says to XTVERSION."""
    return b"\x1bP>|" + name.encode() + b"\x1b\\"


def held_back(data: bytes) -> int:
    """How many bytes at the end of `data` may be the start of a question
    the next read finishes: those wait for it."""
    for n in range(max(len(a) for a in ASKS) - 1, 0, -1):
        tail = data[-n:]
        if any(a.startswith(tail) and a != tail for a in ASKS):
            return n
    return 0


def take_asks(data: bytes) -> tuple[bytes, int]:
    """`data` without its questions for the terminal's name, and how many
    there were."""
    asked = 0
    for a in ASKS:
        asked += data.count(a)
        data = data.replace(a, b"")
    return data, asked


def size_of(fd: int) -> bytes | None:
    try:
        return fcntl.ioctl(fd, termios.TIOCGWINSZ, b"\0" * 8)
    except OSError:
        return None


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(prog="relay.py", description=__doc__.split("\n\n")[0])
    ap.add_argument("--as", dest="name", required=True, help="the terminal's XTVERSION answer")
    ap.add_argument("--log", required=True, help="where the program's bytes are kept")
    ap.add_argument("command", nargs=argparse.REMAINDER)
    args = ap.parse_args(argv)
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        ap.error("no program to run")
    pid, fd = pty.fork()
    if pid == 0:
        try:
            os.execvp(command[0], command)
        finally:
            os._exit(127)

    def follow(*_: object) -> None:
        size = size_of(sys.stdout.fileno())
        if size and struct.unpack("HHHH", size)[:2] != (0, 0):
            fcntl.ioctl(fd, termios.TIOCSWINSZ, size)
            try:
                os.kill(pid, signal.SIGWINCH)
            except ProcessLookupError:
                pass

    follow()
    signal.signal(signal.SIGWINCH, follow)
    stdin = sys.stdin.fileno()
    saved = None
    if os.isatty(stdin):
        saved = termios.tcgetattr(stdin)
        tty.setraw(stdin)
    log = open(args.log, "ab", buffering=0)
    carry = b""
    try:
        while True:
            try:
                ready, _, _ = select.select([fd, stdin], [], [])
            except InterruptedError:
                continue
            if fd in ready:
                try:
                    data = os.read(fd, 65536)
                except OSError:
                    data = b""
                if not data:
                    if carry:
                        os.write(sys.stdout.fileno(), carry)
                    break
                log.write(data)
                data = carry + data
                keep = held_back(data)
                data, carry = (data[:-keep], data[-keep:]) if keep else (data, b"")
                data, asked = take_asks(data)
                for _ in range(asked):
                    os.write(fd, answer(args.name))
                if data:
                    os.write(sys.stdout.fileno(), data)
            if stdin in ready:
                data = os.read(stdin, 65536)
                if not data:
                    # The terminal went: the program hears it as its own.
                    break
                os.write(fd, data)
    finally:
        if saved is not None:
            termios.tcsetattr(stdin, termios.TCSADRAIN, saved)
        os.close(fd)
        log.close()
    _, status = os.waitpid(pid, 0)
    return os.waitstatus_to_exitcode(status) if status else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
