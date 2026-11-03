"""imece's mark as iTerm2's tab icon (record 101): website/static/img/imece-tab.png.

Five hands round one job, the circles of website/static/img/imece-mark.svg
(viewBox 64) on a transparent ground, black: iTerm2 tints a tab icon one
colour (install.sh gives it imece's amber). 32 x 32 pixels, iTerm2 draws it
at 16 points. Stdlib only:

    python3 -I tools/tab_icon.py website/static/img/imece-tab.png

install.sh carries the same bytes (base64); website/test-install.sh checks
that they are this file.
"""

import struct
import sys
import zlib

SIZE = 32
# (cx, cy, r) in the SVG's 64-unit box: the hands clockwise from the top,
# then the job.
DOTS = [(32, 14, 6), (49.1, 26.4, 6), (42.6, 46.6, 6), (21.4, 46.6, 6), (14.9, 26.4, 6), (32, 33, 5)]


def alpha(x, y):
    """How much of pixel (x, y) the dots cover, 4 x 4 samples."""
    scale = SIZE / 64.0
    hits = 0
    for sy in range(4):
        for sx in range(4):
            px = (x + (sx + 0.5) / 4) / scale
            py = (y + (sy + 0.5) / 4) / scale
            if any((px - cx) ** 2 + (py - cy) ** 2 <= r * r for cx, cy, r in DOTS):
                hits += 1
    return 255 * hits // 16


def png():
    raw = b"".join(
        b"\x00" + b"".join(bytes([0, 0, 0, alpha(x, y)]) for x in range(SIZE)) for y in range(SIZE)
    )

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    head = struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", head) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


if __name__ == "__main__":
    with open(sys.argv[1], "wb") as out:
        out.write(png())
