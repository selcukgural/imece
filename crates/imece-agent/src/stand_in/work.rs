//! What a stand-in worker does with an `assign` (record 130): it keeps a
//! to-do list with Claude Code's task tools, reads the file of its
//! directory the task names best and adds the task to it, notes it in the
//! test file next to it, runs the tests of the project's kind, sometimes
//! first a command imece holds no routine (so the person is asked through
//! the permission prompt), and tells its leader `done` with what it did.
//! Sometimes it asks the person a question once it has read the code, ends
//! its turn, and goes on with the person's answer in the next.
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

/// How a stand-in works: the pause between its steps, how often a worker
/// runs a command that needs the person's yes first (one assign in
/// `asks`, never at 0), how often it asks the person a question (one
/// assign in `questions`, never at 0), and the seed of its choices.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Habits {
    pub pause: Duration,
    pub asks: u64,
    pub questions: u64,
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
            seed: n(SEED_VAR),
        }
    }

    /// The variables that give these habits, for a shell's command line.
    pub fn vars(&self) -> String {
        format!(
            "{PAUSE_VAR}={} {ASKS_VAR}={} {QUESTIONS_VAR}={} {SEED_VAR}={}",
            self.pause.as_millis(),
            self.asks,
            self.questions,
            self.seed
        )
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
}

/// The question the assign `roll` stands for asks about file `path`, and
/// the answers it offers.
pub fn question(path: &str, roll: u64) -> (String, &'static [&'static str]) {
    let (text, options) = QUESTIONS[((roll >> 24) % QUESTIONS.len() as u64) as usize];
    (text.replace("{}", path), options)
}

/// A worker's task while it waits for the person's answer: the task, and
/// its to-do list's tasks by step.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Waiting {
    pub task: String,
    pub list: Vec<Option<u64>>,
}

impl Waiting {
    pub fn to_json(&self) -> serde_json::Value {
        json!({ "task": self.task, "list": self.list })
    }

    /// What a transcript kept; none without a task.
    pub fn from_json(v: &serde_json::Value) -> Option<Waiting> {
        Some(Waiting {
            task: v["task"].as_str()?.to_string(),
            list: (v["list"].as_array().into_iter().flatten())
                .map(serde_json::Value::as_u64)
                .collect(),
        })
    }
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
    let stem = Path::new(main)
        .file_stem()
        .map(|s| words(&s.to_string_lossy()))
        .unwrap_or_default();
    let tests: Vec<&str> = (can.iter().copied())
        .filter(|f| is_test(f) && *f != main)
        .collect();
    let most = tests
        .iter()
        .map(|f| score(f, &stem))
        .max()
        .filter(|&m| m > 0);
    let test = most.and_then(|m| tests.into_iter().find(|f| score(f, &stem) == m));
    Some((main, test))
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
    /// with the person's words.
    pub(super) fn work(&mut self, hands: &mut impl Hands, prompt: &str) -> String {
        if let Some(waiting) = self.waiting.take() {
            let Some(said) = heard(prompt) else {
                self.waiting = Some(waiting);
                return "I still wait for the person's answer.".into();
            };
            return self.go_on(hands, &waiting.task, List(waiting.list), Some(&said));
        }
        let Some(task) = body_of(prompt, "assign") else {
            return "Nothing assigned to me.".into();
        };
        self.worked += 1;
        let me = self.me.get("name").cloned().unwrap_or_default();
        let lead = self.lead_of();
        let habits = *hands.habits();
        let roll = roll(habits.seed, &me, self.worked);
        let files = hands.files();
        let Some((main, _)) = pick(&files, &task, roll) else {
            return write_up(hands, &me, &lead, &task);
        };
        let list = List::made(hands);
        hands.pause();
        list.begin(hands, 0);
        if let Err(why) = hands.tool("Read", json!({ "file_path": main })) {
            return format!("I could not read {main}: {why}");
        }
        list.mark(hands, 0, "completed");
        if habits.questions_now(roll) {
            let (q, options) = question(main, roll);
            let ask = json!({ "to": "you", "type": "ask", "body": q, "options": options });
            if hands.call("send", ask).is_ok() {
                self.waiting = Some(Waiting { task, list: list.0 });
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

    /// The task after its code was read: the change, its tests, a run of
    /// them and `done`, with what the person said when it asked them.
    fn go_on(
        &mut self,
        hands: &mut impl Hands,
        task: &str,
        list: List,
        said: Option<&str>,
    ) -> String {
        let me = self.me.get("name").cloned().unwrap_or_default();
        let lead = self.lead_of();
        let habits = *hands.habits();
        let roll = roll(habits.seed, &me, self.worked);
        let files = hands.files();
        let Some((main, test)) = pick(&files, task, roll) else {
            return write_up(hands, &me, &lead, task);
        };
        list.begin(hands, 1);
        if let Err(why) = append(hands, main, task) {
            return format!("I could not change {main}: {why}");
        }
        list.mark(hands, 1, "completed");
        list.begin(hands, 2);
        let mut changed = vec![main.to_string()];
        let mut notes: Vec<String> = said
            .map(|s| format!("The person said: {s}"))
            .into_iter()
            .collect();
        if let Some(t) = test {
            match append(hands, t, &format!("covers: {}", first_line(task))) {
                Ok(()) => changed.push(t.to_string()),
                Err(why) => notes.push(format!("{t} not changed: {why}")),
            }
        }
        list.mark(hands, 2, "completed");
        list.begin(hands, 3);
        if hands.has(crate::claude::SHELL_TOOL) {
            let kind = kind_of(main);
            let asked = (habits.asks_now(roll) && !kind.asks.is_empty())
                .then(|| kind.asks[((roll >> 8) % kind.asks.len() as u64) as usize]);
            for command in asked.into_iter().chain(kind.test) {
                let input = json!({ "command": command, "description": first_line(task) });
                match hands.tool(crate::claude::SHELL_TOOL, input) {
                    Ok(out) => notes.push(format!("{command}: {}", first_line(&out))),
                    Err(why) => notes.push(format!("{command} did not run: {why}")),
                }
            }
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
}

/// A worker's turn in a directory with nothing to change: the task in a
/// file of its own, [`OUT_DIR`]`/<name>.md`, and a `done` that names it.
fn write_up(hands: &mut impl Hands, me: &str, lead: &str, task: &str) -> String {
    let path = format!("{OUT_DIR}/{me}.md");
    let text = format!("# {}\n\n{task}\n", first_line(task));
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
        let vars = |pause: &str, asks: &str, questions: &str, seed: &str| {
            let [pause, asks, questions, seed] = [pause, asks, questions, seed].map(String::from);
            Habits::of(move |k| match k {
                PAUSE_VAR => Some(pause.clone()),
                ASKS_VAR => Some(asks.clone()),
                QUESTIONS_VAR => Some(questions.clone()),
                SEED_VAR => Some(seed.clone()),
                _ => None,
            })
        };
        let h = vars(" 1500 ", "3", "4", "42");
        assert_eq!(
            h,
            Habits {
                pause: Duration::from_millis(1500),
                asks: 3,
                questions: 4,
                seed: 42
            }
        );
        assert_eq!(
            h.vars(),
            "IMECE_STAND_IN_PAUSE_MS=1500 IMECE_STAND_IN_ASKS=3 IMECE_STAND_IN_QUESTIONS=4 IMECE_STAND_IN_SEED=42"
        );
        assert_eq!(Habits::of(|_| None), Habits::default());
        assert_eq!(
            vars("x", "-1", "y", ""),
            Habits::default(),
            "not numbers: 0"
        );
        assert_eq!(
            vars("10000", "0", "0", "0").pause,
            Duration::from_millis(10_000)
        );
        assert_eq!(
            vars("10001", "0", "0", "0").pause,
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
        // are one program it runs.
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
                    TEST_PROGRAMS.contains(&argv[0].as_str()) || argv[0] == "cargo",
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
}
