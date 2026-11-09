#!/usr/bin/env python3
"""Turn a recording's driver log into cut.py's edit file.

    python3 -I plan.py NAME.cast NAME.log OUT.edit.json [--wait-seconds 5]

The log (written by the driver while recording) has one line per action:
epoch<TAB>kind<TAB>key<TAB>label, kind one of key, type, mark, wait, go.
A `key` with a label becomes a badge; a `mark` a scene's end marker (the
tour pauses there); each wait..go span is sped up so it plays in about
--wait-seconds; time before the first mark is cut to nothing.
"""
import json
import sys
from pathlib import Path


def main():
    cast, log, out = sys.argv[1:4]
    wait_for = 5.0
    if "--wait-seconds" in sys.argv:
        wait_for = float(sys.argv[sys.argv.index("--wait-seconds") + 1])
    start = json.loads(Path(cast).read_text().splitlines()[0])["timestamp"]
    rows = []
    for line in Path(log).read_text().splitlines():
        epoch, kind, key, label = (line.split("\t") + ["", "", ""])[:4]
        rows.append((float(epoch) - start, kind, key, label))
    keys, markers, speed = [], [], []
    opened = None
    first_mark = None
    for t, kind, key, label in rows:
        if kind == "key" and label:
            keys.append([round(t, 3), key, label])
        elif kind == "type" and label:
            keys.append([round(t, 3), "", label])
        elif kind == "mark":
            markers.append([round(t, 3), label])
            if first_mark is None:
                first_mark = t
        elif kind == "wait":
            opened = t
        elif kind == "go" and opened is not None:
            length = t - opened
            if length > wait_for * 1.5:
                factor = max(2, round(length / wait_for))
                speed.append([round(opened, 3), round(t, 3), factor])
            opened = None
    if first_mark:
        speed.insert(0, [0, round(first_mark, 3), 0])
    edit = {"cast": Path(cast).name, "speed": speed, "markers": markers, "keys": keys}
    Path(out).write_text(json.dumps(edit, indent=1, ensure_ascii=False) + "\n")
    print(f"{out}: {len(markers)} markers, {len(keys)} badges, {len(speed)} spans")
    for a, b, f in speed:
        print(f"  {a:7.1f}-{b:7.1f} ×{f}")


if __name__ == "__main__":
    main()
