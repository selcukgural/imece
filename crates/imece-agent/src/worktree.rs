//! One git worktree per agent: outside the person's checkout, detached at
//! its HEAD, kept between runs so an agent's work is never thrown away. A
//! worktree found clean at an older commit is moved to the checkout's HEAD
//! when the agent starts, so an agent never works on what was merged
//! already; one with changes or commits of its own stays where it is.
//!
//! What an agent changed is read as one patch ([`change`]) for the person
//! to see, applied to their checkout ([`apply`]) when they take it, and
//! then committed in the worktree ([`applied`]) so the next patch holds
//! only what came after. Commits imece makes this way are its own: a
//! worktree whose only commits are imece's still follows the checkout.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::child;

/// The author of the commits imece makes in a worktree for a change the
/// person applied; what tells them from an agent's or the person's own.
const IMECE_EMAIL: &str = "imece@localhost";

/// Runs git in `dir` with `env` set, and gives its stdout as it is. Git
/// speaks English here, so its words can be read.
fn git_bytes(dir: &Path, env: &[(&str, &str)], args: &[&str]) -> io::Result<Vec<u8>> {
    git_fed(dir, env, args, None)
}

/// [`git_bytes`] with `input`, when there is one, on git's stdin.
fn git_fed(
    dir: &Path,
    env: &[(&str, &str)],
    args: &[&str],
    input: Option<&[u8]>,
) -> io::Result<Vec<u8>> {
    let mut cmd = child::command("git", dir, &[]);
    cmd.args(args).env("LC_ALL", "C");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = match input {
        None => cmd.output()?,
        Some(input) => {
            let mut child = cmd
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            if let Some(mut stdin) = child.stdin.take() {
                // A closed pipe is git's refusal to report, not ours.
                let _ = stdin.write_all(input);
            }
            child.wait_with_output()?
        }
    };
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(io::Error::other(format!(
            "git {}: {}",
            args.join(" "),
            err.lines().next().unwrap_or("failed")
        )));
    }
    Ok(out.stdout)
}

fn git_env(dir: &Path, env: &[(&str, &str)], args: &[&str]) -> io::Result<String> {
    let out = git_bytes(dir, env, args)?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

fn git(dir: &Path, args: &[&str]) -> io::Result<String> {
    git_env(dir, &[], args)
}

/// The files of the checkout `repo` as git lists them: the ones it
/// tracks and the untracked ones it does not ignore, relative to `repo`,
/// sorted, each once, at most `max`; whether more were cut.
pub fn files(repo: &Path, max: usize) -> io::Result<(Vec<String>, bool)> {
    let out = git_bytes(
        repo,
        &[],
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    )?;
    let mut files: Vec<String> = String::from_utf8_lossy(&out)
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect();
    files.sort();
    files.dedup();
    let cut = files.len() > max;
    files.truncate(max);
    Ok((files, cut))
}

/// The top directory of the checkout `dir` is in.
pub fn top(dir: &Path) -> io::Result<PathBuf> {
    PathBuf::from(git(dir, &["rev-parse", "--show-toplevel"])?).canonicalize()
}

/// Whether the checkout `dir` is in has a first commit: what a worktree
/// is made from. A `git init` with nothing committed yet has none.
pub fn born(dir: &Path) -> bool {
    git(dir, &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"]).is_ok()
}

fn common_dir(dir: &Path) -> io::Result<PathBuf> {
    let d = git(dir, &["rev-parse", "--git-common-dir"])?;
    dir.join(d).canonicalize()
}

/// The branch the checkout `dir` is on and its commit; none on a detached
/// HEAD, or without a first commit.
pub fn on_branch(dir: &Path) -> Option<(String, String)> {
    let branch = git(dir, &["symbolic-ref", "--quiet", "--short", "HEAD"]).ok()?;
    let head = git(dir, &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"]).ok()?;
    Some((branch, head))
}

/// How a checkout stood to the commit it was brought up to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Forward {
    /// It was there already.
    Even,
    /// Its branch moved forward.
    Moved,
    /// It has commits of its own: nothing moved.
    Apart,
}

/// Brings the branch the checkout `dir` is on forward to `to`, when that is
/// a fast-forward. Anything else is left as it is: a checkout with commits
/// of its own is `Apart`, and work in the way is git's refusal.
pub fn fast_forward(dir: &Path, to: &str) -> io::Result<Forward> {
    let head = git(dir, &["rev-parse", "HEAD"])?;
    let target = git(dir, &["rev-parse", to])?;
    if head == target {
        return Ok(Forward::Even);
    }
    if git(dir, &["merge-base", "HEAD", to]).ok().as_ref() != Some(&head) {
        return Ok(Forward::Apart);
    }
    git(dir, &["merge", "--ff-only", "--quiet", to])?;
    Ok(Forward::Moved)
}

/// The commit of the checkout `dir` nearest to `reference`: where HEAD left
/// it, else HEAD, when there is no such reference yet; none without a
/// first commit.
pub fn near(dir: &Path, reference: &str) -> Option<String> {
    (git(dir, &["merge-base", "HEAD", reference]).ok())
        .or_else(|| git(dir, &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"]).ok())
}

/// Agent names become directory names: lowercase letters, digits and
/// dashes, never a path.
pub fn valid_name(name: &str) -> bool {
    (1..=40).contains(&name.len())
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Why a worktree was left at an older commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// It has changes that are not committed.
    Changes,
    /// It has commits the checkout's HEAD does not.
    Commits,
}

/// Where an ensured worktree stands next to the checkout's HEAD. Commits
/// are named by their short ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Placed {
    /// Made just now, at HEAD.
    Fresh,
    /// Made just now at the start point it was asked for, commit `at`.
    From { at: String },
    /// Found, started from a point of its own: it stays where it is.
    Own,
    /// Found at HEAD already.
    AtHead,
    /// Found clean at an older commit and moved to HEAD.
    Moved { from: String, to: String },
    /// Found with work of its own at `at` and left there.
    Kept { at: String, head: String, why: Why },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub placed: Placed,
}

fn short(sha: &str) -> String {
    sha.chars().take(7).collect()
}

/// The longest start point imece takes.
pub const START_MAX: usize = 200;

/// Whether `start` can name a worker's start point: `#N` (pull request N
/// of `origin`) or a branch, tag or commit as git names it, never an
/// option, a range or more than one word.
pub fn valid_start(start: &str) -> bool {
    if let Some(n) = start.strip_prefix('#') {
        return (1..=9).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_digit());
    }
    (1..=START_MAX).contains(&start.len())
        && !start.starts_with('-')
        && !start.contains("..")
        && !start.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// The commit `start` names in `repo`: `#N` is fetched from `origin`
/// first (`refs/pull/N/head`, with no prompt for a password), anything
/// else must be there already.
pub fn resolve(repo: &Path, start: &str) -> io::Result<String> {
    if !valid_start(start) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{start:?} can not name a start point"),
        ));
    }
    let rev = if let Some(n) = start.strip_prefix('#') {
        git_env(
            repo,
            &[("GIT_TERMINAL_PROMPT", "0")],
            &[
                "fetch",
                "--quiet",
                "--no-tags",
                "origin",
                &format!("refs/pull/{n}/head"),
            ],
        )
        .map_err(|e| io::Error::other(format!("pull request {start} of origin: {e}")))?;
        "FETCH_HEAD".to_string()
    } else {
        start.to_string()
    };
    git(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &format!("{rev}^{{commit}}"),
        ],
    )
    .ok()
    .filter(|sha| !sha.is_empty())
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("{start} names no commit in {}", repo.display()),
        )
    })
}

/// The worktree of agent `name` under `root`, created from `repo`'s HEAD
/// the first time and moved to it on a later start when it is clean. A
/// worker hired from a `start` point of its own is created there and
/// never moved: its work is against that point. A directory that is there
/// but is not a worktree of `repo` is refused, never reused or replaced.
pub fn ensure(repo: &Path, root: &Path, name: &str, start: Option<&str>) -> io::Result<Worktree> {
    if !valid_name(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name:?} can not name a worktree"),
        ));
    }
    let path = root.join(name);
    if path.exists() {
        let path = path.canonicalize()?;
        let same =
            top(&path).is_ok_and(|t| t == path) && common_dir(&path).ok() == common_dir(repo).ok();
        if !same {
            return Err(io::Error::other(format!(
                "{} is there but is not a worktree of {}",
                path.display(),
                repo.display()
            )));
        }
        let placed = match start {
            Some(_) => Placed::Own,
            None => place(repo, &path)?,
        };
        return Ok(Worktree { path, placed });
    }
    let at = start.map(|s| resolve(repo, s)).transpose()?;
    std::fs::create_dir_all(root)?;
    let p = path.to_string_lossy();
    let commit = at.as_deref().unwrap_or("HEAD");
    // A sparse checkout's worktree is made empty and sparse first, so
    // what the person left out is never written.
    let sparse = sparse_of(repo)?;
    let mut add = vec!["worktree", "add", "--quiet", "--detach"];
    if sparse.is_some() {
        add.push("--no-checkout");
    }
    if let Err(e) = git(repo, &[add.as_slice(), &[&p, commit]].concat()) {
        // Removed by hand, still on git's list: git's own words say "use
        // add -f", which would throw away what git knows of it. imece
        // leaves that to the person.
        return Err(if registered(repo, &path) {
            io::Error::other(format!(
                "{} is gone, but git still has it as a worktree of {}: whatever it \
                 held that was not committed is lost; `git worktree prune` there lets \
                 imece make it anew",
                path.display(),
                repo.display()
            ))
        } else {
            e
        });
    }
    if let Some(sparse) = &sparse
        && let Err(e) = make_sparse(&path, sparse)
    {
        // Made just now with nothing in it: nothing is lost.
        let _ = git(repo, &["worktree", "remove", "--force", &p]);
        return Err(io::Error::other(format!(
            "{} is a sparse checkout, and its worktree {} could not be made one too \
             (git 2.35 or newer can): {e}",
            repo.display(),
            path.display()
        )));
    }
    Ok(Worktree {
        path: path.canonicalize()?,
        placed: match at {
            Some(at) => Placed::From { at: short(&at) },
            None => Placed::Fresh,
        },
    })
}

/// What a sparse checkout leaves in: its patterns as `git sparse-checkout
/// list` gives them, and whether they are cone mode's directories.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Sparse {
    cone: bool,
    patterns: Vec<u8>,
}

/// Whether `git config` says `key` is true in `dir`; unset is false.
fn config_true(dir: &Path, key: &str) -> bool {
    git(dir, &["config", "--type=bool", "--get", key]).is_ok_and(|v| v == "true")
}

/// How the checkout `repo` is sparse, if it is.
fn sparse_of(repo: &Path) -> io::Result<Option<Sparse>> {
    if !config_true(repo, "core.sparseCheckout") {
        return Ok(None);
    }
    Ok(Some(Sparse {
        cone: config_true(repo, "core.sparseCheckoutCone"),
        patterns: git_bytes(repo, &[], &["sparse-checkout", "list"])?,
    }))
}

/// Makes the worktree at `path`, added with nothing checked out, as
/// sparse as `sparse` and then checks it out: only what it leaves in is
/// written. Its own patterns, never the checkout's.
fn make_sparse(path: &Path, sparse: &Sparse) -> io::Result<()> {
    let mode = if sparse.cone { "--cone" } else { "--no-cone" };
    git_fed(
        path,
        &[],
        &["sparse-checkout", "set", mode, "--stdin"],
        Some(&sparse.patterns),
    )?;
    git(path, &["reset", "--quiet", "--hard", "HEAD"])?;
    Ok(())
}

/// Whether git lists `path` (which may be gone) as one of `repo`'s
/// worktrees. git keeps the real path, so `path`'s directory is resolved
/// first; when that is gone too, git can not list it as there.
fn registered(repo: &Path, path: &Path) -> bool {
    let Some(real) = (path.parent().and_then(|d| d.canonicalize().ok()))
        .zip(path.file_name())
        .map(|(d, n)| d.join(n))
    else {
        return false;
    };
    git(repo, &["worktree", "list", "--porcelain"]).is_ok_and(|list| {
        list.lines()
            .filter_map(|l| l.strip_prefix("worktree "))
            .any(|p| Path::new(p) == real)
    })
}

/// What a worktree holds that would be lost with it: files changed or
/// not tracked (one line of `git status --porcelain` each; what git
/// ignores is not counted), and commits of its own no branch, tag or
/// remote has, imece's for applied changes left out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Held {
    pub changes: usize,
    pub commits: usize,
}

impl Held {
    /// Whether removing it loses nothing.
    pub fn clean(&self) -> bool {
        self.changes == 0 && self.commits == 0
    }
}

/// The checkout the worktree at `path` belongs to, and what it holds.
/// Refused for what is not a worktree's top directory, and for a
/// checkout's own: only a worktree added to one.
pub fn held(path: &Path) -> io::Result<(PathBuf, Held)> {
    let path = path.canonicalize()?;
    let own = git(&path, &["rev-parse", "--absolute-git-dir"])?;
    if top(&path)? != path || Path::new(&own).canonicalize()? == common_dir(&path)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not a worktree added to a checkout", path.display()),
        ));
    }
    let list = git(&path, &["worktree", "list", "--porcelain"])?;
    let checkout = list
        .lines()
        .find_map(|l| l.strip_prefix("worktree "))
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("git lists no checkout"))?;
    let changes = git(&path, &["status", "--porcelain"])?.lines().count();
    let authors = git(
        &path,
        &[
            "log",
            "--format=%ae",
            "HEAD",
            "--not",
            "--branches",
            "--tags",
            "--remotes",
        ],
    )?;
    let commits = authors.lines().filter(|a| *a != IMECE_EMAIL).count();
    Ok((checkout, Held { changes, commits }))
}

/// Removes the worktree at `path` from its checkout, and its directory
/// with what git ignores in it. Without `force` only a clean one
/// ([`Held::clean`], read again now), which git checks once more;
/// `force` is the person's yes to losing what it holds.
pub fn remove(path: &Path, force: bool) -> io::Result<()> {
    let (_, held) = held(path)?;
    if !force && !held.clean() {
        return Err(io::Error::other(format!(
            "{} holds work: {} files changed, {} commits of its own",
            path.display(),
            held.changes,
            held.commits
        )));
    }
    let p = path.canonicalize()?;
    let p = p.to_string_lossy();
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    git(path, &[args.as_slice(), &[&p]].concat())?;
    Ok(())
}

/// Moves the worktree at `path` to `repo`'s HEAD if it is clean and behind
/// it; says where it stands otherwise. Commits imece made for applied
/// changes do not hold it back: they are in the checkout already (or the
/// person let them go). While the checkout has not moved on, a worktree
/// ahead of it by such commits only stays where it is.
pub fn place(repo: &Path, path: &Path) -> io::Result<Placed> {
    let head = git(repo, &["rev-parse", "HEAD"])?;
    let at = git(path, &["rev-parse", "HEAD"])?;
    if at == head {
        return Ok(Placed::AtHead);
    }
    if !git(path, &["status", "--porcelain"])?.is_empty() {
        return Ok(Placed::Kept {
            at: short(&at),
            head: short(&head),
            why: Why::Changes,
        });
    }
    let ancestor = |a: &str, b: &str| git(path, &["merge-base", "--is-ancestor", a, b]).is_ok();
    // Only a worktree whose own commits are all imece's can move without
    // losing work.
    if !ancestor(&at, &head) {
        let own = git(path, &["log", "--format=%ae", &format!("{head}..{at}")])?;
        if own.lines().any(|l| l != IMECE_EMAIL) {
            return Ok(Placed::Kept {
                at: short(&at),
                head: short(&head),
                why: Why::Commits,
            });
        }
        if ancestor(&head, &at) {
            // Applied, not yet committed by the person: nothing to pick up.
            return Ok(Placed::AtHead);
        }
    }
    git(path, &["checkout", "--quiet", "--detach", &head])?;
    Ok(Placed::Moved {
        from: short(&at),
        to: short(&head),
    })
}

/// One file of a [`Change`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    /// Lines added and removed; none for a binary file.
    pub added: u32,
    pub removed: u32,
    pub binary: bool,
}

/// Everything an agent changed in its worktree since the worktree's HEAD,
/// untracked files included, as one patch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    /// The worktree.
    pub path: PathBuf,
    /// The tree the change makes, to commit once it is applied.
    pub tree: String,
    pub files: Vec<FileChange>,
    /// A `git diff` of it all (binary files included), as `git apply`
    /// takes it.
    pub patch: String,
}

impl Change {
    /// Lines added and removed over all files.
    pub fn counts(&self) -> (u32, u32) {
        self.files
            .iter()
            .fold((0, 0), |(a, r), f| (a + f.added, r + f.removed))
    }
}

/// What the worktree at `path` holds against its HEAD, or none when it is
/// clean. It is read through an index file of imece's own, so the agent's
/// index is never touched. A patch over `max_patch` bytes is refused.
pub fn change(path: &Path, max_patch: usize) -> io::Result<Option<Change>> {
    staged(path, "imece-index", |env, tree| {
        if tree == git(path, &["rev-parse", "HEAD^{tree}"])? {
            return Ok(None);
        }
        let stat = git_env(path, env, &["diff", "--cached", "--numstat", "HEAD"])?;
        let patch = git_bytes(
            path,
            env,
            &["diff", "--cached", "--binary", "--no-color", "HEAD"],
        )?;
        if patch.len() > max_patch {
            return Err(io::Error::other(format!(
                "the change is {} KiB, more than imece shows ({} KiB)",
                patch.len() >> 10,
                max_patch >> 10
            )));
        }
        let files = stat.lines().filter_map(file_change).collect();
        Ok(Some(Change {
            path: path.to_path_buf(),
            tree,
            files,
            patch: String::from_utf8_lossy(&patch).into_owned(),
        }))
    })
}

/// What is staged in a checkout, for a commit message: the tree its
/// index makes (what names it), `git diff --cached --stat`, the patch
/// (at most the bytes asked for, cut at a line; `cut` says so) and the
/// subjects of the newest commits, for their style.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Staged {
    pub tree: String,
    pub stat: String,
    pub patch: String,
    pub cut: bool,
    pub subjects: Vec<String>,
}

/// The newest commit subjects [`staged_change`] gives.
const SUBJECTS: usize = 8;

/// What is staged in the checkout `repo` against its HEAD (see
/// [`Staged`]), or none when nothing is. Only reads: the person's index
/// stays as it is (`write-tree` adds the tree's objects, nothing else),
/// and no diff or text-conversion program of theirs runs.
pub fn staged_change(repo: &Path, max_patch: usize) -> io::Result<Option<Staged>> {
    let diff = [
        "diff",
        "--cached",
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
    ];
    let stat = git(repo, &[&diff[..], &["--stat"]].concat())?;
    if stat.is_empty() {
        return Ok(None);
    }
    let tree = git(repo, &["write-tree"])?;
    let patch = git_bytes(repo, &[], &diff)?;
    let cut = patch.len() > max_patch;
    let mut patch = String::from_utf8_lossy(&patch[..patch.len().min(max_patch)]).into_owned();
    if cut {
        // The last whole line, never half of one.
        patch.truncate(patch.rfind('\n').map_or(0, |i| i + 1));
    }
    // A checkout without a commit has no subjects.
    let log = git(repo, &["log", &format!("-{SUBJECTS}"), "--format=%s"]).unwrap_or_default();
    Ok(Some(Staged {
        tree,
        stat,
        patch,
        cut,
        subjects: log.lines().map(str::to_string).collect(),
    }))
}

/// The tree the worktree at `path` makes as it is, untracked files
/// included: what names its content, so a run can be tied to exactly
/// what it ran on. Read like [`change`], through an index file of its own
/// for each call (runs are read while git reads a change).
pub fn tree(path: &Path) -> io::Result<String> {
    staged(path, &own_index("tree"), |_, tree| Ok(tree))
}

/// Makes the worktree at `to` hold what the one at `from` holds as it is
/// now (its change against its HEAD, untracked files included, ignored
/// ones not), as a change of its own against its own HEAD, which a fork
/// starts at `from`'s: its files become `from`'s tree, its index stays at
/// its HEAD, so what was untracked is untracked again. The two share
/// their repository's objects. `to` must be clean (one made just now).
pub fn carry(from: &Path, to: &Path) -> io::Result<()> {
    let tree = tree(from)?;
    git(to, &["read-tree", "-m", "-u", "HEAD", &tree])?;
    git(to, &["reset", "--quiet"])?;
    Ok(())
}

/// A name for an index file of imece's own no other call uses: reads of
/// trees and stands run on several threads at once.
fn own_index(what: &str) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("imece-{what}-{}-{n}", std::process::id())
}

/// Where a worktree stands next to the checkout, as a worker's row in the
/// salon shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stand {
    /// The branch it is on; none while it is detached, as imece makes it.
    pub branch: Option<String>,
    /// Commits of its own the checkout's HEAD does not have; imece's marks
    /// of applied changes do not count (the checkout has those already).
    pub ahead: u32,
    /// Commits of the checkout's HEAD it does not have.
    pub behind: u32,
    /// Lines added and removed against its own HEAD, untracked files
    /// included: what a done would hand the person now.
    pub added: u32,
    pub removed: u32,
}

/// Where the worktree at `path` stands next to the checkout `repo`. The
/// lines are read like [`change`], through an index file of its own, so
/// the agent's index is never touched; no patch is made.
pub fn stand(repo: &Path, path: &Path) -> io::Result<Stand> {
    let head = git(repo, &["rev-parse", "HEAD"])?;
    let branch = git(path, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .ok()
        .filter(|b| !b.is_empty());
    let own = git(path, &["log", "--format=%ae", &format!("{head}..HEAD")])?;
    let ahead = own.lines().filter(|l| *l != IMECE_EMAIL).count();
    let behind = git(path, &["rev-list", "--count", &format!("HEAD..{head}")])?;
    let (added, removed) = staged(path, &own_index("stand"), |env, _| {
        let stat = git_env(path, env, &["diff", "--cached", "--numstat", "HEAD"])?;
        Ok(stat
            .lines()
            .filter_map(file_change)
            .fold((0, 0), |(a, r), f| (a + f.added, r + f.removed)))
    })?;
    Ok(Stand {
        branch,
        ahead: u32::try_from(ahead).unwrap_or(u32::MAX),
        behind: behind.parse().unwrap_or(0),
        added,
        removed,
    })
}

/// Stages everything the worktree at `path` holds in an index file of
/// imece's own (`name` in its git directory, gone afterwards) and hands
/// `then` that index's environment and the tree it makes.
fn staged<T>(
    path: &Path,
    name: &str,
    then: impl FnOnce(&[(&str, &str)], String) -> io::Result<T>,
) -> io::Result<T> {
    let git_dir = PathBuf::from(git(path, &["rev-parse", "--absolute-git-dir"])?);
    let index = git_dir.join(name);
    let _ = std::fs::remove_file(&index);
    let env = [("GIT_INDEX_FILE", index.to_str().unwrap_or_default())];
    let out = (|| {
        git_env(path, &env, &["read-tree", "HEAD"])?;
        git_env(path, &env, &["add", "-A", "--", "."])?;
        let tree = git_env(path, &env, &["write-tree"])?;
        then(&env, tree)
    })();
    let _ = std::fs::remove_file(&index);
    out
}

/// One `--numstat` line: added, removed (`-` for a binary file), path.
fn file_change(line: &str) -> Option<FileChange> {
    let mut parts = line.splitn(3, '\t');
    let (added, removed, path) = (parts.next()?, parts.next()?, parts.next()?);
    let binary = added == "-";
    Some(FileChange {
        path: path.to_string(),
        added: added.parse().unwrap_or(0),
        removed: removed.parse().unwrap_or(0),
        binary,
    })
}

/// Applies `patch` to the checkout `repo`, staged, so what came from an
/// agent is marked for the person to look over and commit. A patch whose
/// lines moved in the checkout since is merged three-way when that merges
/// cleanly. One that does not fit as a whole (the checkout changed the
/// same lines, or has edits of its own in the file) is refused with the
/// conflicting files or git's words, and the checkout is as it was:
/// nothing is half applied.
pub fn apply(repo: &Path, patch: &str) -> io::Result<()> {
    // A file touched but unchanged looks changed to git until its index
    // is refreshed; refreshing changes nothing else. It reports files
    // with real changes as a failure, which is not one here.
    let _ = git_bytes(repo, &[], &["update-index", "-q", "--refresh"]);
    let Err(plain) = git_apply(repo, &[], &["--index"], patch) else {
        return Ok(());
    };
    // The person's own unstaged edits come first: no merge goes over them.
    if plain.to_string().contains("does not match index") {
        return Err(plain);
    }
    // The checkout moved on under the patch (two workers added lines to
    // the same file): it may still merge three-way. Git's own three-way
    // leaves conflict markers in the files and the rest applied, so it is
    // tried on a copy of the checkout's index first and runs for real only
    // when that merged cleanly.
    match conflicts(repo, patch) {
        Ok(None) => git_apply(repo, &[], &["--index", "--3way"], patch),
        Ok(Some(files)) => Err(io::Error::other(format!(
            "conflicts with the checkout in {}",
            files.join(", ")
        ))),
        Err(_) => Err(plain),
    }
}

/// Merges `patch` three-way into a copy of `repo`'s index, which leaves
/// the checkout's files and index alone: none when it merged cleanly, the
/// files that conflict otherwise. An error when it could not merge at all
/// (a file the patch makes already there, a patch git does not read).
fn conflicts(repo: &Path, patch: &str) -> io::Result<Option<Vec<String>>> {
    let index = repo.join(git(repo, &["rev-parse", "--git-path", "index"])?);
    let git_dir = PathBuf::from(git(repo, &["rev-parse", "--absolute-git-dir"])?);
    let copy = git_dir.join("imece-apply-index");
    let env = [("GIT_INDEX_FILE", copy.to_str().unwrap_or_default())];
    let merged = (|| {
        std::fs::copy(&index, &copy)?;
        let Err(e) = git_apply(repo, &env, &["--cached", "--3way"], patch) else {
            return Ok(None);
        };
        // Each conflicting file is listed once per side (stages 1-3).
        let mut files: Vec<String> = git_env(repo, &env, &["ls-files", "--unmerged"])?
            .lines()
            .filter_map(|l| l.split_once('\t').map(|(_, path)| path.to_string()))
            .collect();
        files.dedup();
        if files.is_empty() {
            return Err(e);
        }
        Ok(Some(files))
    })();
    let _ = std::fs::remove_file(&copy);
    merged
}

/// A part of a change that does not go on the checkout as it stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Clash {
    pub file: String,
    /// The hunk's `@@` line; none when it is the file as a whole that does
    /// not fit (a binary one, or hunks that fit only one by one).
    pub hunk: Option<String>,
    /// Git's words on it.
    pub why: String,
}

/// Where a change does not go on the checkout, as [`clashes`] found it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Clashes {
    /// The checkout's directory.
    pub checkout: String,
    /// The short id of its HEAD.
    pub head: String,
    pub hunks: Vec<Clash>,
    /// Those of their files with changes staged in the checkout: HEAD does
    /// not show what they clash with.
    pub staged: Vec<String>,
    /// [`CLASH_TRIES`] hunks were tried and there are more.
    pub cut: bool,
}

/// How many hunks [`clashes`] tries: one that fits costs a process, and
/// git's thread does nothing else meanwhile.
pub const CLASH_TRIES: usize = 200;

/// What of `patch` does not go on the checkout `repo`: tried file by file
/// and, in a file that does not fit, hunk by hunk, plain and then
/// three-way as [`apply`] does, on a copy of the checkout's index, so the
/// person's index and files stay as they are and their own unstaged edits
/// do not count: those are theirs to settle, not a worker's.
pub fn clashes(repo: &Path, patch: &str) -> io::Result<Clashes> {
    let index = repo.join(git(repo, &["rev-parse", "--git-path", "index"])?);
    let git_dir = PathBuf::from(git(repo, &["rev-parse", "--absolute-git-dir"])?);
    let copy = git_dir.join(own_index("clash"));
    let env = [("GIT_INDEX_FILE", copy.to_str().unwrap_or_default())];
    let fresh = || std::fs::copy(&index, &copy).map(|_| ());
    let found = (|| {
        fresh()?;
        let (mut hunks, mut tries, mut cut) = (Vec::new(), 0, false);
        'files: for section in file_sections(patch) {
            let Some(whole) = misses(repo, &env, section, &fresh)? else {
                continue;
            };
            let file = section_file(section);
            let clashed = hunks.len();
            for (at, line) in section.lines().enumerate() {
                if !line.starts_with("@@") {
                    continue;
                }
                if tries == CLASH_TRIES {
                    cut = true;
                    break 'files;
                }
                tries += 1;
                let hunk = subset(section, &[at]);
                if let Some(why) = misses(repo, &env, &hunk, &fresh)? {
                    hunks.push(Clash {
                        file: file.clone(),
                        hunk: Some(line.to_string()),
                        why,
                    });
                }
            }
            if hunks.len() == clashed {
                hunks.push(Clash {
                    file,
                    hunk: None,
                    why: whole,
                });
            }
        }
        Ok::<_, io::Error>((hunks, cut))
    })();
    let _ = std::fs::remove_file(&copy);
    let (hunks, cut) = found?;
    let staged = git(repo, &["diff", "--cached", "--name-only"])?;
    let staged = (staged.lines())
        .filter(|f| hunks.iter().any(|c| c.file == *f))
        .map(str::to_string)
        .collect();
    Ok(Clashes {
        checkout: repo.display().to_string(),
        head: short(&git(repo, &["rev-parse", "HEAD"])?),
        hunks,
        staged,
        cut,
    })
}

/// Git's words on why `piece` of a patch goes neither plainly nor by a
/// three-way merge into the index `env` names; none when it goes. `fresh`
/// puts that index back after a merge wrote in it.
fn misses(
    repo: &Path,
    env: &[(&str, &str)],
    piece: &str,
    fresh: &impl Fn() -> io::Result<()>,
) -> io::Result<Option<String>> {
    let Err(plain) = git_apply(repo, env, &["--cached", "--check"], piece) else {
        return Ok(None);
    };
    let merged = git_apply(repo, env, &["--cached", "--3way"], piece);
    fresh()?;
    Ok(merged.err().map(|_| plain.to_string()))
}

/// Each file's part of `patch`, its header with it. Only a file's header
/// starts a line with `diff --git `: a hunk's lines start with their mark.
fn file_sections(patch: &str) -> Vec<&str> {
    let starts: Vec<usize> = (patch.match_indices("diff --git "))
        .map(|(i, _)| i)
        .filter(|&i| i == 0 || patch.as_bytes()[i - 1] == b'\n')
        .collect();
    (starts.iter().enumerate())
        .map(|(n, &from)| &patch[from..starts.get(n + 1).copied().unwrap_or(patch.len())])
        .collect()
}

/// The file's new name in its part of a patch.
fn section_file(section: &str) -> String {
    let head = section.lines().next().unwrap_or_default();
    let name = head.rsplit_once(" b/").map(|(_, name)| name);
    name.unwrap_or(head).to_string()
}

/// `git apply ARGS -` in `repo` with `env` set and `patch` on stdin; a
/// failure carries git's error lines.
fn git_apply(repo: &Path, env: &[(&str, &str)], args: &[&str], patch: &str) -> io::Result<()> {
    let mut cmd = child::command("git", repo, &[]);
    cmd.arg("apply").args(args).arg("-").env("LC_ALL", "C");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        // A closed pipe is git's refusal to report, not ours.
        let _ = stdin.write_all(patch.as_bytes());
    }
    let out = child.wait_with_output()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        // Git says what failed and why on separate error lines.
        let why: Vec<&str> = err
            .lines()
            .filter_map(|l| l.strip_prefix("error: "))
            .take(3)
            .collect();
        let why = if why.is_empty() {
            err.lines().next().unwrap_or("failed").to_string()
        } else {
            why.join("; ")
        };
        return Err(io::Error::other(why));
    }
    Ok(())
}

/// The hunks of `patch` whose `@@` lines are at `hunks` (line indexes),
/// each file's header before its first chosen one: a patch `git apply`
/// takes, of only those. A file with no chosen hunk is left out, and so
/// is a binary one, which has none.
pub fn subset(patch: &str, hunks: &[usize]) -> String {
    let mut out = String::new();
    let mut header: Vec<&str> = Vec::new();
    let (mut in_hunks, mut said, mut keep) = (false, false, false);
    for (i, l) in patch.lines().enumerate() {
        if l.starts_with("diff --git ") {
            header = vec![l];
            (in_hunks, said, keep) = (false, false, false);
            continue;
        }
        if l.starts_with("@@") {
            in_hunks = true;
            keep = hunks.contains(&i);
            if keep && !said {
                said = true;
                header.iter().for_each(|h| {
                    out.push_str(h);
                    out.push('\n');
                });
            }
        } else if !in_hunks {
            header.push(l);
            continue;
        }
        if keep {
            out.push_str(l);
            out.push('\n');
        }
    }
    out
}

/// The part of `change` that `patch` (a [`subset`] of its patch) makes,
/// as a change of its own: the tree its worktree's HEAD makes with only
/// that applied, read through an index file of imece's own (the agent's
/// index and files are not touched). Applied and marked with
/// [`applied`], it leaves the rest of the change in the worktree, not
/// committed, for the next [`change`] to hold.
pub fn part(change: &Change, patch: &str) -> io::Result<Change> {
    let path = &change.path;
    let git_dir = PathBuf::from(git(path, &["rev-parse", "--absolute-git-dir"])?);
    let index = git_dir.join(own_index("part"));
    let env = [("GIT_INDEX_FILE", index.to_str().unwrap_or_default())];
    let out = (|| {
        git_env(path, &env, &["read-tree", "HEAD"])?;
        git_apply(path, &env, &["--cached"], patch)?;
        let tree = git_env(path, &env, &["write-tree"])?;
        let stat = git_env(path, &env, &["diff", "--cached", "--numstat", "HEAD"])?;
        Ok(Change {
            path: path.clone(),
            tree,
            files: stat.lines().filter_map(file_change).collect(),
            patch: patch.to_string(),
        })
    })();
    let _ = std::fs::remove_file(&index);
    out
}

/// Commits `change` in its worktree as imece, once the person applied it:
/// HEAD and the index move to the change, the files stay as they are, so
/// the next [`change`] holds only what came after. Gives the commit's
/// short id.
pub fn applied(change: &Change, message: &str) -> io::Result<String> {
    let path = &change.path;
    let parent = git(path, &["rev-parse", "HEAD"])?;
    let who = [
        ("GIT_AUTHOR_NAME", "imece"),
        ("GIT_AUTHOR_EMAIL", IMECE_EMAIL),
        ("GIT_COMMITTER_NAME", "imece"),
        ("GIT_COMMITTER_EMAIL", IMECE_EMAIL),
    ];
    let commit = git_env(
        path,
        &who,
        &["commit-tree", &change.tree, "-p", &parent, "-m", message],
    )?;
    git(path, &["reset", "--quiet", &commit])?;
    Ok(short(&commit))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway repo whose directory goes away when the test ends, also
    /// when it fails.
    struct Scratch {
        dir: PathBuf,
        repo: PathBuf,
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn repo(name: &str) -> Scratch {
        let dir = std::env::temp_dir()
            .join(format!("imece-wt-{name}-{}", std::process::id()))
            .canonicalize_or_self();
        let _ = std::fs::remove_dir_all(&dir);
        let r = dir.join("repo");
        std::fs::create_dir_all(&r).unwrap();
        git(&r, &["init", "--quiet"]).unwrap();
        std::fs::write(r.join("a.txt"), "a").unwrap();
        commit(&r, "init");
        Scratch { dir, repo: r }
    }

    #[test]
    fn on_branch_names_the_branch_and_its_commit_and_no_detached_head() {
        let s = repo("on-branch");
        git(&s.repo, &["checkout", "--quiet", "-b", "feature/x"]).unwrap();
        let head = commit(&s.repo, "two");
        assert_eq!(on_branch(&s.repo), Some(("feature/x".to_string(), head)));
        git(&s.repo, &["checkout", "--quiet", "--detach"]).unwrap();
        assert_eq!(on_branch(&s.repo), None, "detached");
    }

    /// `s`'s checkout one commit behind a new branch `ahead`, back on
    /// its own branch, and `ahead`'s commit.
    fn behind_ahead(s: &Scratch) -> String {
        let r = &s.repo;
        let was = git(r, &["symbolic-ref", "--short", "HEAD"]).unwrap();
        git(r, &["checkout", "--quiet", "-b", "ahead"]).unwrap();
        std::fs::write(r.join("b.txt"), "b").unwrap();
        let tip = commit(r, "ahead");
        git(r, &["checkout", "--quiet", &was]).unwrap();
        tip
    }

    #[test]
    fn a_checkout_moves_forward_and_only_forward() {
        let s = repo("forward");
        let r = &s.repo;
        let tip = behind_ahead(&s);
        std::fs::write(r.join("note"), "unrelated").unwrap();
        git(r, &["add", "note"]).unwrap();
        assert_eq!(fast_forward(r, "ahead").unwrap(), Forward::Moved);
        assert_eq!(git(r, &["rev-parse", "HEAD"]).unwrap(), tip);
        assert!(r.join("note").exists(), "what was staged stays");
        assert_eq!(fast_forward(r, "ahead").unwrap(), Forward::Even);
        assert!(fast_forward(r, "no-such-ref").is_err());
    }

    #[test]
    fn a_checkout_with_commits_of_its_own_stays_where_it_is() {
        let s = repo("apart");
        let r = &s.repo;
        behind_ahead(&s);
        std::fs::write(r.join("c.txt"), "c").unwrap();
        let own = commit(r, "own");
        assert_eq!(fast_forward(r, "ahead").unwrap(), Forward::Apart);
        assert_eq!(git(r, &["rev-parse", "HEAD"]).unwrap(), own);
    }

    #[test]
    fn work_in_the_way_is_gits_refusal_and_stays() {
        let s = repo("dirty");
        let r = &s.repo;
        let was = git(r, &["rev-parse", "HEAD"]).unwrap();
        behind_ahead(&s);
        std::fs::write(r.join("b.txt"), "mine").unwrap();
        git(r, &["add", "b.txt"]).unwrap();
        let why = fast_forward(r, "ahead").unwrap_err().to_string();
        assert!(why.contains("merge"), "{why}");
        assert_eq!(git(r, &["rev-parse", "HEAD"]).unwrap(), was);
        assert_eq!(std::fs::read_to_string(r.join("b.txt")).unwrap(), "mine");
    }

    #[test]
    fn near_a_reference_is_where_head_left_it_else_head() {
        let s = repo("near");
        let r = &s.repo;
        let base = git(r, &["rev-parse", "HEAD"]).unwrap();
        assert_eq!(near(r, "no-such-ref"), Some(base.clone()));
        behind_ahead(&s);
        std::fs::write(r.join("c.txt"), "c").unwrap();
        let own = commit(r, "own");
        assert_eq!(near(r, "ahead"), Some(base));
        assert_eq!(near(r, "no-such-ref"), Some(own));
        let bare = s.dir.join("unborn");
        std::fs::create_dir_all(&bare).unwrap();
        git(&bare, &["init", "--quiet"]).unwrap();
        assert_eq!(near(&bare, "ahead"), None);
    }

    #[test]
    fn on_branch_needs_a_first_commit() {
        let s = repo("on-branch-unborn");
        let bare = s.dir.join("unborn");
        std::fs::create_dir_all(&bare).unwrap();
        git(&bare, &["init", "--quiet"]).unwrap();
        assert_eq!(on_branch(&bare), None);
    }

    /// Commits everything in `dir` (a repository or a worktree).
    fn commit(dir: &Path, message: &str) -> String {
        git(dir, &["add", "."]).unwrap();
        git(
            dir,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                message,
            ],
        )
        .unwrap();
        git(dir, &["rev-parse", "HEAD"]).unwrap()
    }

    trait OrSelf {
        fn canonicalize_or_self(self) -> PathBuf;
    }
    impl OrSelf for PathBuf {
        fn canonicalize_or_self(self) -> PathBuf {
            std::fs::create_dir_all(&self).unwrap();
            self.canonicalize().unwrap()
        }
    }

    #[test]
    fn a_checkout_is_born_with_its_first_commit() {
        let s = repo("born");
        assert!(born(&s.repo));
        let empty = s.dir.join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert!(!born(&empty), "no git at all");
        git(&empty, &["init", "--quiet"]).unwrap();
        assert!(!born(&empty), "git init, nothing committed");
        commit(&empty, "first");
        assert!(born(&empty));
    }

    #[test]
    fn a_worktree_carries_another_s_change_as_its_own_against_the_same_head() {
        let s = repo("carry");
        std::fs::write(s.repo.join("b.txt"), "b").unwrap();
        std::fs::write(s.repo.join(".gitignore"), "secret\n").unwrap();
        commit(&s.repo, "more");
        let root = s.dir.join("trees");
        let from = ensure(&s.repo, &root, "one", None).unwrap().path;
        // Changed, deleted, new, new in a new directory, binary, ignored.
        std::fs::write(from.join("a.txt"), "a changed").unwrap();
        std::fs::remove_file(from.join("b.txt")).unwrap();
        std::fs::write(from.join("c.txt"), "c").unwrap();
        std::fs::create_dir_all(from.join("d")).unwrap();
        std::fs::write(from.join("d/e.bin"), [0u8, 159, 146, 150]).unwrap();
        std::fs::write(from.join("secret"), "s").unwrap();
        let head = resolve(&from, "HEAD").unwrap();
        let to = ensure(&s.repo, &root, "two", Some(&head)).unwrap().path;
        carry(&from, &to).unwrap();
        let read = |p: &str| std::fs::read(to.join(p)).ok();
        assert_eq!(read("a.txt").as_deref(), Some(&b"a changed"[..]));
        assert_eq!(read("b.txt"), None, "deleted there, deleted here");
        assert_eq!(read("c.txt").as_deref(), Some(&b"c"[..]));
        assert_eq!(read("d/e.bin").as_deref(), Some(&[0u8, 159, 146, 150][..]));
        assert_eq!(read("secret"), None, "an ignored file is :copy's");
        // A change of its own, the same one, the index left at HEAD.
        let (one, two) = (
            change(&from, 1 << 20).unwrap().unwrap(),
            change(&to, 1 << 20).unwrap().unwrap(),
        );
        assert_eq!(
            (two.tree.as_str(), two.patch.as_str()),
            (one.tree.as_str(), one.patch.as_str())
        );
        assert_eq!(git(&to, &["diff", "--cached", "--name-only"]).unwrap(), "");
        let status = git(&to, &["status", "--porcelain"]).unwrap();
        assert!(
            status.contains("?? c.txt") && status.contains(" D b.txt"),
            "{status}"
        );
        assert_eq!(resolve(&to, "HEAD").unwrap(), head);
        // The one it came from is as it was.
        assert_eq!(change(&from, 1 << 20).unwrap().unwrap(), one);
        // Nothing to carry: nothing changes.
        let three = ensure(&s.repo, &root, "three", Some(&head)).unwrap().path;
        let four = ensure(&s.repo, &root, "four", Some(&head)).unwrap().path;
        carry(&three, &four).unwrap();
        assert_eq!(change(&four, 1 << 20).unwrap(), None);
    }

    #[test]
    fn the_files_are_the_tracked_and_the_untracked_not_ignored_sorted_and_capped() {
        let s = repo("files");
        std::fs::create_dir_all(s.repo.join("src")).unwrap();
        std::fs::write(s.repo.join("src/main.rs"), "fn main() {}").unwrap();
        std::fs::write(s.repo.join(".gitignore"), "target/\n").unwrap();
        std::fs::create_dir_all(s.repo.join("target")).unwrap();
        std::fs::write(s.repo.join("target/out"), "x").unwrap();
        commit(&s.repo, "src");
        // Untracked, not ignored: listed; ignored: not.
        std::fs::write(s.repo.join("b.txt"), "b").unwrap();
        let (listed, cut) = files(&s.repo, 10).unwrap();
        assert_eq!(listed, [".gitignore", "a.txt", "b.txt", "src/main.rs"]);
        assert!(!cut);
        // Capped, and said so; exactly the cap is not a cut.
        assert_eq!(
            files(&s.repo, 2).unwrap(),
            (vec![".gitignore".to_string(), "a.txt".into()], true)
        );
        assert!(!files(&s.repo, 4).unwrap().1);
        assert!(files(&s.dir, 10).is_err(), "not a checkout");
    }

    #[test]
    fn each_own_index_has_a_name_no_other_call_uses() {
        let (a, b) = (own_index("stand"), own_index("stand"));
        assert_ne!(a, b, "two threads at once must not share an index");
        let pid = std::process::id();
        assert!(a.starts_with(&format!("imece-stand-{pid}-")), "{a}");
        assert!(own_index("tree").starts_with("imece-tree-"));
    }

    #[test]
    fn what_is_staged_is_read_without_touching_the_index() {
        let s = repo("staged");
        let r = &s.repo;
        assert_eq!(staged_change(r, 1 << 20).unwrap(), None, "nothing staged");
        // Written but not staged is not staged either.
        std::fs::write(r.join("a.txt"), "a\nb\n").unwrap();
        assert_eq!(staged_change(r, 1 << 20).unwrap(), None);
        git(r, &["add", "a.txt"]).unwrap();
        let before = git(r, &["diff", "--cached", "--name-only"]).unwrap();
        let got = staged_change(r, 1 << 20).unwrap().unwrap();
        assert_eq!(got.tree, git(r, &["write-tree"]).unwrap());
        assert!(got.stat.contains("a.txt"), "{got:?}");
        assert!(got.patch.contains("+b\n"), "{got:?}");
        assert!(!got.cut);
        assert_eq!(got.subjects, ["init"]);
        assert_eq!(
            git(r, &["diff", "--cached", "--name-only"]).unwrap(),
            before
        );
        // Cut at a line, and said.
        let whole = got.patch.len();
        let cut = staged_change(r, whole - 1).unwrap().unwrap();
        assert!(cut.cut);
        assert!(
            cut.patch.ends_with('\n') && cut.patch.len() < whole,
            "{cut:?}"
        );
        assert!(got.patch.starts_with(&cut.patch));
        let exact = staged_change(r, whole).unwrap().unwrap();
        assert!(!exact.cut, "a patch of just the size is whole");
        assert_eq!(exact.patch, got.patch);
    }

    #[test]
    fn the_newest_eight_subjects_come_newest_first() {
        let s = repo("subjects");
        let r = &s.repo;
        for i in 1..=9 {
            commit(r, &format!("c{i}"));
        }
        std::fs::write(r.join("b.txt"), "b").unwrap();
        git(r, &["add", "b.txt"]).unwrap();
        let got = staged_change(r, 1 << 20).unwrap().unwrap();
        assert_eq!(
            got.subjects,
            ["c9", "c8", "c7", "c6", "c5", "c4", "c3", "c2"]
        );
    }

    #[test]
    fn names_never_become_paths() {
        for bad in ["", "../x", "a/b", "-rf", "A", "x y", "é", &"a".repeat(41)] {
            assert!(!valid_name(bad), "{bad:?}");
        }
        assert!(valid_name("claude-1"));
        let s = repo("names");
        let r = &s.repo;
        assert!(ensure(r, &s.dir.join("wt"), "../escape", None).is_err());
    }

    #[test]
    fn a_start_point_is_one_name_never_an_option_or_a_range() {
        for good in [
            "main",
            "feature/x",
            "v1.2",
            "abc1234",
            "HEAD~2",
            "#7",
            "#123456789",
        ] {
            assert!(valid_start(good), "{good:?}");
        }
        let long = "a".repeat(201);
        for bad in [
            "",
            "-x",
            "--output=x",
            "a..b",
            "a...b",
            "a b",
            "a\tb",
            "a\nb",
            "#",
            "#x",
            "#1x",
            "#1234567890",
            "# 1",
            &long,
        ] {
            assert!(!valid_start(bad), "{bad:?}");
        }
        assert!(valid_start(&"a".repeat(200)));
    }

    #[test]
    fn a_start_point_resolves_to_its_commit() {
        let s = repo("resolve");
        let r = &s.repo;
        let first = git(r, &["rev-parse", "HEAD"]).unwrap();
        git(r, &["tag", "v1"]).unwrap();
        git(r, &["branch", "old"]).unwrap();
        let second = commit(r, "second");
        assert_eq!(resolve(r, "v1").unwrap(), first);
        assert_eq!(resolve(r, "old").unwrap(), first);
        assert_eq!(resolve(r, "HEAD").unwrap(), second);
        assert_eq!(resolve(r, &first[..7]).unwrap(), first);
        let e = resolve(r, "nowhere").unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::NotFound);
        assert!(
            e.to_string().starts_with("nowhere names no commit in "),
            "{e}"
        );
        let e = resolve(r, "-x").unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidInput);
        // A tree is not a commit.
        assert!(resolve(r, "HEAD^{tree}").is_err());
    }

    #[test]
    fn a_pull_request_is_fetched_from_origin() {
        let s = repo("pull");
        let up = s.dir.join("up");
        git(
            &s.dir,
            &["clone", "--quiet", &s.repo.to_string_lossy(), "up"],
        )
        .unwrap();
        let theirs = commit(&up, "theirs");
        git(&up, &["update-ref", "refs/pull/7/head", &theirs]).unwrap();
        let r = &s.repo;
        let e = resolve(r, "#7").unwrap_err();
        assert!(
            e.to_string().starts_with("pull request #7 of origin: "),
            "{e}"
        );
        git(r, &["remote", "add", "origin", &up.to_string_lossy()]).unwrap();
        assert_eq!(resolve(r, "#7").unwrap(), theirs);
        assert!(resolve(r, "#8").is_err(), "no such pull request");
    }

    #[test]
    fn a_worktree_from_a_start_point_is_made_there_and_stays() {
        let s = repo("from");
        let r = &s.repo;
        let root = s.dir.join("wt");
        git(r, &["branch", "old"]).unwrap();
        let old = git(r, &["rev-parse", "HEAD"]).unwrap();
        std::fs::write(r.join("a.txt"), "new").unwrap();
        commit(r, "new");
        let w = ensure(r, &root, "claude-1", Some("old")).unwrap();
        assert_eq!(
            w.placed,
            Placed::From {
                at: old[..7].to_string()
            }
        );
        assert_eq!(std::fs::read_to_string(w.path.join("a.txt")).unwrap(), "a");
        // Clean and behind the checkout, yet never moved to its HEAD.
        let again = ensure(r, &root, "claude-1", Some("old")).unwrap();
        assert_eq!(again.placed, Placed::Own);
        assert_eq!(git(&w.path, &["rev-parse", "HEAD"]).unwrap(), old);
        // Without one, the same worktree follows HEAD as any other.
        let moved = ensure(r, &root, "claude-1", None).unwrap();
        assert!(matches!(moved.placed, Placed::Moved { .. }), "{moved:?}");
        // A start point that names nothing makes no worktree.
        let e = ensure(r, &root, "claude-2", Some("nowhere")).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::NotFound);
        assert!(!root.join("claude-2").exists());
    }

    /// A checkout `name` with files `a/x`, `b/y` and `t` at its top,
    /// made sparse by `file` as `.git/info/sparse-checkout`, cone mode or
    /// not, checked out again under it.
    fn sparse_repo(name: &str, cone: bool, file: &str) -> Scratch {
        let s = repo(name);
        let r = &s.repo;
        for (dir, f) in [("a", "x"), ("b", "y")] {
            std::fs::create_dir_all(r.join(dir)).unwrap();
            std::fs::write(r.join(dir).join(f), f).unwrap();
        }
        std::fs::write(r.join("t"), "t").unwrap();
        commit(r, "tree");
        git(r, &["config", "core.sparseCheckout", "true"]).unwrap();
        git(r, &["config", "core.sparseCheckoutCone", &cone.to_string()]).unwrap();
        std::fs::write(r.join(".git/info/sparse-checkout"), file).unwrap();
        git(r, &["read-tree", "-m", "-u", "HEAD"]).unwrap();
        s
    }

    /// The files under `dir`, `.git` left out, relative and sorted.
    fn files_under(dir: &Path) -> Vec<String> {
        let mut out = Vec::new();
        let mut todo = vec![dir.to_path_buf()];
        while let Some(d) = todo.pop() {
            for e in std::fs::read_dir(&d).unwrap() {
                let p = e.unwrap().path();
                if p.file_name().is_some_and(|n| n == ".git") {
                    continue;
                }
                if p.is_dir() {
                    todo.push(p);
                } else {
                    let rel = p.strip_prefix(dir).unwrap();
                    out.push(rel.to_string_lossy().into_owned());
                }
            }
        }
        out.sort();
        out
    }

    #[test]
    fn a_sparse_checkout_s_worktree_is_as_sparse_and_the_rest_never_written() {
        // Cone mode: a directory left in, and every file at the top.
        let s = sparse_repo("sparse-cone", true, "/*\n!/*/\n/a/\n");
        let r = &s.repo;
        assert_eq!(files_under(r), ["a.txt", "a/x", "t"]);
        let list = git(r, &["sparse-checkout", "list"]).unwrap();
        let w = ensure(r, &s.dir.join("wt"), "claude-1", None).unwrap();
        assert_eq!(w.placed, Placed::Fresh);
        assert_eq!(files_under(&w.path), ["a.txt", "a/x", "t"]);
        assert!(
            !w.path.join("b").exists(),
            "a left-out directory was written"
        );
        assert_eq!(git(&w.path, &["sparse-checkout", "list"]).unwrap(), "a");
        assert!(config_true(&w.path, "core.sparseCheckoutCone"));
        // Nothing to commit: what is left out is not a deletion.
        assert_eq!(git(&w.path, &["status", "--porcelain"]).unwrap(), "");
        assert_eq!(change(&w.path, 1 << 20).unwrap(), None);
        // The person's checkout is as it was.
        assert_eq!(git(r, &["sparse-checkout", "list"]).unwrap(), list);
        assert_eq!(files_under(r), ["a.txt", "a/x", "t"]);

        // Not cone mode: the patterns as they are, nothing else at the top.
        let s = sparse_repo("sparse-patterns", false, "/b/\n");
        let r = &s.repo;
        assert_eq!(files_under(r), ["b/y"]);
        let w = ensure(r, &s.dir.join("wt"), "claude-1", None).unwrap();
        assert_eq!(files_under(&w.path), ["b/y"]);
        assert_eq!(git(&w.path, &["sparse-checkout", "list"]).unwrap(), "/b/");
        assert!(!config_true(&w.path, "core.sparseCheckoutCone"));
        assert_eq!(git(&w.path, &["status", "--porcelain"]).unwrap(), "");
        assert_eq!(git(r, &["sparse-checkout", "list"]).unwrap(), "/b/");
        assert_eq!(files_under(r), ["b/y"]);
    }

    #[test]
    fn a_checkout_not_sparse_has_a_whole_worktree() {
        let s = sparse_repo("sparse-off", true, "/*\n!/*/\n/a/\n");
        let r = &s.repo;
        git(r, &["sparse-checkout", "disable"]).unwrap();
        assert!(!config_true(r, "core.sparseCheckout"));
        assert_eq!(sparse_of(r).unwrap(), None);
        let w = ensure(r, &s.dir.join("wt"), "claude-1", None).unwrap();
        assert_eq!(files_under(&w.path), ["a.txt", "a/x", "b/y", "t"]);
    }

    #[test]
    fn a_worktree_that_can_not_be_as_sparse_is_not_left_half_made() {
        // `set` refuses a directory out of the tree, which `list` gives.
        let s = sparse_repo("sparse-bad", true, "/*\n!/*/\n/a/\n");
        let r = &s.repo;
        std::fs::write(r.join(".git/info/sparse-checkout"), "/*\n!/*/\n/../x/\n").unwrap();
        let root = s.dir.join("wt");
        let e = ensure(r, &root, "claude-1", None).unwrap_err().to_string();
        assert!(e.contains("is a sparse checkout"), "{e}");
        assert!(e.contains("could not be made one too"), "{e}");
        assert!(
            !root.join("claude-1").exists(),
            "a half-made worktree stays"
        );
        let list = git(r, &["worktree", "list", "--porcelain"]).unwrap();
        assert_eq!(list.matches("worktree ").count(), 1, "{list}");
    }

    #[test]
    fn git_takes_what_it_is_fed() {
        let s = repo("fed");
        let out = git_fed(&s.repo, &[], &["hash-object", "--stdin"], Some(b"a")).unwrap();
        // `git hash-object` of the one byte "a", as `a.txt` holds it.
        let a = git(&s.repo, &["rev-parse", "HEAD:a.txt"]).unwrap();
        assert_eq!(String::from_utf8_lossy(&out).trim(), a);
        let err = git_fed(&s.repo, &[], &["no-such-command"], Some(b"a")).unwrap_err();
        assert!(err.to_string().contains("git no-such-command"), "{err}");
    }

    #[test]
    fn a_worktree_holds_its_changes_and_its_own_commits() {
        let s = repo("held");
        let r = &s.repo;
        let w = ensure(r, &s.dir.join("wt"), "claude-1", None).unwrap().path;
        assert_eq!(held(&w).unwrap(), (r.clone(), Held::default()));
        assert!(Held::default().clean());
        // What git ignores is not held; a file not tracked is.
        std::fs::write(w.join(".gitignore"), "target\n").unwrap();
        commit(&w, "ignore");
        // Its own commit, on no branch.
        assert_eq!(
            held(&w).unwrap().1,
            Held {
                changes: 0,
                commits: 1
            }
        );
        std::fs::create_dir_all(w.join("target")).unwrap();
        std::fs::write(w.join("target/x"), "x").unwrap();
        assert_eq!(held(&w).unwrap().1.changes, 0);
        std::fs::write(w.join("new.txt"), "n").unwrap();
        std::fs::write(w.join("a.txt"), "changed").unwrap();
        let h = held(&w).unwrap().1;
        assert_eq!(
            h,
            Held {
                changes: 2,
                commits: 1
            }
        );
        assert!(!h.clean());
        assert!(
            !Held {
                changes: 1,
                commits: 0
            }
            .clean()
        );
        assert!(
            !Held {
                changes: 0,
                commits: 1
            }
            .clean()
        );
        // On a branch, its commits are kept by the branch.
        git(&w, &["checkout", "--quiet", "-b", "kept"]).unwrap();
        assert_eq!(
            held(&w).unwrap().1,
            Held {
                changes: 2,
                commits: 0
            }
        );
        git(&w, &["checkout", "--quiet", "--detach"]).unwrap();
        // imece's own commits are in the checkout already.
        git(&w, &["checkout", "--quiet", "."]).unwrap();
        std::fs::remove_file(w.join("new.txt")).unwrap();
        git(&w, &["branch", "--quiet", "-D", "kept"]).unwrap();
        std::fs::write(w.join("a.txt"), "applied").unwrap();
        git(&w, &["add", "a.txt"]).unwrap();
        git_env(
            &w,
            &[],
            &[
                "-c",
                "user.name=imece",
                "-c",
                &format!("user.email={IMECE_EMAIL}"),
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                "applied",
            ],
        )
        .unwrap();
        assert_eq!(
            held(&w).unwrap().1,
            Held {
                changes: 0,
                commits: 1
            }
        );
    }

    #[test]
    fn only_a_worktree_added_to_a_checkout_is_held() {
        let s = repo("held-not");
        let r = &s.repo;
        let e = held(r).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidInput, "{e}");
        let w = ensure(r, &s.dir.join("wt"), "claude-1", None).unwrap().path;
        std::fs::create_dir_all(w.join("sub")).unwrap();
        let e = held(&w.join("sub")).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidInput, "{e}");
        assert!(held(&s.dir.join("nothing")).is_err());
        let plain = s.dir.join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        assert!(held(&plain).is_err());
    }

    #[test]
    fn a_clean_worktree_is_removed_and_one_with_work_only_when_forced() {
        let s = repo("remove");
        let r = &s.repo;
        let root = s.dir.join("wt");
        let w = ensure(r, &root, "claude-1", None).unwrap().path;
        std::fs::write(w.join("new.txt"), "n").unwrap();
        let e = remove(&w, false).unwrap_err().to_string();
        assert!(e.contains("holds work: 1 files changed, 0 commits"), "{e}");
        assert!(w.join("new.txt").exists());
        std::fs::remove_file(w.join("new.txt")).unwrap();
        commit(&w, "own");
        let e = remove(&w, false).unwrap_err().to_string();
        assert!(e.contains("0 files changed, 1 commits"), "{e}");
        assert!(w.exists());
        remove(&w, true).unwrap();
        assert!(!w.exists());
        let list = git(r, &["worktree", "list", "--porcelain"]).unwrap();
        assert_eq!(list.matches("worktree ").count(), 1, "{list}");
        // A clean one goes without force; git's ignored files with it.
        let w = ensure(r, &root, "claude-2", None).unwrap().path;
        std::fs::write(w.join(".git-ignored-by-info"), "").unwrap();
        let info = common_dir(r).unwrap().join("info");
        std::fs::create_dir_all(&info).unwrap();
        std::fs::write(info.join("exclude"), ".git-ignored-by-info\n").unwrap();
        remove(&w, false).unwrap();
        assert!(!w.exists());
        // And it is made anew, with no prune.
        let again = ensure(r, &root, "claude-2", None).unwrap();
        assert_eq!(again.placed, Placed::Fresh);
        // A checkout is never removed.
        assert!(remove(r, true).is_err());
        assert!(r.join("a.txt").exists());
    }

    #[test]
    fn a_worktree_is_made_once_and_reused() {
        let s = repo("reuse");
        let r = &s.repo;
        let root = s.dir.join("wt");
        let w = ensure(r, &root, "claude-1", None).unwrap();
        assert_eq!(w.placed, Placed::Fresh);
        let w = w.path;
        assert_eq!(std::fs::read_to_string(w.join("a.txt")).unwrap(), "a");
        std::fs::write(w.join("work.txt"), "kept").unwrap();
        let again = ensure(r, &root, "claude-1", None).unwrap();
        assert_eq!(again.path, w);
        assert_eq!(again.placed, Placed::AtHead);
        assert!(w.join("work.txt").exists(), "a reuse threw work away");
    }

    #[test]
    fn a_clean_worktree_moves_to_the_checkout_s_head() {
        let s = repo("move");
        let r = &s.repo;
        let root = s.dir.join("wt");
        let w = ensure(r, &root, "claude-1", None).unwrap().path;
        let old = git(&w, &["rev-parse", "HEAD"]).unwrap();
        std::fs::write(r.join("b.txt"), "b").unwrap();
        let new = commit(r, "second");
        let again = ensure(r, &root, "claude-1", None).unwrap();
        assert_eq!(
            again.placed,
            Placed::Moved {
                from: short(&old),
                to: short(&new)
            }
        );
        assert_eq!(git(&w, &["rev-parse", "HEAD"]).unwrap(), new);
        assert_eq!(std::fs::read_to_string(w.join("b.txt")).unwrap(), "b");
        assert_eq!(short(&new).len(), 7);
    }

    #[test]
    fn a_worktree_with_changes_stays_where_it_is() {
        let s = repo("dirty");
        let r = &s.repo;
        let root = s.dir.join("wt");
        let w = ensure(r, &root, "claude-1", None).unwrap().path;
        let old = git(&w, &["rev-parse", "HEAD"]).unwrap();
        std::fs::write(w.join("a.txt"), "changed").unwrap();
        let new = commit(r, "second");
        let again = ensure(r, &root, "claude-1", None).unwrap();
        assert_eq!(
            again.placed,
            Placed::Kept {
                at: short(&old),
                head: short(&new),
                why: Why::Changes
            }
        );
        assert_eq!(git(&w, &["rev-parse", "HEAD"]).unwrap(), old);
        assert_eq!(std::fs::read_to_string(w.join("a.txt")).unwrap(), "changed");
        // An untracked file is work too.
        std::fs::write(w.join("a.txt"), "a").unwrap();
        std::fs::write(w.join("new.txt"), "n").unwrap();
        assert!(matches!(
            ensure(r, &root, "claude-1", None).unwrap().placed,
            Placed::Kept {
                why: Why::Changes,
                ..
            }
        ));
        assert_eq!(git(&w, &["rev-parse", "HEAD"]).unwrap(), old);
    }

    #[test]
    fn a_worktree_with_commits_of_its_own_stays_where_it_is() {
        let s = repo("commits");
        let r = &s.repo;
        let root = s.dir.join("wt");
        let w = ensure(r, &root, "claude-1", None).unwrap().path;
        std::fs::write(w.join("w.txt"), "w").unwrap();
        let own = commit(&w, "own work");
        let new = commit(r, "second");
        let again = ensure(r, &root, "claude-1", None).unwrap();
        assert_eq!(
            again.placed,
            Placed::Kept {
                at: short(&own),
                head: short(&new),
                why: Why::Commits
            }
        );
        assert_eq!(git(&w, &["rev-parse", "HEAD"]).unwrap(), own);
    }

    #[test]
    fn a_change_is_everything_against_the_worktree_s_head_and_touches_no_index() {
        let s = repo("change");
        let r = &s.repo;
        std::fs::write(r.join("gone.txt"), "gone").unwrap();
        std::fs::write(r.join("same.txt"), "same").unwrap();
        commit(r, "more");
        let w = ensure(r, &s.dir.join("wt"), "claude-1", None).unwrap().path;
        assert_eq!(change(&w, 1 << 20).unwrap(), None, "clean");
        std::fs::write(w.join("a.txt"), "a\nb\nc\n").unwrap();
        std::fs::write(w.join("new.txt"), "new\n").unwrap();
        std::fs::write(w.join("blob.bin"), [0u8, 1, 2, 255, 0, 10]).unwrap();
        std::fs::remove_file(w.join("gone.txt")).unwrap();
        let c = change(&w, 1 << 20).unwrap().unwrap();
        assert_eq!(c.path, w);
        let files: Vec<(&str, u32, u32, bool)> = c
            .files
            .iter()
            .map(|f| (f.path.as_str(), f.added, f.removed, f.binary))
            .collect();
        assert_eq!(
            files,
            [
                ("a.txt", 3, 1, false),
                ("blob.bin", 0, 0, true),
                ("gone.txt", 0, 1, false),
                ("new.txt", 1, 0, false),
            ]
        );
        assert_eq!(c.counts(), (4, 2));
        assert!(
            c.patch.contains("diff --git a/new.txt b/new.txt"),
            "{}",
            c.patch
        );
        assert!(c.patch.contains("GIT binary patch"), "{}", c.patch);
        assert!(c.patch.ends_with('\n'), "a patch ends with its newline");
        assert_ne!(c.tree, git(&w, &["rev-parse", "HEAD^{tree}"]).unwrap());
        // The agent's own index is as it was: nothing staged.
        let status = git(&w, &["status", "--porcelain"]).unwrap();
        assert!(
            status.contains("M a.txt") && status.contains("?? new.txt") && !status.contains("A "),
            "{status}"
        );
        assert!(!w.join(".git").join("imece-index").exists());
        // A change past the cap is refused, not cut.
        let e = change(&w, 10).unwrap_err().to_string();
        assert!(e.contains("more than imece shows"), "{e}");
        // Lines a stat could not parse are not a crash.
        assert_eq!(file_change("x"), None);
        assert_eq!(
            file_change("-\t-\tb.bin"),
            Some(FileChange {
                path: "b.bin".into(),
                added: 0,
                removed: 0,
                binary: true
            })
        );
    }

    #[test]
    fn a_tree_names_what_the_worktree_holds_and_touches_no_index() {
        let s = repo("tree");
        let w = ensure(&s.repo, &s.dir.join("wt"), "claude-1", None)
            .unwrap()
            .path;
        let head = git(&w, &["rev-parse", "HEAD^{tree}"]).unwrap();
        assert_eq!(tree(&w).unwrap(), head, "clean");
        std::fs::write(w.join("new.txt"), "new\n").unwrap();
        let t = tree(&w).unwrap();
        assert_ne!(t, head);
        assert_eq!(t, change(&w, 1 << 20).unwrap().unwrap().tree);
        assert_eq!(tree(&w).unwrap(), t, "the same content, the same tree");
        std::fs::write(w.join("new.txt"), "newer\n").unwrap();
        assert_ne!(tree(&w).unwrap(), t);
        // The agent's index is as it was, and imece's own index files go.
        assert!(
            git(&w, &["status", "--porcelain"])
                .unwrap()
                .contains("?? new.txt")
        );
        let git_dir = PathBuf::from(git(&w, &["rev-parse", "--absolute-git-dir"]).unwrap());
        let left: Vec<_> = std::fs::read_dir(&git_dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("imece-"))
            .collect();
        assert!(left.is_empty(), "{left:?}");
        assert!(tree(&s.dir.join("nowhere")).is_err());
    }

    #[test]
    fn an_applied_change_is_staged_in_the_checkout_and_committed_in_the_worktree() {
        let s = repo("apply");
        let r = &s.repo;
        let w = ensure(r, &s.dir.join("wt"), "claude-1", None).unwrap().path;
        std::fs::write(w.join("a.txt"), "a\nb\n").unwrap();
        std::fs::write(w.join("new.txt"), "new\n").unwrap();
        let c = change(&w, 1 << 20).unwrap().unwrap();
        apply(r, &c.patch).unwrap();
        assert_eq!(std::fs::read_to_string(r.join("a.txt")).unwrap(), "a\nb\n");
        assert_eq!(std::fs::read_to_string(r.join("new.txt")).unwrap(), "new\n");
        let staged = git(r, &["diff", "--cached", "--name-only"]).unwrap();
        assert_eq!(staged, "a.txt\nnew.txt");
        // The same patch again merges into what it already made: no change.
        apply(r, &c.patch).unwrap();
        assert_eq!(std::fs::read_to_string(r.join("a.txt")).unwrap(), "a\nb\n");
        assert_eq!(std::fs::read_to_string(r.join("new.txt")).unwrap(), "new\n");
        let e = apply(r, "not a patch\n").unwrap_err().to_string();
        assert!(e.starts_with("No valid patches in input"), "{e}");
        // The person's own unstaged edit in a file is never overwritten.
        std::fs::write(r.join("a.txt"), "theirs\n").unwrap();
        std::fs::write(w.join("a.txt"), "a\nb\nc\n").unwrap();
        let more = change(&w, 1 << 20).unwrap().unwrap();
        let e = apply(r, &more.patch).unwrap_err().to_string();
        assert!(e.contains("does not match index"), "{e}");
        assert_eq!(
            std::fs::read_to_string(r.join("a.txt")).unwrap(),
            "theirs\n"
        );
        std::fs::write(r.join("a.txt"), "a\nb\n").unwrap();
        std::fs::write(w.join("a.txt"), "a\nb\n").unwrap();

        let before = git(&w, &["rev-parse", "HEAD"]).unwrap();
        let id = applied(&c, "applied: fix it").unwrap();
        let head = git(&w, &["rev-parse", "HEAD"]).unwrap();
        assert_ne!(head, before);
        assert_eq!(short(&head), id);
        assert_eq!(
            git(&w, &["log", "-1", "--format=%ae %s"]).unwrap(),
            "imece@localhost applied: fix it"
        );
        assert_eq!(git(&w, &["rev-parse", "HEAD^"]).unwrap(), before);
        assert_eq!(
            git(&w, &["status", "--porcelain"]).unwrap(),
            "",
            "files and index agree with the commit"
        );
        assert_eq!(std::fs::read_to_string(w.join("new.txt")).unwrap(), "new\n");
        assert_eq!(
            change(&w, 1 << 20).unwrap(),
            None,
            "nothing since the commit"
        );
        // What comes after is the next change, alone.
        std::fs::write(w.join("later.txt"), "later\n").unwrap();
        let next = change(&w, 1 << 20).unwrap().unwrap();
        assert_eq!(next.files.len(), 1);
        assert_eq!(next.files[0].path, "later.txt");
    }

    #[test]
    fn a_change_whose_lines_moved_in_the_checkout_merges_and_a_clash_changes_nothing() {
        let s = repo("merge");
        let r = &s.repo;
        let lines = "a\nb\nc\nd\ne\nf\ng\nh\n";
        std::fs::write(r.join("log.md"), lines).unwrap();
        std::fs::write(r.join("x.md"), "x\n").unwrap();
        let base = commit(r, "base");
        let w = ensure(r, &s.dir.join("wt"), "claude-1", None).unwrap().path;
        std::fs::write(w.join("log.md"), "a\nb\nc\nd\nWORKER\ne\nf\ng\nh\n").unwrap();
        std::fs::write(w.join("x.md"), "x\ny\n").unwrap();
        let c = change(&w, 1 << 20).unwrap().unwrap();
        let index = r.join(".git").join("imece-apply-index");

        // The person changed a line next to the worker's: the plain patch
        // does not fit, the three-way merge keeps both, staged.
        std::fs::write(r.join("log.md"), "a\nb\nC\nd\ne\nf\ng\nh\n").unwrap();
        commit(r, "near");
        apply(r, &c.patch).unwrap();
        assert_eq!(
            std::fs::read_to_string(r.join("log.md")).unwrap(),
            "a\nb\nC\nd\nWORKER\ne\nf\ng\nh\n"
        );
        assert_eq!(std::fs::read_to_string(r.join("x.md")).unwrap(), "x\ny\n");
        let staged = git(r, &["diff", "--cached", "--name-only"]).unwrap();
        assert_eq!(staged, "log.md\nx.md");
        assert!(!index.exists());

        // The person wrote the same line: refused with the file, and
        // nothing of the patch reaches the checkout, not even x.md.
        git(r, &["reset", "--quiet", "--hard", &base]).unwrap();
        std::fs::write(r.join("log.md"), "a\nb\nc\nd\nPERSON\ne\nf\ng\nh\n").unwrap();
        commit(r, "clash");
        let e = apply(r, &c.patch).unwrap_err().to_string();
        assert_eq!(e, "conflicts with the checkout in log.md");
        assert_eq!(
            std::fs::read_to_string(r.join("log.md")).unwrap(),
            "a\nb\nc\nd\nPERSON\ne\nf\ng\nh\n"
        );
        assert_eq!(std::fs::read_to_string(r.join("x.md")).unwrap(), "x\n");
        assert_eq!(git(r, &["status", "--porcelain"]).unwrap(), "");
        assert!(!index.exists());

        // Every conflicting file is named, once.
        std::fs::write(r.join("x.md"), "x\nz\n").unwrap();
        commit(r, "clash too");
        let e = apply(r, &c.patch).unwrap_err().to_string();
        assert_eq!(e, "conflicts with the checkout in log.md, x.md");
        assert_eq!(git(r, &["status", "--porcelain"]).unwrap(), "");
    }

    /// A checkout and a worker's worktree on it, whose change touches the
    /// top and the bottom of `log.md` and all of `x.md`.
    fn at_odds(name: &str) -> (Scratch, PathBuf, String) {
        let s = repo(name);
        let r = s.repo.clone();
        let lines: Vec<String> = (1..=30).map(|n| format!("{n}\n")).collect();
        std::fs::write(r.join("log.md"), lines.concat()).unwrap();
        std::fs::write(r.join("x.md"), "x\n").unwrap();
        commit(&r, "base");
        let w = ensure(&r, &s.dir.join("wt"), "claude-1", None)
            .unwrap()
            .path;
        let mut ours = lines.clone();
        ours[1] = "TOP\n".into();
        ours[28] = "BOTTOM\n".into();
        std::fs::write(w.join("log.md"), ours.concat()).unwrap();
        std::fs::write(w.join("x.md"), "x\ny\n").unwrap();
        let patch = change(&w, 1 << 20).unwrap().unwrap().patch;
        (s, w, patch)
    }

    #[test]
    fn clashes_name_the_hunks_that_do_not_go_on_the_checkout_and_touch_nothing() {
        let (s, _, patch) = at_odds("clash");
        let r = &s.repo;
        // It fits: nothing clashes, whatever HEAD is.
        let none = clashes(r, &patch).unwrap();
        assert_eq!(none.hunks, []);
        assert!(!none.cut && none.staged.is_empty());
        assert_eq!(none.checkout, r.display().to_string());
        // The checkout wrote the bottom's line too: that hunk alone, with
        // git's words on it; the top's hunk and x.md go.
        let log = std::fs::read_to_string(r.join("log.md")).unwrap();
        std::fs::write(r.join("log.md"), log.replace("29\n", "PERSON\n")).unwrap();
        let head = commit(r, "clash");
        let found = clashes(r, &patch).unwrap();
        assert_eq!(found.head, short(&head));
        assert_eq!(found.hunks.len(), 1, "{:?}", found.hunks);
        let c = &found.hunks[0];
        assert_eq!(c.file, "log.md");
        let hunk = c.hunk.as_deref().unwrap();
        assert!(hunk.starts_with("@@ -26,"), "{hunk}");
        assert!(c.why.contains("patch failed: log.md:"), "{}", c.why);
        assert!(c.why.contains("log.md: patch does not apply"), "{}", c.why);
        assert_eq!(found.staged, Vec::<String>::new());
        // Nothing of it is left behind: no file, no index, no staging.
        assert_eq!(git(r, &["status", "--porcelain"]).unwrap(), "");
        let git_dir = r.join(".git");
        let left: Vec<_> = (std::fs::read_dir(&git_dir).unwrap())
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| n.starts_with("imece-"))
            .collect();
        assert_eq!(left, Vec::<String>::new());
        // A merge's stages do not carry over: every hunk is tried on the
        // checkout's index, so x.md after a clash in log.md still fits.
        std::fs::write(r.join("x.md"), "x\nz\n").unwrap();
        commit(r, "clash too");
        let both = clashes(r, &patch).unwrap();
        let files: Vec<&str> = both.hunks.iter().map(|c| c.file.as_str()).collect();
        assert_eq!(files, ["log.md", "x.md"]);
        assert!(both.hunks[1].why.contains("x.md"), "{:?}", both.hunks);
    }

    #[test]
    fn a_line_that_only_moved_merges_and_is_no_clash() {
        let (s, _, patch) = at_odds("clash-moved");
        let r = &s.repo;
        // A line of the top hunk's context changed, not next to its own:
        // the plain patch does not fit, the three-way does, as `apply`
        // takes it.
        let log = std::fs::read_to_string(r.join("log.md")).unwrap();
        let near = log.replacen("\n5\n", "\nFIVE\n", 1);
        std::fs::write(r.join("log.md"), near).unwrap();
        commit(r, "near");
        assert!(
            git_apply(r, &[], &["--check"], &patch).is_err(),
            "not plain"
        );
        assert_eq!(clashes(r, &patch).unwrap().hunks, []);
    }

    #[test]
    fn staged_changes_clash_too_and_are_named_and_unstaged_edits_do_not() {
        let (s, _, patch) = at_odds("clash-staged");
        let r = &s.repo;
        // Staged and not committed (another worker's change, applied): the
        // file that clashes is said to be staged; other.md, staged too, is
        // no clash and is not named.
        std::fs::write(r.join("x.md"), "x\nz\n").unwrap();
        std::fs::write(r.join("other.md"), "o\n").unwrap();
        git(r, &["add", "x.md", "other.md"]).unwrap();
        let found = clashes(r, &patch).unwrap();
        let files: Vec<&str> = found.hunks.iter().map(|c| c.file.as_str()).collect();
        assert_eq!(files, ["x.md"]);
        assert_eq!(found.staged, ["x.md"]);
        assert_eq!(
            git(r, &["diff", "--cached", "--name-only"]).unwrap(),
            "other.md\nx.md"
        );
        git(r, &["reset", "--quiet", "--hard"]).unwrap();
        // The person's own unstaged edit stops an apply, yet no hunk
        // clashes: there is nothing for the worker to resolve.
        std::fs::write(r.join("x.md"), "theirs\n").unwrap();
        assert!(apply(r, &patch).is_err());
        assert_eq!(clashes(r, &patch).unwrap().hunks, []);
        assert_eq!(std::fs::read_to_string(r.join("x.md")).unwrap(), "theirs\n");
    }

    #[test]
    fn a_file_without_hunks_or_with_hunks_that_only_clash_together_is_named_whole() {
        let s = repo("clash-whole");
        let r = &s.repo;
        std::fs::write(r.join("b.bin"), [0u8, 1, 2]).unwrap();
        commit(r, "base");
        let w = ensure(r, &s.dir.join("wt"), "claude-1", None).unwrap().path;
        std::fs::write(w.join("b.bin"), [0u8, 3, 4]).unwrap();
        let patch = change(&w, 1 << 20).unwrap().unwrap().patch;
        std::fs::write(r.join("b.bin"), [0u8, 5, 6]).unwrap();
        commit(r, "clash");
        let found = clashes(r, &patch).unwrap();
        assert_eq!(found.hunks.len(), 1, "{:?}", found.hunks);
        assert_eq!(found.hunks[0].file, "b.bin");
        assert_eq!(found.hunks[0].hunk, None);
        assert!(!found.hunks[0].why.is_empty());
    }

    #[test]
    fn clashes_stop_at_their_tries_and_say_so() {
        let s = repo("clash-cut");
        let r = &s.repo;
        // Hunks far apart, each a change of its own.
        let lines: Vec<String> = (0..(CLASH_TRIES + 1) * 8)
            .map(|n| format!("{n}\n"))
            .collect();
        std::fs::write(r.join("long.md"), lines.concat()).unwrap();
        commit(r, "base");
        let w = ensure(r, &s.dir.join("wt"), "claude-1", None).unwrap().path;
        let edit = |to: &str| {
            (lines.iter().enumerate())
                .map(|(n, l)| {
                    if n % 8 == 4 {
                        to.to_string() + "\n"
                    } else {
                        l.clone()
                    }
                })
                .collect::<String>()
        };
        std::fs::write(w.join("long.md"), edit("WORKER")).unwrap();
        let patch = change(&w, 1 << 22).unwrap().unwrap().patch;
        assert_eq!(patch.matches("\n@@").count(), CLASH_TRIES + 1);
        std::fs::write(r.join("long.md"), edit("PERSON")).unwrap();
        commit(r, "clash");
        let found = clashes(r, &patch).unwrap();
        assert!(found.cut);
        assert_eq!(found.hunks.len(), CLASH_TRIES);
    }

    #[test]
    fn a_patch_splits_at_its_files_headers_only() {
        let patch =
            "diff --git a/x b/x\n@@ -1 +1 @@\n-diff --git a/y b/y\n+z\ndiff --git a/b c b/b c\n";
        assert_eq!(
            file_sections(patch),
            [
                "diff --git a/x b/x\n@@ -1 +1 @@\n-diff --git a/y b/y\n+z\n",
                "diff --git a/b c b/b c\n"
            ]
        );
        assert_eq!(section_file("diff --git a/b c b/b c\n"), "b c");
        assert_eq!(
            section_file("diff --git \"a/\\303\" \"b/\\303\"\n"),
            "diff --git \"a/\\303\" \"b/\\303\""
        );
        assert_eq!(file_sections(""), Vec::<&str>::new());
    }

    #[test]
    fn a_worktree_whose_own_commits_are_imece_s_follows_the_checkout() {
        let s = repo("applied");
        let r = &s.repo;
        let root = s.dir.join("wt");
        let w = ensure(r, &root, "claude-1", None).unwrap().path;
        std::fs::write(w.join("new.txt"), "new\n").unwrap();
        let c = change(&w, 1 << 20).unwrap().unwrap();
        apply(r, &c.patch).unwrap();
        applied(&c, "applied").unwrap();
        let at = git(&w, &["rev-parse", "HEAD"]).unwrap();
        // The checkout has not committed: the worktree stays, and says nothing.
        assert_eq!(place(r, &w).unwrap(), Placed::AtHead);
        assert_eq!(git(&w, &["rev-parse", "HEAD"]).unwrap(), at);
        // The person committed (the change, and more): the worktree follows.
        std::fs::write(r.join("b.txt"), "b").unwrap();
        let new = commit(r, "took it");
        assert_eq!(
            ensure(r, &root, "claude-1", None).unwrap().placed,
            Placed::Moved {
                from: short(&at),
                to: short(&new)
            }
        );
        assert_eq!(git(&w, &["rev-parse", "HEAD"]).unwrap(), new);
        assert_eq!(std::fs::read_to_string(w.join("new.txt")).unwrap(), "new\n");
        assert_eq!(std::fs::read_to_string(w.join("b.txt")).unwrap(), "b");
        // An agent's own commit on top of imece's holds the worktree back.
        std::fs::write(w.join("mine.txt"), "mine").unwrap();
        let mine = commit(&w, "mine");
        std::fs::write(r.join("c.txt"), "c").unwrap();
        let newer = commit(r, "third");
        assert_eq!(
            place(r, &w).unwrap(),
            Placed::Kept {
                at: short(&mine),
                head: short(&newer),
                why: Why::Commits
            }
        );
    }

    #[test]
    fn a_stand_counts_lines_and_own_commits_both_ways_and_names_a_branch() {
        let s = repo("stand");
        let r = &s.repo;
        let w = ensure(r, &s.dir.join("wt"), "claude-1", None).unwrap().path;
        assert_eq!(stand(r, &w).unwrap(), Stand::default(), "fresh: nothing");
        // Two files, so a sum is told from a product or a difference.
        std::fs::write(w.join("a.txt"), "x\ny\nz\n").unwrap();
        std::fs::write(w.join("new.txt"), "1\n2\n3\n4\n5\n").unwrap();
        assert_eq!(
            stand(r, &w).unwrap(),
            Stand {
                added: 8,
                removed: 1,
                ..Stand::default()
            },
            "untracked lines count"
        );
        // imece's mark is not the worktree's own; the agent's commit is.
        let c = change(&w, 1 << 20).unwrap().unwrap();
        applied(&c, "applied").unwrap();
        assert_eq!(stand(r, &w).unwrap(), Stand::default());
        std::fs::write(w.join("mine.txt"), "m\n").unwrap();
        commit(&w, "mine");
        std::fs::write(r.join("b.txt"), "b").unwrap();
        commit(r, "one");
        commit(r, "two");
        assert_eq!(
            stand(r, &w).unwrap(),
            Stand {
                ahead: 1,
                behind: 2,
                ..Stand::default()
            }
        );
        // The agent made a branch of its own.
        git(&w, &["checkout", "--quiet", "-b", "fix-parser"]).unwrap();
        assert_eq!(stand(r, &w).unwrap().branch.as_deref(), Some("fix-parser"));
        // The agent's index is as it was, and imece's own index files go.
        let git_dir = PathBuf::from(git(&w, &["rev-parse", "--absolute-git-dir"]).unwrap());
        let left: Vec<_> = std::fs::read_dir(&git_dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("imece-"))
            .collect();
        assert!(left.is_empty(), "{left:?}");
        assert!(stand(r, &s.dir.join("nowhere")).is_err());
    }

    #[test]
    fn a_stranger_directory_is_refused() {
        let s = repo("stranger");
        let r = &s.repo;
        let root = s.dir.join("wt");
        std::fs::create_dir_all(root.join("claude-1")).unwrap();
        std::fs::write(root.join("claude-1/mine.txt"), "x").unwrap();
        assert!(ensure(r, &root, "claude-1", None).is_err());
        assert!(root.join("claude-1/mine.txt").exists());
    }

    #[test]
    fn a_worktree_removed_by_hand_is_said_to_be_gone_and_made_anew_only_after_a_prune() {
        let s = repo("vanished");
        let r = &s.repo;
        // Reached through a link, as a temp dir may be: git keeps the real
        // path.
        let link = s.dir.join("link");
        std::os::unix::fs::symlink(&s.dir, &link).unwrap();
        let root = link.join("wt");
        let w = ensure(r, &root, "claude-1", None).unwrap().path;
        std::fs::remove_dir_all(&w).unwrap();
        let e = ensure(r, &root, "claude-1", None).expect_err("refused");
        assert_eq!(
            e.to_string(),
            format!(
                "{} is gone, but git still has it as a worktree of {}: whatever it held \
                 that was not committed is lost; `git worktree prune` there lets imece make \
                 it anew",
                root.join("claude-1").display(),
                r.display()
            )
        );
        assert!(!w.exists(), "nothing made behind the person's back");
        git(r, &["worktree", "prune"]).unwrap();
        let again = ensure(r, &root, "claude-1", None).unwrap();
        assert_eq!((again.path, again.placed), (w, Placed::Fresh));
    }

    #[test]
    fn a_worktree_git_can_not_add_says_what_git_said() {
        // No commit yet: no HEAD to check out, and nothing of git's list.
        let s = repo("headless");
        let empty = s.dir.join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        git(&empty, &["init", "--quiet"]).unwrap();
        let e = ensure(&empty, &s.dir.join("wt"), "claude-1", None)
            .expect_err("refused")
            .to_string();
        assert!(e.starts_with("git worktree add --quiet --detach "), "{e}");
        assert!(!e.contains("is gone"), "{e}");
    }

    #[test]
    fn a_worktree_of_another_repository_is_refused() {
        let ours = repo("foreign");
        let theirs = repo("foreign-other");
        let root = ours.dir.join("wt");
        // A real worktree under our name, but of another repository.
        let w = ensure(&theirs.repo, &root, "claude-1", None).unwrap().path;
        assert!(ensure(&ours.repo, &root, "claude-1", None).is_err());
        assert!(w.join("a.txt").exists(), "nothing of theirs was touched");
    }

    #[test]
    fn a_subset_keeps_the_chosen_hunks_under_their_file_s_header() {
        let patch = "\
diff --git a/a b/a
index 1..2 100644
--- a/a
+++ b/a
@@ -1 +1 @@
-x
+y
@@ -9 +9 @@
-p
+q
diff --git a/img b/img
index 3..4 100644
GIT binary patch
literal 1
Ic${NkU|@&?00JHW1^@s6

diff --git a/old b/new
similarity index 90%
rename from old
rename to new
--- a/old
+++ b/new
@@ -2 +2 @@
-m
+n
";
        assert_eq!(subset(patch, &[]), "");
        assert_eq!(
            subset(patch, &[7]),
            "diff --git a/a b/a\nindex 1..2 100644\n--- a/a\n+++ b/a\n@@ -9 +9 @@\n-p\n+q\n"
        );
        assert_eq!(
            subset(patch, &[4, 22]),
            "diff --git a/a b/a\nindex 1..2 100644\n--- a/a\n+++ b/a\n@@ -1 +1 @@\n-x\n+y\n\
             diff --git a/old b/new\nsimilarity index 90%\nrename from old\nrename to new\n\
             --- a/old\n+++ b/new\n@@ -2 +2 @@\n-m\n+n\n"
        );
        // A line index that is no hunk's chooses nothing.
        assert_eq!(subset(patch, &[5, 12, 0]), "");
    }

    #[test]
    fn a_part_applied_and_marked_leaves_the_rest_for_the_next_change() {
        let s = repo("part");
        let r = &s.repo;
        let lines: Vec<String> = (1..=20).map(|n| format!("{n}\n")).collect();
        std::fs::write(r.join("a.txt"), lines.concat()).unwrap();
        commit(r, "twenty");
        let mut edited = lines.clone();
        edited[1] = "two\n".into();
        edited[17] = "eighteen\n".into();
        std::fs::write(r.join("a.txt"), edited.concat()).unwrap();
        std::fs::write(r.join("b.txt"), "b\n").unwrap();
        let c = change(r, 1 << 20).unwrap().unwrap();
        let hunks: Vec<usize> = (c.patch.lines().enumerate())
            .filter(|(_, l)| l.starts_with("@@"))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(hunks.len(), 3, "{}", c.patch);
        // The first hunk of a.txt and b.txt's.
        let sub = subset(&c.patch, &[hunks[0], hunks[2]]);
        let p = part(&c, &sub).unwrap();
        assert_eq!(p.patch, sub);
        assert_ne!(p.tree, c.tree);
        let files: Vec<(String, u32, u32)> = (p.files.iter())
            .map(|f| (f.path.clone(), f.added, f.removed))
            .collect();
        assert_eq!(files, [("a.txt".into(), 1, 1), ("b.txt".into(), 1, 0)]);
        // Nothing of the worktree moved yet.
        assert_eq!(
            git(r, &["status", "--porcelain"]).unwrap(),
            "M a.txt\n?? b.txt"
        );
        applied(&p, "applied: part").unwrap();
        // The files are as they were; only the rest is a change now.
        assert_eq!(
            std::fs::read_to_string(r.join("a.txt")).unwrap(),
            edited.concat()
        );
        let rest = change(r, 1 << 20).unwrap().unwrap();
        let files: Vec<(String, u32, u32)> = (rest.files.iter())
            .map(|f| (f.path.clone(), f.added, f.removed))
            .collect();
        assert_eq!(files, [("a.txt".into(), 1, 1)]);
        assert!(rest.patch.contains("+eighteen") && !rest.patch.contains("+two"));
        // A part that does not fit is refused; no index file is left.
        assert!(
            part(
                &c,
                "diff --git a/z b/z\n--- a/z\n+++ b/z\n@@ -1 +1 @@\n-no\n+yes\n"
            )
            .is_err()
        );
        let git_dir = PathBuf::from(git(r, &["rev-parse", "--absolute-git-dir"]).unwrap());
        let left: Vec<String> = std::fs::read_dir(&git_dir)
            .unwrap()
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| n.starts_with("imece-part"))
            .collect();
        assert!(left.is_empty(), "{left:?}");
    }
}
