"""Tests of tools/acceptance/drive.py's pure parts: a script's steps, and
which processes a case's cleanup leaves alone (record 147). Run with
`python3 -I -m unittest discover -s tools -p 'test_*.py'`."""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

_spec = importlib.util.spec_from_file_location(
    "acceptance_drive", Path(__file__).parent / "acceptance" / "drive.py"
)
drive = sys.modules.get("acceptance_drive") or importlib.util.module_from_spec(_spec)
if "acceptance_drive" not in sys.modules:
    sys.modules["acceptance_drive"] = drive
    _spec.loader.exec_module(drive)


class DriveTest(unittest.TestCase):
    def test_tmux_is_known_by_its_name_on_both_systems(self):
        # A case's cleanup kills what works in its directory, never the
        # tmux server every case's screen lives on.
        self.assertTrue(drive.is_tmux("tmux"))
        self.assertTrue(drive.is_tmux("tmux: server"))  # Linux's comm
        self.assertTrue(drive.is_tmux("/opt/homebrew/bin/tmux\n"))  # macOS's ps
        self.assertFalse(drive.is_tmux("imece"))
        self.assertFalse(drive.is_tmux("python3"))

    def test_a_case_s_cleanup_ends_what_works_in_its_directory(self):
        # Its command line does not name the directory: only where it works
        # does, as a worker's `nohup … &` server.
        import subprocess
        import tempfile
        import time

        with tempfile.TemporaryDirectory() as d:
            d = Path(d).resolve()
            proc = subprocess.Popen(
                [sys.executable, "-I", "-c", "import time; time.sleep(60)"],
                cwd=d,
                start_new_session=True,
            )
            try:
                deadline = time.monotonic() + 10
                while not any(e.split()[0] == str(proc.pid) for e in drive.running_in(d)):
                    self.assertLess(time.monotonic(), deadline, "never seen working there")
                    time.sleep(0.1)
                drive.leftovers(d)
                proc.wait(timeout=10)
            finally:
                proc.kill()
                proc.wait()

    def test_a_screen_is_named_whole(self):
        # tmux takes `-t case-scr-1` for case-scr-14 when no session has
        # that name exactly, so a case's cleanup ended another's screen.
        self.assertEqual(drive.exact("case-scr-1"), "=case-scr-1:")

    def test_a_script_is_a_step_a_line(self):
        steps = drive.parse("# a comment\n\nstart --size 60x20\nwait 5 TO DECIDE\nkey g d\n")
        self.assertEqual(
            [(s.line, s.verb, s.rest) for s in steps],
            [(3, "start", "--size 60x20"), (4, "wait", "5 TO DECIDE"), (5, "key", "g d")],
        )

    def test_the_real_cases_steps_parse(self):
        steps = drive.parse(
            "bg lonca daemon --home x\nallow 600 ready || look first\n"
            "until 30 test -S x\nasked-none unittest\nenv A b\n"
        )
        self.assertEqual([s.verb for s in steps], ["bg", "allow", "until", "asked-none", "env"])
        for text, why in [
            ("allow soon x\n", "`allow SECONDS TEXT`"),
            ("until x\n", "`until SECONDS TEXT`"),
            ("asked-none\n", "`asked-none` needs its text"),
            ("bg\n", "`bg` needs its text"),
        ]:
            with self.assertRaises(drive.Failed) as got:
                drive.parse(text)
            self.assertIn(why, str(got.exception))

    def test_a_wrong_step_is_refused_with_its_line(self):
        for text, why in [
            ("start\npeek it\n", "line 2: `peek` is not a step"),
            ("see\n", "line 1: `see` needs its text"),
            ("wait soon TO DECIDE\n", "line 1: `wait SECONDS TEXT`"),
        ]:
            with self.assertRaises(drive.Failed) as got:
                drive.parse(text)
            self.assertIn(why, str(got.exception))


if __name__ == "__main__":
    unittest.main()
