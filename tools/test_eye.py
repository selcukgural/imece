"""Tests of tools/acceptance/eye.py's pure parts: how a look's colours are
read. Run with `python3 -I -m unittest discover -s tools -p 'test_*.py'`."""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

_spec = importlib.util.spec_from_file_location(
    "acceptance_eye", Path(__file__).parent / "acceptance" / "eye.py"
)
eye = sys.modules.get("acceptance_eye") or importlib.util.module_from_spec(_spec)
if "acceptance_eye" not in sys.modules:
    sys.modules["acceptance_eye"] = eye
    _spec.loader.exec_module(eye)


class EyeTest(unittest.TestCase):
    def test_a_background_lasts_until_reset_and_across_rows(self):
        cells, bg = eye.backgrounds("a\x1b[48;2;1;2;3mbc\x1b[0md")
        self.assertEqual(cells, [False, True, True, False])
        self.assertFalse(bg)
        # tmux sets colours only where they change: the next row goes on.
        _, bg = eye.backgrounds("\x1b[48;5;236mx")
        self.assertTrue(bg)
        cells, _ = eye.backgrounds("yz", bg)
        self.assertEqual(cells, [True, True])

    def test_a_colour_s_numbers_are_not_attributes(self):
        # The 0 of an RGB foreground is no reset.
        cells, bg = eye.backgrounds("\x1b[48;2;9;9;9ma\x1b[38;2;157;0;6mb\x1b[38;5;0mc")
        self.assertEqual(cells, [True, True, True])
        self.assertTrue(bg)
        cells, _ = eye.backgrounds("\x1b[49ma\x1b[44mb\x1b[7mc")
        self.assertEqual(cells, [False, True, True])

    def test_the_english_pieces_are_those_turkish_does_not_share(self):
        pieces = eye.english_only()
        self.assertIn("TO DECIDE", pieces)
        import re

        words = lambda p: re.findall(r"[A-Za-z']+", p)  # noqa: E731
        self.assertTrue(all(len(words(p)) >= 3 or p.isupper() for p in pieces))
        # A piece Turkish says as well is not English left behind.
        self.assertNotIn("imece", pieces)

    def test_a_row_cut_in_turkish_only_is_found(self):
        import tempfile

        with tempfile.TemporaryDirectory() as d:
            en, tr = Path(d, "en"), Path(d, "tr")
            en.write_text("a whole row\nboth cut…\n", encoding="utf-8")
            tr.write_text("bir sat…\nikisi de…\n", encoding="utf-8")
            self.assertEqual(eye.cut(str(en), str(tr)), 1)
            tr.write_text("bütün satır\nikisi de…\n", encoding="utf-8")
            self.assertEqual(eye.cut(str(en), str(tr)), 0)


if __name__ == "__main__":
    unittest.main()
