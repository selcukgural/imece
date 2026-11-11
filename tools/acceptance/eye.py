#!/usr/bin/env python3
"""What a person's eye judged in an acceptance case, as checks a script
runs with `sh` (docs/acceptance/README.md): each exits 0 when it holds and
says what it saw.

    eye.py quit BYTES             after imece's quit (`start --as`'s bytes):
                                  only its last words left on the main
                                  screen, the tab's progress cleared, the
                                  title given back; prints the last words
    eye.py english LOOK           a Turkish screen holds none of the
                                  English screen's words (i18n.rs's EN
                                  texts that the TR texts do not share)
    eye.py cut EN TR              no row of TR ends cut (`…`) whose row of
                                  EN, the same screen in English, is whole
    eye.py whole LOOK FROM UNTIL  no row from the one holding FROM up to
                                  the one holding UNTIL ends cut
    eye.py themed LOOK            every cell of every row has a background
                                  of the theme's (a whole screen drawn)
    eye.py same ROW A B …         the rows holding ROW are the same in every
                                  look, colours too (nothing moved)
    eye.py moved ROW A B …        the rows holding ROW have the same words in
                                  every look (the activity bar's cells left
                                  out), and not the same colours in all
    eye.py turns ROW A B …        the rows holding ROW do not say the same in
                                  every look (a spinner)
    eye.py samebg A B             the first cell's background is one, and the
                                  same in both looks (a theme kept)
    eye.py notes BYTES            how many of imece's notes kitty was sent
                                  (`OSC 99`, by their `i=imece-N`), and which
    eye.py dots LOOK …            the mark's cells count up from look to
                                  look, the last more than the first
    eye.py has LOOK TEXT          LOOK's words hold TEXT
    eye.py child CASE NAME        an imece working in CASE has a child
                                  process called NAME (`caffeinate`)
    eye.py nochild CASE NAME      it has none
    eye.py serve PORTFILE LOG     a loopback HTTP server, for a push (ntfy's
                                  shape): its port in PORTFILE, each request
                                  a line of LOG (method, path, body)

A LOOK is `drive.py`'s `look FILE`: `capture-pane -e`, colours as SGR.

Standard library only; run with `python3 -I`.
"""

from __future__ import annotations

import http.server
import os
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
I18N = HERE.parents[1] / "crates" / "imece-tui" / "src" / "i18n.rs"
sys.path.insert(0, str(HERE))

CSI = re.compile(rb"\x1b\[[0-?]*[ -/]*[@-~]")
OSC = re.compile(rb"\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)")
OTHER = re.compile(rb"\x1b[PX^_].*?\x1b\\|\x1b[@-Z\\-_]", re.S)
SGR = re.compile(r"\x1b\[([0-9;:]*)m")
CUT = "…"
# The mark's cells (record 101: half blocks).
MARK = set("▀▄█▌▐▖▗▘▝▙▚▛▜▞▟")


def plain(data: bytes) -> str:
    """`data` without its escapes, as text."""
    for pattern in (OSC, CSI, OTHER):
        data = pattern.sub(b"", data)
    return data.decode(errors="replace")


def rows(look: str) -> list[str]:
    """A look's rows as words, its colours dropped."""
    return [SGR.sub("", row) for row in look.split("\n")]


def read(path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


def quit_check(path: str) -> int:
    data = Path(path).read_bytes()
    leave = data.rfind(b"\x1b[?1049l")
    if leave < 0:
        print("the screen was never given back (no ESC[?1049l)")
        return 1
    after = data[leave:]
    words = [line.strip() for line in plain(after).replace("\r", "\n").split("\n")]
    words = [w for w in words if w]
    progress = re.findall(rb"\x1b\]9;4;([0-9;]*)(?:\x07|\x1b\\)", data)
    problems = []
    if progress and progress[-1] != b"0":
        problems.append(f"the tab's progress was left at {progress[-1].decode()}")
    if b"\x1b[23;0t" not in after:
        problems.append("the title was not given back (no ESC[23;0t)")
    if len(words) > 3:
        problems.append(f"more than its last words left: {words}")
    if not words:
        problems.append("no last words")
    print("\n".join(words))
    for p in problems:
        print(p)
    return 1 if problems else 0


def literals(block: str) -> list[str]:
    """The string literals of a block of Rust, their `\\` line ends joined."""
    found = re.findall(r'"((?:[^"\\]|\\.)*)"', block, re.S)
    joined = [re.sub(r"\\\n\s*", "", f) for f in found]
    return [f.replace('\\"', '"').replace("\\n", "\n") for f in joined]


def english_only() -> list[str]:
    """The pieces of the English texts the Turkish ones do not share:
    each text cut at its `{}`s and line ends, three words or more, or a
    heading in capitals of five letters or more."""
    src = I18N.read_text(encoding="utf-8")
    en = src[src.index("pub const EN: Text") : src.index("pub const TR: Text")]
    tr = src[src.index("pub const TR: Text") :]
    tr_text = "\n".join(literals(tr))
    pieces = set()
    for text in literals(en):
        for piece in re.split(r"\{[^}]*\}|\n", text):
            piece = piece.strip(" ·:,.;()[]\"'–—→←")
            words = re.findall(r"[A-Za-z']+", piece)
            heading = piece.isupper() and len("".join(words)) >= 5
            if (len(words) >= 3 or heading) and piece not in tr_text:
                pieces.add(piece)
    return sorted(pieces, key=len, reverse=True)


def english(path: str) -> int:
    screen = "\n".join(rows(read(path)))
    left = [p for p in english_only() if p in screen]
    for p in left:
        print(f"English left: {p!r}")
    return 1 if left else 0


def cut(en_path: str, tr_path: str) -> int:
    en, tr = rows(read(en_path)), rows(read(tr_path))
    bad = [
        (i, t.rstrip())
        for i, t in enumerate(tr)
        if t.rstrip().endswith(CUT) and not (i < len(en) and en[i].rstrip().endswith(CUT))
    ]
    for i, t in bad:
        print(f"row {i} cut in Turkish only: {t!r}")
    return 1 if bad else 0


def whole(path: str, start: str, until: str) -> int:
    lines = rows(read(path))
    begin = next((i for i, l in enumerate(lines) if start in l), None)
    if begin is None:
        print(f"no row holds {start!r}")
        return 1
    end = next((i for i, l in enumerate(lines) if i > begin and until in l), len(lines))
    bad = [l.rstrip() for l in lines[begin:end] if l.rstrip().endswith(CUT)]
    for l in bad:
        print(f"cut: {l!r}")
    return 1 if bad else 0


def backgrounds(row: str, bg: bool = False) -> tuple[list[bool], bool]:
    """For each cell of `row` (a look's row), whether it has a background,
    from `bg` at its start; and whether one is set at its end. tmux carries
    the colours from one row of a look to the next: it sets them only where
    they change."""
    cells = []
    pos = 0
    for m in SGR.finditer(row):
        cells += [bg] * len(row[pos : m.start()])
        params = [p for p in re.split(r"[;:]", m.group(1)) if p != ""] or ["0"]
        i = 0
        while i < len(params):
            p = params[i]
            if p in ("38", "48", "58"):
                # A colour by its number (`;5;N`) or its RGB (`;2;R;G;B`):
                # those numbers are no attributes of their own.
                bg = bg or p == "48"
                i += 4 if params[i + 1 : i + 2] == ["2"] else 2
            elif p in ("0", "49"):
                bg = False
            elif p.isdigit() and (40 <= int(p) <= 47 or 100 <= int(p) <= 107):
                bg = True
            elif p == "7":
                bg = True
            i += 1
        pos = m.end()
    cells += [bg] * len(row[pos:])
    return cells, bg


def themed(path: str) -> int:
    looked = read(path).rstrip("\n").split("\n")
    bare, bg = [], False
    for i, row in enumerate(looked):
        cells, bg = backgrounds(row, bg)
        if not row or not all(cells):
            bare.append(i)
    if bare:
        print(f"rows not drawn whole in the theme's background: {bare[:10]}")
        return 1
    return 0


# The activity bar's cells (record 85), which change with the tick.
BAR = str.maketrans("", "", "▁▂▃▄▅▆▇█")


def row_of(look: str, text: str) -> tuple[str, str]:
    """The row holding `text`: with its colours, and its words."""
    for row in look.split("\n"):
        words = SGR.sub("", row)
        if text in words:
            return row, words.translate(BAR)
    return "", ""


def same(text: str, paths: list[str]) -> int:
    found = [row_of(read(p), text) for p in paths]
    if not found[0][0]:
        print(f"no row holds {text!r}")
        return 1
    if len({f for f in found}) == 1:
        return 0
    print(f"the row holding {text!r} moved")
    return 1


def moved(text: str, paths: list[str]) -> int:
    found = [row_of(read(p), text) for p in paths]
    if not found[0][0]:
        print(f"no row holds {text!r}")
        return 1
    if len({words for _, words in found}) != 1:
        print(f"the row's words changed: {[w for _, w in found]}")
        return 1
    if len({row for row, _ in found}) == 1:
        print("nothing moved")
        return 1
    return 0


def turns(text: str, paths: list[str]) -> int:
    found = [row_of(read(p), text)[1] for p in paths]
    if not found[0]:
        print(f"no row holds {text!r}")
        return 1
    if len(set(found)) > 1:
        return 0
    print("it says the same in every look")
    return 1


def first_bg(path: str) -> str | None:
    """The background SGR of the first cell of the first row."""
    row = read(path).split("\n")[0]
    bg = None
    for m in SGR.finditer(row):
        if m.start() > 0 and SGR.sub("", row[: m.start()]):
            break
        params = m.group(1).split(";")
        if "48" in params:
            i = params.index("48")
            bg = ";".join(params[i : i + 5])
        elif "0" in params or "49" in params or params == [""]:
            bg = None
    return bg


def samebg(a: str, b: str) -> int:
    ba, bb = first_bg(a), first_bg(b)
    print(ba, bb)
    return 0 if ba and ba == bb else 1


def notes(path: str) -> int:
    data = Path(path).read_bytes()
    ids = sorted(set(re.findall(rb"\x1b\]99;i=imece-([0-9]+)", data)), key=int)
    print(len(ids), " ".join(i.decode() for i in ids))
    return 0


def dots(paths: list[str]) -> int:
    counts = [sum(ch in MARK for ch in "\n".join(rows(read(p)))) for p in paths]
    print(" ".join(map(str, counts)))
    rising = all(a <= b for a, b in zip(counts, counts[1:]))
    return 0 if rising and counts[0] < counts[-1] else 1


def has(path: str, text: str) -> int:
    return 0 if text in "\n".join(rows(read(path))) else 1


def children(case: str, name: str) -> list[str]:
    import drive

    procs = [line.split(" ", 1) for line in drive.running_in(Path(case).resolve())]
    imeces = {pid for pid, comm in procs if "imece" in os.path.basename(comm)}
    out = subprocess.run(["ps", "-A", "-o", "pid=,ppid=,comm="], capture_output=True, text=True).stdout
    found = []
    for line in out.splitlines():
        parts = line.split(None, 2)
        if len(parts) == 3 and parts[1] in imeces and os.path.basename(parts[2]) == name:
            found.append(parts[0])
    return found


def serve(port_file: str, log_file: str) -> int:
    class Push(http.server.BaseHTTPRequestHandler):
        def do_POST(self) -> None:  # noqa: N802 (the library's name)
            size = int(self.headers.get("Content-Length") or 0)
            body = self.rfile.read(size).decode(errors="replace")
            with open(log_file, "a", encoding="utf-8") as f:
                f.write(f"{self.command} {self.path} {body!r}\n")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(b'{"id":"acc"}')

        do_PUT = do_POST

        def log_message(self, *_: object) -> None:
            pass

    server = http.server.HTTPServer(("127.0.0.1", 0), Push)
    Path(port_file).write_text(f"{server.server_address[1]}\n")
    server.serve_forever()
    return 0


def main(argv: list[str]) -> int:
    what, args = (argv[0], argv[1:]) if argv else ("", [])
    table = {
        "quit": (1, lambda: quit_check(args[0])),
        "english": (1, lambda: english(args[0])),
        "cut": (2, lambda: cut(args[0], args[1])),
        "whole": (3, lambda: whole(args[0], args[1], args[2])),
        "themed": (1, lambda: themed(args[0])),
        "samebg": (2, lambda: samebg(args[0], args[1])),
        "notes": (1, lambda: notes(args[0])),
        "has": (2, lambda: has(args[0], args[1])),
        "child": (2, lambda: 0 if children(args[0], args[1]) else 1),
        "nochild": (2, lambda: 1 if children(args[0], args[1]) else 0),
        "serve": (2, lambda: serve(args[0], args[1])),
    }
    if what == "dots" and args:
        return dots(args)
    if what in ("same", "moved", "turns") and len(args) >= 3:
        return {"same": same, "moved": moved, "turns": turns}[what](args[0], args[1:])
    if what not in table or len(args) != table[what][0]:
        print(__doc__.split("\n\n")[1], file=sys.stderr)
        return 2
    return table[what][1]()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
