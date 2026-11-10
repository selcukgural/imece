"""Tests of tools/acceptance.py: what the catalogue check and the release
gate refuse (record 147). Run with
`python3 -I -m unittest discover -s tools -p 'test_*.py'`."""

from __future__ import annotations

import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path

import acceptance as acc

CASES = """\
# Acceptance catalogue

## Some

### TEAM-1 · The leader answers

- records: 01
- commands: :open
- tier: real

Steps:
1. Write.

Expect: an answer.

### TEAM-2 · A worker works

- records: 02
- commands: :close
- tier: real
- skip-if: no terminal at hand
- after: TEAM-1

Steps:
1. Ask.

Expect: work.

## Not covered

- record 03: replaced
- command :demo: the demo's own

## Retired

- TEAM-9: gone
"""


def world(root: Path, catalogue: str = CASES, version: str = "1.2.3") -> None:
    for name in ("01-one.md", "02-two.md", "03-three.md", "README.md"):
        path = root / acc.DECISIONS / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("# x\n")
    complete = root / acc.COMPLETE
    complete.parent.mkdir(parents=True, exist_ok=True)
    complete.write_text('pub const COMMANDS: [&str; 3] = [\n    "close", "demo", "open",\n];\n')
    (root / acc.CARGO).write_text(f'[workspace.package]\nversion = "{version}"\n')
    cat = root / acc.CATALOGUE
    cat.parent.mkdir(parents=True, exist_ok=True)
    cat.write_text(catalogue)


def refusal(root: Path) -> str:
    try:
        acc.check(root)
    except acc.Refused as why:
        return str(why)
    return ""


def report(version: str, commit: str, rows: str) -> str:
    return textwrap.dedent(
        f"""\
        # Acceptance run v{version}

        - version: {version}
        - commit: {commit}
        - date: 2026-11-10
        - claude code: 2.1.322
        - lonca: 0.2.2
        - terminal: tmux 3.5, 120x40
        - spent: $9.80

        | case | result | commit | note |
        | --- | --- | --- | --- |
        """
    ) + rows


PASSED = "| TEAM-1 | pass | | |\n| TEAM-2 | pass | | |\n"


class CheckTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def test_a_whole_catalogue_passes(self):
        world(self.root)
        cat = acc.check(self.root)
        self.assertEqual([c.id for c in cat.cases], ["TEAM-1", "TEAM-2"])
        self.assertEqual(cat.cases[1].after, ["TEAM-1"])
        self.assertEqual(cat.cases[1].skip_if, "no terminal at hand")

    def test_a_record_no_case_names_is_refused(self):
        world(self.root, CASES.replace("- record 03: replaced\n", ""))
        self.assertIn("record 03 is named by no case", refusal(self.root))

    def test_a_command_no_case_names_is_refused(self):
        world(self.root, CASES.replace("- commands: :close\n", "- commands: —\n"))
        self.assertIn("`:close` is named by no case", refusal(self.root))

    def test_named_and_not_covered_is_refused(self):
        world(self.root, CASES.replace("- records: 02", "- records: 02, 03"))
        self.assertIn("record 03 is named by a case and under Not covered", refusal(self.root))

    def test_what_does_not_exist_is_refused(self):
        world(self.root, CASES.replace("- records: 02", "- records: 02, 07")
              .replace(":close", ":close, :nothing"))
        why = refusal(self.root)
        self.assertIn("record 07 is not in", why)
        self.assertIn("`:nothing` is not a command", why)

    def test_a_malformed_case_is_refused(self):
        bad = CASES.replace("- tier: real\n- skip-if", "- tier: maybe\n- skip-if")
        bad = bad.replace("Steps:\n1. Ask.\n\nExpect: work.\n", "")
        world(self.root, bad)
        why = refusal(self.root)
        self.assertIn("TEAM-2: tier `maybe`", why)
        self.assertIn("TEAM-2: no `Steps:`", why)
        self.assertIn("TEAM-2: no `Expect:`", why)

    def test_an_id_twice_or_retired_is_refused(self):
        world(self.root, CASES.replace("### TEAM-2 · A worker", "### TEAM-1 · A worker"))
        self.assertIn("TEAM-1: twice", refusal(self.root))
        world(self.root, CASES.replace("### TEAM-2 · A worker", "### TEAM-9 · A worker")
              .replace("- after: TEAM-1\n", ""))
        self.assertIn("TEAM-9: retired", refusal(self.root))

    def test_after_must_name_a_case_above(self):
        world(self.root, CASES.replace("- after: TEAM-1", "- after: TEAM-3"))
        self.assertIn("after `TEAM-3`", refusal(self.root))

    def test_a_stand_in_case_has_its_script_and_a_script_its_case(self):
        world(self.root, CASES.replace("- tier: real\n- skip-if", "- tier: stand-in\n- skip-if"))
        self.assertIn("TEAM-2: a stand-in case has its script", refusal(self.root))
        scripts = self.root / acc.SCENARIOS
        scripts.mkdir(parents=True)
        (scripts / "TEAM-2.scn").write_text("start\nsee words\n")
        self.assertEqual(refusal(self.root), "")
        # A step drive.py does not know.
        (scripts / "TEAM-2.scn").write_text("start\nlook words\n")
        self.assertIn("TEAM-2.scn: line 2: `look` is not a step", refusal(self.root))
        (scripts / "TEAM-2.scn").write_text("start\n")
        # A script with no case.
        (scripts / "TEAM-8.scn").write_text("start\n")
        self.assertIn("TEAM-8.scn: no case TEAM-8", refusal(self.root))
        (scripts / "TEAM-8.scn").unlink()
        # A real case's script goes under real/, and only there.
        (scripts / "TEAM-1.scn").write_text("start\n")
        self.assertIn("TEAM-1.scn: a real case's script goes in", refusal(self.root))
        (scripts / "TEAM-1.scn").unlink()
        (scripts / "real").mkdir()
        (scripts / "real" / "TEAM-1.scn").write_text("start\nallow 60 words\n")
        self.assertEqual(refusal(self.root), "")
        (scripts / "real" / "TEAM-2.scn").write_text("start\n")
        self.assertIn("real/TEAM-2.scn: a stand-in case's script goes in", refusal(self.root))

    def test_a_heading_not_of_a_case_is_refused(self):
        world(self.root, CASES.replace("### TEAM-1 · The", "### TEAM-1 The"))
        self.assertIn("a case heading is", refusal(self.root))


class LintTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        world(self.root)
        self.cat = acc.check(self.root)

    def tearDown(self):
        self.tmp.cleanup()

    def lint(self, text: str, version: str = "1.2.3") -> list[str]:
        return acc.lint(self.cat, acc.read_report(text), version)

    def test_a_whole_run_passes(self):
        self.assertEqual(self.lint(report("1.2.3", "a" * 40, PASSED)), [])

    def test_the_skeleton_is_not_a_run(self):
        text = acc.skeleton(self.cat, "1.2.3", "a" * 40, "2026-11-10")
        wrong = self.lint(text)
        self.assertIn("TEAM-1: `todo` is not pass, fail or skip", wrong)
        self.assertIn("`claude code` is not filled in", wrong)

    def test_a_failed_or_missing_row_holds_it(self):
        wrong = self.lint(report("1.2.3", "a" * 40, "| TEAM-1 | fail | | crashed |\n"))
        self.assertIn("TEAM-1: failed (crashed)", wrong)
        self.assertIn("TEAM-2: no row", wrong)

    def test_a_skip_needs_skip_if_and_why(self):
        rows = "| TEAM-1 | skip | | no time |\n| TEAM-2 | skip | | |\n"
        wrong = self.lint(report("1.2.3", "a" * 40, rows))
        self.assertIn("TEAM-1: skipped, but the case may not be (no skip-if)", wrong)
        self.assertIn("TEAM-2: skipped without saying why", wrong)
        rows = "| TEAM-1 | pass | | |\n| TEAM-2 | skip | | kitty only here |\n"
        self.assertEqual(self.lint(report("1.2.3", "a" * 40, rows)), [])

    def test_another_version_extra_rows_and_a_short_commit(self):
        wrong = self.lint(report("1.2.4", "abc", PASSED + "| TEAM-7 | pass | | |\n"))
        self.assertIn("the report is for 1.2.4, not 1.2.3", wrong)
        self.assertIn("commit `abc` is not a whole sha", wrong)
        self.assertIn("TEAM-7: not a case of the catalogue", wrong)

    def test_a_row_twice_is_refused(self):
        with self.assertRaises(acc.Refused):
            acc.read_report(report("1.2.3", "a" * 40, PASSED + "| TEAM-1 | pass | | |\n"))


def run(root: Path, *args: str) -> str:
    return subprocess.run(
        ["git", "-C", str(root), *args], check=True, capture_output=True, text=True
    ).stdout.strip()


class GateTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        world(self.root)
        run(self.root, "init", "-q", "-b", "main")
        run(self.root, "config", "user.email", "t@localhost")
        run(self.root, "config", "user.name", "t")
        run(self.root, "config", "commit.gpgsign", "false")
        self.commit("the candidate")
        self.candidate = run(self.root, "rev-parse", "HEAD")

    def tearDown(self):
        self.tmp.cleanup()

    def commit(self, message: str) -> None:
        run(self.root, "add", "-A")
        run(self.root, "-c", "core.hooksPath=/dev/null", "commit", "-q", "-m", message)

    def write_report(self, rows: str = PASSED, commit: str | None = None) -> None:
        path = self.root / acc.report_path("1.2.3")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(report("1.2.3", commit or self.candidate, rows))
        self.commit("the run")

    def test_the_run_on_what_is_tagged_lets_it_go(self):
        self.write_report()
        self.assertEqual(acc.gate(self.root, "v1.2.3"), [])

    def test_no_report_holds_it(self):
        self.assertIn("no acceptance run for v1.2.3", acc.gate(self.root, "v1.2.3")[0])

    def test_a_tag_that_is_not_the_version_holds_it(self):
        self.write_report()
        self.assertIn("is not v1.2.3", acc.gate(self.root, "v1.2.4")[0])
        self.assertIn("is not vX.Y.Z", acc.gate(self.root, "1.2.3")[0])

    def test_a_change_after_the_run_holds_it(self):
        self.write_report()
        (self.root / "late.rs").write_text("fn main() {}\n")
        self.commit("a fix after the run")
        wrong = acc.gate(self.root, "v1.2.3")
        self.assertIn("holds changes made after the run", wrong[0])
        self.assertIn("  late.rs", wrong)

    def test_a_commit_not_in_the_tag_holds_it(self):
        self.write_report(commit="b" * 40)
        self.assertIn("is not in what v1.2.3 holds", acc.gate(self.root, "v1.2.3")[0])

    def test_a_row_on_a_commit_not_before_the_run_holds_it(self):
        rows = f"| TEAM-1 | pass | {'c' * 12} | |\n| TEAM-2 | pass | | |\n"
        self.write_report(rows=rows)
        self.assertIn("TEAM-1: commit", acc.gate(self.root, "v1.2.3")[0])

    def test_a_row_on_an_earlier_commit_of_the_run_passes(self):
        earlier = self.candidate
        (self.root / "fix.rs").write_text("fn fixed() {}\n")
        self.commit("a fix")
        self.candidate = run(self.root, "rev-parse", "HEAD")
        rows = f"| TEAM-1 | pass | {earlier[:12]} | |\n| TEAM-2 | pass | | |\n"
        self.write_report(rows=rows)
        self.assertEqual(acc.gate(self.root, "v1.2.3"), [])

    def test_a_failed_case_holds_it(self):
        self.write_report(rows="| TEAM-1 | pass | | |\n| TEAM-2 | fail | | stuck |\n")
        self.assertIn("TEAM-2: failed (stuck)", acc.gate(self.root, "v1.2.3"))


if __name__ == "__main__":
    unittest.main()
