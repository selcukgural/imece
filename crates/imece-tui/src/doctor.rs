//! `imece doctor` (record 138): one report of what a start needs, before
//! any agent spends a cent. A check per category (imece's own state,
//! Claude Code, git and the checkout, its tests, Lonca, the terminal, the
//! person's files), each marked by its worst finding, with what to do
//! under what is wrong.
//!
//! Two halves. [`gather`] looks at the machine and brings back [`Facts`]:
//! it asks Claude Code and Lonca through their own commands, reads the
//! store without taking it, and writes nothing. [`report`] judges facts
//! and words them, and is pure: the tests give it facts, and so can a
//! window on the screen. Nothing here calls a model.

use std::path::{Path, PathBuf};

use imece_agent::{Login, claude, copy, lonca, worktree};
use serde_json::{Value, json};

use crate::i18n::{Doctor as Words, Lang, clean, fill};
use crate::model::{StoreDown, home_tilde};
use crate::notify::Style;
use crate::screen::Who;
use crate::ui::words::{push_why, store_down};
use crate::welcome::{SIGN_IN, Seen};
use crate::{awake, editor, keys, live, push, runners, screen, store, theme, ui};

/// The oldest git whose worktrees can be as sparse as their checkout
/// (record 96).
pub const GIT_LEAST: (u64, u64) = (2, 35);
/// Less free space than this where the state is, and it is said: a
/// worker's worktree is a copy of the checkout's files.
pub const SPACE_LOW: u64 = 1 << 30;
/// The size the screens are designed at (record 125); smaller is the
/// narrow layout (record 131).
pub const DESIGNED: (u16, u16) = (100, 40);
/// The most files of a checkout read to find its tests.
const FILES_MOST: usize = 20_000;

/// How one finding stands. Ordered: a category is as bad as its worst.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mark {
    Ok,
    /// Something to know that is nothing to mend: no issue.
    Note,
    /// Works, but not as well as it could, or something will be refused.
    Warn,
    /// A start, or the part named, does not work.
    Fail,
}

impl Mark {
    fn glyph(self) -> &'static str {
        match self {
            Mark::Ok => "✓",
            Mark::Note => "•",
            Mark::Warn => "!",
            Mark::Fail => "✗",
        }
    }

    /// Its name in `--json`.
    fn name(self) -> &'static str {
        match self {
            Mark::Ok => "ok",
            Mark::Note => "note",
            Mark::Warn => "warn",
            Mark::Fail => "fail",
        }
    }

    /// The SGR colour it is written in on a terminal.
    fn colour(self) -> &'static str {
        match self {
            Mark::Ok => "32",
            Mark::Note => "2",
            Mark::Warn => "33",
            Mark::Fail => "31",
        }
    }
}

/// One finding: what was found, and what to do when it is not right.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// Its name in `--json`, the same in every language.
    pub id: &'static str,
    pub mark: Mark,
    pub text: String,
    pub fix: Option<String>,
}

impl Item {
    fn new(id: &'static str, mark: Mark, text: String) -> Item {
        Item {
            id,
            mark,
            text,
            fix: None,
        }
    }

    fn fix(mut self, fix: impl Into<String>) -> Item {
        self.fix = Some(fix.into());
        self
    }
}

/// The findings of one kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    pub id: &'static str,
    pub title: String,
    /// What its one line says when nothing is wrong.
    pub summary: String,
    pub items: Vec<Item>,
}

impl Category {
    /// Its worst finding; a note is no issue.
    pub fn mark(&self) -> Mark {
        let worst = self.items.iter().map(|i| i.mark).max();
        match worst {
            Some(Mark::Warn) => Mark::Warn,
            Some(Mark::Fail) => Mark::Fail,
            _ => Mark::Ok,
        }
    }
}

/// The whole report, worded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub head: String,
    pub categories: Vec<Category>,
    /// Its last line, without the mark.
    pub foot: String,
}

/// One line of a report as it is written: how deep, its mark, its words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// 0 the report's own, 1 a finding, 2 what to do about it.
    pub depth: u8,
    /// A category's line has its mark in brackets.
    pub heading: bool,
    pub mark: Option<Mark>,
    pub text: String,
}

impl Report {
    /// The categories with an issue.
    pub fn issues(&self) -> usize {
        (self.categories.iter())
            .filter(|c| c.mark() > Mark::Note)
            .count()
    }

    /// Whether something does not work: what the exit code says.
    pub fn failed(&self) -> bool {
        self.categories.iter().any(|c| c.mark() == Mark::Fail)
    }

    /// Its worst category.
    fn mark(&self) -> Mark {
        let worst = self.categories.iter().map(Category::mark).max();
        worst.unwrap_or(Mark::Ok)
    }

    /// The report line by line: a category with nothing wrong is one line
    /// (all of its findings with `verbose`), one with an issue lists
    /// every finding, and what to do under each that has it.
    pub fn lines(&self, verbose: bool) -> Vec<Line> {
        let plain = |text: &str| Line {
            depth: 0,
            heading: false,
            mark: None,
            text: text.to_string(),
        };
        let mut out = vec![plain(&self.head), plain("")];
        for c in &self.categories {
            let mark = c.mark();
            let open = verbose || mark > Mark::Note;
            let text = match open || c.summary.is_empty() {
                true => c.title.clone(),
                false => format!("{}: {}", c.title, c.summary),
            };
            out.push(Line {
                depth: 0,
                heading: true,
                mark: Some(mark),
                text,
            });
            for i in c.items.iter().filter(|_| open) {
                out.push(Line {
                    depth: 1,
                    heading: false,
                    mark: Some(i.mark),
                    text: i.text.clone(),
                });
                if let Some(fix) = &i.fix {
                    out.push(Line {
                        depth: 2,
                        heading: false,
                        mark: None,
                        text: fix.clone(),
                    });
                }
            }
        }
        out.push(plain(""));
        out.push(Line {
            depth: 0,
            heading: false,
            mark: Some(self.mark()),
            text: self.foot.clone(),
        });
        out
    }

    /// The report as text, a line break after every line; the marks in
    /// colour when `colour`.
    pub fn render(&self, verbose: bool, colour: bool) -> String {
        let mut out = String::new();
        for l in self.lines(verbose) {
            let mark = l.mark.map(|m| {
                let glyph = match l.heading {
                    true => format!("[{}]", m.glyph()),
                    false => m.glyph().to_string(),
                };
                match colour {
                    true => format!("\x1b[{}m{glyph}\x1b[0m ", m.colour()),
                    false => format!("{glyph} "),
                }
            });
            let indent = match l.depth {
                0 => "",
                1 => "    ",
                _ => "      ",
            };
            out.push_str(indent);
            out.push_str(&mark.unwrap_or_default());
            out.push_str(&l.text);
            out.push('\n');
        }
        out
    }

    /// The report for a script: every finding, by its name.
    pub fn json(&self) -> Value {
        let categories: Vec<Value> = (self.categories.iter())
            .map(|c| {
                let items: Vec<Value> = (c.items.iter())
                    .map(|i| {
                        json!({
                            "id": i.id,
                            "status": i.mark.name(),
                            "text": i.text,
                            "fix": i.fix,
                        })
                    })
                    .collect();
                json!({
                    "id": c.id,
                    "title": c.title,
                    "status": c.mark().name(),
                    "items": items,
                })
            })
            .collect();
        json!({
            "imece": crate::welcome::VERSION,
            "issues": self.issues(),
            "failed": self.failed(),
            "categories": categories,
        })
    }
}

/// Everything [`gather`] found.
#[derive(Debug)]
pub struct Facts {
    /// The folder asked about, as the report's first line names it.
    pub checkout: String,
    pub imece: Imece,
    pub claude: Claude,
    pub git: Git,
    /// None without a git checkout with a commit: nothing to look in.
    pub tests: Option<Result<Vec<String>, String>>,
    /// None when `lonca` is not on PATH and the project is not on Lonca.
    pub lonca: Option<Lonca>,
    /// None when imece's input and output are not a terminal.
    pub terminal: Option<Terminal>,
    pub files: Files,
}

/// imece's own state.
#[derive(Debug)]
pub struct Imece {
    pub version: String,
    /// The state directory, as shown.
    pub state: String,
    pub dir: Dir,
    /// Bytes free where the state is.
    pub free: Option<u64>,
    /// The store's file, as shown.
    pub file: String,
    pub store: Kept,
    pub lock: Lock,
    /// Worktrees under the state directory.
    pub trees: usize,
}

/// The state directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    There,
    /// Not there yet, and it can be made.
    New,
    /// Not the person's to write to, or it can not be made.
    Closed,
}

/// The store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kept {
    None,
    /// Its schema.
    At(i64),
    Unreadable(String),
}

/// Who holds the store's lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lock {
    Free,
    /// Another imece.
    Other,
    /// The imece that asks.
    Own,
}

/// Claude Code.
#[derive(Debug)]
pub struct Claude {
    pub seen: Seen,
    /// The options imece passes that its help does not name; none when
    /// the help did not answer.
    pub flags: Option<Vec<String>>,
}

/// git and the folder.
#[derive(Debug)]
pub struct Git {
    /// What `git --version` said.
    pub version: Option<String>,
    pub checkout: Checkout,
}

/// The folder asked about.
#[derive(Debug)]
pub enum Checkout {
    /// Not a folder: its path and why.
    Bad(String, String),
    NoGit(String),
    /// A checkout without a first commit.
    Unborn(String),
    Here {
        top: String,
        /// None on a detached HEAD.
        branch: Option<String>,
        /// What its `.worktreeinclude` names, when it has one.
        include: Option<copy::Included>,
    },
}

/// Lonca.
#[derive(Debug)]
pub struct Lonca {
    /// What `lonca --version` said; none when it is not on PATH.
    pub version: Option<String>,
    /// The repository this checkout's project is on.
    pub repo: Option<String>,
    /// Whether the machine's install is hardened; none when its file is
    /// not what the installer left.
    pub hardened: Option<bool>,
    /// The daemon reached and the person's session taken, then what it
    /// says of the repository, when one is named.
    pub reach: Result<Option<Result<Repo, StoreDown>>, StoreDown>,
}

/// A repository on Lonca.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Repo {
    pub frozen: bool,
    /// Whether its policy keeps an unobserved session's proposal for a
    /// human; none when it could not be read.
    pub guarded: Option<bool>,
    /// The checks its policy defines, when Lonca said.
    pub checks: Option<usize>,
}

/// What the terminal said of itself.
#[derive(Debug)]
pub struct Terminal {
    pub who: Option<Who>,
    pub keys: bool,
    pub truecolor: bool,
    pub progress: bool,
    pub notify: Style,
    pub tmux: bool,
    pub ssh: bool,
    /// Columns and rows.
    pub size: Option<(u16, u16)>,
}

/// The person's own files and tools.
#[derive(Debug)]
pub struct Files {
    /// `instructions.md`: where it is looked for and what is there.
    pub instructions: Option<(String, Plain)>,
    pub keys: Option<(PathBuf, keys::Read)>,
    /// The push file as shown, and its server or why it is refused.
    pub push: (String, Result<String, push::Why>),
    pub editor: Editor,
    /// The program `:awake` runs and whether it is on PATH; none where
    /// there is no such tool.
    pub awake: Option<(String, bool)>,
}

/// What is at a path that should be a plain file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plain {
    Missing,
    /// Its length.
    File(u64),
    Other,
}

/// The editor `^x ^e` opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Editor {
    pub name: String,
    /// Whether `VISUAL` or `EDITOR` names it.
    pub set: bool,
    pub found: bool,
}

/// `bytes` as a person reads a size.
fn size_words(bytes: u64) -> String {
    const GIB: u64 = 1 << 30;
    match bytes >= GIB {
        true => format!("{}.{} GB", bytes / GIB, bytes % GIB * 10 / GIB),
        false => format!("{} MB", bytes >> 20),
    }
}

/// The first two numbers of the version in what `git --version` said
/// (`git version 2.39.5 (Apple Git-154)`).
fn git_numbers(line: &str) -> Option<(u64, u64)> {
    line.split_whitespace().find_map(|word| {
        let mut parts = word.split('.').map(|p| p.parse::<u64>().ok());
        Some((parts.next()??, parts.next()??))
    })
}

fn imece(t: &Words, f: &Imece) -> Category {
    let mut items = vec![Item::new(
        "imece.version",
        Mark::Ok,
        fill(t.imece_version, &[&f.version]),
    )];
    items.push(match f.dir {
        Dir::There => Item::new("imece.state", Mark::Ok, fill(t.state_ok, &[&f.state])),
        Dir::New => Item::new("imece.state", Mark::Ok, fill(t.state_new, &[&f.state])),
        Dir::Closed => {
            Item::new("imece.state", Mark::Fail, fill(t.state_closed, &[&f.state])).fix(t.state_fix)
        }
    });
    if let Some(free) = f.free {
        let size = size_words(free);
        items.push(match free < SPACE_LOW {
            true => {
                Item::new("imece.space", Mark::Warn, fill(t.space_low, &[&size])).fix(t.space_fix)
            }
            false => Item::new("imece.space", Mark::Ok, fill(t.space_ok, &[&size])),
        });
    }
    let knows = store::SCHEMA_VERSION;
    let schemas = |at: i64| [at.to_string(), knows.to_string()];
    items.push(match &f.store {
        Kept::None => Item::new("imece.store", Mark::Ok, t.store_none.to_string()),
        Kept::At(at) if *at > knows => {
            let [at, knows] = schemas(*at);
            Item::new(
                "imece.store",
                Mark::Fail,
                fill(t.store_newer, &[&at, &knows]),
            )
            .fix(t.store_newer_fix)
        }
        Kept::At(at) if *at < knows => {
            let [at, knows] = schemas(*at);
            Item::new("imece.store", Mark::Ok, fill(t.store_older, &[&at, &knows]))
        }
        Kept::At(at) => Item::new(
            "imece.store",
            Mark::Ok,
            fill(t.store_ok, &[&at.to_string()]),
        ),
        Kept::Unreadable(why) => Item::new(
            "imece.store",
            Mark::Fail,
            fill(t.store_bad, &[&f.file, why]),
        )
        .fix(t.store_bad_fix),
    });
    match f.lock {
        Lock::Free => {}
        Lock::Other => items.push(
            Item::new("imece.lock", Mark::Warn, t.lock_held.to_string()).fix(t.lock_held_fix),
        ),
        Lock::Own => items.push(Item::new("imece.lock", Mark::Ok, t.lock_own.to_string())),
    }
    if f.trees > 0 {
        let trees = fill(t.trees, &[&f.state, &f.trees.to_string()]);
        items.push(Item::new("imece.worktrees", Mark::Note, trees));
    }
    Category {
        id: "imece",
        title: t.cat_imece.to_string(),
        summary: format!("{}, {}", clean(&f.version), fill(t.state_ok, &[&f.state])),
        items,
    }
}

fn claude_code(t: &Words, f: &Claude) -> Category {
    let mut items = Vec::new();
    let mut summary = String::new();
    match &f.seen {
        Seen::Checking | Seen::Missing => items.push(
            Item::new("claude.program", Mark::Fail, t.claude_missing.to_string())
                .fix(t.claude_missing_fix),
        ),
        Seen::Here { version, login } => {
            summary = clean(version);
            items.push(Item::new(
                "claude.program",
                Mark::Ok,
                fill(t.claude_here, &[version]),
            ));
            items.push(match login {
                Login::Yes { subscription: true } => {
                    Item::new("claude.login", Mark::Ok, t.login_plan.to_string())
                }
                Login::Yes {
                    subscription: false,
                } => Item::new("claude.login", Mark::Ok, t.login_key.to_string()),
                Login::No => Item::new("claude.login", Mark::Fail, t.login_no.to_string())
                    .fix(fill(t.run_fix, &[SIGN_IN])),
                Login::Unknown => {
                    Item::new("claude.login", Mark::Warn, t.login_unknown.to_string())
                        .fix(t.login_unknown_fix)
                }
            });
            items.push(match &f.flags {
                None => Item::new("claude.flags", Mark::Warn, t.flags_unasked.to_string()),
                Some(missing) if missing.is_empty() => {
                    Item::new("claude.flags", Mark::Ok, t.flags_ok.to_string())
                }
                Some(missing) => Item::new(
                    "claude.flags",
                    Mark::Warn,
                    fill(t.flags_missing, &[&missing.join(" ")]),
                )
                .fix(t.flags_missing_fix),
            });
        }
    }
    Category {
        id: "claude",
        title: t.cat_claude.to_string(),
        summary,
        items,
    }
}

fn git(t: &Words, f: &Git) -> Category {
    let least = format!("{}.{}", GIT_LEAST.0, GIT_LEAST.1);
    let mut items = Vec::new();
    let mut summary = String::new();
    match &f.version {
        None => items.push(
            Item::new("git.program", Mark::Fail, t.git_missing.to_string())
                .fix(fill(t.git_missing_fix, &[&least])),
        ),
        Some(line) => {
            summary = clean(line);
            items.push(match git_numbers(line) {
                Some(v) if v < GIT_LEAST => {
                    let has = format!("{}.{}", v.0, v.1);
                    Item::new("git.program", Mark::Warn, fill(t.git_old, &[&has, &least]))
                        .fix(t.git_old_fix)
                }
                _ => Item::new("git.program", Mark::Ok, clean(line)),
            });
        }
    }
    match &f.checkout {
        Checkout::Bad(path, why) => items.push(
            Item::new("git.checkout", Mark::Fail, fill(t.co_bad, &[path, why])).fix(t.co_bad_fix),
        ),
        Checkout::NoGit(dir) => items.push(
            Item::new("git.checkout", Mark::Warn, fill(t.co_no_git, &[dir])).fix(t.co_no_git_fix),
        ),
        Checkout::Unborn(dir) => items.push(
            Item::new("git.checkout", Mark::Warn, fill(t.co_unborn, &[dir])).fix(t.co_unborn_fix),
        ),
        Checkout::Here {
            top,
            branch,
            include,
        } => {
            items.push(match branch {
                Some(b) => Item::new("git.checkout", Mark::Ok, fill(t.co_branch, &[top, b])),
                None => Item::new("git.checkout", Mark::Warn, fill(t.co_detached, &[top]))
                    .fix(t.co_detached_fix),
            });
            if let Some(b) = branch {
                summary = format!("{summary}, {}", fill(t.co_branch, &[top, b]));
            }
            if let Some(named) = include {
                items.push(match named.refused.first() {
                    None => {
                        let n = named.paths.len().to_string();
                        Item::new("git.include", Mark::Ok, fill(t.include_ok, &[&n]))
                    }
                    Some(r) => {
                        let text = match &r.line {
                            Some((n, _)) => fill(t.include_line, &[&n.to_string(), &r.why]),
                            None => fill(t.include_whole, &[&r.why]),
                        };
                        Item::new("git.include", Mark::Warn, text).fix(t.include_fix)
                    }
                });
            }
        }
    }
    Category {
        id: "git",
        title: t.cat_git.to_string(),
        summary,
        items,
    }
}

fn tests(t: &Words, f: &Result<Vec<String>, String>) -> Category {
    let (item, summary) = match f {
        Err(why) => (
            Item::new("tests.found", Mark::Warn, fill(t.tests_unread, &[why])),
            String::new(),
        ),
        Ok(lines) if lines.is_empty() => (
            Item::new("tests.found", Mark::Warn, t.tests_none.to_string()).fix(t.tests_none_fix),
            String::new(),
        ),
        Ok(lines) => {
            let all = lines.join(" · ");
            (
                Item::new("tests.found", Mark::Ok, fill(t.tests_found, &[&all])),
                clean(&all),
            )
        }
    };
    Category {
        id: "tests",
        title: t.cat_tests.to_string(),
        summary,
        items: vec![item],
    }
}

/// What to do about a daemon that can not be read, when there is
/// something the person can do.
fn down_fix(t: &Words, why: StoreDown) -> Option<&'static str> {
    match why {
        StoreDown::NoDaemon => Some(t.lonca_fix_daemon),
        StoreDown::SignedOut => Some(t.lonca_fix_login),
        StoreDown::Old => Some(t.lonca_old_fix),
        _ => None,
    }
}

fn lonca_store(lang: Lang, f: &Lonca) -> Category {
    let (t, all) = (&lang.text().doctor, lang.text());
    // A project on Lonca that can not reach it is closed to hires and
    // decisions; with none on it, Lonca is only on offer, and a daemon
    // that is not running is nothing to mend.
    let bad = match f.repo {
        Some(_) => Mark::Fail,
        None => Mark::Note,
    };
    let mut items = Vec::new();
    let mut summary = String::new();
    match &f.version {
        None => items.push(
            Item::new("lonca.program", Mark::Fail, t.lonca_missing.to_string())
                .fix(t.lonca_missing_fix),
        ),
        Some(line) => {
            summary = clean(line);
            let version = line.split_whitespace().last();
            items.push(match lonca::new_enough(version) {
                true => Item::new("lonca.program", Mark::Ok, clean(line)),
                false => Item::new(
                    "lonca.program",
                    bad,
                    fill(t.lonca_old, &[line, &lonca::least()]),
                )
                .fix(t.lonca_old_fix),
            });
        }
    }
    if f.hardened == Some(false) {
        items.push(Item::new("lonca.mode", Mark::Ok, t.lonca_coop.to_string()));
    }
    let down = |id, mark, text: String, why| {
        let item = Item::new(id, mark, text);
        match down_fix(t, why) {
            Some(fix) => item.fix(fix),
            None => item,
        }
    };
    match &f.reach {
        Err(why) => {
            if f.repo.is_none() {
                summary = format!("{summary}, {}", t.lonca_unused);
            }
            let text = store_down(all, *why).to_string();
            items.push(down("lonca.daemon", bad, text, *why));
        }
        Ok(seen) => {
            items.push(Item::new("lonca.daemon", Mark::Ok, t.lonca_up.to_string()));
            let name = f.repo.as_deref().unwrap_or_default();
            let said = |why| fill(t.lonca_repo_down, &[name, store_down(all, why)]);
            match seen {
                None => {
                    summary = format!("{summary}, {}", t.lonca_unused);
                    items.push(Item::new("lonca.repo", Mark::Note, t.lonca_off.to_string()));
                }
                Some(Err(why)) => items.push(down("lonca.repo", Mark::Fail, said(*why), *why)),
                Some(Ok(repo)) if repo.frozen => {
                    items.push(Item::new("lonca.repo", Mark::Fail, said(StoreDown::Frozen)))
                }
                Some(Ok(repo)) if repo.guarded == Some(false) => items.push(Item::new(
                    "lonca.repo",
                    Mark::Fail,
                    said(StoreDown::Unguarded),
                )),
                Some(Ok(repo)) => {
                    let text = fill(t.lonca_repo, &[name]);
                    summary = format!("{summary}, {text}");
                    items.push(Item::new("lonca.repo", Mark::Ok, text));
                    if let Some(n) = repo.checks {
                        let text = fill(t.lonca_checks, &[&n.to_string()]);
                        items.push(Item::new("lonca.checks", Mark::Note, text));
                    }
                }
            }
        }
    }
    Category {
        id: "lonca",
        title: t.cat_lonca.to_string(),
        summary,
        items,
    }
}

/// A terminal's name and version as it said them.
fn who_words(who: &Who) -> String {
    match who.version {
        Some(v) => format!("{} {}.{}.{}", who.name, v[0], v[1], v[2]),
        None => who.name.clone(),
    }
}

fn terminal(t: &Words, f: Option<&Terminal>) -> Category {
    let Some(f) = f else {
        return Category {
            id: "terminal",
            title: t.cat_terminal.to_string(),
            summary: t.term_none.to_string(),
            items: vec![Item::new(
                "terminal.none",
                Mark::Note,
                t.term_none.to_string(),
            )],
        };
    };
    let mut items = Vec::new();
    let name = f.who.as_ref().map(who_words);
    items.push(match &name {
        Some(name) => Item::new("terminal.name", Mark::Ok, clean(name)),
        None => Item::new("terminal.name", Mark::Note, t.term_unnamed.to_string()),
    });
    let mut summary = name.map(|n| clean(&n)).unwrap_or_default();
    if let Some((columns, rows)) = f.size {
        let (c, r) = (columns.to_string(), rows.to_string());
        let n = |n: u16| n.to_string();
        let floor = (ui::MIN_COLUMNS, ui::MIN_ROWS);
        items.push(if columns < floor.0 || rows < floor.1 {
            let text = fill(t.term_size_small, &[&c, &r, &n(floor.0), &n(floor.1)]);
            Item::new("terminal.size", Mark::Fail, text).fix(t.term_size_fix)
        } else if columns < DESIGNED.0 || rows < DESIGNED.1 {
            let text = fill(
                t.term_size_narrow,
                &[&c, &r, &n(DESIGNED.0), &n(DESIGNED.1)],
            );
            Item::new("terminal.size", Mark::Note, text)
        } else {
            Item::new("terminal.size", Mark::Ok, fill(t.term_size, &[&c, &r]))
        });
        let size = fill(t.term_size, &[&c, &r]);
        summary = match summary.is_empty() {
            true => size,
            false => format!("{summary}, {size}"),
        };
    }
    let either = |id, yes: bool, ok: &str, not: &str| match yes {
        true => Item::new(id, Mark::Ok, ok.to_string()),
        false => Item::new(id, Mark::Note, not.to_string()),
    };
    items.push(either(
        "terminal.colour",
        f.truecolor,
        t.term_true,
        t.term_256,
    ));
    items.push(either("terminal.keys", f.keys, t.term_keys, t.term_no_keys));
    items.push(either(
        "terminal.progress",
        f.progress,
        t.term_progress,
        t.term_no_progress,
    ));
    let style = match f.notify {
        Style::Osc9 => "OSC 9",
        Style::Osc777 => "OSC 777",
        Style::Kitty => "OSC 99",
        Style::Tmux => "OSC 9, tmux",
    };
    items.push(Item::new(
        "terminal.notices",
        Mark::Ok,
        fill(t.term_notify, &[style]),
    ));
    if f.tmux {
        items.push(Item::new(
            "terminal.tmux",
            Mark::Note,
            t.term_tmux.to_string(),
        ));
    }
    if f.ssh {
        items.push(Item::new(
            "terminal.ssh",
            Mark::Note,
            t.term_ssh.to_string(),
        ));
    }
    Category {
        id: "terminal",
        title: t.cat_terminal.to_string(),
        summary,
        items,
    }
}

fn files(lang: Lang, f: &Files) -> Category {
    let t = &lang.text().doctor;
    let mut items = Vec::new();
    if let Some((path, what)) = &f.instructions {
        items.push(match what {
            Plain::Missing => Item::new(
                "files.instructions",
                Mark::Note,
                fill(t.file_no_instructions, &[path]),
            ),
            Plain::File(n) => Item::new(
                "files.instructions",
                Mark::Ok,
                fill(t.file_instructions, &[path, &n.to_string()]),
            ),
            Plain::Other => Item::new(
                "files.instructions",
                Mark::Warn,
                fill(t.file_instructions_bad, &[path]),
            ),
        });
    }
    if let Some((path, read)) = &f.keys {
        let shown = home_tilde(path);
        items.push(match read {
            keys::Read::Missing => {
                Item::new("files.keys", Mark::Note, fill(t.file_no_keys, &[&shown]))
            }
            keys::Read::Good(map, _) => Item::new(
                "files.keys",
                Mark::Ok,
                fill(t.file_keys, &[&shown, &map.len().to_string()]),
            ),
            keys::Read::Bad(why) => {
                Item::new("files.keys", Mark::Warn, keys::refusal(lang, path, why))
                    .fix(t.file_keys_fix)
            }
        });
    }
    let (path, read) = &f.push;
    items.push(match read {
        Ok(server) => Item::new("files.push", Mark::Ok, fill(t.file_push, &[path, server])),
        Err(push::Why::NoFile(_)) => {
            Item::new("files.push", Mark::Note, fill(t.file_no_push, &[path]))
        }
        Err(why) => Item::new("files.push", Mark::Warn, push_why(lang, why)).fix(t.file_push_fix),
    });
    let e = &f.editor;
    items.push(match (e.found, e.set) {
        (false, _) => Item::new(
            "files.editor",
            Mark::Warn,
            fill(t.file_editor_missing, &[&e.name]),
        )
        .fix(t.file_editor_fix),
        (true, true) => Item::new("files.editor", Mark::Ok, fill(t.file_editor, &[&e.name])),
        (true, false) => Item::new(
            "files.editor",
            Mark::Ok,
            fill(t.file_editor_default, &[&e.name]),
        ),
    });
    items.push(match &f.awake {
        None => Item::new("files.awake", Mark::Note, t.file_awake_os.to_string()),
        Some((program, true)) => Item::new("files.awake", Mark::Ok, fill(t.file_awake, &[program])),
        Some((program, false)) => {
            Item::new("files.awake", Mark::Note, fill(t.file_no_awake, &[program]))
        }
    });
    Category {
        id: "files",
        title: t.cat_files.to_string(),
        summary: fill(t.file_editor, &[&e.name]),
        items,
    }
}

/// The report of `facts` in `lang`.
pub fn report(lang: Lang, facts: &Facts) -> Report {
    let t = &lang.text().doctor;
    let mut categories = vec![
        imece(t, &facts.imece),
        claude_code(t, &facts.claude),
        git(t, &facts.git),
    ];
    categories.extend(facts.tests.as_ref().map(|f| tests(t, f)));
    categories.extend(facts.lonca.as_ref().map(|f| lonca_store(lang, f)));
    categories.push(terminal(t, facts.terminal.as_ref()));
    categories.push(files(lang, &facts.files));
    let mut report = Report {
        head: fill(t.title, &[&facts.imece.version, &facts.checkout]),
        categories,
        foot: String::new(),
    };
    report.foot = match report.issues() {
        0 => t.ok_all.to_string(),
        1 => t.issues_one.to_string(),
        n => fill(t.issues_many, &[&n.to_string()]),
    };
    report
}

/// Whether `path` is a file someone may run.
fn runs(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Whether `program` is found as a shell finds it: by its path when it
/// has a `/`, else in one of the directories of `path` (`PATH`).
fn on_path_in(program: &str, path: Option<&std::ffi::OsStr>) -> bool {
    if program.contains('/') {
        return runs(Path::new(program));
    }
    path.is_some_and(|p| std::env::split_paths(p).any(|dir| runs(&dir.join(program))))
}

fn on_path(program: &str) -> bool {
    on_path_in(program, std::env::var_os("PATH").as_deref())
}

/// Whether this process may write into the directory `dir`.
fn writable(dir: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(path) = std::ffi::CString::new(dir.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: a C string that lives through the call.
    dir.is_dir() && unsafe { libc::access(path.as_ptr(), libc::W_OK | libc::X_OK) } == 0
}

/// The bytes free for this user on the file system `dir` is on.
fn free_at(dir: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    // SAFETY: statvfs fills the struct it is given, which is all zeroes
    // (a valid one) before; the C string lives through the call.
    let stat = unsafe {
        let mut stat: libc::statvfs = std::mem::zeroed();
        (libc::statvfs(path.as_ptr(), &mut stat) == 0).then_some(stat)?
    };
    #[allow(clippy::unnecessary_cast)]
    Some((stat.f_bavail as u64).saturating_mul(stat.f_frsize as u64))
}

/// How the state directory `dir` stands, and the space where it is (or
/// will be): judged at the nearest directory that is there.
fn dir_seen(dir: &Path) -> (Dir, Option<u64>) {
    let near = dir.ancestors().find(|a| a.exists());
    let free = near.and_then(free_at);
    let seen = match near {
        Some(near) if near == dir && writable(dir) => Dir::There,
        Some(near) if near != dir && writable(near) => Dir::New,
        _ => Dir::Closed,
    };
    (seen, free)
}

/// The worktrees under `state`: a directory per checkout, one per agent
/// in it.
fn trees_under(state: &Path) -> usize {
    let dirs = |dir: &Path| -> Vec<PathBuf> {
        let entries = std::fs::read_dir(dir).into_iter().flatten().flatten();
        entries
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| e.path())
            .collect()
    };
    let roots = dirs(&state.join("worktrees"));
    roots.iter().map(|root| dirs(root).len()).sum()
}

fn gather_imece(state: &Path, own: bool) -> (Imece, Option<store::Peek>) {
    let file = state.join(store::FILE);
    let (dir, free) = dir_seen(state);
    let peek = store::peek(&file);
    let lock = match (own, store::held(&file)) {
        (true, _) => Lock::Own,
        (false, true) => Lock::Other,
        (false, false) => Lock::Free,
    };
    let imece = Imece {
        version: crate::welcome::VERSION.to_string(),
        state: home_tilde(state),
        dir,
        free,
        file: home_tilde(&file),
        store: match &peek {
            Ok(None) => Kept::None,
            Ok(Some(p)) => Kept::At(p.schema),
            Err(why) => Kept::Unreadable(why.clone()),
        },
        lock,
        trees: trees_under(state),
    };
    (imece, peek.ok().flatten())
}

fn gather_claude(program: &str) -> Claude {
    let Some(version) = imece_agent::version(program, claude::ENV) else {
        return Claude {
            seen: Seen::Missing,
            flags: None,
        };
    };
    let login = claude::login(program);
    let flags = claude::flags_missing(program);
    Claude {
        seen: Seen::Here { version, login },
        flags: flags.map(|f| f.into_iter().map(str::to_string).collect()),
    }
}

/// The folder at `repo`, and its top when it is a git checkout with a
/// commit.
fn gather_checkout(repo: &Path) -> (Checkout, Option<PathBuf>) {
    match live::checkout_of(repo) {
        Err(why) => (Checkout::Bad(home_tilde(repo), why), None),
        Ok((dir, false)) => {
            let shown = home_tilde(&dir);
            match worktree::top(&dir) {
                Ok(_) => (Checkout::Unborn(shown), None),
                Err(_) => (Checkout::NoGit(shown), None),
            }
        }
        Ok((top, true)) => {
            let here = Checkout::Here {
                top: home_tilde(&top),
                branch: worktree::on_branch(&top).map(|(branch, _)| branch),
                include: copy::has_include(&top).then(|| copy::included(&top)),
            };
            (here, Some(top))
        }
    }
}

/// The test commands imece finds for the checkout at `top` (record 136),
/// cargo's among them.
fn gather_tests(top: &Path) -> Result<Vec<String>, String> {
    let (files, _) = worktree::files(top, FILES_MOST).map_err(|e| e.to_string())?;
    let mut lines = Vec::new();
    if files.iter().any(|f| f == "Cargo.toml") {
        lines.push("cargo test …".to_string());
    }
    lines.extend(runners::of(&files).lines());
    Ok(lines)
}

fn gather_lonca(repo: Option<String>) -> Option<Lonca> {
    let version = imece_agent::version("lonca", &[]);
    if version.is_none() && repo.is_none() {
        return None;
    }
    let place = lonca::Place::here();
    let reach = place
        .open()
        .map(|mut client| {
            repo.as_deref().map(|name| {
                let status = client.status(name)?;
                Ok(Repo {
                    frozen: status.frozen.is_some(),
                    guarded: client.holds_unobserved(name, &status.policy).ok(),
                    checks: status.policy_checks.map(|c| c.len()),
                })
            })
        })
        .map(|seen| seen.map(|r| r.map_err(live::store_down_of)))
        .map_err(live::store_down_of);
    Some(Lonca {
        version,
        repo,
        hardened: place.trust.map(|t| t.is_hardened()),
        reach,
    })
}

/// What the terminal on stdin and stdout says of itself, asked as a
/// start asks it; none when they are not a terminal.
pub fn gather_terminal() -> Option<Terminal> {
    // SAFETY: isatty only reads the descriptor's state.
    let tty = unsafe { libc::isatty(0) == 1 && libc::isatty(1) == 1 };
    if !tty {
        return None;
    }
    let said = crate::probe::terminal();
    let var = |k: &str| std::env::var(k).ok();
    Some(Terminal {
        keys: said.keys,
        truecolor: theme::truecolor(var("COLORTERM").as_deref()),
        progress: screen::shows_progress(said.who.as_ref(), var),
        notify: Style::of(said.who.as_ref(), var),
        tmux: screen::in_tmux(said.who.as_ref(), &var),
        ssh: crate::motion::over_ssh(var),
        // A pty nobody gave a size says 0 by 0: not known.
        size: (ratatui::crossterm::terminal::size().ok()).filter(|&(c, r)| c > 0 && r > 0),
        who: said.who,
    })
}

fn gather_files() -> Files {
    let plain = |path: &Path| match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_file() => Plain::File(m.len()),
        Ok(_) => Plain::Other,
        Err(_) => Plain::Missing,
    };
    let push_file = push::file();
    let push_shown = push_file.as_deref().map(home_tilde);
    let set = |name: &str| std::env::var(name).is_ok_and(|v| !v.trim().is_empty());
    let name = editor::of_env();
    let program = name.split_whitespace().next().unwrap_or_default();
    Files {
        instructions: keys::config_file(live::INSTRUCTIONS)
            .map(|path| (home_tilde(&path), plain(&path))),
        keys: keys::file().map(|path| {
            let read = keys::load(&path);
            (path, read)
        }),
        push: (
            push_shown.unwrap_or_else(|| "~/.config/imece/push".to_string()),
            push::read(push_file.as_deref()).map(|p| p.server),
        ),
        editor: Editor {
            found: on_path(program),
            set: set("VISUAL") || set("EDITOR"),
            name,
        },
        awake: awake::command(0)
            .and_then(|words| words.into_iter().next())
            .map(|program| {
                let found = on_path(&program);
                (program, found)
            }),
    }
}

/// Looks at the machine for the folder `repo`, with imece's state in
/// `state`: asks Claude Code, git and Lonca through their own commands
/// (a few seconds, offline, free), reads and writes nothing of theirs,
/// and makes nothing. `own` when the imece that asks holds the store;
/// `terminal` is what [`gather_terminal`] found, asked by the caller
/// since only it knows whether the terminal is free to be asked.
pub fn gather(repo: &Path, state: &Path, own: bool, terminal: Option<Terminal>) -> Facts {
    let (imece, peek) = gather_imece(state, own);
    let (checkout, top) = gather_checkout(repo);
    // The project this checkout is, as the store has it.
    let on_lonca = top.as_deref().and_then(|top| {
        let lonca = peek.map(|p| p.lonca).unwrap_or_default();
        let top = top.to_string_lossy();
        (lonca.into_iter()).find_map(|(repo, name)| (repo == top).then_some(name))
    });
    Facts {
        checkout: home_tilde(top.as_deref().unwrap_or(repo)),
        imece,
        claude: gather_claude(claude::PROGRAM),
        git: Git {
            version: imece_agent::version("git", &[]),
            checkout,
        },
        tests: top.as_deref().map(gather_tests),
        lonca: gather_lonca(on_lonca),
        terminal,
        files: gather_files(),
    }
}

/// What `imece doctor` was asked for.
#[derive(Debug, PartialEq, Eq)]
struct Asked {
    repo: PathBuf,
    lang: Lang,
    verbose: bool,
    json: bool,
}

/// The command line after `doctor`, and the environment's language.
fn parse(argv: &[String], env_lang: Option<&str>) -> Result<Asked, String> {
    let lang = |v: &str| Lang::parse(v).ok_or_else(|| format!("{v:?} is not en or tr"));
    let mut asked = Asked {
        repo: PathBuf::from("."),
        lang: match env_lang {
            Some(v) => lang(v).map_err(|e| format!("IMECE_LANG: {e}"))?,
            None => Lang::En,
        },
        verbose: false,
        json: false,
    };
    let usage = |lang: Lang| lang.text().doctor.usage.to_string();
    let mut it = argv.iter();
    while let Some(flag) = it.next() {
        let mut value = |flag: &str| {
            let v = it.next().cloned();
            v.ok_or_else(|| format!("{flag} needs a value\n\n{}", usage(asked.lang)))
        };
        match flag.as_str() {
            "-v" | "--verbose" => asked.verbose = true,
            "--json" => asked.json = true,
            "--repo" => asked.repo = PathBuf::from(value("--repo")?),
            "--lang" => asked.lang = lang(&value("--lang")?).map_err(|e| format!("--lang: {e}"))?,
            "-h" | "--help" => return Err(usage(asked.lang)),
            _ => {
                return Err(format!("unknown option {flag}\n\n{}", usage(asked.lang)));
            }
        }
    }
    Ok(asked)
}

/// Whether the marks go out in colour: to a terminal that is not `dumb`,
/// unless `NO_COLOR` says no.
fn coloured(tty: bool, var: impl Fn(&str) -> Option<String>) -> bool {
    tty && var("NO_COLOR").is_none_or(|v| v.is_empty()) && var("TERM").is_none_or(|t| t != "dumb")
}

/// `imece doctor ARGS…`: the report on stdout, and the exit code: 0, 1
/// when something does not work, 2 for a command line refused.
pub fn main(argv: &[String]) -> i32 {
    use std::io::Write as _;
    let var = |k: &str| std::env::var(k).ok();
    let asked = match parse(argv, var("IMECE_LANG").as_deref()) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            return 2;
        }
    };
    let state = crate::prefs::state_dir();
    let facts = gather(&asked.repo, &state, false, gather_terminal());
    let report = report(asked.lang, &facts);
    // SAFETY: isatty only reads the descriptor's state.
    let tty = unsafe { libc::isatty(1) == 1 };
    let said = match asked.json {
        true => format!("{:#}\n", report.json()),
        false => report.render(asked.verbose, coloured(tty, var)),
    };
    // A reader gone away (`| head`) is no fault of the report's.
    let _ = std::io::stdout().write_all(said.as_bytes());
    i32::from(report.failed())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::EN;

    /// A machine with nothing wrong, a project on Lonca.
    fn well() -> Facts {
        Facts {
            checkout: "~/code/shelf".into(),
            imece: Imece {
                version: "0.1.5".into(),
                state: "~/.local/state/imece".into(),
                dir: Dir::There,
                free: Some(20 << 30),
                file: "~/.local/state/imece/imece.sqlite".into(),
                store: Kept::At(store::SCHEMA_VERSION),
                lock: Lock::Free,
                trees: 0,
            },
            claude: Claude {
                seen: Seen::Here {
                    version: "2.1.321 (Claude Code)".into(),
                    login: Login::Yes { subscription: true },
                },
                flags: Some(Vec::new()),
            },
            git: Git {
                version: Some("git version 2.56.0".into()),
                checkout: Checkout::Here {
                    top: "~/code/shelf".into(),
                    branch: Some("main".into()),
                    include: None,
                },
            },
            tests: Some(Ok(vec!["cargo test …".into(), "go test …".into()])),
            lonca: Some(Lonca {
                version: Some("lonca 0.2.2".into()),
                repo: Some("shelf".into()),
                hardened: Some(false),
                reach: Ok(Some(Ok(Repo {
                    frozen: false,
                    guarded: Some(true),
                    checks: Some(2),
                }))),
            }),
            terminal: Some(Terminal {
                who: Who::parse("iTerm2 3.6.6"),
                keys: true,
                truecolor: true,
                progress: true,
                notify: Style::Osc9,
                tmux: false,
                ssh: false,
                size: Some((120, 40)),
            }),
            files: Files {
                instructions: Some(("~/.config/imece/instructions.md".into(), Plain::File(120))),
                keys: Some(("/home/me/.config/imece/keys".into(), keys::Read::Missing)),
                push: (
                    "~/.config/imece/push".into(),
                    Err(push::Why::NoFile("x".into())),
                ),
                editor: Editor {
                    name: "nvim".into(),
                    set: true,
                    found: true,
                },
                awake: Some(("caffeinate".into(), true)),
            },
        }
    }

    /// The finding `id` of the report of `facts`.
    fn found(facts: &Facts, id: &str) -> Item {
        let report = report(Lang::En, facts);
        let mut items = report.categories.iter().flat_map(|c| &c.items);
        let item = items.find(|i| i.id == id);
        item.unwrap_or_else(|| panic!("no {id}")).clone()
    }

    /// The mark and the fix of the finding `id` once `change` is made to
    /// a machine with nothing wrong.
    fn after(id: &str, change: impl FnOnce(&mut Facts)) -> (Mark, Option<String>) {
        let mut facts = well();
        change(&mut facts);
        let item = found(&facts, id);
        (item.mark, item.fix)
    }

    fn fixed(s: &str) -> Option<String> {
        Some(s.to_string())
    }

    #[test]
    fn a_machine_with_nothing_wrong_is_a_line_a_category_and_exit_zero() {
        let report = report(Lang::En, &well());
        assert_eq!(report.issues(), 0);
        assert!(!report.failed());
        assert_eq!(
            report.render(false, false),
            "Doctor summary (imece 0.1.5, checkout ~/code/shelf)\n\
             \n\
             [✓] imece: 0.1.5, state in ~/.local/state/imece\n\
             [✓] Claude Code: 2.1.321 (Claude Code)\n\
             [✓] git: git version 2.56.0, ~/code/shelf on branch main\n\
             [✓] This checkout's tests: cargo test … · go test …\n\
             [✓] Lonca: lonca 0.2.2, repository shelf\n\
             [✓] Terminal: iterm2 3.6.6, 120×40\n\
             [✓] Your files: ^x ^e opens nvim\n\
             \n\
             ✓ No issues found.\n"
        );
        // Every finding is there with `verbose`, each a tick or a note.
        let lines = report.lines(true);
        let findings: Vec<&Line> = lines.iter().filter(|l| l.depth == 1).collect();
        let all: usize = report.categories.iter().map(|c| c.items.len()).sum();
        assert_eq!(findings.len(), all);
        assert!(all > 20, "{all}");
        assert!(findings.iter().all(|l| l.mark <= Some(Mark::Note)));
        assert!(lines.iter().all(|l| l.depth < 2), "nothing to do");
    }

    #[test]
    fn a_category_with_an_issue_lists_its_findings_and_what_to_do() {
        let mut facts = well();
        facts.claude.seen = Seen::Here {
            version: "2.1.321 (Claude Code)".into(),
            login: Login::No,
        };
        facts.git.checkout = Checkout::Here {
            top: "~/code/shelf".into(),
            branch: None,
            include: None,
        };
        facts.lonca = None;
        facts.tests = None;
        let report = report(Lang::En, &facts);
        assert_eq!((report.issues(), report.failed()), (2, true));
        assert_eq!(
            report.render(false, false),
            "Doctor summary (imece 0.1.5, checkout ~/code/shelf)\n\
             \n\
             [✓] imece: 0.1.5, state in ~/.local/state/imece\n\
             [✗] Claude Code\n\
             \x20   ✓ claude --version: 2.1.321 (Claude Code)\n\
             \x20   ✗ not signed in\n\
             \x20     run `claude auth login`\n\
             \x20   ✓ takes every option imece passes\n\
             [!] git\n\
             \x20   ✓ git version 2.56.0\n\
             \x20   ! ~/code/shelf is on no branch (a detached HEAD)\n\
             \x20     agents work as ever; :lonca import needs a branch (`git switch NAME`)\n\
             [✓] Terminal: iterm2 3.6.6, 120×40\n\
             [✓] Your files: ^x ^e opens nvim\n\
             \n\
             ✗ Doctor found issues in 2 categories.\n"
        );
        // Warnings alone are issues, and no failure.
        facts.claude.seen = well().claude.seen;
        let report = super::report(Lang::En, &facts);
        assert_eq!((report.issues(), report.failed()), (1, false));
        let said = report.render(false, false);
        assert!(
            said.ends_with("\n! Doctor found issues in 1 category.\n"),
            "{said}"
        );
    }

    #[test]
    fn the_marks_are_coloured_only_when_asked() {
        let mut facts = well();
        facts.imece.dir = Dir::Closed;
        let report = report(Lang::En, &facts);
        let plain = report.render(false, false);
        assert!(!plain.contains('\x1b'));
        let colour = report.render(false, true);
        assert!(colour.contains("\x1b[31m[✗]\x1b[0m imece\n"), "{colour:?}");
        assert!(colour.contains("    \x1b[32m✓\x1b[0m imece 0.1.5\n"));
        assert!(colour.contains("\x1b[32m[✓]\x1b[0m Claude Code: "));
        assert!(colour.ends_with("\x1b[31m✗\x1b[0m Doctor found issues in 1 category.\n"));
    }

    #[test]
    fn imece_s_own_state_is_judged() {
        let t = &EN.doctor;
        assert_eq!(found(&well(), "imece.state").mark, Mark::Ok);
        let new = after("imece.state", |f| f.imece.dir = Dir::New);
        assert_eq!(new, (Mark::Ok, None));
        let closed = after("imece.state", |f| f.imece.dir = Dir::Closed);
        assert_eq!(closed, (Mark::Fail, fixed(t.state_fix)));
        // Space: said from under a gibibyte, not at it.
        let space = |free| after("imece.space", |f| f.imece.free = Some(free));
        assert_eq!(space(SPACE_LOW - 1), (Mark::Warn, fixed(t.space_fix)));
        assert_eq!(space(SPACE_LOW), (Mark::Ok, None));
        let mut facts = well();
        facts.imece.free = Some(3 << 29);
        assert_eq!(found(&facts, "imece.space").text, "1.5 GB free there");
        facts.imece.free = Some(300 << 20);
        assert!(
            found(&facts, "imece.space")
                .text
                .starts_with("300 MB free where")
        );
        // The store: none yet, an older one, this one's, a newer one.
        let knows = store::SCHEMA_VERSION;
        let store = |kept: Kept| {
            let mut facts = well();
            facts.imece.store = kept;
            found(&facts, "imece.store")
        };
        assert_eq!(store(Kept::None).mark, Mark::Ok);
        assert_eq!(
            store(Kept::At(knows)).text,
            format!("store at schema {knows}")
        );
        let older = store(Kept::At(knows - 1));
        assert_eq!(older.mark, Mark::Ok);
        assert!(
            older
                .text
                .ends_with(&format!("brings it to {knows} at its next start"))
        );
        let newer = store(Kept::At(knows + 1));
        assert_eq!(
            (newer.mark, newer.fix),
            (Mark::Fail, fixed(t.store_newer_fix))
        );
        assert!(
            newer
                .text
                .contains(&format!("schema {}, this one knows {knows}", knows + 1))
        );
        let bad = store(Kept::Unreadable("not a database".into()));
        assert_eq!((bad.mark, bad.fix), (Mark::Fail, fixed(t.store_bad_fix)));
        assert_eq!(
            bad.text,
            "~/.local/state/imece/imece.sqlite can not be read: not a database"
        );
        // The lock.
        let report = report(Lang::En, &well());
        let ids =
            |r: &Report| -> Vec<&str> { r.categories[0].items.iter().map(|i| i.id).collect() };
        assert!(
            !ids(&report).contains(&"imece.lock"),
            "nobody's is not said"
        );
        let other = after("imece.lock", |f| f.imece.lock = Lock::Other);
        assert_eq!(other, (Mark::Warn, fixed(t.lock_held_fix)));
        assert_eq!(
            after("imece.lock", |f| f.imece.lock = Lock::Own),
            (Mark::Ok, None)
        );
        // Worktrees are a note, said only when there are some.
        assert!(!ids(&report).contains(&"imece.worktrees"));
        let mut facts = well();
        facts.imece.trees = 3;
        let trees = found(&facts, "imece.worktrees");
        assert_eq!(trees.mark, Mark::Note);
        assert!(
            trees
                .text
                .starts_with("worktrees kept under ~/.local/state/imece: 3 ")
        );
    }

    #[test]
    fn claude_code_is_judged() {
        let t = &EN.doctor;
        let missing = after("claude.program", |f| f.claude.seen = Seen::Missing);
        assert_eq!(missing, (Mark::Fail, fixed(t.claude_missing_fix)));
        // Not there: nothing more is said of it.
        let mut facts = well();
        facts.claude.seen = Seen::Missing;
        let report = report(Lang::En, &facts);
        assert_eq!(report.categories[1].items.len(), 1);
        assert!(report.failed());
        let login = |login| {
            after("claude.login", |f| {
                f.claude.seen = Seen::Here {
                    version: "2".into(),
                    login,
                }
            })
        };
        assert_eq!(login(Login::Yes { subscription: true }), (Mark::Ok, None));
        assert_eq!(
            login(Login::Yes {
                subscription: false
            }),
            (Mark::Ok, None)
        );
        assert_eq!(
            login(Login::No),
            (Mark::Fail, fixed("run `claude auth login`"))
        );
        assert_eq!(
            login(Login::Unknown),
            (Mark::Warn, fixed(t.login_unknown_fix))
        );
        let key = |subscription| {
            let mut facts = well();
            facts.claude.seen = Seen::Here {
                version: "2".into(),
                login: Login::Yes { subscription },
            };
            found(&facts, "claude.login").text
        };
        assert_eq!(key(false), t.login_key);
        assert_eq!(key(true), t.login_plan);
        // The options.
        assert_eq!(found(&well(), "claude.flags").mark, Mark::Ok);
        assert_eq!(
            after("claude.flags", |f| f.claude.flags = None),
            (Mark::Warn, None)
        );
        let mut facts = well();
        facts.claude.flags = Some(vec!["--restricted".into(), "--tools".into()]);
        let flags = found(&facts, "claude.flags");
        assert_eq!(
            (flags.mark, flags.fix),
            (Mark::Warn, fixed(t.flags_missing_fix))
        );
        assert!(
            flags.text.ends_with(": --restricted --tools"),
            "{}",
            flags.text
        );
    }

    #[test]
    fn git_and_the_folder_are_judged() {
        let t = &EN.doctor;
        let none = after("git.program", |f| f.git.version = None);
        assert_eq!(none, (Mark::Fail, fixed("install git (2.35 or newer)")));
        let version = |line: &str| {
            let mut facts = well();
            facts.git.version = Some(line.to_string());
            found(&facts, "git.program")
        };
        // The floor itself is new enough; one under it is said.
        assert_eq!(version("git version 2.35.0").mark, Mark::Ok);
        assert_eq!(version("git version 3.0.1").mark, Mark::Ok);
        let old = version("git version 2.34.9 (Apple Git-1)");
        assert_eq!((old.mark, old.fix), (Mark::Warn, fixed(t.git_old_fix)));
        assert!(old.text.starts_with("git 2.34: "), "{}", old.text);
        assert!(old.text.ends_with("needs 2.35 or newer"), "{}", old.text);
        assert_eq!(version("git version 1.99.0").mark, Mark::Warn);
        // What is no version is shown as it was said.
        assert_eq!(version("git, probably").mark, Mark::Ok);
        assert_eq!(
            git_numbers("git version 2.39.5 (Apple Git-154)"),
            Some((2, 39))
        );
        assert_eq!(git_numbers("git version 2"), None);
        assert_eq!(git_numbers(""), None);
        // The folder.
        let folder = |checkout| after("git.checkout", |f| f.git.checkout = checkout);
        let bad = folder(Checkout::Bad("~/x".into(), "no such file".into()));
        assert_eq!(bad, (Mark::Fail, fixed(t.co_bad_fix)));
        let no_git = folder(Checkout::NoGit("~/x".into()));
        assert_eq!(no_git, (Mark::Warn, fixed(t.co_no_git_fix)));
        let unborn = folder(Checkout::Unborn("~/x".into()));
        assert_eq!(unborn, (Mark::Warn, fixed(t.co_unborn_fix)));
        // Its include file.
        let include = |named: copy::Included| {
            let mut facts = well();
            facts.git.checkout = Checkout::Here {
                top: "~/x".into(),
                branch: Some("main".into()),
                include: Some(named),
            };
            found(&facts, "git.include")
        };
        let ok = include(copy::Included {
            paths: vec![".env".into(), "target".into()],
            refused: Vec::new(),
        });
        assert_eq!(ok.mark, Mark::Ok);
        assert!(ok.text.starts_with(".worktreeinclude: 2 ignored paths"));
        let line = include(copy::Included {
            paths: vec![".env".into()],
            refused: vec![copy::Refused {
                line: Some((3, "../x".into())),
                why: "it leaves the checkout".into(),
            }],
        });
        assert_eq!((line.mark, line.fix), (Mark::Warn, fixed(t.include_fix)));
        assert_eq!(
            line.text,
            ".worktreeinclude line 3 is not followed: it leaves the checkout"
        );
        let whole = include(copy::Included {
            paths: Vec::new(),
            refused: vec![copy::Refused {
                line: None,
                why: "too big".into(),
            }],
        });
        assert_eq!(whole.text, ".worktreeinclude is not followed: too big");
    }

    #[test]
    fn the_tests_found_are_said_and_none_is_a_hint() {
        let t = &EN.doctor;
        assert_eq!(
            found(&well(), "tests.found").text,
            "found: cargo test … · go test …"
        );
        let none = after("tests.found", |f| f.tests = Some(Ok(Vec::new())));
        assert_eq!(none, (Mark::Warn, fixed(t.tests_none_fix)));
        let unread = after("tests.found", |f| f.tests = Some(Err("git failed".into())));
        assert_eq!(unread, (Mark::Warn, None));
    }

    #[test]
    fn lonca_is_judged_by_whether_the_project_is_on_it() {
        let t = &EN.doctor;
        let reach = |repo: Option<&str>, reach| {
            let mut facts = well();
            let lonca = facts.lonca.as_mut().unwrap();
            lonca.repo = repo.map(str::to_string);
            lonca.reach = reach;
            facts
        };
        // A daemon that is not there closes a project on Lonca; with no
        // project on it, it is only said.
        let down = found(
            &reach(Some("shelf"), Err(StoreDown::NoDaemon)),
            "lonca.daemon",
        );
        assert_eq!(
            (down.mark, down.fix),
            (Mark::Fail, fixed(t.lonca_fix_daemon))
        );
        assert_eq!(down.text, EN.store_no_daemon);
        let offered = found(&reach(None, Err(StoreDown::NoDaemon)), "lonca.daemon");
        assert_eq!(
            (offered.mark, offered.fix),
            (Mark::Note, fixed(t.lonca_fix_daemon))
        );
        let unused = report(Lang::En, &reach(None, Err(StoreDown::NoDaemon)));
        assert_eq!(unused.issues(), 0);
        let line = "[✓] Lonca: lonca 0.2.2, this checkout is not on Lonca\n";
        assert!(unused.render(false, false).contains(line));
        assert!(
            report(Lang::En, &reach(None, Ok(None)))
                .render(false, false)
                .contains(line)
        );
        let out = found(&reach(None, Err(StoreDown::SignedOut)), "lonca.daemon");
        assert_eq!(out.fix, fixed(t.lonca_fix_login));
        let old = found(&reach(None, Err(StoreDown::Old)), "lonca.daemon");
        assert_eq!(old.fix, fixed(t.lonca_old_fix));
        let hard = found(&reach(None, Err(StoreDown::Hardened)), "lonca.daemon");
        assert_eq!((hard.mark, hard.fix), (Mark::Note, None));
        let hard = found(
            &reach(Some("shelf"), Err(StoreDown::Hardened)),
            "lonca.daemon",
        );
        assert_eq!((hard.mark, hard.fix), (Mark::Fail, None));
        // Reached, with no repository named: a note of how to get one.
        let off = found(&reach(None, Ok(None)), "lonca.repo");
        assert_eq!((off.mark, off.text.as_str()), (Mark::Note, t.lonca_off));
        // The repository.
        let repo = |seen| found(&reach(Some("shelf"), Ok(Some(seen))), "lonca.repo");
        let fine = Repo {
            frozen: false,
            guarded: Some(true),
            checks: None,
        };
        assert_eq!(repo(Ok(fine)).text, "repository shelf");
        let unread = repo(Ok(Repo {
            guarded: None,
            ..fine
        }));
        assert_eq!(
            unread.mark,
            Mark::Ok,
            "a policy not read is not called unguarded"
        );
        let frozen = repo(Ok(Repo {
            frozen: true,
            ..fine
        }));
        assert_eq!(frozen.mark, Mark::Fail);
        assert_eq!(
            frozen.text,
            format!("repository shelf: {}", EN.store_frozen)
        );
        let open = repo(Ok(Repo {
            guarded: Some(false),
            ..fine
        }));
        assert_eq!(open.mark, Mark::Fail);
        assert!(open.text.ends_with(EN.store_unguarded), "{}", open.text);
        let gone = repo(Err(StoreDown::NoRepo));
        assert_eq!((gone.mark, gone.fix), (Mark::Fail, None));
        assert!(gone.text.ends_with(EN.store_no_repo));
        let out = repo(Err(StoreDown::SignedOut));
        assert_eq!(out.fix, fixed(t.lonca_fix_login));
        // Its checks are a note, when Lonca said.
        let checks = found(&well(), "lonca.checks");
        assert_eq!(checks.mark, Mark::Note);
        assert_eq!(checks.text, "its policy defines 2 checks of its own");
        let report = report(Lang::En, &reach(Some("shelf"), Ok(Some(Ok(fine)))));
        let lonca = report.categories.iter().find(|c| c.id == "lonca").unwrap();
        assert!(lonca.items.iter().all(|i| i.id != "lonca.checks"));
        // The program: missing with a project on it, or too old.
        let program = |version: Option<&str>, repo: Option<&str>| {
            let mut facts = reach(repo, Ok(None));
            facts.lonca.as_mut().unwrap().version = version.map(str::to_string);
            found(&facts, "lonca.program")
        };
        let missing = program(None, Some("shelf"));
        assert_eq!(
            (missing.mark, missing.fix),
            (Mark::Fail, fixed(t.lonca_missing_fix))
        );
        let least = lonca::least();
        assert_eq!(
            program(Some(&format!("lonca {least}")), None).mark,
            Mark::Ok
        );
        let old = program(Some("lonca 0.1.9"), None);
        assert_eq!((old.mark, old.fix), (Mark::Note, fixed(t.lonca_old_fix)));
        assert_eq!(
            old.text,
            format!("lonca 0.1.9 is older than Lonca {least}, which imece needs")
        );
        assert_eq!(program(Some("lonca 0.1.9"), Some("shelf")).mark, Mark::Fail);
        // The mode is said only when the install says cooperative.
        assert_eq!(found(&well(), "lonca.mode").text, t.lonca_coop);
        for hardened in [None, Some(true)] {
            let mut facts = well();
            facts.lonca.as_mut().unwrap().hardened = hardened;
            let report = super::report(Lang::En, &facts);
            let lonca = report.categories.iter().find(|c| c.id == "lonca").unwrap();
            assert!(lonca.items.iter().all(|i| i.id != "lonca.mode"));
        }
    }

    #[test]
    fn the_terminal_is_told_and_only_too_small_a_one_is_an_issue() {
        let t = &EN.doctor;
        let mut facts = well();
        facts.terminal = None;
        let none = found(&facts, "terminal.none");
        assert_eq!((none.mark, none.text.as_str()), (Mark::Note, t.term_none));
        assert_eq!(report(Lang::En, &facts).issues(), 0);
        let size = |size| {
            let mut facts = well();
            facts.terminal.as_mut().unwrap().size = Some(size);
            found(&facts, "terminal.size")
        };
        // The floor and the designed size are themselves enough.
        let (floor, designed) = ((ui::MIN_COLUMNS, ui::MIN_ROWS), DESIGNED);
        assert_eq!(size(designed).mark, Mark::Ok);
        assert_eq!(size(designed).text, "100×40");
        assert_eq!(size((designed.0 - 1, designed.1)).mark, Mark::Note);
        assert_eq!(size((designed.0, designed.1 - 1)).mark, Mark::Note);
        assert_eq!(size(floor).mark, Mark::Note);
        assert_eq!(
            size(floor).text,
            "40×12: imece's narrow layout (its screens are designed at 100×40)"
        );
        let narrow = size((floor.0 - 1, 50));
        assert_eq!(
            (narrow.mark, narrow.fix),
            (Mark::Fail, fixed(t.term_size_fix))
        );
        assert_eq!(
            narrow.text,
            "39×50: under 40×12 imece draws one line and waits"
        );
        assert_eq!(size((200, floor.1 - 1)).mark, Mark::Fail);
        // What a terminal lacks is a note.
        let mut facts = well();
        let term = facts.terminal.as_mut().unwrap();
        (term.who, term.keys, term.truecolor, term.progress) = (None, false, false, false);
        (term.tmux, term.ssh, term.notify) = (true, true, Style::Tmux);
        let said = |id: &str| {
            let item = found(&facts, id);
            (item.mark, item.text)
        };
        assert_eq!(said("terminal.name"), (Mark::Note, t.term_unnamed.into()));
        assert_eq!(said("terminal.colour"), (Mark::Note, t.term_256.into()));
        assert_eq!(said("terminal.keys"), (Mark::Note, t.term_no_keys.into()));
        assert_eq!(
            said("terminal.progress"),
            (Mark::Note, t.term_no_progress.into())
        );
        assert_eq!(said("terminal.tmux"), (Mark::Note, t.term_tmux.into()));
        assert_eq!(said("terminal.ssh"), (Mark::Note, t.term_ssh.into()));
        assert_eq!(said("terminal.notices").1, "notices go as OSC 9, tmux");
        assert_eq!(report(Lang::En, &facts).issues(), 0);
        let well = well();
        let has = |id: &str| found(&well, id);
        assert_eq!(has("terminal.colour").text, t.term_true);
        assert_eq!(has("terminal.keys").text, t.term_keys);
        assert_eq!(has("terminal.progress").text, t.term_progress);
        assert_eq!(has("terminal.notices").text, "notices go as OSC 9");
        let style = |notify| {
            let mut facts = super::tests::well();
            facts.terminal.as_mut().unwrap().notify = notify;
            found(&facts, "terminal.notices").text
        };
        assert!(style(Style::Kitty).ends_with("OSC 99"));
        assert!(style(Style::Osc777).ends_with("OSC 777"));
        let named = Who {
            name: "foot".into(),
            version: None,
        };
        assert_eq!(who_words(&named), "foot");
    }

    #[test]
    fn the_person_s_files_are_judged() {
        let t = &EN.doctor;
        let instructions = |what| {
            after("files.instructions", |f| {
                f.files.instructions = Some(("~/i.md".into(), what))
            })
        };
        assert_eq!(instructions(Plain::File(1)).0, Mark::Ok);
        assert_eq!(instructions(Plain::Missing).0, Mark::Note);
        assert_eq!(instructions(Plain::Other).0, Mark::Warn);
        assert_eq!(
            found(&well(), "files.instructions").text,
            "~/.config/imece/instructions.md reaches every agent (120 bytes)"
        );
        // The key file.
        assert_eq!(found(&well(), "files.keys").mark, Mark::Note);
        let keys_read = |read| {
            let mut facts = well();
            facts.files.keys = Some(("/k/keys".into(), read));
            found(&facts, "files.keys")
        };
        let map = keys::Keymap::read("x = j\ny = k\n").unwrap();
        let good = keys_read(keys::Read::Good(map, String::new()));
        assert_eq!(
            (good.mark, good.text.as_str()),
            (Mark::Ok, "/k/keys: 2 keys of yours")
        );
        let bad = keys_read(keys::Read::Bad(keys::Why::TooBig));
        assert_eq!((bad.mark, bad.fix), (Mark::Warn, fixed(t.file_keys_fix)));
        assert!(bad.text.starts_with("/k/keys: "), "{}", bad.text);
        // Push: its server is said, never its topic.
        assert_eq!(found(&well(), "files.push").mark, Mark::Note);
        let push_read = |read| {
            let mut facts = well();
            facts.files.push.1 = read;
            found(&facts, "files.push")
        };
        let set = push_read(Ok("https://ntfy.example".into()));
        assert_eq!(set.mark, Mark::Ok);
        assert_eq!(
            set.text,
            "~/.config/imece/push: notices can go to https://ntfy.example"
        );
        let refused = push_read(Err(push::Why::NoTopic));
        assert_eq!(
            (refused.mark, refused.fix),
            (Mark::Warn, fixed(t.file_push_fix))
        );
        assert_eq!(refused.text, EN.push_no_topic);
        // The editor.
        let editor = |set, found_it| {
            let mut facts = well();
            (facts.files.editor.set, facts.files.editor.found) = (set, found_it);
            found(&facts, "files.editor")
        };
        assert_eq!(editor(true, true).text, "^x ^e opens nvim");
        assert!(
            editor(false, true)
                .text
                .ends_with("(VISUAL and EDITOR are not set)")
        );
        assert_eq!(editor(false, true).mark, Mark::Ok);
        for set in [true, false] {
            let missing = editor(set, false);
            assert_eq!(
                (missing.mark, missing.fix),
                (Mark::Warn, fixed(t.file_editor_fix))
            );
        }
        // What holds the machine's sleep.
        let awake = |awake| {
            let mut facts = well();
            facts.files.awake = awake;
            found(&facts, "files.awake")
        };
        assert_eq!(awake(Some(("caffeinate".into(), true))).mark, Mark::Ok);
        let missing = awake(Some(("systemd-inhibit".into(), false)));
        assert_eq!(missing.mark, Mark::Note);
        assert!(missing.text.starts_with("systemd-inhibit is not on PATH"));
        assert_eq!(awake(None).text, t.file_awake_os);
    }

    #[test]
    fn what_a_program_said_can_not_drive_the_terminal() {
        let mut facts = well();
        facts.claude.seen = Seen::Here {
            version: "2.0\x1b[2J\x1b]0;owned\x07".into(),
            login: Login::Yes { subscription: true },
        };
        facts.git.version = Some("git \x1b[31mversion 2.56.0".into());
        facts.lonca.as_mut().unwrap().version = Some("lonca\x1b[0m 0.2.2".into());
        facts.tests = Some(Ok(vec!["go\x07 test".into()]));
        for verbose in [false, true] {
            let said = report(Lang::En, &facts).render(verbose, false);
            assert!(
                !said.chars().any(|c| c.is_control() && c != '\n'),
                "{said:?}"
            );
        }
    }

    #[test]
    fn both_languages_find_the_same_and_say_it_in_their_words() {
        let mut facts = well();
        facts.claude.seen = Seen::Missing;
        facts.imece.lock = Lock::Other;
        let (en, tr) = (report(Lang::En, &facts), report(Lang::Tr, &facts));
        let shape = |r: &Report| -> Vec<(&str, Mark, bool)> {
            let items = r.categories.iter().flat_map(|c| &c.items);
            items.map(|i| (i.id, i.mark, i.fix.is_some())).collect()
        };
        assert_eq!(shape(&en), shape(&tr));
        assert_eq!(tr.foot, "Doktor 2 başlıkta sorun buldu.");
        assert_eq!(
            tr.categories[1].items[0].text,
            crate::i18n::TR.doctor.claude_missing
        );
        assert!(
            tr.head.starts_with("Doktor özeti (imece 0.1.5, "),
            "{}",
            tr.head
        );
    }

    #[test]
    fn json_names_every_finding_and_says_whether_it_failed() {
        let mut facts = well();
        facts.claude.seen = Seen::Here {
            version: "2".into(),
            login: Login::No,
        };
        let said = report(Lang::En, &facts).json();
        assert_eq!(said["imece"], crate::welcome::VERSION);
        assert_eq!(
            (&said["issues"], &said["failed"]),
            (&json!(1), &json!(true))
        );
        let categories = said["categories"].as_array().unwrap();
        let ids: Vec<&str> = categories
            .iter()
            .map(|c| c["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            ids,
            [
                "imece", "claude", "git", "tests", "lonca", "terminal", "files"
            ]
        );
        let claude = &categories[1];
        assert_eq!(
            (&claude["title"], &claude["status"]),
            (&json!("Claude Code"), &json!("fail"))
        );
        let login = &claude["items"][1];
        assert_eq!(
            login,
            &json!({"id": "claude.login", "status": "fail", "text": "not signed in",
                    "fix": "run `claude auth login`"})
        );
        assert_eq!(claude["items"][0]["fix"], Value::Null);
        assert_eq!(categories[0]["status"], "ok");
        // A note is its own status on a finding, and no issue of its category.
        let files = &categories[6];
        assert_eq!(files["status"], "ok");
        assert!(
            files["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|i| i["status"] == "note")
        );
        let warned = report(Lang::En, &{
            let mut f = well();
            f.imece.lock = Lock::Other;
            f
        })
        .json();
        assert_eq!(
            (&warned["issues"], &warned["failed"]),
            (&json!(1), &json!(false))
        );
        assert_eq!(warned["categories"][0]["status"], "warn");
    }

    #[test]
    fn the_command_line_takes_its_four_options_and_refuses_the_rest() {
        let parse = |argv: &[&str], lang| {
            let argv: Vec<String> = argv.iter().map(|a| a.to_string()).collect();
            super::parse(&argv, lang)
        };
        let plain = parse(&[], None).unwrap();
        assert_eq!(
            plain,
            Asked {
                repo: PathBuf::from("."),
                lang: Lang::En,
                verbose: false,
                json: false
            }
        );
        let all = parse(&["--repo", "/x", "--json", "-v", "--lang", "tr"], None).unwrap();
        assert_eq!((all.repo, all.lang), (PathBuf::from("/x"), Lang::Tr));
        assert!(all.verbose && all.json);
        assert!(parse(&["--verbose"], None).unwrap().verbose);
        // The environment's language, which the flag wins over.
        assert_eq!(parse(&[], Some("tr")).unwrap().lang, Lang::Tr);
        assert_eq!(parse(&["--lang", "en"], Some("tr")).unwrap().lang, Lang::En);
        assert_eq!(
            parse(&[], Some("xx")).unwrap_err(),
            "IMECE_LANG: \"xx\" is not en or tr"
        );
        assert_eq!(
            parse(&["--lang", "xx"], None).unwrap_err(),
            "--lang: \"xx\" is not en or tr"
        );
        let usage = EN.doctor.usage;
        assert_eq!(parse(&["-h"], None).unwrap_err(), usage);
        assert_eq!(
            parse(&["--help"], Some("tr")).unwrap_err(),
            crate::i18n::TR.doctor.usage
        );
        assert_eq!(
            parse(&["--repo"], None).unwrap_err(),
            format!("--repo needs a value\n\n{usage}")
        );
        assert_eq!(
            parse(&["--fix"], None).unwrap_err(),
            format!("unknown option --fix\n\n{usage}")
        );
    }

    #[test]
    fn colour_goes_only_to_a_terminal_that_did_not_say_no() {
        let env = |pairs: &'static [(&str, &str)]| {
            move |k: &str| pairs.iter().find(|p| p.0 == k).map(|p| p.1.to_string())
        };
        assert!(coloured(true, env(&[])));
        assert!(coloured(
            true,
            env(&[("TERM", "xterm-256color"), ("NO_COLOR", "")])
        ));
        assert!(!coloured(false, env(&[])));
        assert!(!coloured(true, env(&[("NO_COLOR", "1")])));
        assert!(!coloured(true, env(&[("TERM", "dumb")])));
    }

    #[test]
    fn a_size_is_megabytes_then_gigabytes_with_a_tenth() {
        assert_eq!(size_words(0), "0 MB");
        assert_eq!(size_words((1 << 30) - 1), "1023 MB");
        assert_eq!(size_words(1 << 30), "1.0 GB");
        assert_eq!(size_words(5 * (1 << 30) + (1 << 29)), "5.5 GB");
        assert_eq!(size_words(40 << 30), "40.0 GB");
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("imece-doctor-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.canonicalize().unwrap()
    }

    #[test]
    fn a_program_is_found_as_a_shell_finds_it() {
        use std::os::unix::fs::PermissionsExt;
        let d = dir("path");
        let (bin, other) = (d.join("bin"), d.join("other"));
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let tool = bin.join("tool");
        std::fs::write(&tool, "#!/bin/sh\n").unwrap();
        std::fs::write(bin.join("notes"), "text").unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::set_permissions(bin.join("notes"), std::fs::Permissions::from_mode(0o644))
            .unwrap();
        std::fs::create_dir_all(bin.join("folder")).unwrap();
        let path = std::env::join_paths([&other, &bin]).unwrap();
        let on = |program: &str| on_path_in(program, Some(&path));
        assert!(on("tool"));
        assert!(!on("notes"), "not to be run");
        assert!(!on("folder"), "a directory");
        assert!(!on("missing"));
        assert!(!on_path_in("tool", None));
        assert!(!on_path_in("tool", Some(other.as_os_str())));
        // A path is looked at as it is, PATH or none.
        assert!(on_path_in(tool.to_str().unwrap(), None));
        assert!(!on(bin.join("notes").to_str().unwrap()));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_state_directory_is_there_to_be_made_or_closed() {
        use std::os::unix::fs::PermissionsExt;
        let d = dir("state");
        let (seen, free) = dir_seen(&d);
        assert_eq!(seen, Dir::There);
        assert!(free.is_some_and(|f| f > 0));
        // Not there yet, two levels down: judged at the nearest there is.
        assert_eq!(dir_seen(&d.join("a/b")).0, Dir::New);
        assert!(dir_seen(&d.join("a/b")).1.is_some());
        assert!(!d.join("a").exists(), "nothing is made");
        // A file in the way, or a directory not to be written to.
        std::fs::write(d.join("file"), "x").unwrap();
        assert_eq!(dir_seen(&d.join("file")).0, Dir::Closed);
        assert_eq!(dir_seen(&d.join("file/under")).0, Dir::Closed);
        let closed = d.join("closed");
        std::fs::create_dir_all(&closed).unwrap();
        std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o500)).unwrap();
        // Root may write anywhere: nothing to tell then.
        // SAFETY: geteuid has no failure and touches nothing.
        if unsafe { libc::geteuid() } != 0 {
            assert_eq!(dir_seen(&closed).0, Dir::Closed);
            assert_eq!(dir_seen(&closed.join("under")).0, Dir::Closed);
        }
        std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o700)).unwrap();
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn worktrees_are_counted_a_directory_per_agent_under_one_per_checkout() {
        let d = dir("trees");
        assert_eq!(trees_under(&d), 0);
        for tree in ["shelf-1/claude-1", "shelf-1/claude-2", "tally-2/solo-tally"] {
            std::fs::create_dir_all(d.join("worktrees").join(tree)).unwrap();
        }
        // Files, and directories elsewhere, are no worktrees.
        std::fs::write(d.join("worktrees/shelf-1/note"), "x").unwrap();
        std::fs::write(d.join("worktrees/file"), "x").unwrap();
        std::fs::create_dir_all(d.join("worktrees-scratch/claude-1")).unwrap();
        assert_eq!(trees_under(&d), 3);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn imece_s_state_is_read_without_making_or_taking_it() {
        let d = dir("gather");
        let state = d.join("state");
        let (seen, peek) = gather_imece(&state, false);
        assert_eq!(
            (seen.dir, &seen.store, seen.lock),
            (Dir::New, &Kept::None, Lock::Free)
        );
        assert_eq!(seen.version, crate::welcome::VERSION);
        assert!(peek.is_none() && !state.exists(), "nothing was made");
        // A store another imece holds.
        let file = state.join(store::FILE);
        let held = store::Store::open(&file).unwrap();
        let (seen, peek) = gather_imece(&state, false);
        assert_eq!(seen.dir, Dir::There);
        assert_eq!(seen.store, Kept::At(store::SCHEMA_VERSION));
        assert_eq!(seen.lock, Lock::Other);
        assert!(peek.is_some_and(|p| p.lonca.is_empty()));
        assert!(seen.file.ends_with("state/imece.sqlite"), "{}", seen.file);
        // Asked from inside the imece that holds it.
        assert_eq!(gather_imece(&state, true).0.lock, Lock::Own);
        drop(held);
        assert_eq!(gather_imece(&state, false).0.lock, Lock::Free);
        assert_eq!(gather_imece(&state, true).0.lock, Lock::Own);
        // What is no store.
        std::fs::write(&file, "not a database, whatever its name says it is").unwrap();
        let _ = std::fs::remove_file(state.join("imece.sqlite-wal"));
        let _ = std::fs::remove_file(state.join("imece.sqlite-shm"));
        assert!(matches!(
            gather_imece(&state, false).0.store,
            Kept::Unreadable(_)
        ));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_folder_is_told_from_a_checkout_and_one_without_a_commit() {
        let d = dir("checkout");
        let missing = d.join("missing");
        let (seen, top) = gather_checkout(&missing);
        assert!(matches!(seen, Checkout::Bad(..)), "{seen:?}");
        assert!(top.is_none());
        let (seen, top) = gather_checkout(&d);
        assert!(matches!(seen, Checkout::NoGit(_)), "{seen:?}");
        assert!(top.is_none());
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(&d)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .output()
                .unwrap();
            assert!(out.status.success(), "{args:?}: {out:?}");
        };
        git(&["init", "-q", "-b", "main"]);
        let (seen, top) = gather_checkout(&d);
        assert!(matches!(seen, Checkout::Unborn(_)), "{seen:?}");
        assert!(top.is_none());
        std::fs::write(d.join("go.mod"), "module x\n").unwrap();
        std::fs::write(d.join("Cargo.toml"), "[package]\n").unwrap();
        git(&["add", "."]);
        let commit = [
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "first",
        ];
        git(&commit);
        let (seen, top) = gather_checkout(&d);
        let Checkout::Here {
            branch, include, ..
        } = seen
        else {
            panic!("{seen:?}");
        };
        assert_eq!(branch.as_deref(), Some("main"));
        assert!(include.is_none());
        assert_eq!(top.as_deref(), Some(d.as_path()));
        // Its tests: cargo's first, then what its files call for.
        assert_eq!(gather_tests(&d).unwrap(), ["cargo test …", "go test …"]);
        // Off a branch, and with an include file.
        git(&["checkout", "-q", "--detach"]);
        std::fs::write(d.join(copy::INCLUDE), "").unwrap();
        let (seen, _) = gather_checkout(&d);
        let Checkout::Here {
            branch, include, ..
        } = seen
        else {
            panic!("{seen:?}");
        };
        assert_eq!(branch, None);
        assert_eq!(include, Some(copy::Included::default()));
        let _ = std::fs::remove_dir_all(&d);
    }
}
