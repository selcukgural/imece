#!/usr/bin/env python3
"""A process that left an imece session, for a scripted case
(docs/acceptance/README.md): it carries the session's mark
(`IMECE_SESSION`, record 129) as a `nohup … &` server does, so imece's
quit must end it.

    marked.py start CASE_DIR     one, with the mark of the imece working
                                 in CASE_DIR; its number in CASE_DIR/marked.pid
    marked.py gone CASE_DIR      exits 0 once that one is gone

`start` exits non-zero while no session there carries a mark yet, so a
script asks it again with `until`.

Standard library only; run with `python3 -I`.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import drive  # noqa: E402  (its `running_in`, Linux and macOS alike)

MARK = "IMECE_SESSION"


def environ(pid: str) -> dict[str, str]:
    """Process `pid`'s environment, as far as this user may read it."""
    proc = Path(f"/proc/{pid}/environ")
    if proc.exists():
        try:
            pairs = proc.read_bytes().split(b"\0")
        except OSError:
            return {}
        text = [p.decode(errors="replace") for p in pairs if b"=" in p]
    else:
        out = subprocess.run(
            ["ps", "-E", "-ww", "-o", "command=", "-p", pid], capture_output=True, text=True
        ).stdout
        text = [w for w in out.split() if "=" in w]
    return dict(p.split("=", 1) for p in text)


def mark_in(case: Path) -> str | None:
    """A mark of the run of the imece working in `case`."""
    procs = [line.split(" ", 1) for line in drive.running_in(case)]
    imeces = [pid for pid, name in procs if "imece" in os.path.basename(name)]
    for pid, _ in procs:
        mark = environ(pid).get(MARK, "")
        if any(mark.startswith(f"{i}-") for i in imeces):
            return mark
    return None


def start(case: Path) -> int:
    mark = mark_in(case)
    if not mark:
        print("no session carries a mark yet")
        return 1
    env = dict(os.environ, **{MARK: mark})
    # Its own session, as one that left does: nothing ties it to imece
    # but the mark. In the case's directory, so a run that fails still
    # ends it with what else outlived the case.
    proc = subprocess.Popen(
        [sys.executable, "-I", "-c", "import time; time.sleep(600)"],
        env=env,
        cwd=case,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    (case / "marked.pid").write_text(f"{proc.pid}\n")
    print(f"{proc.pid} carries {mark}")
    return 0


def gone(case: Path) -> int:
    pid = int((case / "marked.pid").read_text())
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return 0
    print(f"{pid} still runs")
    return 1


def main(argv: list[str]) -> int:
    if len(argv) != 2 or argv[0] not in ("start", "gone"):
        print(__doc__.split("\n\n")[1], file=sys.stderr)
        return 2
    case = Path(argv[1]).resolve()
    return start(case) if argv[0] == "start" else gone(case)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
