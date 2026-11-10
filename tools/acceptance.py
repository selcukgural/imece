#!/usr/bin/env python3
"""imece's acceptance catalogue and its runs (docs/acceptance, record 147).

    python3 -I tools/acceptance.py check          # the catalogue is whole
    python3 -I tools/acceptance.py new [--commit SHA]
                                                  # a run's report to fill, for
                                                  # the version in Cargo.toml
    python3 -I tools/acceptance.py lint REPORT    # a report as the gate reads it,
                                                  # without git
    python3 -I tools/acceptance.py gate TAG       # release.yml: the run for TAG
                                                  # passed, on what TAG holds

`check` runs in CI on every pull request: every case is well formed, and
every decision record and every `:` command is named by a case or under
"Not covered" with its reason, so a feature added without its acceptance
case fails the build. `gate` runs in release.yml before anything is built:
no release without a run of every case on the commit that is tagged.

Standard library only; run with `-I`.
"""

from __future__ import annotations

import argparse
import datetime
import importlib.util
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CATALOGUE = Path("docs/acceptance/catalogue.md")
RUNS = Path("docs/acceptance/runs")
DECISIONS = Path("docs/decisions")
COMPLETE = Path("crates/imece-tui/src/complete.rs")
SCENARIOS = Path("tools/acceptance/scenarios")
CARGO = Path("Cargo.toml")

TIERS = ("auto", "demo", "stand-in", "real")
RESULTS = ("pass", "fail", "skip")
HEAD_FIELDS = ("version", "commit", "date", "claude code", "lonca", "terminal", "spent")

CASE = re.compile(r"^### ([A-Z]+-[0-9]+) · (\S.*)$")
FIELD = re.compile(r"^- (records|commands|tier|skip-if|after): (.*)$")
NOT_COVERED = re.compile(r"^- (record ([0-9]+)|command :([a-z]+)): (\S.*)$")
RETIRED = re.compile(r"^- ([A-Z]+-[0-9]+): (\S.*)$")
HEX = re.compile(r"^[0-9a-f]{7,40}$")


class Refused(Exception):
    """What makes a catalogue, a report or a release wrong, in words."""


@dataclass
class Case:
    id: str
    title: str
    line: int
    records: list[int] = field(default_factory=list)
    commands: list[str] = field(default_factory=list)
    tier: str = ""
    skip_if: str = ""
    after: list[str] = field(default_factory=list)
    steps: bool = False
    expect: bool = False


@dataclass
class Catalogue:
    cases: list[Case]
    records_out: dict[int, str]
    commands_out: dict[str, str]
    retired: dict[str, str]

    def by_id(self) -> dict[str, Case]:
        return {c.id: c for c in self.cases}


def records(root: Path) -> set[int]:
    """The numbers of the decision records, from their file names."""
    found = set()
    for p in (root / DECISIONS).glob("*.md"):
        m = re.match(r"^([0-9]+)-", p.name)
        if m:
            found.add(int(m.group(1)))
    return found


def commands(root: Path) -> set[str]:
    """The `:` commands the screen completes (complete::COMMANDS)."""
    text = (root / COMPLETE).read_text(encoding="utf-8")
    m = re.search(r"pub const COMMANDS: \[&str; \d+\] = \[(.*?)\];", text, re.S)
    if not m:
        raise Refused(f"{COMPLETE}: no COMMANDS")
    return set(re.findall(r'"([a-z]+)"', m.group(1)))


def _list(value: str) -> list[str]:
    value = value.strip()
    if value in ("—", "-", ""):
        return []
    return [v.strip() for v in value.split(",") if v.strip()]


def parse(text: str) -> tuple[Catalogue, list[str]]:
    """The catalogue's cases and lists, and what is malformed in it."""
    wrong: list[str] = []
    cases: list[Case] = []
    records_out: dict[int, str] = {}
    commands_out: dict[str, str] = {}
    retired: dict[str, str] = {}
    section = ""
    case: Case | None = None
    for n, line in enumerate(text.splitlines(), 1):
        if line.startswith("## "):
            section = line[3:].strip()
            case = None
            continue
        m = CASE.match(line)
        if m:
            case = Case(m.group(1), m.group(2), n)
            cases.append(case)
            continue
        if line.startswith("### "):
            wrong.append(f"line {n}: a case heading is `### AREA-N · Title`: {line}")
            case = None
            continue
        if section == "Not covered":
            m = NOT_COVERED.match(line)
            if m:
                if m.group(2):
                    records_out[int(m.group(2))] = m.group(4)
                else:
                    commands_out[m.group(3)] = m.group(4)
            elif line.startswith("- "):
                wrong.append(f"line {n}: not `- record NN: why` or `- command :name: why`")
            continue
        if section == "Retired":
            m = RETIRED.match(line)
            if m:
                retired[m.group(1)] = m.group(2)
            elif line.startswith("- "):
                wrong.append(f"line {n}: not `- AREA-N: why`")
            continue
        if case is None:
            continue
        m = FIELD.match(line)
        if m:
            key, value = m.group(1), m.group(2).strip()
            if key == "records":
                for v in _list(value):
                    if not v.isdigit():
                        wrong.append(f"{case.id}: record `{v}` is not a number")
                    else:
                        case.records.append(int(v))
            elif key == "commands":
                for v in _list(value):
                    if not re.fullmatch(r":[a-z]+", v):
                        wrong.append(f"{case.id}: command `{v}` is not `:name`")
                    else:
                        case.commands.append(v[1:])
            elif key == "tier":
                case.tier = value
            elif key == "skip-if":
                case.skip_if = value
            elif key == "after":
                case.after = _list(value)
        elif line.startswith("Steps:"):
            case.steps = True
        elif line.startswith("Expect:"):
            case.expect = True
    return Catalogue(cases, records_out, commands_out, retired), wrong


def check(root: Path) -> Catalogue:
    """The catalogue, refused with every reason when it is not whole."""
    path = root / CATALOGUE
    if not path.is_file():
        raise Refused(f"no {CATALOGUE}")
    cat, wrong = parse(path.read_text(encoding="utf-8"))
    known_records = records(root)
    known_commands = commands(root)
    seen: dict[str, Case] = {}
    for c in cat.cases:
        if c.id in seen:
            wrong.append(f"{c.id}: twice (lines {seen[c.id].line} and {c.line})")
        if c.id in cat.retired:
            wrong.append(f"{c.id}: retired, its id is not used again")
        if c.tier not in TIERS:
            wrong.append(f"{c.id}: tier `{c.tier}` is not one of {', '.join(TIERS)}")
        if not c.steps:
            wrong.append(f"{c.id}: no `Steps:`")
        if not c.expect:
            wrong.append(f"{c.id}: no `Expect:`")
        for r in c.records:
            if r not in known_records:
                wrong.append(f"{c.id}: record {r:02} is not in {DECISIONS}")
        for name in c.commands:
            if name not in known_commands:
                wrong.append(f"{c.id}: `:{name}` is not a command (complete::COMMANDS)")
        for a in c.after:
            if a not in seen:
                wrong.append(f"{c.id}: after `{a}`, which is not a case above it")
        seen.setdefault(c.id, c)
    named_records = {r for c in cat.cases for r in c.records}
    named_commands = {name for c in cat.cases for name in c.commands}
    for r in sorted(known_records):
        if r not in named_records and r not in cat.records_out:
            wrong.append(f"record {r:02} is named by no case and not under Not covered")
        if r in named_records and r in cat.records_out:
            wrong.append(f"record {r:02} is named by a case and under Not covered")
    for r in sorted(cat.records_out):
        if r not in known_records:
            wrong.append(f"Not covered: record {r:02} is not in {DECISIONS}")
    for name in sorted(known_commands):
        if name not in named_commands and name not in cat.commands_out:
            wrong.append(f"`:{name}` is named by no case and not under Not covered")
        if name in named_commands and name in cat.commands_out:
            wrong.append(f"`:{name}` is named by a case and under Not covered")
    for name in sorted(cat.commands_out):
        if name not in known_commands:
            wrong.append(f"Not covered: `:{name}` is not a command")
    # A stand-in case is a scripted one: drive.py plays its scenario.
    scripted = {p.stem: p for p in sorted((root / SCENARIOS).glob("*.scn"))}
    for c in cat.cases:
        if c.tier == "stand-in" and c.id not in scripted:
            wrong.append(f"{c.id}: a stand-in case has its script {SCENARIOS}/{c.id}.scn")
    for cid, path in scripted.items():
        if cid not in seen:
            wrong.append(f"{SCENARIOS}/{path.name}: no case {cid} in the catalogue")
            continue
        why = scenario_steps(path)
        if why:
            wrong.append(f"{SCENARIOS}/{path.name}: {why}")
    if wrong:
        raise Refused("\n".join(wrong))
    return cat


def scenario_steps(path: Path) -> str:
    """Why scripted case `path` does not parse as drive.py plays it; empty
    when it does."""
    spec = importlib.util.spec_from_file_location("drive", Path(__file__).parent / "acceptance" / "drive.py")
    drive = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(drive)
    try:
        drive.parse(path.read_text(encoding="utf-8"))
    except drive.Failed as why:
        return str(why)
    return ""


def version(root: Path) -> str:
    text = (root / CARGO).read_text(encoding="utf-8")
    m = re.search(r'^version = "([^"]+)"$', text, re.M)
    if not m:
        raise Refused(f"{CARGO}: no version")
    return m.group(1)


def report_path(v: str) -> Path:
    return RUNS / f"v{v}.md"


def skeleton(cat: Catalogue, v: str, commit: str, today: str) -> str:
    lines = [
        f"# Acceptance run v{v}",
        "",
        f"- version: {v}",
        f"- commit: {commit}",
        f"- date: {today}",
        "- claude code: ",
        "- lonca: ",
        "- terminal: ",
        "- spent: ",
        "",
        "Each row: `pass`, `fail` or `skip` (a skip only where the case has",
        "`skip-if`, with why). `commit` empty means the commit above; an earlier",
        "commit of the same candidate only where nothing since touches the case.",
        "",
        "| case | result | commit | note |",
        "| --- | --- | --- | --- |",
    ]
    lines += [f"| {c.id} | todo | | |" for c in cat.cases]
    lines += ["", "## Findings", "", "What failed or rubbed, with the issue or PR that took it.", ""]
    return "\n".join(lines)


@dataclass
class Report:
    head: dict[str, str]
    rows: dict[str, tuple[str, str, str]]


def read_report(text: str) -> Report:
    head: dict[str, str] = {}
    rows: dict[str, tuple[str, str, str]] = {}
    twice: list[str] = []
    for line in text.splitlines():
        m = re.match(r"^- ([a-z ]+): ?(.*)$", line)
        if m and m.group(1) in HEAD_FIELDS and m.group(1) not in head:
            head[m.group(1)] = m.group(2).strip()
            continue
        if line.startswith("| ") and not line.startswith("| case") and not line.startswith("| ---"):
            cells = [c.strip() for c in line.strip().strip("|").split("|")]
            if len(cells) != 4:
                raise Refused(f"a row is `| case | result | commit | note |`: {line}")
            if cells[0] in rows:
                twice.append(cells[0])
            rows[cells[0]] = (cells[1], cells[2], cells[3])
    if twice:
        raise Refused("rows twice: " + ", ".join(twice))
    return Report(head, rows)


def lint(cat: Catalogue, report: Report, v: str) -> list[str]:
    """What keeps a report from letting version `v` go; empty when nothing does."""
    wrong: list[str] = []
    for f in HEAD_FIELDS:
        if not report.head.get(f):
            wrong.append(f"`{f}` is not filled in")
    if report.head.get("version") not in (None, "", v):
        wrong.append(f"the report is for {report.head['version']}, not {v}")
    commit = report.head.get("commit", "")
    if commit and not re.fullmatch(r"[0-9a-f]{40}", commit):
        wrong.append(f"commit `{commit}` is not a whole sha")
    by_id = cat.by_id()
    for c in cat.cases:
        if c.id not in report.rows:
            wrong.append(f"{c.id}: no row")
    for cid, (result, row_commit, note) in report.rows.items():
        case = by_id.get(cid)
        if case is None:
            wrong.append(f"{cid}: not a case of the catalogue")
            continue
        if result not in RESULTS:
            wrong.append(f"{cid}: `{result}` is not pass, fail or skip")
        elif result == "fail":
            wrong.append(f"{cid}: failed ({note or 'no note'})")
        elif result == "skip":
            if not case.skip_if:
                wrong.append(f"{cid}: skipped, but the case may not be (no skip-if)")
            elif not note:
                wrong.append(f"{cid}: skipped without saying why")
        if row_commit and not HEX.match(row_commit):
            wrong.append(f"{cid}: commit `{row_commit}` is not a sha")
    return wrong


def git(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True)


def ancestor(root: Path, older: str, newer: str) -> bool:
    return git(root, "merge-base", "--is-ancestor", older, newer).returncode == 0


def gate(root: Path, tag: str) -> list[str]:
    """What keeps `tag` from being released; empty when nothing does."""
    if not tag.startswith("v"):
        return [f"tag {tag} is not vX.Y.Z"]
    v = tag[1:]
    if v != version(root):
        return [f"tag {tag} is not v{version(root)} (Cargo.toml)"]
    cat = check(root)
    path = root / report_path(v)
    if not path.is_file():
        return [f"no acceptance run for {tag}: {report_path(v)} (docs/acceptance/README.md)"]
    report = read_report(path.read_text(encoding="utf-8"))
    wrong = lint(cat, report, v)
    commit = report.head.get("commit", "")
    if wrong or not commit:
        return wrong
    if not ancestor(root, commit, "HEAD"):
        return [f"the run's commit {commit} is not in what {tag} holds"]
    changed = git(root, "diff", "--name-only", commit, "HEAD")
    if changed.returncode != 0:
        return [f"git diff {commit}: {changed.stderr.strip()}"]
    runs = RUNS.as_posix() + "/"
    since = [p for p in changed.stdout.splitlines() if p and not p.startswith(runs)]
    if since:
        return [
            f"{tag} holds changes made after the run on {commit[:12]}; run again on them:",
            *(f"  {p}" for p in since),
        ]
    for cid, (_, row_commit, _) in report.rows.items():
        if row_commit and not ancestor(root, row_commit, commit):
            wrong.append(f"{cid}: commit {row_commit} is not an earlier commit of the run's")
    return wrong


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(prog="tools/acceptance.py", description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="what", required=True)
    sub.add_parser("check", help="the catalogue is whole")
    new = sub.add_parser("new", help="a run's report to fill in")
    new.add_argument("--commit", help="the commit the run is on (default: HEAD)")
    lint_p = sub.add_parser("lint", help="a report as the gate reads it, without git")
    lint_p.add_argument("report", type=Path)
    gate_p = sub.add_parser("gate", help="the run for TAG passed on what TAG holds")
    gate_p.add_argument("tag")
    args = ap.parse_args(argv)
    try:
        if args.what == "check":
            cat = check(ROOT)
            tiers = {t: sum(c.tier == t for c in cat.cases) for t in TIERS}
            said = ", ".join(f"{n} {t}" for t, n in tiers.items() if n)
            print(f"acceptance: {len(cat.cases)} cases ({said}); every record and command named")
            return 0
        if args.what == "new":
            cat = check(ROOT)
            v = version(ROOT)
            commit = args.commit or git(ROOT, "rev-parse", "HEAD").stdout.strip()
            full = git(ROOT, "rev-parse", "--verify", f"{commit}^{{commit}}")
            if full.returncode != 0:
                raise Refused(f"no commit {commit}")
            path = ROOT / report_path(v)
            if path.exists():
                raise Refused(f"{report_path(v)} is there already")
            path.parent.mkdir(parents=True, exist_ok=True)
            today = datetime.date.today().isoformat()
            path.write_text(skeleton(cat, v, full.stdout.strip(), today), encoding="utf-8")
            print(f"acceptance: {report_path(v)} written, {len(cat.cases)} rows to fill in")
            return 0
        if args.what == "lint":
            cat = check(ROOT)
            report = read_report(args.report.read_text(encoding="utf-8"))
            v = report.head.get("version", "")
            wrong = lint(cat, report, v)
        else:
            wrong = gate(ROOT, args.tag)
    except Refused as why:
        print(f"acceptance: {why}", file=sys.stderr)
        return 1
    if wrong:
        print("acceptance: not passed:", *wrong, sep="\n  ", file=sys.stderr)
        return 1
    print("acceptance: passed")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
