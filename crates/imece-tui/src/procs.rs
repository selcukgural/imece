//! The processes the agents' sessions run (`:ps`, record 129, E16 of
//! #58): each session's process and every process under it, with the CPU
//! and memory `ps` reports, and ending one of them (never the session
//! itself, never imece's own bus relay) after checking again, just before
//! the signal, that it is still under that session. And the processes left
//! from an earlier run of imece (E16's second half): a process whose
//! command line names imece's worktrees with no imece above it, and what
//! runs under it; listed, and ended only on the person's word.
//!
//! Every process a session starts carries the session's mark in its
//! environment ([`MARK_ENV`]), one that leaves the session's tree too
//! (`nohup … &`: its own group, its parent gone; acceptance, 2026-11-11):
//! `:ps` lists such a one under its agent and may end it, imece's quit
//! ends whatever carries this run's mark, and a later start finds what
//! carries the mark of a run no imece runs any more.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use imece_agent::child;
pub use imece_agent::claude::MARK_ENV;

use crate::model::AgentId;

/// How long `ps` may take.
pub const PS_WAIT: Duration = Duration::from_secs(5);

/// The most processes listed under one session: a fork bomb's tree is cut.
pub const MOST_UNDER: usize = 200;

/// After `SIGTERM`, how long a process has before `SIGKILL`.
pub const KILL_GRACE: Duration = Duration::from_secs(2);

/// The most trees left from an earlier run listed.
pub const MOST_LEFT: usize = 20;

/// How far up a process's parents are followed.
const MOST_UP: usize = 64;

/// The longest place said of a tree left from an earlier run.
const PLACE_MOST: usize = 80;

/// One process as `ps` reported it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proc {
    pub pid: u32,
    pub ppid: u32,
    /// Its CPU in tenths of a percent of one core (`ps`'s `pcpu`).
    pub cpu: u32,
    /// Its resident memory, in KiB.
    pub rss: u64,
    /// Its program's name, without a directory.
    pub name: String,
}

/// The processes of the agents' sessions as last read. Memory only.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Procs {
    /// Per agent with a session running: its session's process first,
    /// then those under it, parents before children.
    pub agents: Vec<(AgentId, Vec<Proc>)>,
    /// A look is under way.
    pub looking: bool,
    /// Why the last look failed; none when it did not.
    pub failed: Option<String>,
    /// The trees left from an earlier run of imece.
    pub left: Vec<Left>,
}

/// A tree of processes left from an earlier run of imece: a process whose
/// command line names imece's worktrees, with no imece above it (a run
/// that ended without ending its sessions), and those under it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Left {
    /// What its command line names under the worktrees, as
    /// `imece-b69361b1-notes/claude-3`; empty when it was not read.
    pub place: String,
    /// It first, then those under it, parents before children.
    pub procs: Vec<Proc>,
}

impl Procs {
    /// All of an agent's processes together: CPU (tenths of a percent)
    /// and memory (KiB).
    pub fn sum(procs: &[Proc]) -> (u32, u64) {
        procs
            .iter()
            .fold((0, 0), |(c, r), p| (c.saturating_add(p.cpu), r + p.rss))
    }
}

/// `ps`'s `pid= ppid=` lines; what is not two numbers is left out.
pub fn pairs(text: &str) -> Vec<(u32, u32)> {
    text.lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let pair = (w.next()?.parse().ok()?, w.next()?.parse().ok()?);
            w.next().is_none().then_some(pair)
        })
        .collect()
}

/// `root` and every process under it, parents before children, at most
/// [`MOST_UNDER`] in all.
pub fn under(pairs: &[(u32, u32)], root: u32) -> Vec<u32> {
    let mut found = vec![root];
    // A bounded range: each listed process is a parent looked at once.
    for i in 0..MOST_UNDER {
        let Some(&parent) = found.get(i) else {
            break;
        };
        for &(pid, ppid) in pairs {
            if ppid == parent && pid != parent && !found.contains(&pid) {
                if found.len() == MOST_UNDER {
                    return found;
                }
                found.push(pid);
            }
        }
    }
    found
}

/// `12.3` as tenths: 123. Anything else is none.
fn tenths(s: &str) -> Option<u32> {
    let (whole, part) = s.split_once('.').unwrap_or((s, "0"));
    let whole: u32 = whole.parse().ok()?;
    let digit = part.chars().next()?.to_digit(10)?;
    whole.checked_mul(10)?.checked_add(digit)
}

/// `ps`'s `pid= ppid= pcpu= rss= comm=` lines (the name the rest of the
/// line, spaces and all, its directory left out); what is not read is
/// left out.
pub fn parse(text: &str) -> Vec<Proc> {
    text.lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let pid = w.next()?.parse().ok()?;
            let ppid = w.next()?.parse().ok()?;
            let cpu = tenths(w.next()?)?;
            let rss = w.next()?.parse().ok()?;
            let rest: Vec<&str> = w.collect();
            let path = rest.join(" ");
            let name = path.rsplit('/').next().unwrap_or_default().to_string();
            (!name.is_empty()).then_some(Proc {
                pid,
                ppid,
                cpu,
                rss,
                name,
            })
        })
        .collect()
}

/// `ps` with `args`, in the C locale (a decimal point, not a comma), on
/// the system's own path.
fn ps(args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("ps");
    cmd.args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C");
    match child::ask(cmd, PS_WAIT) {
        Some((status, out)) if status.success() => Ok(out),
        Some((status, out)) => Err(format!(
            "ps ended with {status}: {}",
            out.lines().next().unwrap_or_default()
        )),
        None => Err("ps did not answer".into()),
    }
}

/// The processes of each session (`roots`: an agent and its session's
/// process): every process's parent first (a short answer whatever the
/// machine runs), then CPU and memory of those under the sessions alone;
/// a session whose process is gone is left out. After each session's
/// tree, what carries its mark (`marked`: an agent and its session's
/// mark) outside it: a process that left the session.
pub fn read(
    roots: &[(AgentId, u32)],
    marked: &[(AgentId, String)],
) -> Result<Vec<(AgentId, Vec<Proc>)>, String> {
    if roots.is_empty() {
        return Ok(Vec::new());
    }
    let pairs = pairs(&ps(&["-A", "-o", "pid=,ppid="])?);
    // A session gone is not asked about: `ps` refuses a whole list for
    // one number it does not take.
    let mut trees: Vec<(AgentId, Vec<u32>)> = (roots.iter())
        .filter(|&&(_, root)| pairs.iter().any(|&(pid, _)| pid == root))
        .map(|&(id, root)| (id, under(&pairs, root)))
        .collect();
    if trees.is_empty() {
        return Ok(Vec::new());
    }
    if !marked.is_empty() {
        let marks = marks().unwrap_or_default();
        for (id, mark) in marked {
            let all: Vec<Vec<u32>> = trees.iter().map(|(_, t)| t.clone()).collect();
            if let Some((_, tree)) = trees.iter_mut().find(|(a, _)| a == id) {
                tree.extend(left_session(mark, &marks, &pairs, &all));
            }
        }
    }
    let pids: Vec<u32> = trees.iter().flat_map(|(_, p)| p.clone()).collect();
    Ok(group(&trees, &listed(&pids)?))
}

/// `ps -o pid=,ppid=,pcpu=,rss=,comm= -p LIST` for `pids`: what it read.
/// A process gone meanwhile is simply not listed (`ps` says so by its
/// status alone, so what it wrote is read either way).
fn listed(pids: &[u32]) -> Result<Vec<Proc>, String> {
    if pids.is_empty() {
        return Ok(Vec::new());
    }
    let list: Vec<String> = pids.iter().map(u32::to_string).collect();
    let mut cmd = Command::new("ps");
    cmd.args(["-o", "pid=,ppid=,pcpu=,rss=,comm=", "-p", &list.join(",")])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C");
    match child::ask(cmd, PS_WAIT) {
        Some((_, out)) => Ok(parse(&out)),
        None => Err("ps did not answer".into()),
    }
}

/// The processes `procs` in each tree's order; one not read is left out,
/// and a tree whose session's process is gone with it.
pub fn group(trees: &[(AgentId, Vec<u32>)], procs: &[Proc]) -> Vec<(AgentId, Vec<Proc>)> {
    trees
        .iter()
        .filter_map(|(id, pids)| {
            let listed: Vec<Proc> = (pids.iter())
                .filter_map(|pid| procs.iter().find(|p| p.pid == *pid).cloned())
                .collect();
            let root = listed.first().map(|p| p.pid);
            (root == pids.first().copied()).then_some((*id, listed))
        })
        .collect()
}

/// A run's part of a session's mark: this process's number, the second
/// it was made at, and a count, so each `Live` (tests side by side
/// included) has one of its own; the number tells whether the run's
/// imece still runs.
pub fn new_run() -> String {
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let n = COUNT.fetch_add(1, Ordering::Relaxed);
    format!("{}-{secs}-{n}", std::process::id())
}

/// The process a run's mark was made by.
fn run_pid(run: &str) -> Option<u32> {
    run.split('-').next()?.parse().ok()
}

/// `word` as one part of a mark: letters, digits, `.`, `_` and `-` as
/// they are, anything else `_`, at most 40 characters.
fn mark_word(word: &str) -> String {
    (word.chars().take(40))
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => c,
            _ => '_',
        })
        .collect()
}

/// The mark of a session of agent `agent` of project `project` in run
/// `run`: `RUN/PROJECT/AGENT`, one word, no space.
pub fn mark(run: &str, project: &str, agent: &str) -> String {
    format!("{run}/{}/{}", mark_word(project), mark_word(agent))
}

/// A mark's run and its place (`PROJECT/AGENT`).
pub fn mark_parts(mark: &str) -> Option<(&str, &str)> {
    mark.split_once('/')
        .filter(|(run, place)| !run.is_empty() && !place.is_empty())
}

/// The mark in one process's environment (`NAME=value` a NUL apart).
fn mark_in_environ(bytes: &[u8]) -> Option<String> {
    let key = format!("{MARK_ENV}=");
    (bytes.split(|b| *b == 0))
        .find_map(|v| v.strip_prefix(key.as_bytes()))
        .and_then(|v| std::str::from_utf8(v).ok())
        .filter(|v| mark_parts(v).is_some())
        .map(str::to_string)
}

/// Process `pid`'s environment as it started, `NAME=value` a NUL apart;
/// none for one of another user's, or gone.
#[cfg(target_os = "linux")]
fn environ(pid: u32) -> Option<Vec<u8>> {
    std::fs::read(format!("/proc/{pid}/environ")).ok()
}

/// Process `pid`'s environment as it started (`KERN_PROCARGS2`: the
/// count of its arguments, its program's path, the arguments, then the
/// environment, each ending in a NUL); none for one of another user's,
/// one gone, and macOS's own programs (it shows none of theirs, as `ps
/// -E` shows none: a server of the system's own goes unmarked).
#[cfg(target_os = "macos")]
fn environ(pid: u32) -> Option<Vec<u8>> {
    let mut mib = [
        libc::CTL_KERN,
        libc::KERN_PROCARGS2,
        i32::try_from(pid).ok()?,
    ];
    let mut size: libc::size_t = 0;
    let null = std::ptr::null_mut();
    // SAFETY: a size query, then a read into a buffer of that size.
    if unsafe { libc::sysctl(mib.as_mut_ptr(), 3, null, &mut size, null, 0) } != 0 {
        return None;
    }
    let mut buf = vec![0u8; size];
    let at = buf.as_mut_ptr().cast();
    if unsafe { libc::sysctl(mib.as_mut_ptr(), 3, at, &mut size, null, 0) } != 0 {
        return None;
    }
    buf.truncate(size);
    environ_in_procargs(&buf)
}

/// The environment in `KERN_PROCARGS2`'s answer.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn environ_in_procargs(buf: &[u8]) -> Option<Vec<u8>> {
    let argc = i32::from_ne_bytes(buf.get(..4)?.try_into().ok()?);
    let mut rest = buf.get(4..)?;
    // The program's path, then the NULs that pad it.
    rest = &rest[rest.iter().position(|b| *b == 0)?..];
    rest = &rest[rest.iter().position(|b| *b != 0)?..];
    for _ in 0..argc.max(0) {
        let end = rest.iter().position(|b| *b == 0)?;
        rest = rest.get(end + 1..)?;
    }
    Some(rest.to_vec())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn environ(_: u32) -> Option<Vec<u8>> {
    None
}

/// Of the processes `pids`, those that carry a mark, each with it (only
/// one's own processes' environments can be read).
pub fn marks_of(pids: &[u32]) -> Vec<(u32, String)> {
    (pids.iter())
        .filter_map(|&pid| Some((pid, mark_in_environ(&environ(pid)?)?)))
        .collect()
}

/// Every process that carries a mark, each with it.
pub fn marks() -> Result<Vec<(u32, String)>, String> {
    let pairs = pairs(&ps(&["-A", "-o", "pid=,ppid="])?);
    Ok(marks_of(
        &pairs.iter().map(|&(pid, _)| pid).collect::<Vec<_>>(),
    ))
}

/// The mark process `pid` carries, read now; none when it carries none
/// or is gone.
fn mark_of(pid: u32) -> Option<String> {
    marks_of(&[pid]).pop().map(|(_, m)| m)
}

/// Of `marks`, those that carry `mark` and are in none of `trees`: the
/// first of each (its parent carries no such mark) and those under it,
/// parents first, as `pairs` has them. A process that left its session
/// is listed with it so.
pub fn left_session(
    mark: &str,
    marks: &[(u32, String)],
    pairs: &[(u32, u32)],
    trees: &[Vec<u32>],
) -> Vec<u32> {
    let listed = |pid: u32| trees.iter().any(|t| t.contains(&pid));
    let mut found: Vec<u32> = Vec::new();
    for root in marked_roots(mark, marks, pairs, trees) {
        for pid in under(pairs, root) {
            if !found.contains(&pid) && !listed(pid) && found.len() < MOST_UNDER {
                found.push(pid);
            }
        }
    }
    found
}

/// Of `marks`, the processes that carry `mark`, are in none of `trees`
/// and whose parent carries no such mark: the first of each tree of them.
fn marked_roots(
    mark: &str,
    marks: &[(u32, String)],
    pairs: &[(u32, u32)],
    trees: &[Vec<u32>],
) -> Vec<u32> {
    let listed = |pid: u32| trees.iter().any(|t| t.contains(&pid));
    let ours: Vec<u32> = (marks.iter())
        .filter(|(pid, m)| m == mark && !listed(*pid))
        .map(|&(pid, _)| pid)
        .collect();
    let parent = |pid: u32| pairs.iter().find(|&&(p, _)| p == pid).map(|&(_, up)| up);
    (ours.iter().copied())
        .filter(|&pid| !parent(pid).is_some_and(|up| ours.contains(&up)))
        .collect()
}

/// Ends every process that carries run `run`'s mark but this one:
/// `SIGTERM`, and `SIGKILL` for each still there after [`KILL_GRACE`].
/// imece's quit, once its sessions ended: what left a session goes too,
/// and a session that outlived its own end. How many were sent the first.
pub fn end_marked(run: &str) -> usize {
    let Ok(marks) = marks() else {
        return 0;
    };
    let me = std::process::id();
    let ours: Vec<u32> = (marks.into_iter())
        .filter(|(pid, m)| *pid != me && mark_parts(m).is_some_and(|(r, _)| r == run))
        .map(|(pid, _)| pid)
        .collect();
    for &pid in &ours {
        signal(pid, libc::SIGTERM);
    }
    let alive = |pid: u32| i32::try_from(pid).is_ok_and(|p| unsafe { libc::kill(p, 0) } == 0);
    let start = Instant::now();
    while start.elapsed() < KILL_GRACE && ours.iter().any(|&p| alive(p)) {
        std::thread::sleep(Duration::from_millis(50));
    }
    for &pid in &ours {
        if alive(pid) && mark_of(pid).is_some_and(|m| m.starts_with(&format!("{run}/"))) {
            signal(pid, libc::SIGKILL);
        }
    }
    ours.len()
}

/// Of `marks`, those of a run no imece runs any more: not this one
/// (process `me`), and its process gone from `pairs` (a number taken
/// again by another program meanwhile keeps it, as a run alive would).
pub fn dead_runs(marks: &[(u32, String)], pairs: &[(u32, u32)], me: u32) -> Vec<(u32, String)> {
    (marks.iter())
        .filter(|(_, m)| {
            mark_parts(m)
                .and_then(|(r, _)| run_pid(r))
                .is_some_and(|run| run != me && !pairs.iter().any(|&(p, _)| p == run))
        })
        .cloned()
        .collect()
}

/// Why a process was not ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// It is the agent's session itself: `:forget` or a stop ends that.
    Session,
    /// It is imece's own bus relay for the session.
    Bus,
    /// It is not under the session (any more): gone, or someone else's.
    NotUnder,
}

/// Ends process `pid` (named `name` when read) under session `root`, or
/// one that left it carrying its `mark`: checked again first, then
/// `SIGTERM`, and `SIGKILL` after [`KILL_GRACE`] if it is still there and
/// still the session's. The session's own process and imece's relay (a
/// program named as imece is, right under the session) are refused.
pub fn end(pid: u32, root: u32, name: &str, mark: Option<&str>) -> Result<(), Refusal> {
    if pid == root {
        return Err(Refusal::Session);
    }
    let mark = mark.map(str::to_string);
    let ours = move |pid: u32| {
        child::descends(pid, root)
            || (mark.as_deref()).is_some_and(|m| mark_of(pid).as_deref() == Some(m))
    };
    if !ours(pid) {
        return Err(Refusal::NotUnder);
    }
    if relay(pid, root, name, own_name().as_deref()) {
        return Err(Refusal::Bus);
    }
    signal(pid, libc::SIGTERM);
    std::thread::spawn(move || {
        std::thread::sleep(KILL_GRACE);
        if ours(pid) {
            signal(pid, libc::SIGKILL);
        }
    });
    Ok(())
}

/// What [`end`] would refuse of process `pid`, by a session's processes
/// as last read (`listed`, the session's own first) and the name imece's
/// program runs under (`own`): asked before the `y`, so a refusal is said
/// at once and no `y` is asked for nothing. [`end`] checks again.
pub fn refused(listed: &[Proc], pid: u32, own: Option<&str>) -> Option<Refusal> {
    let root = listed.first().map(|p| p.pid);
    let Some(p) = listed.iter().find(|p| p.pid == pid) else {
        return Some(Refusal::NotUnder);
    };
    if Some(pid) == root {
        return Some(Refusal::Session);
    }
    (Some(p.name.as_str()) == own && Some(p.ppid) == root).then_some(Refusal::Bus)
}

/// Sends `sig` to process `pid`; a number past i32 is no process of ours.
fn signal(pid: u32, sig: i32) {
    if let Ok(p) = i32::try_from(pid) {
        unsafe { libc::kill(p, sig) };
    }
}

/// Whether `pid`, named `name`, is imece's relay for session `root`: a
/// program named as imece's own (`own`) right under the session.
fn relay(pid: u32, root: u32, name: &str, own: Option<&str>) -> bool {
    Some(name) == own && child::parent_of(pid) == Some(root)
}

/// The name imece's own program runs under (its relay's too).
pub(crate) fn own_name() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.file_name()?.to_string_lossy().into_owned())
}

/// `s` with every character an extended regular expression gives a
/// meaning to escaped: only the text itself matches.
pub fn literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "\\.^$*+?()[]{}|".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// How a command line names something under imece's worktrees directory
/// `worktrees`: the directory and a `/`.
fn named(worktrees: &Path) -> String {
    format!("{}/", worktrees.to_string_lossy())
}

/// The numbers in `pgrep`'s answer, one a line; anything else is left out.
fn numbers(text: &str) -> Vec<u32> {
    text.lines().filter_map(|l| l.trim().parse().ok()).collect()
}

/// This user's processes whose command line names something under
/// `worktrees` (`pgrep -f`); none is an empty list.
fn naming(worktrees: &Path) -> Result<Vec<u32>, String> {
    let uid = unsafe { libc::getuid() }.to_string();
    let mut cmd = Command::new("pgrep");
    cmd.args(["-U", &uid, "-f", &literal(&named(worktrees))])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C");
    match child::ask(cmd, PS_WAIT) {
        // 1: nothing matched.
        Some((status, out)) if status.success() || status.code() == Some(1) => Ok(numbers(&out)),
        Some((status, out)) => Err(format!(
            "pgrep ended with {status}: {}",
            out.lines().next().unwrap_or_default()
        )),
        None => Err("pgrep did not answer".into()),
    }
}

/// The processes above `pid` in `pairs`, its parent first, at most
/// [`MOST_UP`]; the walk stops at one with no parent listed, at 0, and at
/// a loop.
pub fn above(pairs: &[(u32, u32)], pid: u32) -> Vec<u32> {
    let mut up: Vec<u32> = Vec::new();
    let mut at = pid;
    // A bounded range: each step is one parent.
    for _ in 0..MOST_UP {
        let Some(&(_, parent)) = pairs.iter().find(|&&(p, _)| p == at) else {
            break;
        };
        if parent == 0 || parent == pid || up.contains(&parent) {
            break;
        }
        up.push(parent);
        at = parent;
    }
    up
}

/// Whether a process `ps` names `name` is imece's own program, named
/// `own` (Linux cuts a process's name at 15 bytes).
pub fn is_imece(name: &str, own: &str) -> bool {
    name == own || (name.len() == 15 && own.starts_with(name))
}

/// Of the processes `naming` imece's worktrees, those left from an earlier
/// run, in their order, at most [`MOST_LEFT`]: one is not imece (named
/// `own`, its relay too), nothing above it is this imece (process `me`)
/// or another imece, and none above it names the worktrees too (it is in
/// that one's tree then). `procs` names them and those above them.
pub fn left_roots(
    naming: &[u32],
    pairs: &[(u32, u32)],
    procs: &[Proc],
    own: &str,
    me: u32,
) -> Vec<u32> {
    let imece = |pid: u32| {
        pid == me
            || (procs.iter())
                .find(|p| p.pid == pid)
                .is_some_and(|p| is_imece(&p.name, own))
    };
    (naming.iter().copied())
        .filter(|&pid| {
            !imece(pid)
                && (above(pairs, pid).into_iter()).all(|up| !imece(up) && !naming.contains(&up))
        })
        .take(MOST_LEFT)
        .collect()
}

/// What a command line `args` names under `worktrees`: the word after
/// it, cut at [`PLACE_MOST`] characters; empty when it names nothing.
pub fn place_in(args: &str, worktrees: &Path) -> String {
    let named = named(worktrees);
    let Some(at) = args.find(&named) else {
        return String::new();
    };
    let rest = &args[at + named.len()..];
    let word = rest.split_whitespace().next().unwrap_or_default();
    word.chars().take(PLACE_MOST).collect()
}

/// The trees left from an earlier run of imece (named `own`; this one is
/// process `me`): each process whose command line names something under
/// `worktrees` with no imece above it, and those under it.
pub fn leftovers(worktrees: &Path, own: &str, me: u32) -> Result<Vec<Left>, String> {
    let naming = naming(worktrees)?;
    let marks = marks().unwrap_or_default();
    if naming.is_empty() && marks.is_empty() {
        return Ok(Vec::new());
    }
    let pairs = pairs(&ps(&["-A", "-o", "pid=,ppid="])?);
    let mut found = named_leftovers(&naming, &pairs, worktrees, own, me)?;
    found.extend(marked_leftovers(&marks, &pairs, &found, me)?);
    found.truncate(MOST_LEFT);
    Ok(found)
}

/// What carries the mark of a run no imece runs any more (`marks`), in
/// none of the trees `found` already: a tree for each process whose
/// parent carries no such mark, its place the mark's.
fn marked_leftovers(
    marks: &[(u32, String)],
    pairs: &[(u32, u32)],
    found: &[Left],
    me: u32,
) -> Result<Vec<Left>, String> {
    let dead = dead_runs(marks, pairs, me);
    let taken: Vec<Vec<u32>> = (found.iter())
        .map(|l| l.procs.iter().map(|p| p.pid).collect())
        .collect();
    let mut trees: Vec<(usize, Vec<u32>)> = Vec::new();
    let mut places: Vec<String> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for (_, m) in &dead {
        if seen.contains(m) || trees.len() + found.len() >= MOST_LEFT {
            continue;
        }
        seen.push(m.clone());
        let place: String = mark_parts(m)
            .map_or("", |(_, p)| p)
            .chars()
            .take(PLACE_MOST)
            .collect();
        let mut all = taken.clone();
        all.extend(trees.iter().map(|(_, t)| t.clone()));
        // Each a tree of its own, its first first, as a session's is.
        for root in marked_roots(m, &dead, pairs, &all) {
            let tree: Vec<u32> = (under(pairs, root).into_iter())
                .filter(|p| !all.iter().any(|t| t.contains(p)))
                .collect();
            all.push(tree.clone());
            places.push(place.clone());
            trees.push((trees.len(), tree));
        }
    }
    let pids: Vec<u32> = trees.iter().flat_map(|(_, t)| t.clone()).collect();
    let procs = listed(&pids)?;
    Ok((group(&trees, &procs).into_iter())
        .map(|(i, procs)| Left {
            place: places[i].clone(),
            procs,
        })
        .collect())
}

/// The trees of the processes `naming` imece's worktrees left from an
/// earlier run.
fn named_leftovers(
    naming: &[u32],
    pairs: &[(u32, u32)],
    worktrees: &Path,
    own: &str,
    me: u32,
) -> Result<Vec<Left>, String> {
    if naming.is_empty() {
        return Ok(Vec::new());
    }
    let naming = naming.to_vec();
    let pairs = pairs.to_vec();
    let mut asked = naming.clone();
    for &pid in &naming {
        for up in above(&pairs, pid) {
            if !asked.contains(&up) {
                asked.push(up);
            }
        }
    }
    let roots = left_roots(&naming, &pairs, &listed(&asked)?, own, me);
    let trees: Vec<(usize, Vec<u32>)> = (roots.iter().enumerate())
        .map(|(i, &root)| (i, under(&pairs, root)))
        .collect();
    let pids: Vec<u32> = trees.iter().flat_map(|(_, p)| p.clone()).collect();
    let procs = listed(&pids)?;
    Ok((group(&trees, &procs).into_iter())
        .map(|(i, procs)| {
            let args = ps(&["-ww", "-o", "args=", "-p", &roots[i].to_string()]);
            Left {
                place: place_in(&args.unwrap_or_default(), worktrees),
                procs,
            }
        })
        .collect())
}

/// Of `roots`, those still left from an earlier run, as [`leftovers`]
/// tells one; none when that cannot be read.
fn still_left(roots: &[u32], worktrees: &Path, own: &str, me: u32) -> Vec<u32> {
    let Ok(naming) = naming(worktrees) else {
        return Vec::new();
    };
    let Ok(text) = ps(&["-A", "-o", "pid=,ppid="]) else {
        return Vec::new();
    };
    let pairs = pairs(&text);
    // Those that carry the mark of a run no imece runs any more.
    let marks = marks().unwrap_or_default();
    let dead = dead_runs(&marks, &pairs, me);
    let marked: Vec<u32> = (roots.iter().copied())
        .filter(|r| dead.iter().any(|(p, _)| p == r))
        .collect();
    let roots: Vec<u32> = roots
        .iter()
        .copied()
        .filter(|r| naming.contains(r))
        .collect();
    let mut asked = roots.clone();
    for &root in &roots {
        asked.extend(above(&pairs, root));
    }
    let mut left = match listed(&asked) {
        Ok(named) => left_roots(&roots, &pairs, &named, own, me),
        Err(_) => Vec::new(),
    };
    for r in marked {
        if !left.contains(&r) {
            left.push(r);
        }
    }
    left
}

/// Of `asked` (each a process and the first of the tree left from an
/// earlier run it was listed in), those still in that tree: the tree
/// still left (`roots`), and the process it or under it.
fn in_trees(asked: &[(u32, u32)], roots: &[u32]) -> Vec<(u32, u32)> {
    (asked.iter().copied())
        .filter(|&(pid, root)| roots.contains(&root) && (pid == root || child::descends(pid, root)))
        .collect()
}

/// Ends the processes `asked` left from an earlier run (each with the
/// first of its tree): checked again first (the tree still left, the
/// process still in it), then `SIGTERM`, and `SIGKILL` after
/// [`KILL_GRACE`] for each still there and still in its tree. How many
/// were sent the first.
pub fn end_left(asked: &[(u32, u32)], worktrees: &Path, own: &str, me: u32) -> usize {
    let mut roots: Vec<u32> = asked.iter().map(|&(_, root)| root).collect();
    roots.sort_unstable();
    roots.dedup();
    let ended = in_trees(asked, &still_left(&roots, worktrees, own, me));
    for &(pid, _) in &ended {
        signal(pid, libc::SIGTERM);
    }
    // Nothing sent, nothing to follow up.
    if ended.is_empty() {
        return 0;
    }
    let n = ended.len();
    let (worktrees, own) = (worktrees.to_path_buf(), own.to_string());
    std::thread::spawn(move || {
        std::thread::sleep(KILL_GRACE);
        let roots = still_left(&roots, &worktrees, &own, me);
        for (pid, _) in in_trees(&ended, &roots) {
            signal(pid, libc::SIGKILL);
        }
    });
    n
}

/// What looks for what an earlier run left sees every test's: the tests
/// that look, or leave one, go one at a time.
#[cfg(test)]
pub(crate) fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    ONE.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
/// A `sleep` whose environment `ps` shows: macOS shows none of its
/// own programs' (`/bin/sleep` is one), so there a copy signed anew.
pub(crate) fn sleeper() -> String {
    static MADE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    MADE.get_or_init(|| {
        if !cfg!(target_os = "macos") {
            return "sleep".into();
        }
        let dir = std::env::temp_dir().join(format!("imece-sleeper-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let copy = dir.join("sleep");
        std::fs::copy("/bin/sleep", &copy).unwrap();
        let signed = Command::new("codesign")
            .args(["-s", "-", "-f"])
            .arg(&copy)
            .output()
            .unwrap();
        assert!(signed.status.success(), "{signed:?}");
        copy.display().to_string()
    })
    .clone()
}

#[cfg(test)]
/// A process that left its session: started by one in the background
/// with the mark, its parent gone; its number.
pub(crate) fn left_behind(mark: &str) -> u32 {
    let out = Command::new("sh")
        .args([
            "-c",
            &format!("'{}' 60 >/dev/null 2>&1 & echo $!", sleeper()),
        ])
        .env(MARK_ENV, mark)
        .output()
        .unwrap();
    let pid = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap();
    // Until it runs the sleeper it is the shell, whose environment
    // macOS does not show.
    let start = std::time::Instant::now();
    while mark_of(pid).is_none() && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(10));
    }
    pid
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Stdio};
    use std::time::Instant;

    use super::*;

    #[test]
    fn parents_are_read_in_pairs_and_a_tree_parents_first() {
        let text = " 10  1\n11 10\n12 10\nx 3\n13 11\n14 99\n15 11 7\n16\n10 10\n";
        let p = pairs(text);
        assert_eq!(
            p,
            vec![(10, 1), (11, 10), (12, 10), (13, 11), (14, 99), (10, 10)]
        );
        assert_eq!(
            under(&p, 10),
            vec![10, 11, 12, 13],
            "parents first, once each"
        );
        assert_eq!(under(&p, 11), vec![11, 13]);
        assert_eq!(under(&p, 13), vec![13]);
        assert_eq!(under(&p, 77), vec![77], "a root not listed is itself");
        // A wide tree is cut, a deep one too.
        let wide: Vec<(u32, u32)> = (2..400).map(|pid| (pid, 1)).collect();
        assert_eq!(under(&wide, 1).len(), MOST_UNDER);
        let deep: Vec<(u32, u32)> = (2..400).map(|pid| (pid, pid - 1)).collect();
        let found = under(&deep, 1);
        assert_eq!(found.len(), MOST_UNDER);
        assert_eq!(found[..3], [1, 2, 3]);
        assert_eq!(under(&deep, 398), vec![398, 399]);
    }

    #[test]
    fn a_process_line_gives_its_cpu_in_tenths_memory_and_name() {
        assert_eq!(tenths("12.3"), Some(123));
        assert_eq!(tenths("0.0"), Some(0));
        assert_eq!(tenths("100"), Some(1000));
        assert_eq!(tenths("7.25"), Some(72));
        assert_eq!(tenths("401.9"), Some(4019));
        for no in ["x", "1.", "1,5", "-1.0", ".5", "4294967295.0"] {
            assert_eq!(tenths(no), None, "{no:?}");
        }
        let text = "  501   1  12.3  20480 /usr/local/bin/claude\n\
                    600 501 99.0 1024 /Applications/Foo Bar.app/Contents/MacOS/Foo Bar\n\
                    601 501 0.0 8 cargo\n\
                    602 501 0.0 8 /\n\
                    603 501 x 8 sh\n\
                    604 501 1.0 8\n";
        let got = parse(text);
        let proc = |pid, ppid, cpu, rss, name: &str| Proc {
            pid,
            ppid,
            cpu,
            rss,
            name: name.into(),
        };
        assert_eq!(
            got,
            vec![
                proc(501, 1, 123, 20480, "claude"),
                proc(600, 501, 990, 1024, "Foo Bar"),
                proc(601, 501, 0, 8, "cargo"),
            ]
        );
        assert_eq!(Procs::sum(&got), (1113, 21512));
        assert_eq!(Procs::sum(&[]), (0, 0));
        let hot = [proc(1, 0, u32::MAX, 1, "a"), proc(2, 1, 5, 2, "b")];
        assert_eq!(Procs::sum(&hot), (u32::MAX, 3));
    }

    #[test]
    fn each_session_lists_what_was_read_in_its_order_and_a_gone_one_none() {
        let proc = |pid, ppid| Proc {
            pid,
            ppid,
            cpu: 0,
            rss: 0,
            name: format!("p{pid}"),
        };
        let procs = [proc(13, 11), proc(10, 1), proc(11, 10), proc(20, 1)];
        let trees = [(1, vec![10, 11, 12, 13]), (2, vec![30, 31]), (3, vec![20])];
        let got = group(&trees, &procs);
        let pids: Vec<(AgentId, Vec<u32>)> = (got.iter())
            .map(|(id, ps)| (*id, ps.iter().map(|p| p.pid).collect()))
            .collect();
        assert_eq!(pids, vec![(1, vec![10, 11, 13]), (3, vec![20])]);
        // Its session's process gone, a tree is none, what is left under
        // it too.
        assert_eq!(group(&[(1, vec![10, 11])], &[proc(11, 10)]), vec![]);
    }

    /// A shell standing in for a session, with `inner` started under it
    /// in the background: the shell, and the number of what it started.
    #[test]
    fn a_mark_is_one_word_and_reads_back_into_its_run_and_place() {
        let run = new_run();
        assert_ne!(new_run(), run, "each Live its own");
        assert_eq!(run_pid(&run), Some(std::process::id()));
        let m = mark(&run, "my shop/v2", "claude-1");
        assert_eq!(m, format!("{run}/my_shop_v2/claude-1"));
        assert!(!m.contains(char::is_whitespace));
        assert_eq!(mark_parts(&m), Some((run.as_str(), "my_shop_v2/claude-1")));
        assert_eq!(mark_parts("nothing"), None);
        assert_eq!(mark_parts("/a/b"), None);
        assert_eq!(mark_parts("r/"), None);
        assert_eq!(mark_word(&"x".repeat(50)).len(), 40);
        assert_eq!(mark_word("çay é"), "_ay__");
        assert_eq!(run_pid("x-1-2"), None);
    }

    #[test]
    fn a_mark_is_read_from_an_environment_and_procargs() {
        let environ = format!("HOME=/h\0{MARK_ENV}=1-2-0/shop/claude-1\0PATH=/bin\0");
        assert_eq!(
            mark_in_environ(environ.as_bytes()).as_deref(),
            Some("1-2-0/shop/claude-1")
        );
        assert_eq!(mark_in_environ(b"HOME=/h\0"), None);
        assert_eq!(mark_in_environ(format!("{MARK_ENV}=x\0").as_bytes()), None);
        assert_eq!(
            mark_in_environ(format!("X{MARK_ENV}=1-2-0/a/b\0").as_bytes()),
            None
        );
        // argc 2, the path padded, two arguments, then the environment.
        let mut buf = 2i32.to_ne_bytes().to_vec();
        buf.extend(b"/bin/sh\0\0\0sh\0-c\0A=1\0B=2\0");
        assert_eq!(
            environ_in_procargs(&buf).as_deref(),
            Some(&b"A=1\0B=2\0"[..])
        );
        assert_eq!(environ_in_procargs(&buf[..2]), None, "cut short");
        let mut short = 5i32.to_ne_bytes().to_vec();
        short.extend(b"/bin/sh\0sh\0");
        assert_eq!(
            environ_in_procargs(&short),
            None,
            "fewer arguments than said"
        );
        // This process carries none; one gone is none.
        assert_eq!(mark_of(std::process::id()), None);
        assert_eq!(mark_of(u32::MAX - 1), None);
    }

    #[test]
    fn what_left_a_session_is_listed_after_it_its_first_first() {
        let m = "1-2-0/shop/claude-1".to_string();
        let other = "1-2-0/shop/claude-2".to_string();
        // 10 the session, 11 under it; 20 left it (its parent gone), 21
        // under 20; 30 carries another's mark; 40 left it too.
        let pairs = [(10, 1), (11, 10), (20, 1), (21, 20), (30, 1), (40, 1)];
        let marks = [
            (10, m.clone()),
            (11, m.clone()),
            (20, m.clone()),
            (21, m.clone()),
            (30, other.clone()),
            (40, m.clone()),
        ];
        let trees = [vec![10, 11]];
        assert_eq!(marked_roots(&m, &marks, &pairs, &trees), [20, 40]);
        assert_eq!(left_session(&m, &marks, &pairs, &trees), [20, 21, 40]);
        assert_eq!(left_session(&other, &marks, &pairs, &trees), [30]);
        assert!(left_session("none", &marks, &pairs, &trees).is_empty());
        // A process under one that left goes with it, marked or not.
        let pairs = [(20, 1), (21, 20), (22, 21)];
        let marks = [(20, m.clone())];
        assert_eq!(left_session(&m, &marks, &pairs, &[]), [20, 21, 22]);
    }

    #[test]
    fn a_run_is_dead_once_its_process_is_gone_and_never_this_one() {
        let pairs = [(100, 1), (200, 1)];
        let marks = [
            (1, "100-5-0/a/b".to_string()),
            (2, "300-5-0/a/b".to_string()),
            (3, "200-5-0/a/c".to_string()),
            (4, "999-5-0/a/c".to_string()),
            (5, "x/a/b".to_string()),
        ];
        let dead: Vec<u32> = dead_runs(&marks, &pairs, 999)
            .iter()
            .map(|(p, _)| *p)
            .collect();
        assert_eq!(dead, [2], "100 and 200 run; 999 is this one; x is no run");
    }

    fn alive(pid: u32) -> bool {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }

    fn ended_soon(pid: u32) -> bool {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if !alive(pid) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn a_process_that_left_its_session_is_found_listed_ended_and_left_over() {
        let _one = one_at_a_time();
        let run = new_run();
        let m = mark(&run, "shop", "claude-1");
        let (root, under_it) = session("sleep 60");
        let gone = left_behind(&m);
        // Its mark reads back, its parent is no longer the session.
        assert_eq!(mark_of(gone).as_deref(), Some(m.as_str()));
        assert!(marks().unwrap().contains(&(gone, m.clone())));
        assert!(!child::descends(gone, root.id()));
        // `:ps` lists it after its session's tree.
        let got = read(&[(4, root.id())], &[(4, m.clone())]).unwrap();
        let pids: Vec<u32> = got[0].1.iter().map(|p| p.pid).collect();
        assert_eq!(pids, [root.id(), under_it, gone]);
        // Ended as the session's, by its mark; another mark is refused.
        assert_eq!(
            end(gone, root.id(), "sleep", Some("1-1-1/x/y")),
            Err(Refusal::NotUnder)
        );
        assert_eq!(end(gone, root.id(), "sleep", None), Err(Refusal::NotUnder));
        end(gone, root.id(), "sleep", Some(&m)).unwrap();
        assert!(ended_soon(gone));
        // The quit ends what carries its run's mark, and nothing else.
        let first = left_behind(&m);
        let theirs = left_behind(&mark(&new_run(), "shop", "claude-1"));
        assert_eq!(end_marked(&run), 1);
        assert!(ended_soon(first));
        assert!(alive(theirs), "another run's");
        // One whose run is gone is left over from an earlier run.
        let old = left_behind("4000000000-1-0/shop/claude-7");
        let w = std::env::temp_dir().join(format!("imece-marks-{}", std::process::id()));
        let all = leftovers(&w, "imece", std::process::id()).unwrap();
        let left = all
            .iter()
            .find(|l| l.procs[0].pid == old)
            .expect("left over");
        assert_eq!(left.place, "shop/claude-7");
        assert!(
            !all.iter().any(|l| l.procs.iter().any(|p| p.pid == theirs)),
            "a live run's"
        );
        assert_eq!(end_left(&[(old, old)], &w, "imece", std::process::id()), 1);
        assert!(ended_soon(old));
        stop(root, &[under_it, theirs]);
    }

    fn session(inner: &str) -> (Child, u32) {
        let mut root = Command::new("sh")
            .args(["-c", &format!("{inner} & echo $!; wait")])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(root.stdout.as_mut().unwrap())
            .read_line(&mut line)
            .unwrap();
        (root, line.trim().parse().unwrap())
    }

    fn gone_soon(pid: u32, root: u32, within: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < within {
            if !child::descends(pid, root) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    fn stop(mut root: Child, pids: &[u32]) {
        for &pid in pids {
            unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        }
        let _ = root.kill();
        let _ = root.wait();
    }

    #[test]
    fn what_end_refuses_is_known_from_the_rows_before_a_y() {
        let p = |pid, ppid, name: &str| Proc {
            pid,
            ppid,
            cpu: 0,
            rss: 0,
            name: name.into(),
        };
        let rows = [
            p(10, 1, "claude"),
            p(11, 10, "imece"),
            p(12, 10, "cargo"),
            p(13, 12, "imece"),
        ];
        let own = Some("imece");
        assert_eq!(refused(&rows, 10, own), Some(Refusal::Session));
        assert_eq!(refused(&rows, 11, own), Some(Refusal::Bus));
        assert_eq!(refused(&rows, 12, own), None, "a command ends");
        assert_eq!(
            refused(&rows, 13, own),
            None,
            "imece's name, not under the session"
        );
        assert_eq!(refused(&rows, 11, Some("other")), None, "another name");
        assert_eq!(refused(&rows, 11, None), None);
        assert_eq!(refused(&rows, 99, own), Some(Refusal::NotUnder));
        assert_eq!(refused(&[], 10, own), Some(Refusal::NotUnder));
    }

    #[test]
    fn a_session_s_process_ends_its_session_and_imece_s_relay_stay() {
        let (root, sleep) = session("sleep 60");
        let r = root.id();
        assert_eq!(end(r, r, "sh", None), Err(Refusal::Session));
        assert_eq!(end(sleep, 1, "sleep", None), Err(Refusal::NotUnder));
        let (other, other_sleep) = session("sleep 60");
        assert_eq!(
            end(sleep, other.id(), "sleep", None),
            Err(Refusal::NotUnder)
        );
        // A program named as imece's, right under the session, is its
        // relay; under something else, or named otherwise, it is not.
        assert!(relay(sleep, r, "sleep", Some("sleep")));
        assert!(!relay(sleep, r, "sleep", Some("imece")));
        assert!(!relay(sleep, r, "sleep", None));
        assert!(!relay(sleep, other.id(), "sleep", Some("sleep")));
        assert!(!relay(r, r, "sh", Some("sh")), "the session is no relay");
        stop(other, &[other_sleep]);
        stop(root, &[sleep]);
    }

    #[test]
    fn a_process_under_a_session_ends_and_one_that_will_not_is_killed() {
        let (root, sleep) = session("sleep 60");
        let r = root.id();
        assert_eq!(end(sleep, r, "sleep", None), Ok(()));
        assert!(gone_soon(sleep, r, Duration::from_secs(1)), "TERM ends it");
        stop(root, &[]);
        // One that ignores TERM goes at KILL, after the grace.
        let (root, stubborn) = session("sh -c 'trap \"\" TERM; exec sleep 60'");
        let r = root.id();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(end(stubborn, r, "sleep", None), Ok(()));
        assert!(!gone_soon(stubborn, r, KILL_GRACE / 2), "TERM is ignored");
        assert!(
            gone_soon(stubborn, r, KILL_GRACE * 2),
            "KILL after the grace"
        );
        stop(root, &[stubborn]);
    }

    #[test]
    fn ps_reads_each_session_s_tree() {
        assert_eq!(read(&[], &[]), Ok(Vec::new()));
        let refused = ps(&["-p", "x"]).unwrap_err();
        assert!(refused.starts_with("ps ended with "), "{refused}");
        let me = std::env::current_exe().unwrap();
        let me = me.file_name().unwrap().to_string_lossy();
        assert_eq!(own_name().as_deref(), Some(me.as_ref()));
        let (root, sleep) = session("sleep 60");
        let r = root.id();
        let got = read(&[(4, r), (5, u32::MAX - 1)], &[]).unwrap();
        assert_eq!(got.len(), 1, "{got:?}");
        let (id, procs) = &got[0];
        assert_eq!(*id, 4);
        assert_eq!((procs[0].pid, procs[0].name.as_str()), (r, "sh"));
        assert!(
            (procs.iter()).any(|p| p.pid == sleep && p.ppid == r && p.name == "sleep"),
            "{procs:?}"
        );
        assert!(procs[0].rss > 0);
        stop(root, &[sleep]);
    }
    #[test]
    fn a_path_is_matched_as_written_and_a_place_is_the_word_after_it() {
        assert_eq!(literal("a.b"), "a\\.b");
        assert_eq!(
            literal(r"/x y/\.^$*+?()[]{}|z"),
            r"/x y/\\\.\^\$\*\+\?\(\)\[\]\{\}\|z"
        );
        assert_eq!(literal("/plain/dir-1_ü"), "/plain/dir-1_ü");
        let w = Path::new("/s/imece/worktrees");
        assert_eq!(named(w), "/s/imece/worktrees/");
        let args = "claude -p --add-dir /s/imece/worktrees/imece-b6-notes/claude-3 --x";
        assert_eq!(place_in(args, w), "imece-b6-notes/claude-3");
        assert_eq!(place_in("claude /s/imece/worktrees/a", w), "a");
        assert_eq!(place_in("claude /s/imece/worktreesx/a", w), "");
        assert_eq!(place_in("claude /s/imece/worktrees", w), "");
        assert_eq!(place_in("", w), "");
        let long = format!("x /s/imece/worktrees/{} y", "a".repeat(100));
        assert_eq!(place_in(&long, w), "a".repeat(PLACE_MOST));
        assert_eq!(numbers("12\n x\n 13 \n\n-1\n"), vec![12, 13]);
    }

    #[test]
    fn the_processes_above_one_are_its_parents_nearest_first() {
        let pairs = [(5, 4), (4, 3), (3, 1), (1, 0), (9, 8), (8, 9), (7, 7)];
        assert_eq!(above(&pairs, 5), vec![4, 3, 1]);
        assert!(above(&pairs, 1).is_empty());
        assert!(above(&pairs, 6).is_empty(), "not listed");
        assert_eq!(above(&pairs, 9), vec![8], "a loop stops");
        assert!(above(&pairs, 7).is_empty(), "its own parent");
        let deep: Vec<(u32, u32)> = (2..200).map(|pid| (pid, pid - 1)).collect();
        let up = above(&deep, 150);
        assert_eq!(up.len(), MOST_UP);
        assert_eq!(up[..2], [149, 148]);
        // A loop past the start: 3 → 2 → 1 → 2.
        assert_eq!(above(&[(3, 2), (2, 1), (1, 2)], 3), vec![2, 1]);
    }

    #[test]
    fn a_tree_is_left_over_with_no_imece_above_it() {
        assert!(is_imece("imece", "imece"));
        assert!(!is_imece("imec", "imece"));
        assert!(!is_imece("imece2", "imece"));
        // Linux cuts a name at 15 bytes.
        assert!(is_imece("imece_tui-0123a", "imece_tui-0123abcdef"));
        assert!(!is_imece("imece_tui-0123", "imece_tui-0123abcdef"));
        assert!(!is_imece("imece_tui-9999a", "imece_tui-0123abcdef"));
        let proc = |pid, name: &str| Proc {
            pid,
            ppid: 0,
            cpu: 0,
            rss: 0,
            name: name.into(),
        };
        // 1 launchd; 100 a live imece (me) and 110 its session; 200 another
        // imece and 210 its; 300 a shell and 310 a left session, 320 under
        // it naming the worktrees too; 400 imece's own command line.
        let pairs = [
            (1, 0),
            (100, 1),
            (110, 100),
            (200, 300),
            (210, 200),
            (300, 1),
            (310, 300),
            (320, 310),
            (400, 1),
            (500, 1),
        ];
        let procs = [
            proc(1, "launchd"),
            proc(100, "imece"),
            proc(110, "claude"),
            proc(200, "imece"),
            proc(210, "claude"),
            proc(300, "zsh"),
            proc(310, "claude"),
            proc(320, "node"),
            proc(400, "imece"),
        ];
        let naming = [110, 210, 310, 320, 400, 500, 600];
        assert_eq!(
            left_roots(&naming, &pairs, &procs, "imece", 100),
            vec![310, 500, 600],
            "not unnamed ones"
        );
        // This imece is known by its number whatever it is named.
        assert_eq!(
            left_roots(&naming, &pairs, &procs, "other", 100),
            vec![210, 310, 400, 500, 600]
        );
        assert_eq!(
            left_roots(&naming, &pairs, &procs, "other", 7),
            vec![110, 210, 310, 400, 500, 600]
        );
        assert!(left_roots(&[100], &pairs, &procs, "other", 100).is_empty());
        let many: Vec<u32> = (1000..1100).collect();
        assert_eq!(left_roots(&many, &[], &[], "imece", 1).len(), MOST_LEFT);
    }

    /// A process whose command line names `place` under `worktrees`, its
    /// parent gone (as a run that ended leaves it), with a `sleep` under
    /// it: its number.
    fn orphan(worktrees: &Path, place: &str) -> u32 {
        let arg = format!("{}/{place}", worktrees.display());
        let out = Command::new("sh")
            .args([
                "-c",
                "sh -c 'sleep 60 & wait' left \"$1\" </dev/null >/dev/null 2>&1 & echo $!",
                "outer",
                &arg,
            ])
            .output()
            .unwrap();
        String::from_utf8(out.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap()
    }

    fn tree_of(worktrees: &Path, root: u32) -> Option<Left> {
        let me = std::process::id();
        let own = own_name().unwrap();
        (leftovers(worktrees, &own, me).unwrap().into_iter()).find(|l| l.procs[0].pid == root)
    }

    #[test]
    fn a_tree_left_from_an_earlier_run_is_found_and_ended() {
        let _one = one_at_a_time();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let dir = std::env::temp_dir().join(format!("imece-left-{}-{nanos}", std::process::id()));
        let w = dir.join("worktrees");
        assert_eq!(leftovers(&w, "imece", 1), Ok(vec![]), "none names it");
        let root = orphan(&w, "p-notes/claude-1");
        // Ours, named the same: under this process, so not left over.
        let (mine, mine_sleep) =
            session(&format!("sh -c 'sleep 60' mine {}/p/claude-2", w.display()));
        let start = Instant::now();
        let left = loop {
            if let Some(l) = tree_of(&w, root).filter(|l| l.procs.len() > 1) {
                break l;
            }
            assert!(start.elapsed() < Duration::from_secs(5), "not found");
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(left.place, "p-notes/claude-1");
        assert_eq!(left.procs[0].name, "sh");
        let sleep = left.procs[1].pid;
        assert_eq!(
            (left.procs[1].ppid, left.procs[1].name.as_str()),
            (root, "sleep")
        );
        let me = std::process::id();
        let own = own_name().unwrap();
        let all = leftovers(&w, &own, me).unwrap();
        assert_eq!(all.len(), 1, "{all:?}");
        // Not ours, nor one asked under the wrong tree.
        assert_eq!(end_left(&[(mine.id(), mine.id())], &w, &own, me), 0);
        assert_eq!(end_left(&[(mine_sleep, root)], &w, &own, me), 0);
        assert_eq!(end_left(&[(sleep, root), (root, root)], &w, &own, me), 2);
        assert!(gone_soon(sleep, root, Duration::from_secs(2)));
        let start = Instant::now();
        while tree_of(&w, root).is_some() {
            assert!(start.elapsed() < Duration::from_secs(3), "still there");
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(end_left(&[(root, root)], &w, &own, me), 0, "gone");
        stop(mine, &[mine_sleep]);
    }
    /// A unique worktrees directory for one test.
    fn worktrees_for(test: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let dir = format!("imece-{test}-{}-{nanos}", std::process::id());
        std::env::temp_dir().join(dir).join("worktrees")
    }

    /// Waits for what `look` finds.
    fn soon<T>(mut look: impl FnMut() -> Option<T>) -> T {
        let start = Instant::now();
        loop {
            if let Some(found) = look() {
                return found;
            }
            assert!(start.elapsed() < Duration::from_secs(5), "not found");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn one_under_a_program_named_as_imece_is_not_left_over() {
        let _one = one_at_a_time();
        let w = worktrees_for("named");
        // `xargs`, its parent gone, runs a `sh` that names the worktrees;
        // xargs itself does not.
        let arg = format!("{}/p/claude-1", w.display());
        let out = Command::new("sh")
            .args([
                "-c",
                "printf '%s\\n' \"$1\" | xargs sh -c 'sleep 60; :' child >/dev/null 2>&1 & echo $!",
                "outer",
                &arg,
            ])
            .output()
            .unwrap();
        let xargs: u32 = String::from_utf8(out.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let left = soon(|| {
            let all = leftovers(&w, "imece", u32::MAX).unwrap();
            all.into_iter().find(|l| l.procs.len() > 1)
        });
        let child = left.procs[0].pid;
        assert_eq!(left.procs[0].ppid, xargs);
        assert_eq!(left.place, "p/claude-1");
        // xargs named as imece: what runs under it is that imece's.
        assert_eq!(leftovers(&w, "xargs", u32::MAX), Ok(vec![]));
        assert_eq!(end_left(&[(child, child)], &w, "xargs", u32::MAX), 0);
        // Under this process by number, whatever its name.
        assert_eq!(leftovers(&w, "imece", xargs), Ok(vec![]));
        let sleep = left.procs[1].pid;
        assert_eq!(
            end_left(&[(sleep, child), (child, child)], &w, "imece", u32::MAX),
            2
        );
        unsafe { libc::kill(xargs as i32, libc::SIGKILL) };
    }

    #[test]
    fn one_left_over_that_will_not_end_is_killed_after_the_grace() {
        let _one = one_at_a_time();
        let w = worktrees_for("stubborn");
        let arg = format!("{}/p/claude-1", w.display());
        let out = Command::new("sh")
            .args([
                "-c",
                "sh -c 'trap \"\" TERM; sleep 60 & wait' left \"$1\" </dev/null >/dev/null 2>&1 & echo $!",
                "outer",
                &arg,
            ])
            .output()
            .unwrap();
        let root: u32 = String::from_utf8(out.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let left = soon(|| tree_of(&w, root).filter(|l| l.procs.len() > 1));
        let me = std::process::id();
        let own = own_name().unwrap();
        assert_eq!(end_left(&[(root, root)], &w, &own, me), 1);
        std::thread::sleep(KILL_GRACE / 2);
        assert!(tree_of(&w, root).is_some(), "TERM is ignored");
        soon(|| tree_of(&w, root).is_none().then_some(()));
        for p in &left.procs {
            unsafe { libc::kill(p.pid as i32, libc::SIGKILL) };
        }
    }
}
