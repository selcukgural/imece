#!/usr/bin/env python3
"""Cut a raw asciinema recording of imece into the cast a tour plays.

    python3 -I cut.py EDIT.json OUT.cast

EDIT.json:
    {
      "cast": "raw.cast",                 # relative to EDIT.json
      "speed": [[from, to, factor], ...], # raw seconds; factor 0 = instant
      "markers": [[t, "scene-id"], ...],  # raw seconds: the tour pauses there
      "keys": [[t, "a", "approve"], ...]  # raw seconds: the badge shown
    }

Nothing the program wrote is ever dropped (the screen is the sum of every
byte): a span sped up or made instant only loses time. The raw input events
(what tmux typed, the terminal's answers) are dropped; the keys the viewer
should see are written as input events of their own, `KEY\\tMEANING`, which
the tour page draws as a badge.
"""
import json
import sys
from pathlib import Path


def factor_at(t, spans):
    for a, b, f in spans:
        if a <= t < b:
            return f
    return 1.0


def main(edit_path, out_path):
    edit_path = Path(edit_path)
    edit = json.loads(edit_path.read_text())
    lines = (edit_path.parent / edit["cast"]).read_text().splitlines()
    header = json.loads(lines[0])
    spans = sorted(edit.get("speed", []))
    raw = []  # (raw time, code, data)
    t = 0.0
    for line in lines[1:]:
        if not line.strip():
            continue
        dt, code, data = json.loads(line)
        t += dt
        if code == "o":
            raw.append((t, "o", data))
    for t_, label in edit.get("markers", []):
        raw.append((t_, "m", label))
    # A sped-up span says so in the corner while it plays.
    for a, b, f in spans:
        if f > 1:
            raw.append((a, "m", f"x{f:g}"))
            raw.append((b, "m", "x1"))
    for t_, key, says in edit.get("keys", []):
        raw.append((t_, "i", f"{key}\t{says}"))
    raw.sort(key=lambda e: (e[0], {"o": 0, "i": 1, "m": 2}[e[1]]))

    # Video time: each raw second counts 1/factor; a factor of 0 counts
    # nothing. Integrated piecewise between consecutive events.
    def video(a, b):
        cuts = sorted({a, b, *[x for s in spans for x in s[:2] if a < x < b]})
        total = 0.0
        for x, y in zip(cuts, cuts[1:]):
            f = factor_at(x, spans)
            total += 0.0 if f == 0 else (y - x) / f
        return total

    out = [json.dumps(header | {"idle_time_limit": None}, ensure_ascii=False)]
    last = 0.0
    for t_, code, data in raw:
        out.append(json.dumps([round(video(last, t_), 6), code, data], ensure_ascii=False))
        last = t_
    Path(out_path).write_text("\n".join(out) + "\n")
    length = video(0.0, last)
    print(f"{out_path}: {length:.1f} s from {last:.1f} s raw, "
          f"{len(edit.get('markers', []))} markers, {len(edit.get('keys', []))} keys")


if __name__ == "__main__":
    main(*sys.argv[1:3])
