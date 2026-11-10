//! What a stand-in worker does with an `assign` (record 130): it keeps a
//! to-do list with Claude Code's task tools, reads the file of its
//! directory the task names best and adds the task to it, notes it in the
//! test file next to it, runs the tests of the project's kind, sometimes
//! first a command imece holds no routine (so the person is asked through
//! the permission prompt), and tells its leader `done` with what it did.
//! Sometimes it asks the person a question once it has read the code, ends
//! its turn, and goes on with the person's answer in the next. Sometimes
//! it hires a helper for the tests once it has made the change, ends its
//! turn, and goes on when the helper says done (or does them itself when
//! the helper stops first); the helper notes them in the test file, runs
//! them and tells its worker.
//! A card that owns paths (record 114) is done in the files it owns, and
//! sometimes the worker changes one file more, outside them, and says so
//! in its `done`. Offered the board's open cards, it takes the first with
//! `claim` and does it as an assign.
//! A directory with nothing to change gets the task written in a file of
//! its own. Its commands are never run: what they say is made up.

use std::path::Path;
use std::time::Duration;

use serde_json::json;

use super::{Hands, Mind, OUT_DIR, body_of, first_line};

/// The worker's to-do list, as Claude Code's task tools hold it.
pub const STEPS: [&str; 5] = [
    "Read the code around it",
    "Make the change",
    "Write the tests",
    "Run the tests",
    "Write it up for the leader",
];

/// The most files a worker looks at in its directory.
pub const FILES_MAX: usize = 2_000;

/// The longest pause between a worker's steps, in milliseconds.
pub const PAUSE_MOST: u64 = 10_000;

/// What a session's habits come from: variables its program sets
/// ([`super::install`]).
pub const PAUSE_VAR: &str = "IMECE_STAND_IN_PAUSE_MS";
pub const ASKS_VAR: &str = "IMECE_STAND_IN_ASKS";
pub const SEED_VAR: &str = "IMECE_STAND_IN_SEED";
pub const QUESTIONS_VAR: &str = "IMECE_STAND_IN_QUESTIONS";
pub const HELPERS_VAR: &str = "IMECE_STAND_IN_HELPERS";
pub const FAULTS_VAR: &str = "IMECE_STAND_IN_FAULTS";
pub const LIMIT_VAR: &str = "IMECE_STAND_IN_LIMIT";
pub const FILL_VAR: &str = "IMECE_STAND_IN_FILL";
pub const CARDS_VAR: &str = "IMECE_STAND_IN_CARDS";
pub const STRAYS_VAR: &str = "IMECE_STAND_IN_STRAYS";

/// Of the turns that fail at Claude's API, one in this many is the
/// account's (it waits for the person); the rest pass (imece tries them
/// again), as the simulation's two to one.
pub const ACCOUNT_IN: u64 = 3;

/// What a worker asks the person about the file it read (`{}`), with the
/// answers it offers: every other one offers some, as live agents are
/// told to when the answers are few (record 125); the rest take words.
pub const QUESTIONS: [(&str, &[&str]); 4] = [
    (
        "Should {} keep its old behaviour behind a flag, or change outright?",
        &["behind a flag", "change outright"],
    ),
    (
        "May I change what {} returns, or do callers depend on its shape?",
        &[],
    ),
    (
        "Does the change to {} need a line in the release notes?",
        &["yes, a line", "no, it is internal", "ask the leader"],
    ),
    (
        "Is there anything in {} you would rather I left alone?",
        &[],
    ),
];

/// What a test run says: the stand-in runs nothing.
pub const PASSED: &str = "All 12 tests passed.";
/// What any other command says.
pub const RAN: &str = "Done.";

/// How a helper's `done` begins when it wrote the tests.
pub const WROTE_TESTS: &str = "Wrote the tests in ";

/// How a stand-in works: the pause between its steps, how often a worker
/// runs a command that needs the person's yes first (one assign in
/// `asks`, never at 0), how often it asks the person a question (one
/// assign in `questions`, never at 0), how often it hires a helper for
/// the tests (one assign in `helpers`, never at 0), how often a turn of
/// any session fails at Claude's API (one in `faults`, never at 0), what
/// of the sessions' spending is a percent of the plan's five-hour window
/// (`limit` millionths of a dollar, four times that for the week's; at 0
/// no window is said), the tokens each message adds to a session's
/// context (`fill`; at 0 a tenth of [`super::CONTEXT_TURN`]), into how
/// many cards a leader cuts the person's task (`cards`; under 2 the task
/// goes whole to one worker), how often a worker whose card owns paths
/// changes a file outside them (one assign in `strays`, never at 0), and
/// the seed of its choices.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Habits {
    pub pause: Duration,
    pub asks: u64,
    pub questions: u64,
    pub helpers: u64,
    pub faults: u64,
    pub limit: u64,
    pub fill: u64,
    pub cards: u64,
    pub strays: u64,
    pub seed: u64,
}

impl Habits {
    /// The habits the variables `var` gives say; a variable not set or
    /// not a number is 0, a pause at most [`PAUSE_MOST`].
    pub fn of(var: impl Fn(&str) -> Option<String>) -> Habits {
        let n = |k: &str| {
            var(k)
                .and_then(|v| v.trim().parse::<u64>().ok())
                .unwrap_or(0)
        };
        Habits {
            pause: Duration::from_millis(n(PAUSE_VAR).min(PAUSE_MOST)),
            asks: n(ASKS_VAR),
            questions: n(QUESTIONS_VAR),
            helpers: n(HELPERS_VAR),
            faults: n(FAULTS_VAR),
            limit: n(LIMIT_VAR),
            fill: n(FILL_VAR),
            cards: n(CARDS_VAR),
            strays: n(STRAYS_VAR),
            seed: n(SEED_VAR),
        }
    }

    /// The variables that give these habits, for a shell's command line.
    pub fn vars(&self) -> String {
        format!(
            "{PAUSE_VAR}={} {ASKS_VAR}={} {QUESTIONS_VAR}={} {HELPERS_VAR}={} {FAULTS_VAR}={} \
             {LIMIT_VAR}={} {FILL_VAR}={} {CARDS_VAR}={} {STRAYS_VAR}={} {SEED_VAR}={}",
            self.pause.as_millis(),
            self.asks,
            self.questions,
            self.helpers,
            self.faults,
            self.limit,
            self.fill,
            self.cards,
            self.strays,
            self.seed
        )
    }

    /// The tokens a message adds to a session's context.
    pub fn message(&self) -> u64 {
        if self.fill == 0 {
            super::CONTEXT_TURN / 10
        } else {
            self.fill
        }
    }

    /// Whether the assign `roll` stands for runs a command that asks
    /// first.
    pub fn asks_now(&self, roll: u64) -> bool {
        self.asks > 0 && roll.is_multiple_of(self.asks)
    }

    /// Whether the assign `roll` stands for asks the person a question,
    /// by other bits of it than [`Habits::asks_now`]'s.
    pub fn questions_now(&self, roll: u64) -> bool {
        self.questions > 0 && (roll >> 16).is_multiple_of(self.questions)
    }

    /// Whether the assign `roll` stands for hires a helper for the
    /// tests, by other bits of it than the two above's.
    pub fn helps_now(&self, roll: u64) -> bool {
        self.helpers > 0 && (roll >> 32).is_multiple_of(self.helpers)
    }

    /// Whether the assign `roll` stands for changes a file its card does
    /// not own, by the roll's bits from the 56th.
    pub fn strays_now(&self, roll: u64) -> bool {
        self.strays > 0 && (roll >> 56).is_multiple_of(self.strays)
    }

    /// The fault the turn `roll` stands for fails on at Claude's API, if
    /// it fails, by the roll's bits from the 40th (its kind by those from
    /// the 48th): none at 0.
    pub fn fault_now(&self, roll: u64) -> Option<crate::Fault> {
        if self.faults == 0 || !(roll >> 40).is_multiple_of(self.faults) {
            None
        } else if (roll >> 48).is_multiple_of(ACCOUNT_IN) {
            Some(crate::Fault::Account)
        } else {
            Some(crate::Fault::Passing)
        }
    }
}

/// What the person's words ask of the stand-ins, whatever their habits
/// say: the acceptance scripts' way to make a case happen when they want
/// it (`docs/acceptance`), not one assign in so many. A cue is a word in
/// brackets anywhere in the task: `[ask]` runs a command that asks first,
/// `[question]` asks the person, `[helper]` hires a helper for the tests,
/// `[stray]` changes a file the card does not own, `[fault]` and
/// `[account]` fail the leader's turn at Claude's API (one imece tries
/// again, one that is the account's), `[cards N]` cuts the task into N
/// cards (2 to [`super::lead::CARDS_MOST`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cues {
    pub ask: bool,
    pub question: bool,
    pub helper: bool,
    pub stray: bool,
    pub fault: Option<crate::Fault>,
    pub cards: Option<u64>,
}

impl Cues {
    pub fn of(text: &str) -> Cues {
        let has = |cue: &str| text.contains(cue);
        let fault = if has("[account]") {
            Some(crate::Fault::Account)
        } else if has("[fault]") {
            Some(crate::Fault::Passing)
        } else {
            None
        };
        let cards = text.split("[cards ").nth(1).and_then(|rest| {
            let (n, _) = rest.split_once(']')?;
            n.trim().parse::<u64>().ok().filter(|n| *n >= 2)
        });
        Cues {
            ask: has("[ask]"),
            question: has("[question]"),
            helper: has("[helper]"),
            stray: has("[stray]"),
            fault,
            cards,
        }
    }
}

/// The question the assign `roll` stands for asks about file `path`, and
/// the answers it offers.
pub fn question(path: &str, roll: u64) -> (String, &'static [&'static str]) {
    let (text, options) = QUESTIONS[((roll >> 24) % QUESTIONS.len() as u64) as usize];
    (text.replace("{}", path), options)
}

/// A worker's task while it waits: the task, its to-do list's tasks by
/// step, the helper it waits for (none: the person's answer), and what
/// the person said when it asked them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Waiting {
    pub task: String,
    pub list: Vec<Option<u64>>,
    pub helper: Option<String>,
    pub said: Option<String>,
}

impl Waiting {
    pub fn to_json(&self) -> serde_json::Value {
        json!({ "task": self.task, "list": self.list, "helper": self.helper, "said": self.said })
    }

    /// What a transcript kept; none without a task.
    pub fn from_json(v: &serde_json::Value) -> Option<Waiting> {
        let text = |k: &str| v[k].as_str().map(String::from);
        Some(Waiting {
            task: text("task")?,
            list: (v["list"].as_array().into_iter().flatten())
                .map(serde_json::Value::as_u64)
                .collect(),
            helper: text("helper"),
            said: text("said"),
        })
    }
}

/// What a worker gives its helper: the test file to write in, the file
/// it changed, and the task, as [`part_of`] reads them back.
pub fn helper_task(test: &str, main: &str, task: &str) -> String {
    format!("Write the tests in {test} for the change to {main}:\n\n{task}")
}

/// The test file, the file changed and the task of a helper's assign
/// `body` ([`helper_task`]'s); none for anything else.
pub fn part_of(body: &str) -> Option<(&str, &str, &str)> {
    let (head, task) = body.split_once("\n\n").unwrap_or((body, ""));
    let rest = head
        .strip_prefix("Write the tests in ")?
        .strip_suffix(':')?;
    let (test, main) = rest.split_once(" for the change to ")?;
    Some((test, main, task.trim()))
}

/// Whether helper `name` was lost before its done, as imece tells its
/// worker in a note (`helper_lost` in the screen's crate).
fn lost(prompt: &str, name: &str) -> bool {
    prompt.contains(&format!(
        "Your helper {name} stopped before it sent you done"
    ))
}

/// The body of the first `done` helper `name` sent in a mail prompt.
fn done_from(prompt: &str, name: &str) -> Option<String> {
    let mut lines = prompt.lines();
    lines.find(|l| {
        l.strip_prefix("[done").is_some_and(|l| {
            (l.starts_with(' ') || l.starts_with(']')) && l.ends_with(&format!("] from {name}:"))
        })
    })?;
    let body: Vec<&str> = lines.take_while(|l| !l.starts_with('[')).collect();
    Some(body.join("\n").trim().to_string())
}

/// The person's words in prompt `prompt`: its last paragraph before the
/// mail that came with it (imece puts its own notes first), at most
/// [`super::TASK_MAX`] characters; none when only mail came.
pub fn heard(prompt: &str) -> Option<String> {
    let words = prompt.split(super::MAIL).next().unwrap_or_default();
    let last = (words.rsplit("\n\n").map(str::trim)).find(|p| !p.is_empty())?;
    Some(last.chars().take(super::TASK_MAX).collect())
}

/// A number for agent `name`'s `n`th assign under `seed`, the same every
/// run (FNV-1a).
pub fn roll(seed: u64, name: &str, n: u64) -> u64 {
    let bytes = (seed.to_le_bytes().into_iter())
        .chain(name.bytes())
        .chain(n.to_le_bytes());
    bytes.fold(0xcbf2_9ce4_8422_2325, |h: u64, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// What a project of a kind runs: its tests, and commands imece holds no
/// routine (a shell's operators, programs not on the run list), which a
/// worker sometimes runs first.
#[derive(Debug, PartialEq)]
pub struct Kind {
    pub test: Option<&'static str>,
    pub asks: &'static [&'static str],
}

/// The kinds, by the extensions of their files.
const KINDS: &[(&[&str], Kind)] = &[
    (
        &["cs"],
        Kind {
            test: Some("dotnet test"),
            asks: &[
                "dotnet ef migrations script > migrate.sql",
                "docker compose up -d db",
            ],
        },
    ),
    (
        &["kt"],
        Kind {
            test: Some("gradle test"),
            asks: &[
                "gradle dependencies --refresh-dependencies > deps.txt",
                "adb devices",
            ],
        },
    ),
    (
        &["ts", "tsx", "js"],
        Kind {
            test: Some("npm test"),
            asks: &[
                "npm install zod@3 && npm test",
                "gh pr view \"$(git branch --show-current)\"",
            ],
        },
    ),
    (
        &["go"],
        Kind {
            test: Some("go test ./..."),
            asks: &[
                "go get github.com/shopspring/decimal && go mod tidy",
                "docker compose up -d db",
            ],
        },
    ),
    (
        &["rs"],
        Kind {
            test: Some("cargo test"),
            asks: &["cargo add serde --features derive"],
        },
    ),
    (
        &["py"],
        Kind {
            test: Some("python3 -m unittest"),
            asks: &["python3 -m pip --version"],
        },
    ),
];

/// A file of no kind above: no tests, one command that asks.
pub const OTHER: Kind = Kind {
    test: None,
    asks: &["curl -sS https://example.com/status"],
};

/// The programs the kinds' tests run: the demo names them as the run
/// list's (`--run`), so a test run is routine and allowed at once.
/// (`cargo` is always one.)
pub const TEST_PROGRAMS: [&str; 4] = ["dotnet", "gradle", "npm", "go"];

fn extension(path: &str) -> &str {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
}

/// The kind of the project file `path` is of.
pub fn kind_of(path: &str) -> &'static Kind {
    let ext = extension(path);
    (KINDS.iter())
        .find(|(exts, _)| exts.contains(&ext))
        .map_or(&OTHER, |(_, k)| k)
}

/// How a line of a note starts in file `path`: a comment of its kind of
/// code, else nothing.
pub fn mark_of(path: &str) -> &'static str {
    match extension(path) {
        "cs" | "kt" | "ts" | "tsx" | "js" | "go" | "rs" => "// ",
        "py" | "sh" | "toml" | "yml" | "yaml" => "# ",
        _ => "",
    }
}

/// `text` as a note in file `path`, a line each.
pub fn noted(path: &str, text: &str) -> String {
    let mark = mark_of(path);
    let lines: Vec<String> = text.lines().map(|l| format!("{mark}{l}")).collect();
    lines.join("\n")
}

/// The files of `dir` a worker may look at, relative and sorted: none
/// whose name starts with a dot, none under `node_modules` or `target`,
/// no link, at most [`FILES_MAX`].
pub fn files_in(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut dirs = vec![dir.to_path_buf()];
    while let Some(d) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name == "node_modules" || name == "target" {
                continue;
            }
            let Ok(t) = e.file_type() else { continue };
            if t.is_dir() {
                dirs.push(e.path());
            } else if t.is_file()
                && found.len() < FILES_MAX
                && let Ok(rel) = e.path().strip_prefix(dir)
            {
                found.push(rel.to_string_lossy().into_owned());
            }
        }
    }
    found.sort();
    found
}

/// The words of `text` a path may name: letters and digits, three at
/// least, folded.
fn words(text: &str) -> Vec<String> {
    (text.split(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.chars().count() >= 3)
        .map(str::to_lowercase)
        .collect()
}

/// How many of `words` path `path` names.
fn score(path: &str, words: &[String]) -> usize {
    let path = path.to_lowercase();
    words.iter().filter(|w| path.contains(w.as_str())).count()
}

fn is_test(path: &str) -> bool {
    path.to_lowercase().contains("test")
}

/// What a note can go in: no JSON, which has no comments.
fn editable(path: &str) -> bool {
    extension(path) != "json"
}

/// The file of `files` to change for `task`, and the test file to note
/// it in: the file that names most of the task's words, not a test when
/// another is there (equals by `roll`); the test file that names most of
/// its name's words, the first of equals, none when no test names one.
/// None when nothing can be changed.
pub fn pick<'a>(files: &'a [String], task: &str, roll: u64) -> Option<(&'a str, Option<&'a str>)> {
    let can: Vec<&str> = (files.iter().map(String::as_str))
        .filter(|f| editable(f))
        .collect();
    let code: Vec<&str> = can.iter().copied().filter(|f| !is_test(f)).collect();
    let from = if code.is_empty() { &can } else { &code };
    let task = words(task);
    let best = from.iter().map(|f| score(f, &task)).max()?;
    let ties: Vec<&str> = (from.iter().copied())
        .filter(|f| score(f, &task) == best)
        .collect();
    let main = ties[(roll % ties.len() as u64) as usize];
    Some((main, test_for(files, main)))
}

/// The test file of `files` to note a change to `main` in: the one that
/// names most of its name's words, the first of equals; none when no
/// test names one.
pub fn test_for<'a>(files: &'a [String], main: &str) -> Option<&'a str> {
    let stem = Path::new(main)
        .file_stem()
        .map(|s| words(&s.to_string_lossy()))
        .unwrap_or_default();
    let tests: Vec<&str> = (files.iter().map(String::as_str))
        .filter(|f| editable(f) && is_test(f) && *f != main)
        .collect();
    let most = tests
        .iter()
        .map(|f| score(f, &stem))
        .max()
        .filter(|&m| m > 0);
    most.and_then(|m| tests.into_iter().find(|f| score(f, &stem) == m))
}

/// The files of `files` that are code (of a kind that has tests: no
/// document, no settings), those that name most of `task`'s words first,
/// equals in their order.
pub fn code_for<'a>(files: &'a [String], task: &str) -> Vec<&'a str> {
    let task = words(task);
    let mut code: Vec<&str> = (files.iter().map(String::as_str))
        .filter(|f| kind_of(f).test.is_some() && !is_test(f))
        .collect();
    code.sort_by_key(|f| std::cmp::Reverse(score(f, &task)));
    code
}

/// How imece says what a card owns at the end of its task (`bus::owns`
/// in the screen's crate), up to its paths and after them.
pub const OWNS: &str = "\n\nThis card owns ";
pub const OWNS_END: &str = ": stay in them";
/// How it says the checks a card asks for (`bus::asks` there).
pub const ASKS_CHECKS: &str = "\n\nThis card asks for ";

/// The paths the card of task `task` owns, as imece listed them; none
/// when it owns none.
pub fn owned_of(task: &str) -> Vec<&str> {
    let Some((_, rest)) = task.split_once(OWNS) else {
        return Vec::new();
    };
    let Some((paths, _)) = rest.split_once(OWNS_END) else {
        return Vec::new();
    };
    (paths.split(", "))
        .map(|p| p.strip_suffix(" (shared)").unwrap_or(p))
        .collect()
}

/// Task `task` as it was written: without what imece added on its card's
/// paths and checks.
pub fn plain(task: &str) -> &str {
    let end = [OWNS, ASKS_CHECKS]
        .iter()
        .filter_map(|m| task.find(m))
        .min();
    &task[..end.unwrap_or(task.len())]
}

/// The file to change for `task` and its test file: the code and the
/// test its card owns, when it owns a file that is there; else what
/// [`pick`] finds.
pub fn chosen<'a>(
    files: &'a [String],
    task: &str,
    roll: u64,
) -> Option<(&'a str, Option<&'a str>)> {
    let owned = owned_of(task);
    let mine = |test: bool| {
        (files.iter().map(String::as_str))
            .find(|f| owned.contains(f) && editable(f) && is_test(f) == test)
    };
    match mine(false) {
        Some(main) => Some((main, mine(true))),
        None => pick(files, plain(task), roll),
    }
}

/// The file a worker whose card owns `owned` strays into: code a note
/// can go in that the card does not own, by the roll's bits from the
/// 60th; none when the card owns nothing or all of it.
pub fn outside<'a>(files: &'a [String], owned: &[&str], roll: u64) -> Option<&'a str> {
    if owned.is_empty() {
        return None;
    }
    let out: Vec<&str> = (files.iter().map(String::as_str))
        .filter(|f| editable(f) && !is_test(f) && !owned.contains(f))
        .collect();
    let n = u64::try_from(out.len()).ok().filter(|&n| n > 0)?;
    Some(out[((roll >> 60) % n) as usize])
}

/// How the prompt that offers a worker the board's open cards begins
/// (`board_prompt` in the screen's crate).
pub const BOARD: &str = "Open cards on imece's board";

/// The first card prompt `prompt` offers, by its number; none when it
/// offers none.
pub fn offered(prompt: &str) -> Option<u64> {
    let (_, cards) = prompt.split_once(BOARD)?;
    let line = cards.lines().find(|l| l.starts_with('#'))?;
    line[1..].split(' ').next()?.parse().ok()
}

/// The task of a card taken, from `claim`'s answer `said`: what was
/// posted, and what imece added on the card's paths and checks.
pub fn claimed(said: &str) -> Option<String> {
    let (_, rest) = said.split_once("):\n\n")?;
    let (body, after) = rest.rsplit_once("\n\nWhen it is done, send ")?;
    let (_, added) = after.split_once(" a done message.")?;
    Some(format!("{body}{added}"))
}

/// What the stand-in's shell says for `command`.
pub fn ran(command: &str) -> &'static str {
    if command.contains("test") {
        PASSED
    } else {
        RAN
    }
}

/// What a `Read` of an empty file says, as Claude Code's does.
pub const EMPTY: &str =
    "<system-reminder>Warning: the file exists but the contents are empty.</system-reminder>";

/// A `Read` answer for a file holding `text`: each line after its number
/// and an arrow, as Claude Code's; [`EMPTY`] for nothing.
pub fn numbered(text: &str) -> String {
    if text.is_empty() {
        return EMPTY.into();
    }
    let lines: Vec<String> = (text.lines().enumerate())
        .map(|(n, l)| format!("{:>6}\u{2192}{l}", n + 1))
        .collect();
    lines.join("\n")
}

/// `was` with `old` replaced by `new`, as Claude Code's `Edit`: `old`
/// must be there once.
pub fn edited(was: &str, old: &str, new: &str) -> Result<String, String> {
    if old.is_empty() {
        return Err("old_string is empty".into());
    }
    match was.matches(old).count() {
        0 => Err("String to replace not found in file.".into()),
        1 => Ok(was.replacen(old, new, 1)),
        n => Err(format!("Found {n} matches of the string to replace.")),
    }
}

/// The text of a `Read` answer: its lines without their numbers.
pub fn unnumbered(read: &str) -> String {
    let lines: Vec<&str> = read
        .lines()
        .filter_map(|l| l.split_once('\u{2192}').map(|(_, t)| t))
        .collect();
    lines.join("\n")
}

/// A to-do list's tasks, by step; none for a step whose task was not
/// made.
struct List(Vec<Option<u64>>);

impl List {
    /// The list made, when the session has the task tools.
    fn made(hands: &mut impl Hands) -> List {
        if !hands.has("TaskCreate") {
            return List(Vec::new());
        }
        let made = STEPS.map(|s| {
            let said = hands.tool(
                "TaskCreate",
                json!({ "subject": s, "description": s, "activeForm": s }),
            );
            said.ok().and_then(|t| crate::claude::created(&json!(t)))
        });
        List(made.into())
    }

    fn mark(&self, hands: &mut impl Hands, step: usize, status: &str) {
        if let Some(Some(id)) = self.0.get(step) {
            let _ = hands.tool(
                "TaskUpdate",
                json!({ "taskId": id.to_string(), "status": status }),
            );
        }
    }

    /// Step `step` begins: in progress, then a pause.
    fn begin(&self, hands: &mut impl Hands, step: usize) {
        self.mark(hands, step, "in_progress");
        hands.pause();
    }
}

/// Adds `text` to file `path` as Claude Code would: reads it, then an
/// `Edit` of its whole text (unique), or a `Write` when it is empty.
fn append(hands: &mut impl Hands, path: &str, text: &str) -> Result<(), String> {
    let was = unnumbered(&hands.tool("Read", json!({ "file_path": path }))?);
    let note = noted(path, text);
    if was.is_empty() {
        return hands
            .tool(
                "Write",
                json!({ "file_path": path, "content": note + "\n" }),
            )
            .map(|_| ());
    }
    let new = format!("{was}\n{note}");
    let input = json!({ "file_path": path, "old_string": was, "new_string": new });
    hands.tool("Edit", input).map(|_| ())
}

impl Mind {
    /// A worker's turn: the first `assign` of its mail done, else
    /// nothing; when it waits for the person's answer, the task goes on
    /// with the person's words, and when it waits for its helper, with
    /// the helper's done (or without it, the helper lost).
    pub(super) fn work(&mut self, hands: &mut impl Hands, prompt: &str) -> String {
        if let Some(waiting) = self.waiting.take() {
            return match waiting.helper.clone() {
                Some(helper) => self.helped(hands, prompt, waiting, &helper),
                None => match heard(prompt) {
                    Some(said) => self.go_on(hands, &waiting.task, List(waiting.list), Some(&said)),
                    None => {
                        self.waiting = Some(waiting);
                        "I still wait for the person's answer.".into()
                    }
                },
            };
        }
        let task = match (body_of(prompt, "assign"), offered(prompt)) {
            (Some(task), _) => task,
            (None, Some(card)) => match hands.call("claim", json!({ "card": card })) {
                Ok(said) => match claimed(&said) {
                    Some(task) => task,
                    None => return format!("I took #{card}, but could not read its task."),
                },
                Err(why) => return format!("I could not take #{card}: {why}"),
            },
            (None, None) => return "Nothing assigned to me.".into(),
        };
        self.worked += 1;
        let me = self.me.get("name").cloned().unwrap_or_default();
        let lead = self.lead_of();
        let habits = *hands.habits();
        let roll = roll(habits.seed, &me, self.worked);
        let files = hands.files();
        let Some((main, _)) = chosen(&files, &task, roll) else {
            return write_up(hands, &me, &lead, &task);
        };
        let list = List::made(hands);
        hands.pause();
        list.begin(hands, 0);
        if let Err(why) = hands.tool("Read", json!({ "file_path": main })) {
            return format!("I could not read {main}: {why}");
        }
        list.mark(hands, 0, "completed");
        if habits.questions_now(roll) || Cues::of(&task).question {
            let (q, options) = question(main, roll);
            let ask = json!({ "to": "you", "type": "ask", "body": q, "options": options });
            if hands.call("send", ask).is_ok() {
                self.waiting = Some(Waiting {
                    task,
                    list: list.0,
                    ..Waiting::default()
                });
                return format!("I asked you: {q}");
            }
        }
        self.go_on(hands, &task, list, None)
    }

    /// Whom a worker answers to: its leader's name.
    fn lead_of(&self) -> String {
        (self.me.get("you answer to"))
            .and_then(|o| o.strip_prefix("the leader "))
            .unwrap_or_default()
            .to_string()
    }

    /// The roll of the assign at hand.
    fn roll_now(&self, hands: &impl Hands) -> u64 {
        let me = self.me.get("name").map_or("", String::as_str);
        roll(hands.habits().seed, me, self.worked)
    }

    /// The task after its code was read: the change, then its tests by a
    /// helper (the turn ends) or by itself, a run of them and `done`,
    /// with what the person said when it asked them.
    fn go_on(
        &mut self,
        hands: &mut impl Hands,
        task: &str,
        list: List,
        said: Option<&str>,
    ) -> String {
        let me = self.me.get("name").cloned().unwrap_or_default();
        let lead = self.lead_of();
        let roll = self.roll_now(hands);
        let files = hands.files();
        let Some((main, test)) = chosen(&files, task, roll) else {
            return write_up(hands, &me, &lead, task);
        };
        list.begin(hands, 1);
        if let Err(why) = append(hands, main, plain(task)) {
            return format!("I could not change {main}: {why}");
        }
        list.mark(hands, 1, "completed");
        list.begin(hands, 2);
        if let Some(t) = test
            && (hands.habits().helps_now(roll) || Cues::of(task).helper)
            && let Some(helper) = hire_for(hands, t, main, task)
        {
            self.waiting = Some(Waiting {
                task: task.to_string(),
                list: list.0,
                helper: Some(helper.clone()),
                said: said.map(String::from),
            });
            return format!("{helper} writes the tests in {t}; I wait for its done.");
        }
        let mut notes = said_note(said);
        let changed = tested(hands, main, test, task, &mut notes);
        self.finish(hands, task, list, changed, notes)
    }

    /// A worker that waits for helper `helper` hears prompt `prompt`:
    /// with the helper's done, its tests are the helper's; with the
    /// helper lost, or a done that wrote none, it writes them itself;
    /// else it waits on.
    fn helped(
        &mut self,
        hands: &mut impl Hands,
        prompt: &str,
        waiting: Waiting,
        helper: &str,
    ) -> String {
        let done = done_from(prompt, helper);
        if done.is_none() && !lost(prompt, helper) {
            self.waiting = Some(waiting);
            return format!("I still wait for {helper}.");
        }
        let roll = self.roll_now(hands);
        let files = hands.files();
        let Some((main, test)) = chosen(&files, &waiting.task, roll) else {
            return format!("I can no longer find what {helper} tested.");
        };
        let mut notes = said_note(waiting.said.as_deref());
        let task = waiting.task.as_str();
        let changed = match done {
            Some(d) if d.starts_with(WROTE_TESTS) => {
                notes.push(format!("{helper}: {}", first_line(&d)));
                let mut changed = vec![main.to_string()];
                changed.extend(test.map(String::from));
                changed
            }
            d => {
                let why = d.as_deref().map_or("it stopped", first_line).to_string();
                notes.push(format!("{helper} did not write the tests ({why}); I did."));
                tested(hands, main, test, task, &mut notes)
            }
        };
        self.finish(hands, task, List(waiting.list), changed, notes)
    }

    /// The task's last steps: a run of the tests, and `done` with what
    /// changed and `notes`.
    fn finish(
        &self,
        hands: &mut impl Hands,
        task: &str,
        list: List,
        mut changed: Vec<String>,
        mut notes: Vec<String>,
    ) -> String {
        let lead = self.lead_of();
        let roll = self.roll_now(hands);
        let habits = *hands.habits();
        let cues = Cues::of(task);
        let files = hands.files();
        if (habits.strays_now(roll) || cues.stray)
            && let Some(other) = outside(&files, &owned_of(task), roll)
            && append(hands, other, plain(task)).is_ok()
        {
            changed.push(other.to_string());
            notes.push(format!(
                "Also changed {other}, which the card does not own."
            ));
        }
        list.mark(hands, 2, "completed");
        list.begin(hands, 3);
        if hands.has(crate::claude::SHELL_TOOL) {
            let kind = kind_of(&changed[0]);
            let asked = ((habits.asks_now(roll) || cues.ask) && !kind.asks.is_empty())
                .then(|| kind.asks[((roll >> 8) % kind.asks.len() as u64) as usize]);
            notes.extend(run(hands, asked.into_iter().chain(kind.test), task));
        }
        list.mark(hands, 3, "completed");
        list.begin(hands, 4);
        let mut body = format!("{}\n\nChanged {}.", first_line(task), changed.join(", "));
        for n in &notes {
            body.push_str(&format!("\n{n}"));
        }
        let sent = hands.call("send", json!({ "to": lead, "type": "done", "body": body }));
        list.mark(hands, 4, "completed");
        match sent {
            Ok(_) => format!("Done: changed {}.", changed.join(", ")),
            Err(why) => format!(
                "Changed {}, but {lead} did not hear: {why}",
                changed.join(", ")
            ),
        }
    }

    /// A helper's turn: the tests its worker's assign names written in
    /// their file, run, and its worker told with `done` (imece lets it
    /// go then).
    pub(super) fn help(&mut self, hands: &mut impl Hands, prompt: &str) -> String {
        let Some(body) = body_of(prompt, "assign") else {
            return "Nothing assigned to me.".into();
        };
        let worker = (self.me.get("you answer to"))
            .and_then(|o| o.strip_suffix(", who hired you"))
            .unwrap_or_default()
            .to_string();
        self.worked += 1;
        let said = match part_of(&body) {
            None => "I could not tell which tests to write.".to_string(),
            Some((test, main, task)) => {
                hands.pause();
                match append(hands, test, &format!("covers: {}", first_line(task))) {
                    Err(why) => format!("I could not write the tests in {test}: {why}"),
                    Ok(()) => {
                        hands.pause();
                        let ran = if hands.has(crate::claude::SHELL_TOOL) {
                            run(hands, kind_of(main).test, task)
                        } else {
                            Vec::new()
                        };
                        let mut said = format!("{WROTE_TESTS}{test}.");
                        for r in ran {
                            said.push_str(&format!("\n{r}"));
                        }
                        said
                    }
                }
            }
        };
        let done = json!({ "to": worker, "type": "done", "body": said });
        match hands.call("send", done) {
            Ok(_) => first_line(&said).to_string(),
            Err(why) => format!(
                "{}, but {worker} did not hear: {why}",
                first_line(&said).trim_end_matches('.')
            ),
        }
    }
}

/// What the person said, as a line of a done.
fn said_note(said: Option<&str>) -> Vec<String> {
    said.map(|s| format!("The person said: {s}"))
        .into_iter()
        .collect()
}

/// The worker's tests written in `test` itself: the files changed, with
/// what failed in `notes`.
fn tested(
    hands: &mut impl Hands,
    main: &str,
    test: Option<&str>,
    task: &str,
    notes: &mut Vec<String>,
) -> Vec<String> {
    let mut changed = vec![main.to_string()];
    if let Some(t) = test {
        match append(hands, t, &format!("covers: {}", first_line(task))) {
            Ok(()) => changed.push(t.to_string()),
            Err(why) => notes.push(format!("{t} not changed: {why}")),
        }
    }
    changed
}

/// Runs `commands` in the shell, described by the task's first line: a
/// line each of what they said.
fn run<'a>(
    hands: &mut impl Hands,
    commands: impl IntoIterator<Item = &'a str>,
    task: &str,
) -> Vec<String> {
    let mut said = Vec::new();
    for command in commands {
        let input = json!({ "command": command, "description": first_line(task) });
        said.push(match hands.tool(crate::claude::SHELL_TOOL, input) {
            Ok(out) => format!("{command}: {}", first_line(&out)),
            Err(why) => format!("{command} did not run: {why}"),
        });
    }
    said
}

/// Hires a helper for the tests in `test` of the change to `main` and
/// gives it them: its name, none when it could not be hired or told.
fn hire_for(hands: &mut impl Hands, test: &str, main: &str, task: &str) -> Option<String> {
    let said = hands.call("hire", json!({ "tier": "cheap" })).ok()?;
    let name = said
        .split(' ')
        .next()
        .filter(|n| !n.is_empty())?
        .to_string();
    let part = helper_task(test, main, plain(task));
    let assign = json!({ "to": name, "type": "assign", "body": part });
    hands.call("send", assign).ok()?;
    Some(name)
}

/// A worker's turn in a directory with nothing to change: the task in a
/// file of its own, [`OUT_DIR`]`/<name>.md`, and a `done` that names it.
fn write_up(hands: &mut impl Hands, me: &str, lead: &str, task: &str) -> String {
    let path = format!("{OUT_DIR}/{me}.md");
    let text = format!("# {}\n\n{}\n", first_line(task), plain(task));
    if let Err(why) = hands.tool("Write", json!({ "file_path": path, "content": text })) {
        return format!("I could not write {path}: {why}");
    }
    let body = format!("wrote {path}");
    match hands.call("send", json!({ "to": lead, "type": "done", "body": body })) {
        Ok(_) => format!("Done: {body}."),
        Err(why) => format!("{body}, but {lead} did not hear: {why}"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::MAIL;
    use super::super::tests::{Fake, WORKER, fake, strings};
    use super::*;

    #[test]
    fn habits_come_from_their_variables_and_go_back_to_them() {
        let vars = |v: [&str; 10]| {
            let [
                pause,
                asks,
                questions,
                helpers,
                faults,
                limit,
                fill,
                cards,
                strays,
                seed,
            ] = v.map(String::from);
            Habits::of(move |k| match k {
                PAUSE_VAR => Some(pause.clone()),
                ASKS_VAR => Some(asks.clone()),
                QUESTIONS_VAR => Some(questions.clone()),
                HELPERS_VAR => Some(helpers.clone()),
                FAULTS_VAR => Some(faults.clone()),
                LIMIT_VAR => Some(limit.clone()),
                FILL_VAR => Some(fill.clone()),
                CARDS_VAR => Some(cards.clone()),
                STRAYS_VAR => Some(strays.clone()),
                SEED_VAR => Some(seed.clone()),
                _ => None,
            })
        };
        let h = vars([" 1500 ", "3", "4", "5", "6", "7", "8", "9", "10", "42"]);
        assert_eq!(
            h,
            Habits {
                pause: Duration::from_millis(1500),
                asks: 3,
                questions: 4,
                helpers: 5,
                faults: 6,
                limit: 7,
                fill: 8,
                cards: 9,
                strays: 10,
                seed: 42,
            }
        );
        assert_eq!(
            h.vars(),
            "IMECE_STAND_IN_PAUSE_MS=1500 IMECE_STAND_IN_ASKS=3 IMECE_STAND_IN_QUESTIONS=4 \
             IMECE_STAND_IN_HELPERS=5 IMECE_STAND_IN_FAULTS=6 IMECE_STAND_IN_LIMIT=7 \
             IMECE_STAND_IN_FILL=8 IMECE_STAND_IN_CARDS=9 IMECE_STAND_IN_STRAYS=10 \
             IMECE_STAND_IN_SEED=42"
        );
        assert_eq!(Habits::of(|_| None), Habits::default());
        assert_eq!(
            vars(["x", "-1", "y", "z", "f", "l", "c", "k", "s", ""]),
            Habits::default(),
            "not numbers: 0"
        );
        assert_eq!(
            vars(["10000", "0", "0", "0", "0", "0", "0", "0", "0", "0"]).pause,
            Duration::from_millis(10_000)
        );
        assert_eq!(
            vars(["10001", "0", "0", "0", "0", "0", "0", "0", "0", "0"]).pause,
            Duration::from_millis(PAUSE_MOST),
            "at most"
        );
    }

    #[test]
    fn one_assign_in_asks_runs_a_command_that_asks_never_at_0() {
        let h = |asks| Habits {
            asks,
            ..Habits::default()
        };
        assert!(!h(0).asks_now(0));
        assert!(h(1).asks_now(5));
        assert!(h(3).asks_now(6) && !h(3).asks_now(7) && !h(3).asks_now(8));
    }

    #[test]
    fn one_turn_in_faults_fails_by_the_40th_bit_on_one_in_three_the_account_s() {
        use crate::Fault::{Account, Passing};
        let h = |faults| Habits {
            faults,
            ..Habits::default()
        };
        let at = |fails: u64, kind: u64| (fails << 40) | (kind << 48);
        assert_eq!(h(0).fault_now(0), None, "never at 0");
        // One in `faults` by the bits from the 40th; those below count
        // for nothing.
        assert_eq!(h(1).fault_now(at(5, 1)), Some(Passing));
        assert_eq!(h(4).fault_now(at(8, 1) | 0xff_ffff_ffff), Some(Passing));
        assert_eq!(h(4).fault_now(at(9, 1)), None);
        assert_eq!(h(4).fault_now(at(12, 1)), Some(Passing));
        assert_eq!(h(4).fault_now(1 << 40), None);
        // Of those, one in three is the account's, by the bits from the
        // 48th (which the fault's own bits include).
        assert_eq!(ACCOUNT_IN, 3);
        assert_eq!(h(1).fault_now(at(0, 0)), Some(Account));
        assert_eq!(h(1).fault_now(at(0, 3)), Some(Account));
        assert_eq!(h(1).fault_now(at(0, 2)), Some(Passing));
        assert_eq!(h(1).fault_now(at(0, 4)), Some(Passing));
    }

    #[test]
    fn a_roll_is_fnv_of_the_seed_the_name_and_the_number() {
        // FNV-1a's offset basis for nothing; one byte moves it as FNV does.
        let fnv = |bytes: &[u8]| {
            bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
                (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
            })
        };
        let mut bytes = 7u64.to_le_bytes().to_vec();
        bytes.extend(b"claude-1");
        bytes.extend(2u64.to_le_bytes());
        assert_eq!(roll(7, "claude-1", 2), fnv(&bytes));
        assert_ne!(roll(7, "claude-1", 2), roll(8, "claude-1", 2));
        assert_ne!(roll(7, "claude-1", 2), roll(7, "claude-2", 2));
        assert_ne!(roll(7, "claude-1", 2), roll(7, "claude-1", 3));
    }

    #[test]
    fn a_file_s_kind_says_its_tests_and_its_note_s_mark() {
        assert_eq!(kind_of("src/A.cs").test, Some("dotnet test"));
        assert_eq!(kind_of("app/B.kt").test, Some("gradle test"));
        for f in ["a.ts", "a.tsx", "a.js"] {
            assert_eq!(kind_of(f).test, Some("npm test"), "{f}");
        }
        assert_eq!(kind_of("x/y.go").test, Some("go test ./..."));
        assert_eq!(kind_of("src/main.rs").test, Some("cargo test"));
        assert_eq!(kind_of("a.txt"), &OTHER);
        assert_eq!(kind_of("Makefile"), &OTHER);
        for f in ["a.cs", "a.kt", "a.ts", "a.tsx", "a.js", "a.go", "a.rs"] {
            assert_eq!(mark_of(f), "// ", "{f}");
        }
        for f in ["a.py", "a.sh", "a.toml", "a.yml", "a.yaml"] {
            assert_eq!(mark_of(f), "# ", "{f}");
        }
        assert_eq!(mark_of("a.md"), "");
        assert_eq!(noted("a.go", "one\ntwo"), "// one\n// two");
        assert_eq!(noted("a.md", "one"), "one");
    }

    #[test]
    fn the_commands_that_ask_are_no_routine_and_the_tests_are() {
        // Each kind's asks are a shell's line or a program the demo does
        // not run (`TEST_PROGRAMS`), so imece asks the person; its tests
        // are one program it runs, or for Python the runner imece reads
        // from the checkout (record 136: `python3 -m unittest`).
        for (_, k) in KINDS.iter().chain([&(&[][..], OTHER)]) {
            for a in k.asks {
                let program = a.split(' ').next().unwrap();
                let shell = crate::run::split(a).is_err();
                let off = !TEST_PROGRAMS.contains(&program) && program != "cargo";
                assert!(
                    shell || off || a.starts_with("cargo add"),
                    "{a} would be routine"
                );
            }
            if let Some(t) = k.test {
                let argv = crate::run::split(t).unwrap();
                assert!(
                    TEST_PROGRAMS.contains(&argv[0].as_str())
                        || argv[0] == "cargo"
                        || t == "python3 -m unittest",
                    "{t}"
                );
            }
        }
    }

    #[test]
    fn a_directory_s_files_are_its_own_sorted_without_hidden_built_or_linked_ones() {
        let dir = std::env::temp_dir().join(format!("imece-stand-in-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for d in ["src/a", ".git", "node_modules/x", "target/debug"] {
            std::fs::create_dir_all(dir.join(d)).unwrap();
        }
        for f in [
            "src/a/B.cs",
            "src/A.cs",
            "README.md",
            ".env",
            ".git/HEAD",
            "node_modules/x/i.js",
            "target/debug/o",
        ] {
            std::fs::write(dir.join(f), "x").unwrap();
        }
        std::os::unix::fs::symlink(dir.join("README.md"), dir.join("link.md")).unwrap();
        std::os::unix::fs::symlink(dir.join("src"), dir.join("linked")).unwrap();
        assert_eq!(files_in(&dir), ["README.md", "src/A.cs", "src/a/B.cs"]);
        assert!(files_in(&dir.join("none")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_gives_at_most_files_max() {
        let dir = std::env::temp_dir().join(format!("imece-stand-in-many-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for n in 0..=FILES_MAX {
            std::fs::write(dir.join(format!("f{n:05}")), "").unwrap();
        }
        let found = files_in(&dir);
        assert_eq!(found.len(), FILES_MAX);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn owned(f: &[&str]) -> Vec<String> {
        strings(f)
    }

    #[test]
    fn the_file_changed_names_the_task_best_and_its_test_names_the_file() {
        let files = owned(&[
            "docs/payments.md",
            "src/i18n/tr.json",
            "src/Orders/Money.cs",
            "src/Orders/OrderService.cs",
            "tests/MoneyTests.cs",
            "tests/OrderServiceTests.cs",
        ]);
        let task = "Round the Money in orders, half up";
        // Money.cs and OrderService.cs both name "orders"; Money.cs names
        // "money" too.
        assert_eq!(
            pick(&files, task, 0),
            Some(("src/Orders/Money.cs", Some("tests/MoneyTests.cs")))
        );
        assert_eq!(
            pick(&files, "Check the order service", 0),
            Some((
                "src/Orders/OrderService.cs",
                Some("tests/OrderServiceTests.cs")
            ))
        );
        // Equals by the roll: "orders" alone, two files name it.
        assert_eq!(pick(&files, "orders", 0).unwrap().0, "src/Orders/Money.cs");
        assert_eq!(
            pick(&files, "orders", 1).unwrap().0,
            "src/Orders/OrderService.cs"
        );
        assert_eq!(pick(&files, "orders", 2).unwrap().0, "src/Orders/Money.cs");
        // No word named: every file but the tests and JSON ties.
        assert_eq!(pick(&files, "go", 0).unwrap().0, "docs/payments.md");
        assert_eq!(
            pick(&files, "go", 2),
            Some((
                "src/Orders/OrderService.cs",
                Some("tests/OrderServiceTests.cs")
            ))
        );
        // A test that names nothing of the file is not its test.
        assert_eq!(
            pick(&files, "payments", 0),
            Some(("docs/payments.md", None))
        );
        // Only tests: a test is changed, and no other test noted.
        let tests = owned(&["tests/money_test.go", "tests/money_tests.cs"]);
        assert_eq!(
            pick(&tests, "money", 0),
            Some(("tests/money_test.go", Some("tests/money_tests.cs")))
        );
        assert_eq!(
            pick(&owned(&["tests/MoneyTests.cs"]), "money", 0),
            Some(("tests/MoneyTests.cs", None))
        );
        // Nothing to change.
        assert_eq!(pick(&owned(&["a.json"]), "a", 0), None);
        assert_eq!(pick(&[], "a", 0), None);
        // Words of three letters at least, folded; a test by any case.
        assert_eq!(words("A bc DEF ğüş-x9z"), ["def", "ğüş", "x9z"]);
        assert!(is_test("a/TestA.cs") && is_test("a_test.go") && !is_test("a/B.cs"));
    }

    #[test]
    fn a_read_is_numbered_and_unnumbered_back_and_an_edit_is_exact() {
        assert_eq!(numbered("a\nb→c\n"), "     1\u{2192}a\n     2\u{2192}b→c");
        assert_eq!(numbered(""), EMPTY);
        assert_eq!(unnumbered(&numbered("a\nb→c\n")), "a\nb→c");
        assert_eq!(unnumbered(EMPTY), "");
        assert_eq!(edited("a b a", "b", "c"), Ok("a c a".into()));
        assert_eq!(
            edited("a b a", "a", "c"),
            Err("Found 2 matches of the string to replace.".into())
        );
        assert_eq!(
            edited("a", "x", "c"),
            Err("String to replace not found in file.".into())
        );
        assert_eq!(edited("a", "", "c"), Err("old_string is empty".into()));
        assert_eq!(ran("dotnet test"), PASSED);
        assert_eq!(ran("npm install zod@3 && npm test"), PASSED);
        assert_eq!(ran("docker compose up -d db"), RAN);
    }

    const ALL: &[&str] = &["Read", "Edit", "Write", "Bash", "TaskCreate", "TaskUpdate"];

    fn worker(files: &[(&str, &str)]) -> Fake {
        let mut f = fake(WORKER);
        f.tools = strings(ALL);
        for (p, t) in files {
            f.files.insert(p.to_string(), t.to_string());
        }
        f
    }

    fn assign(task: &str) -> String {
        format!("{MAIL}:\n\n[assign 3] from lead:\n{task}\n")
    }

    /// The built-in tools used, by name and the file or command named.
    fn used(f: &Fake) -> Vec<(String, String)> {
        (f.used.iter())
            .map(|(t, i)| {
                let what = (i["file_path"].as_str())
                    .or(i["command"].as_str())
                    .or(i["status"].as_str())
                    .or(i["subject"].as_str())
                    .unwrap_or_default();
                (t.clone(), what.to_string())
            })
            .collect()
    }

    fn pairs(p: &[(&str, &str)]) -> Vec<(String, String)> {
        p.iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn a_worker_follows_its_list_changes_the_file_and_its_test_and_runs_the_tests() {
        let mut f = worker(&[
            ("src/Money.cs", "// Money\n"),
            ("tests/MoneyTests.cs", "// MoneyTests\n"),
            ("src/Other.cs", "// Other\n"),
        ]);
        let mut m = Mind::default();
        let said = m.turn(&mut f, &assign("Round money half up\nnot banker's"));
        assert_eq!(said, "Done: changed src/Money.cs, tests/MoneyTests.cs.");
        assert_eq!(m.worked, 1);
        assert_eq!(
            f.files["src/Money.cs"], "// Money\n// Round money half up\n// not banker's\n",
            "the file's last newline stays last"
        );
        assert_eq!(
            f.files["tests/MoneyTests.cs"],
            "// MoneyTests\n// covers: Round money half up\n"
        );
        assert_eq!(f.files["src/Other.cs"], "// Other\n");
        let want = pairs(&[
            ("TaskCreate", STEPS[0]),
            ("TaskCreate", STEPS[1]),
            ("TaskCreate", STEPS[2]),
            ("TaskCreate", STEPS[3]),
            ("TaskCreate", STEPS[4]),
            ("TaskUpdate", "in_progress"),
            ("Read", "src/Money.cs"),
            ("TaskUpdate", "completed"),
            ("TaskUpdate", "in_progress"),
            ("Read", "src/Money.cs"),
            ("Edit", "src/Money.cs"),
            ("TaskUpdate", "completed"),
            ("TaskUpdate", "in_progress"),
            ("Read", "tests/MoneyTests.cs"),
            ("Edit", "tests/MoneyTests.cs"),
            ("TaskUpdate", "completed"),
            ("TaskUpdate", "in_progress"),
            ("Bash", "dotnet test"),
            ("TaskUpdate", "completed"),
            ("TaskUpdate", "in_progress"),
            ("TaskUpdate", "completed"),
        ]);
        assert_eq!(used(&f), want);
        // Each task by its number, in order.
        let ids: Vec<&str> = (f.used.iter())
            .filter(|(t, _)| t == "TaskUpdate")
            .map(|(_, i)| i["taskId"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["1", "1", "2", "2", "3", "3", "4", "4", "5", "5"]);
        assert_eq!(f.used[0].1["activeForm"], STEPS[0]);
        assert_eq!(f.used[0].1["description"], STEPS[0]);
        assert_eq!(f.used[17].1["description"], "Round money half up");
        assert_eq!(f.pauses, 6, "after the list, then a step each");
        let done = &f.calls[1];
        assert_eq!(done.0, "send");
        assert_eq!(
            done.1,
            json!({ "to": "lead", "type": "done", "body":
                "Round money half up\n\nChanged src/Money.cs, tests/MoneyTests.cs.\ndotnet test: All 12 tests passed." })
        );
        // The next assign is the second, its list numbered on.
        f.used.clear();
        m.turn(&mut f, &assign("Again"));
        assert_eq!(m.worked, 2);
        assert_eq!(f.used[5].1["taskId"], "6");
    }

    #[test]
    fn a_worker_asks_first_one_assign_in_asks_and_says_what_it_heard() {
        let mut f = worker(&[("src/money.go", "package money\n")]);
        f.habits.asks = 1;
        let mut m = Mind::default();
        m.turn(&mut f, &assign("Money in cents"));
        let ran: Vec<String> = (used(&f).into_iter())
            .filter(|(t, _)| t == "Bash")
            .map(|(_, c)| c)
            .collect();
        let asked = kind_of("a.go").asks[((roll(0, "claude-1", 1) >> 8) % 2) as usize];
        assert_eq!(ran, [asked, "go test ./..."]);
        let body = f.calls[1].1["body"].as_str().unwrap().to_string();
        assert!(
            body.ends_with(&format!(
                "\n{asked}: Done.\ngo test ./...: All 12 tests passed."
            )),
            "{body}"
        );
        // A no: said, and the tests still run.
        let mut f = worker(&[("src/money.go", "package money\n")]);
        f.habits.asks = 1;
        f.refuse.insert("Bash".into(), "you said no".into());
        Mind::default().turn(&mut f, &assign("Money in cents"));
        let body = f.calls[1].1["body"].as_str().unwrap().to_string();
        assert!(
            body.ends_with(&format!(
                "\n{asked} did not run: you said no\ngo test ./... did not run: you said no"
            )),
            "{body}"
        );
        // The other command of two by the roll's upper bits.
        let other = (0..64u64)
            .map(|seed| (seed, roll(seed, "claude-1", 1)))
            .find(|(_, r)| (r >> 8) % 2 == 1 - ((roll(0, "claude-1", 1) >> 8) % 2))
            .unwrap()
            .0;
        let mut f = worker(&[("src/money.go", "package money\n")]);
        f.habits = Habits {
            asks: 1,
            seed: other,
            ..Habits::default()
        };
        Mind::default().turn(&mut f, &assign("Money in cents"));
        let first = used(&f).into_iter().find(|(t, _)| t == "Bash").unwrap().1;
        assert_ne!(first, asked);
        assert!(kind_of("a.go").asks.contains(&first.as_str()));
    }

    #[test]
    fn one_assign_in_questions_asks_the_person_by_other_bits_never_at_0() {
        let h = |questions| Habits {
            questions,
            ..Habits::default()
        };
        assert!(!h(0).questions_now(0));
        assert!(h(1).questions_now(5));
        assert!(h(3).questions_now(6 << 16) && !h(3).questions_now(7 << 16));
        assert!(!h(3).questions_now(8 << 16));
        assert!(!h(2).questions_now(1 << 16), "the upper bits, not the roll");
        assert!(h(2).questions_now(1), "below them nothing counts");
    }

    #[test]
    fn a_question_names_the_file_and_is_chosen_by_the_roll() {
        let (q, options) = question("src/Money.cs", 6 << 24);
        assert_eq!(
            q,
            "Does the change to src/Money.cs need a line in the release notes?"
        );
        assert_eq!(options, QUESTIONS[2].1);
        assert_eq!(question("a", 3 << 24).0, QUESTIONS[3].0.replace("{}", "a"));
        assert_eq!(question("a", 4 << 24).0, QUESTIONS[0].0.replace("{}", "a"));
        assert!(question("a", 1 << 24).1.is_empty());
        // Every other offers answers, each as a bus takes them.
        for (i, (text, options)) in QUESTIONS.iter().enumerate() {
            assert!(text.contains("{}"), "{text}");
            assert_eq!(options.is_empty(), i % 2 == 1, "{text}");
            assert!(options.len() <= 4 && options.iter().all(|o| o.len() <= 60));
        }
    }

    #[test]
    fn the_person_s_words_are_the_last_paragraph_before_the_mail() {
        assert_eq!(heard("behind a flag").as_deref(), Some("behind a flag"));
        assert_eq!(
            heard(&format!(
                "A note of imece's.\n\n  my words \n\n{MAIL}:\n\n[ask 2] from lead:\nx"
            ))
            .as_deref(),
            Some("my words")
        );
        assert_eq!(heard(&format!("{MAIL}:\n\n[inform 2] from lead:\nx")), None);
        assert_eq!(heard(" \n\n "), None);
        let long = "y".repeat(super::super::TASK_MAX + 1);
        assert_eq!(heard(&long).unwrap().len(), super::super::TASK_MAX);
    }

    /// A seed under which `claude-1`'s `n`th assign asks the question
    /// `want` (by its index).
    fn seed_asking(n: u64, want: usize) -> u64 {
        (0..1_000u64)
            .find(|&s| (roll(s, "claude-1", n) >> 24) % 4 == want as u64)
            .unwrap()
    }

    #[test]
    fn a_worker_asks_the_person_ends_its_turn_and_goes_on_with_the_answer() {
        let mut f = worker(&[
            ("src/Money.cs", "// Money\n"),
            ("tests/MoneyTests.cs", "// MoneyTests\n"),
        ]);
        f.habits = Habits {
            questions: 1,
            seed: seed_asking(1, 0),
            ..Habits::default()
        };
        let mut m = Mind::default();
        let said = m.turn(&mut f, &assign("Round money half up"));
        let q = QUESTIONS[0].0.replace("{}", "src/Money.cs");
        assert_eq!(said, format!("I asked you: {q}"));
        assert_eq!(
            f.calls[1],
            (
                "send".to_string(),
                json!({ "to": "you", "type": "ask", "body": q,
                    "options": ["behind a flag", "change outright"] })
            )
        );
        assert_eq!(f.calls.len(), 2, "nothing else is sent");
        assert_eq!(f.files["src/Money.cs"], "// Money\n", "nothing changed yet");
        assert_eq!(
            m.waiting,
            Some(Waiting {
                task: "Round money half up".into(),
                list: vec![Some(1), Some(2), Some(3), Some(4), Some(5)],
                ..Waiting::default()
            })
        );
        let last = used(&f).pop().unwrap();
        assert_eq!(last, ("TaskUpdate".to_string(), "completed".to_string()));
        assert_eq!(f.pauses, 2, "after the list, then the first step");
        // Mail alone is no answer: it still waits.
        f.used.clear();
        let mail = format!("{MAIL}:\n\n[inform 4] from lead:\nHow is it going?\n");
        assert_eq!(
            m.turn(&mut f, &mail),
            "I still wait for the person's answer."
        );
        assert!(f.used.is_empty() && m.waiting.is_some());
        // The answer: the list goes on from its second task, the work
        // as ever, and the done says what the person said.
        let said = m.turn(&mut f, "behind a flag");
        assert_eq!(said, "Done: changed src/Money.cs, tests/MoneyTests.cs.");
        assert_eq!(m.waiting, None);
        assert_eq!(m.worked, 1, "the same assign");
        assert_eq!(
            f.used[0],
            (
                "TaskUpdate".to_string(),
                json!({ "taskId": "2", "status": "in_progress" })
            )
        );
        assert!(
            used(&f).iter().all(|(t, _)| t != "TaskCreate"),
            "no list again"
        );
        assert_eq!(
            f.files["src/Money.cs"],
            "// Money\n// Round money half up\n",
        );
        let done = f.calls.last().unwrap();
        assert_eq!(
            done.1,
            json!({ "to": "lead", "type": "done", "body":
                "Round money half up\n\nChanged src/Money.cs, tests/MoneyTests.cs.\nThe person said: behind a flag\ndotnet test: All 12 tests passed." })
        );
        // The next assign asks again, and a question without answers
        // offers none.
        f.habits.seed = seed_asking(2, 1);
        f.calls.clear();
        m.turn(&mut f, &assign("Again"));
        assert_eq!(f.calls[0].1["options"], json!([]));
        assert!(m.waiting.is_some());
    }

    #[test]
    fn the_person_s_cues_say_what_a_task_does_whatever_the_habits() {
        assert_eq!(Cues::of("Mend it"), Cues::default());
        assert_eq!(
            Cues::of("Mend it [ask] [question] [helper] [stray] [cards 3]"),
            Cues {
                ask: true,
                question: true,
                helper: true,
                stray: true,
                fault: None,
                cards: Some(3),
            }
        );
        assert_eq!(Cues::of("x [fault]").fault, Some(crate::Fault::Passing));
        assert_eq!(
            Cues::of("x [account] [fault]").fault,
            Some(crate::Fault::Account)
        );
        // Cards are two at least, a number in the brackets.
        assert_eq!(Cues::of("x [cards 1]").cards, None);
        assert_eq!(Cues::of("x [cards two]").cards, None);
        assert_eq!(Cues::of("x [cards 2").cards, None);
        assert_eq!(Cues::of("x [cards 2]").cards, Some(2));
        // A word not in brackets is no cue.
        assert_eq!(Cues::of("ask a question of the helper"), Cues::default());

        let money = &[
            ("src/money.go", "package money\n"),
            ("src/money_test.go", ""),
        ];
        // `[ask]`: a command that asks first, then the tests.
        let mut f = worker(money);
        Mind::default().turn(&mut f, &assign("Money in cents [ask]"));
        let ran: Vec<String> = (used(&f).into_iter())
            .filter(|(t, _)| t == "Bash")
            .map(|(_, c)| c)
            .collect();
        assert_eq!(ran.len(), 2, "{ran:?}");
        assert!(kind_of("a.go").asks.contains(&ran[0].as_str()), "{ran:?}");
        assert_eq!(ran[1], "go test ./...");
        // `[question]`: the person is asked and the worker waits.
        let mut f = worker(money);
        let mut m = Mind::default();
        m.turn(&mut f, &assign("Money in cents [question]"));
        assert_eq!(f.calls[1].1["to"], "you");
        assert!(m.waiting.is_some());
        // `[helper]`: a helper is hired for the tests.
        let mut f = worker(money);
        f.answers.insert("hire".into(), Ok(HIRED.into()));
        let said = Mind::default().turn(&mut f, &assign("Money in cents [helper]"));
        assert!(
            said.contains("writes the tests in src/money_test.go"),
            "{said}"
        );
        // None of it without the cue.
        let mut f = worker(money);
        Mind::default().turn(&mut f, &assign("Money in cents"));
        assert!(f.calls.iter().all(|(c, _)| c != "hire"));
        assert_eq!(f.calls[1].1["type"], "done");
        // A Python file's tests are `python3 -m unittest`.
        let mut f = worker(&[("shelf.py", "x = 1\n"), ("test_shelf.py", "")]);
        Mind::default().turn(&mut f, &assign("Mend shelf.py"));
        assert!(used(&f).contains(&("Bash".into(), "python3 -m unittest".into())));
    }

    #[test]
    fn a_question_that_can_not_be_sent_is_not_waited_for() {
        let mut f = worker(&[("src/money.go", "package money\n")]);
        f.habits.questions = 1;
        f.answers
            .insert("send".into(), Err("the person is away".into()));
        let mut m = Mind::default();
        let said = m.turn(&mut f, &assign("Money in cents"));
        assert!(
            said.starts_with("Changed src/money.go, but lead did not hear"),
            "{said}"
        );
        assert_eq!(m.waiting, None);
        assert_eq!(f.calls[1].1["to"], "you");
        assert_eq!(f.calls[2].1["type"], "done");
    }

    #[test]
    fn without_a_shell_or_a_list_a_worker_still_changes_and_says_done() {
        let mut f = worker(&[("notes.md", "")]);
        f.tools = strings(&["Read", "Edit", "Write"]);
        f.habits.asks = 1;
        let said = Mind::default().turn(&mut f, &assign("Write it down"));
        assert_eq!(said, "Done: changed notes.md.");
        assert_eq!(
            f.files["notes.md"], "Write it down\n",
            "an empty file is written"
        );
        assert_eq!(
            used(&f),
            pairs(&[
                ("Read", "notes.md"),
                ("Read", "notes.md"),
                ("Write", "notes.md")
            ])
        );
        assert_eq!(f.calls[1].1["body"], "Write it down\n\nChanged notes.md.");
        // A file of no kind runs no tests, only what asks.
        let mut f = worker(&[("a.txt", "a")]);
        f.habits.asks = 1;
        Mind::default().turn(&mut f, &assign("x"));
        let bash: Vec<_> = used(&f).into_iter().filter(|(t, _)| t == "Bash").collect();
        assert_eq!(bash, pairs(&[("Bash", OTHER.asks[0])]));
        let mut f = worker(&[("a.txt", "a")]);
        Mind::default().turn(&mut f, &assign("x"));
        assert!(!used(&f).iter().any(|(t, _)| t == "Bash"), "no ask at 0");
    }

    #[test]
    fn a_worker_says_what_failed() {
        let mut f = worker(&[("lib.rs", "fn a() {}\n"), ("lib_test.rs", "")]);
        f.refuse.insert("Read".into(), "gone".into());
        assert_eq!(
            Mind::default().turn(&mut f, &assign("x")),
            "I could not read lib.rs: gone"
        );
        let mut f = worker(&[("lib.rs", "fn a() {}\n"), ("lib_test.rs", "")]);
        f.refuse.insert("Edit".into(), "locked".into());
        assert_eq!(
            Mind::default().turn(&mut f, &assign("x")),
            "I could not change lib.rs: locked"
        );
        // Its test file not changed: said, the rest goes on.
        let mut f = worker(&[("lib.rs", "fn a() {}\n"), ("lib_test.rs", "")]);
        f.refuse.insert("Write".into(), "full".into());
        assert_eq!(
            Mind::default().turn(&mut f, &assign("x")),
            "Done: changed lib.rs."
        );
        assert_eq!(
            f.calls[1].1["body"],
            "x\n\nChanged lib.rs.\nlib_test.rs not changed: full\ncargo test: All 12 tests passed."
        );
        let mut f = worker(&[("lib.rs", "fn a() {}\n")]);
        f.answers.insert("send".into(), Err("gone".into()));
        assert_eq!(
            Mind::default().turn(&mut f, &assign("x")),
            "Changed lib.rs, but lead did not hear: gone"
        );
        // A list whose tasks were not made is not marked.
        let mut f = worker(&[("lib.rs", "fn a() {}\n")]);
        f.refuse.insert("TaskCreate".into(), "no".into());
        Mind::default().turn(&mut f, &assign("x"));
        assert!(!used(&f).iter().any(|(t, _)| t == "TaskUpdate"));
    }
    #[test]
    fn one_assign_in_helpers_hires_a_helper_by_other_bits_never_at_0() {
        let h = |helpers| Habits {
            helpers,
            ..Habits::default()
        };
        assert!(!h(0).helps_now(0));
        assert!(h(1).helps_now(5));
        assert!(h(3).helps_now(6 << 32) && !h(3).helps_now(7 << 32));
        assert!(!h(3).helps_now(8 << 32));
        assert!(!h(2).helps_now(1 << 32), "the upper bits, not the roll");
        assert!(h(2).helps_now((1 << 32) - 1), "below them nothing counts");
    }

    #[test]
    fn a_helper_s_part_is_read_back_and_its_done_and_its_loss_are_heard() {
        let part = helper_task("tests/T.cs", "src/A.cs", "Round up\nhalf");
        assert_eq!(
            part,
            "Write the tests in tests/T.cs for the change to src/A.cs:\n\nRound up\nhalf"
        );
        assert_eq!(
            part_of(&part),
            Some(("tests/T.cs", "src/A.cs", "Round up\nhalf"))
        );
        assert_eq!(
            part_of("Write the tests in t for the change to a:"),
            Some(("t", "a", "")),
            "no task"
        );
        assert_eq!(part_of("Write the tests in t for the change to a"), None);
        assert_eq!(part_of("Write tests in t for the change to a:"), None);
        assert_eq!(part_of("Write the tests in t for a:"), None);
        assert_eq!(part_of("Round up"), None);
        let mail = format!(
            "{MAIL}:\n\n[done 5] from claude-1.10:\nnot it\n\n[inform 6] from claude-1.1:\nhi\n\n\
             [done 7, re 6] from claude-1.1:\n  Wrote it.\nAll passed.\n\n[done 8] from claude-1.1:\nlater"
        );
        assert_eq!(
            done_from(&mail, "claude-1.1").as_deref(),
            Some("Wrote it.\nAll passed.")
        );
        assert_eq!(done_from(&mail, "claude-1.10").as_deref(), Some("not it"));
        assert_eq!(done_from(&mail, "claude-2"), None);
        assert_eq!(
            done_from("[done] from h:\nx", "h").as_deref(),
            Some("x"),
            "a heading without a number"
        );
        assert_eq!(done_from("[doner 3] from h:\nx", "h"), None);
        assert_eq!(done_from("[assign 3] from h:\nx", "h"), None);
        let note = "imece: Your helper claude-1.1 stopped before it sent you done (gone).";
        assert!(lost(note, "claude-1.1"));
        assert!(!lost(note, "claude-1.2"));
        assert!(!lost("[done 3] from claude-1.1:\nx", "claude-1.1"));
    }

    const HIRED: &str = "claude-1.1 joins the team (cheap, haiku)";

    /// A worker that hires a helper at every assign.
    fn helped(files: &[(&str, &str)]) -> Fake {
        let mut f = worker(files);
        f.habits.helpers = 1;
        f.answers.insert("hire".into(), Ok(HIRED.into()));
        f
    }

    const MONEY: &[(&str, &str)] = &[
        ("src/Money.cs", "// Money\n"),
        ("tests/MoneyTests.cs", "// MoneyTests\n"),
    ];

    fn done_mail(from: &str, body: &str) -> String {
        format!("{MAIL}:\n\n[done 7, re 6] from {from}:\n{body}\n")
    }

    #[test]
    fn a_worker_hires_a_helper_for_its_tests_waits_and_goes_on_with_its_done() {
        let mut f = helped(MONEY);
        let mut m = Mind::default();
        let said = m.turn(&mut f, &assign("Round money half up"));
        assert_eq!(
            said,
            "claude-1.1 writes the tests in tests/MoneyTests.cs; I wait for its done."
        );
        assert_eq!(f.calls[1], ("hire".to_string(), json!({ "tier": "cheap" })));
        assert_eq!(
            f.calls[2],
            (
                "send".to_string(),
                json!({ "to": "claude-1.1", "type": "assign", "body":
                    helper_task("tests/MoneyTests.cs", "src/Money.cs", "Round money half up") })
            )
        );
        assert_eq!(f.calls.len(), 3, "no done yet");
        assert_eq!(
            f.files["src/Money.cs"],
            "// Money\n// Round money half up\n"
        );
        assert_eq!(
            f.files["tests/MoneyTests.cs"], "// MoneyTests\n",
            "the helper's"
        );
        assert_eq!(
            m.waiting,
            Some(Waiting {
                task: "Round money half up".into(),
                list: vec![Some(1), Some(2), Some(3), Some(4), Some(5)],
                helper: Some("claude-1.1".into()),
                said: None,
            })
        );
        assert_eq!(
            used(&f).pop().unwrap(),
            ("TaskUpdate".to_string(), "in_progress".to_string()),
            "its tests' step goes on"
        );
        // Other mail, another's done, or the person's words: it waits on.
        f.used.clear();
        for prompt in [
            done_mail("claude-2", "Wrote the tests in x."),
            "Hurry up".to_string(),
        ] {
            assert_eq!(m.turn(&mut f, &prompt), "I still wait for claude-1.1.");
        }
        assert!(f.used.is_empty() && m.waiting.is_some());
        // Its done: the tests are the helper's, run, and said.
        let said = m.turn(
            &mut f,
            &done_mail("claude-1.1", "Wrote the tests in tests/MoneyTests.cs.\nok"),
        );
        assert_eq!(said, "Done: changed src/Money.cs, tests/MoneyTests.cs.");
        assert_eq!(m.waiting, None);
        assert_eq!(m.worked, 1);
        assert_eq!(f.files["tests/MoneyTests.cs"], "// MoneyTests\n");
        assert_eq!(
            used(&f),
            pairs(&[
                ("TaskUpdate", "completed"),
                ("TaskUpdate", "in_progress"),
                ("Bash", "dotnet test"),
                ("TaskUpdate", "completed"),
                ("TaskUpdate", "in_progress"),
                ("TaskUpdate", "completed"),
            ])
        );
        assert_eq!(f.used[0].1["taskId"], "3");
        assert_eq!(
            f.calls.last().unwrap().1,
            json!({ "to": "lead", "type": "done", "body":
                "Round money half up\n\nChanged src/Money.cs, tests/MoneyTests.cs.\n\
                 claude-1.1: Wrote the tests in tests/MoneyTests.cs.\n\
                 dotnet test: All 12 tests passed." })
        );
    }

    #[test]
    fn a_helper_lost_or_without_tests_leaves_them_to_its_worker() {
        for (prompt, why) in [
            (
                "imece's notes:\n- Your helper claude-1.1 stopped before it sent you done (x).\n\n\
                 Go on with your task."
                    .to_string(),
                "it stopped",
            ),
            (
                done_mail("claude-1.1", "I could not write the tests in t: full\nmore"),
                "I could not write the tests in t: full",
            ),
        ] {
            let mut f = helped(MONEY);
            let mut m = Mind::default();
            m.turn(&mut f, &assign("Round money half up"));
            let said = m.turn(&mut f, &prompt);
            assert_eq!(said, "Done: changed src/Money.cs, tests/MoneyTests.cs.");
            assert_eq!(
                f.files["tests/MoneyTests.cs"],
                "// MoneyTests\n// covers: Round money half up\n"
            );
            assert_eq!(
                f.calls.last().unwrap().1["body"],
                format!(
                    "Round money half up\n\nChanged src/Money.cs, tests/MoneyTests.cs.\n\
                     claude-1.1 did not write the tests ({why}); I did.\n\
                     dotnet test: All 12 tests passed."
                )
            );
        }
        // Its own tests failing too: said.
        let mut f = helped(MONEY);
        let mut m = Mind::default();
        m.turn(&mut f, &assign("Round money half up"));
        f.refuse.insert("Edit".into(), "locked".into());
        m.turn(&mut f, &done_mail("claude-1.1", "no"));
        let body = f.calls.last().unwrap().1["body"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(
            body.starts_with("Round money half up\n\nChanged src/Money.cs.\n")
                && body.contains("\ntests/MoneyTests.cs not changed: locked\n"),
            "{body}"
        );
        // Its files gone meanwhile: nothing to finish.
        let mut f = helped(MONEY);
        let mut m = Mind::default();
        m.turn(&mut f, &assign("Round money half up"));
        f.files.clear();
        assert_eq!(
            m.turn(&mut f, &done_mail("claude-1.1", "x")),
            "I can no longer find what claude-1.1 tested."
        );
    }

    #[test]
    fn what_the_person_said_waits_with_the_helper_and_goes_into_the_done() {
        let mut f = helped(MONEY);
        f.habits.questions = 1;
        let mut m = Mind::default();
        m.turn(&mut f, &assign("Round money half up"));
        assert!(
            m.turn(&mut f, "behind a flag")
                .starts_with("claude-1.1 writes")
        );
        assert_eq!(
            m.waiting.as_ref().unwrap().said.as_deref(),
            Some("behind a flag")
        );
        m.turn(
            &mut f,
            &done_mail("claude-1.1", "Wrote the tests in tests/MoneyTests.cs."),
        );
        let body = f.calls.last().unwrap().1["body"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(
            body.contains(
                "\nThe person said: behind a flag\nclaude-1.1: Wrote the tests in tests/MoneyTests.cs.\n"
            ),
            "{body}"
        );
    }

    #[test]
    fn no_helper_without_a_test_file_a_hire_or_its_assign() {
        // No test file: nothing for a helper.
        let mut f = helped(&[("src/Money.cs", "// Money\n")]);
        assert_eq!(
            Mind::default().turn(&mut f, &assign("Round money")),
            "Done: changed src/Money.cs."
        );
        assert!(f.calls.iter().all(|(t, _)| t != "hire"));
        // A hire refused, or one that names nobody: the worker's own tests.
        for hired in [Err("the team is full".to_string()), Ok(String::new())] {
            let mut f = helped(MONEY);
            f.answers.insert("hire".into(), hired);
            let mut m = Mind::default();
            let said = m.turn(&mut f, &assign("Round money half up"));
            assert_eq!(said, "Done: changed src/Money.cs, tests/MoneyTests.cs.");
            assert_eq!(m.waiting, None);
            assert_eq!(f.calls.len(), 3, "whoami, hire, done");
        }
        // Its assign not sent: not waited for.
        let mut f = helped(MONEY);
        f.answers.insert("send".into(), Err("gone".into()));
        let mut m = Mind::default();
        let said = m.turn(&mut f, &assign("Round money half up"));
        assert!(
            said.starts_with("Changed src/Money.cs, tests/MoneyTests.cs, but"),
            "{said}"
        );
        assert_eq!(m.waiting, None);
        // Never at 0.
        let mut f = helped(MONEY);
        f.habits.helpers = 0;
        Mind::default().turn(&mut f, &assign("Round money half up"));
        assert!(f.calls.iter().all(|(t, _)| t != "hire"));
    }

    const HELPER: &str = "name: claude-1.1\nrole: a helper\nproject: repo\n\
                          you answer to: claude-1, who hired you\n";

    fn helper(files: &[(&str, &str)]) -> Fake {
        let mut f = fake(HELPER);
        f.tools = strings(ALL);
        for (p, t) in files {
            f.files.insert(p.to_string(), t.to_string());
        }
        f
    }

    fn part(body: &str) -> String {
        format!("{MAIL}:\n\n[assign 6] from claude-1:\n{body}\n")
    }

    #[test]
    fn a_helper_writes_and_runs_the_tests_and_tells_its_worker() {
        let task = helper_task(
            "tests/MoneyTests.cs",
            "src/Money.cs",
            "Round money\nhalf up",
        );
        let mut f = helper(MONEY);
        let mut m = Mind::default();
        let said = m.turn(&mut f, &part(&task));
        assert_eq!(said, "Wrote the tests in tests/MoneyTests.cs.");
        assert_eq!(m.worked, 1);
        assert_eq!(
            f.files["tests/MoneyTests.cs"],
            "// MoneyTests\n// covers: Round money\n"
        );
        assert_eq!(f.files["src/Money.cs"], "// Money\n");
        assert_eq!(
            used(&f),
            pairs(&[
                ("Read", "tests/MoneyTests.cs"),
                ("Edit", "tests/MoneyTests.cs"),
                ("Bash", "dotnet test"),
            ])
        );
        assert_eq!(f.used[2].1["description"], "Round money");
        assert_eq!(f.pauses, 2);
        assert_eq!(
            f.calls[1],
            (
                "send".to_string(),
                json!({ "to": "claude-1", "type": "done", "body":
                    "Wrote the tests in tests/MoneyTests.cs.\ndotnet test: All 12 tests passed." })
            )
        );
        // Without a shell, nothing runs.
        let mut f = helper(MONEY);
        f.tools = strings(&["Read", "Edit", "Write"]);
        Mind::default().turn(&mut f, &part(&task));
        assert_eq!(
            f.calls[1].1["body"],
            "Wrote the tests in tests/MoneyTests.cs."
        );
        // Its edit refused, a part it can not read, no assign, no worker
        // that hears: said.
        let mut f = helper(MONEY);
        f.refuse.insert("Edit".into(), "locked".into());
        assert_eq!(
            Mind::default().turn(&mut f, &part(&task)),
            "I could not write the tests in tests/MoneyTests.cs: locked"
        );
        assert_eq!(
            f.calls[1].1["body"],
            "I could not write the tests in tests/MoneyTests.cs: locked"
        );
        assert!(!used(&f).iter().any(|(t, _)| t == "Bash"));
        let mut f = helper(MONEY);
        assert_eq!(
            Mind::default().turn(&mut f, &part("Write some tests")),
            "I could not tell which tests to write."
        );
        assert_eq!(f.calls[1].1["to"], "claude-1");
        assert!(f.used.is_empty());
        let mut f = helper(MONEY);
        let mail = format!("{MAIL}:\n\n[inform 6] from claude-1:\nhi\n");
        assert_eq!(
            Mind::default().turn(&mut f, &mail),
            "Nothing assigned to me."
        );
        assert_eq!(f.calls.len(), 1);
        let mut f = helper(MONEY);
        f.answers.insert("send".into(), Err("gone".into()));
        assert_eq!(
            Mind::default().turn(&mut f, &part(&task)),
            "Wrote the tests in tests/MoneyTests.cs, but claude-1 did not hear: gone"
        );
    }

    const OWNING: &str = "src/alpha.rs: Round up\nwell\n\nThis card owns src/alpha.rs, src/alpha_test.rs \
        (shared): stay in them; what else needs changing goes in your done, or ask for the \
        card to be widened.\n\nThis card asks for Lonca's checks api: once";

    #[test]
    fn a_task_says_what_its_card_owns_and_is_plain_without_what_imece_added() {
        assert_eq!(owned_of(OWNING), ["src/alpha.rs", "src/alpha_test.rs"]);
        assert_eq!(plain(OWNING), "src/alpha.rs: Round up\nwell");
        assert_eq!(owned_of("Round up"), [""; 0]);
        assert_eq!(plain("Round up"), "Round up");
        // Half of imece's words is not its words; the checks alone end
        // the task too.
        assert_eq!(owned_of("x\n\nThis card owns a.rs, b.rs"), [""; 0]);
        assert_eq!(owned_of("x This card owns a.rs: stay in them"), [""; 0]);
        assert_eq!(plain("x\n\nThis card asks for Lonca's checks api"), "x");
        assert_eq!(
            plain("x\n\nThis card asks for y\n\nThis card owns a: stay in them"),
            "x",
            "whichever comes first"
        );
    }

    #[test]
    fn a_card_s_own_files_are_chosen_before_what_the_task_names() {
        let files = owned(&[
            "src/alpha.rs",
            "src/alpha_test.rs",
            "src/beta.rs",
            "src/beta_test.rs",
            "b.json",
        ]);
        // The task names b; the card owns a and its test.
        let task = "beta beta_test\n\nThis card owns src/alpha_test.rs, src/alpha.rs: stay in them";
        assert_eq!(
            chosen(&files, task, 0),
            Some(("src/alpha.rs", Some("src/alpha_test.rs")))
        );
        // Code alone: no test; a test alone, a file that is not there or
        // one no note can go in: what the task names.
        let task = "beta\n\nThis card owns src/alpha.rs: stay in them";
        assert_eq!(chosen(&files, task, 0), Some(("src/alpha.rs", None)));
        for own in ["src/alpha_test.rs", "src/gone.rs", "b.json"] {
            let task = format!("beta.rs\n\nThis card owns {own}: stay in them");
            assert_eq!(
                chosen(&files, &task, 0),
                Some(("src/beta.rs", Some("src/beta_test.rs"))),
                "{own}"
            );
        }
        assert_eq!(chosen(&files, "beta.rs", 0), pick(&files, "beta.rs", 0));
    }

    #[test]
    fn the_code_for_a_task_is_what_it_names_best_first_and_a_test_names_its_file() {
        let files = owned(&[
            "a.json",
            "docs/gamma.md",
            "src/alpha.rs",
            "src/beta.rs",
            "src/gamma.rs",
            "tests/beta_test.rs",
        ]);
        assert_eq!(
            code_for(&files, "the gamma and beta of src/gamma"),
            ["src/gamma.rs", "src/beta.rs", "src/alpha.rs"],
            "gamma twice and src, beta and src, src; no test, no JSON, no document"
        );
        assert_eq!(test_for(&files, "src/beta.rs"), Some("tests/beta_test.rs"));
        assert_eq!(test_for(&files, "src/alpha.rs"), None);
        assert_eq!(test_for(&files, "tests/beta_test.rs"), None, "not itself");
        let json = owned(&["money.rs", "money_test.json"]);
        assert_eq!(test_for(&json, "money.rs"), None, "no note goes in JSON");
    }

    #[test]
    fn a_stray_goes_into_code_the_card_does_not_own_by_the_roll_s_top_bits() {
        let files = owned(&["a.rs", "a_test.rs", "b.rs", "c.json", "d.rs", "e.rs"]);
        // b, d and e are outside: the 60th bit on counts.
        assert_eq!(outside(&files, &["a.rs"], 0), Some("b.rs"));
        assert_eq!(outside(&files, &["a.rs"], 1 << 60), Some("d.rs"));
        assert_eq!(outside(&files, &["a.rs"], 2 << 60), Some("e.rs"));
        assert_eq!(outside(&files, &["a.rs"], 3 << 60), Some("b.rs"));
        assert_eq!(outside(&files, &["a.rs"], (1 << 60) - 1), Some("b.rs"));
        assert_eq!(outside(&files, &["a.rs", "b.rs", "d.rs", "e.rs"], 0), None);
        assert_eq!(outside(&files, &[], 0), None, "a card that owns nothing");
        let h = |strays| Habits {
            strays,
            ..Habits::default()
        };
        assert!(!h(0).strays_now(0), "never at 0");
        assert!(h(1).strays_now(u64::MAX));
        assert!(h(3).strays_now(3 << 56));
        assert!(
            h(3).strays_now((3 << 56) + 1),
            "the bits under the 56th do not count"
        );
        assert!(!h(3).strays_now(4 << 56));
        assert!(!h(3).strays_now(3 << 55));
    }

    #[test]
    fn an_offer_names_its_first_card_and_a_claim_s_answer_holds_the_task() {
        let offer = "From imece (not from the person or a teammate):\n- #9 x\n\nOpen cards on \
            imece's board (posted by your teammates, not by the person):\n\n#12 b.rs: Round \
            (posted by lead)\n#13 c.rs: Round (posted by lead)\n\nIf one fits you, take it";
        assert_eq!(offered(offer), Some(12));
        assert_eq!(offered("#12 b.rs (posted by lead)"), None, "no offer");
        assert_eq!(offered(&format!("{BOARD}:\n\nIf one fits")), None);
        assert_eq!(offered(&format!("{BOARD}:\n\n#x y")), None);
        let said = "#12 b.rs: Round is yours. lead posted it (written by that agent, not by \
            the person):\n\nb.rs: Round\n\nWhen it is done, send it\n\nWhen it is done, send \
            lead a done message.\n\nThis card owns b.rs: stay in them; what";
        assert_eq!(
            claimed(said).as_deref(),
            Some(
                "b.rs: Round\n\nWhen it is done, send it\n\nThis card owns b.rs: stay in \
                 them; what"
            )
        );
        let bare = "#1 t is yours. lead posted it (x):\n\nbody\n\nWhen it is done, send lead \
            a done message.";
        assert_eq!(claimed(bare).as_deref(), Some("body"));
        assert_eq!(claimed("#1 t is yours."), None);
        assert_eq!(claimed("it (x):\n\nbody"), None);
        assert_eq!(
            claimed("it (x):\n\nbody\n\nWhen it is done, send lead"),
            None
        );
    }

    #[test]
    fn a_worker_stays_in_what_its_card_owns_and_says_when_it_strays() {
        let files = [
            ("src/alpha.rs", "// a\n"),
            ("src/alpha_test.rs", "// a test\n"),
            ("src/beta.rs", "// b\n"),
            ("src/beta_test.rs", "// b test\n"),
        ];
        // The task names b; the card owns a.
        let task = "Mend beta.rs\n\nThis card owns src/alpha.rs, src/alpha_test.rs: stay in them; what \
            else";
        let mut f = worker(&files);
        let said = Mind::default().turn(&mut f, &assign(task));
        assert_eq!(said, "Done: changed src/alpha.rs, src/alpha_test.rs.");
        assert_eq!(
            f.files["src/alpha.rs"], "// a\n// Mend beta.rs\n",
            "imece's words stay out"
        );
        assert_eq!(f.files["src/beta.rs"], "// b\n");
        assert_eq!(
            f.calls[1].1["body"],
            "Mend beta.rs\n\nChanged src/alpha.rs, src/alpha_test.rs.\ncargo test: All 12 tests passed."
        );
        // Every assign strays: one file more, outside, before the tests
        // run, and the done says so.
        let mut f = worker(&files);
        f.habits.strays = 1;
        let said = Mind::default().turn(&mut f, &assign(task));
        assert_eq!(
            said,
            "Done: changed src/alpha.rs, src/alpha_test.rs, src/beta.rs."
        );
        assert_eq!(f.files["src/beta.rs"], "// b\n// Mend beta.rs\n");
        assert_eq!(f.files["src/beta_test.rs"], "// b test\n");
        assert_eq!(
            f.calls[1].1["body"],
            "Mend beta.rs\n\nChanged src/alpha.rs, src/alpha_test.rs, src/beta.rs.\nAlso changed src/beta.rs, \
             which the card does not own.\ncargo test: All 12 tests passed."
        );
        let writes: Vec<(String, String)> = (used(&f).into_iter())
            .filter(|(t, _)| t == "Edit" || t == "Bash")
            .collect();
        assert_eq!(
            writes,
            pairs(&[
                ("Edit", "src/alpha.rs"),
                ("Edit", "src/alpha_test.rs"),
                ("Edit", "src/beta.rs"),
                ("Bash", "cargo test"),
            ])
        );
        // `[stray]` does it with no habit.
        let mut f = worker(&files);
        let cued = task.replacen("Mend beta.rs", "Mend beta.rs [stray]", 1);
        let said = Mind::default().turn(&mut f, &assign(&cued));
        assert_eq!(
            said,
            "Done: changed src/alpha.rs, src/alpha_test.rs, src/beta.rs."
        );
        // A card that owns nothing has no outside; an edit refused is
        // not said.
        let mut f = worker(&files);
        f.habits.strays = 1;
        let said = Mind::default().turn(&mut f, &assign("Mend beta.rs"));
        assert_eq!(said, "Done: changed src/beta.rs, src/beta_test.rs.");
        assert_eq!(f.files["src/alpha.rs"], "// a\n");
    }

    #[test]
    fn a_worker_offered_the_board_takes_the_first_card_and_does_it() {
        let offer = format!("{BOARD} (posted by your teammates):\n\n#7 b.rs (posted by lead)\n");
        let mut f = worker(&[("src/alpha.rs", "// a\n"), ("src/beta.rs", "// b\n")]);
        f.answers.insert(
            "claim".into(),
            Ok(
                "#7 b.rs is yours. lead posted it (x):\n\nMend it\n\nWhen it is done, send \
                lead a done message.\n\nThis card owns src/beta.rs: stay in them; what"
                    .into(),
            ),
        );
        let mut m = Mind::default();
        assert_eq!(m.turn(&mut f, &offer), "Done: changed src/beta.rs.");
        assert_eq!(m.worked, 1);
        assert_eq!(f.calls[1], ("claim".into(), json!({ "card": 7 })));
        assert_eq!(f.files["src/beta.rs"], "// b\n// Mend it\n");
        assert_eq!(f.calls[2].1["to"], "lead");
        // An assign in the same prompt comes first; a claim refused or
        // an answer it can not read: said, nothing done.
        let both = format!("{}\n{offer}", assign("Mend a.rs"));
        let mut f = worker(&[("src/alpha.rs", "// a\n")]);
        assert_eq!(
            Mind::default().turn(&mut f, &both),
            "Done: changed src/alpha.rs."
        );
        assert!(f.calls.iter().all(|(t, _)| t != "claim"));
        let mut f = worker(&[("src/alpha.rs", "// a\n")]);
        f.answers.insert("claim".into(), Err("#7 was taken".into()));
        let mut m = Mind::default();
        assert_eq!(m.turn(&mut f, &offer), "I could not take #7: #7 was taken");
        assert_eq!((m.worked, f.used.len()), (0, 0));
        let mut f = worker(&[("src/alpha.rs", "// a\n")]);
        f.answers.insert("claim".into(), Ok("yours".into()));
        assert_eq!(
            Mind::default().turn(&mut f, &offer),
            "I took #7, but could not read its task."
        );
        assert!(f.used.is_empty());
    }
}
