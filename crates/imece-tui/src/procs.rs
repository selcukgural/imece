//! The processes the agents' sessions run (`:ps`, record 129, E16 of
//! #58): each session's process and every process under it, with the CPU
//! and memory `ps` reports, and ending one of them (never the session
//! itself, never imece's own bus relay) after checking again, just before
//! the signal, that it is still under that session.

use std::process::Command;
use std::time::Duration;

use imece_agent::child;

use crate::model::AgentId;

/// How long `ps` may take.
pub const PS_WAIT: Duration = Duration::from_secs(5);

/// The most processes listed under one session: a fork bomb's tree is cut.
pub const MOST_UNDER: usize = 200;

/// After `SIGTERM`, how long a process has before `SIGKILL`.
pub const KILL_GRACE: Duration = Duration::from_secs(2);

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
/// a session whose process is gone is left out.
pub fn read(roots: &[(AgentId, u32)]) -> Result<Vec<(AgentId, Vec<Proc>)>, String> {
    if roots.is_empty() {
        return Ok(Vec::new());
    }
    let pairs = pairs(&ps(&["-A", "-o", "pid=,ppid="])?);
    // A session gone is not asked about: `ps` refuses a whole list for
    // one number it does not take.
    let trees: Vec<(AgentId, Vec<u32>)> = (roots.iter())
        .filter(|&&(_, root)| pairs.iter().any(|&(pid, _)| pid == root))
        .map(|&(id, root)| (id, under(&pairs, root)))
        .collect();
    if trees.is_empty() {
        return Ok(Vec::new());
    }
    let list: Vec<String> = (trees.iter())
        .flat_map(|(_, pids)| pids.iter().map(u32::to_string))
        .collect();
    let list = list.join(",");
    // A process gone between the two looks is simply not listed, and ps
    // says so by its status alone: what it wrote is read either way.
    let mut cmd = Command::new("ps");
    cmd.args(["-o", "pid=,ppid=,pcpu=,rss=,comm=", "-p", &list])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C");
    let out = match child::ask(cmd, PS_WAIT) {
        Some((_, out)) => out,
        None => return Err("ps did not answer".into()),
    };
    let procs = parse(&out);
    Ok(group(&trees, &procs))
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

/// Ends process `pid` (named `name` when read) under session `root`:
/// checked again first, then `SIGTERM`, and `SIGKILL` after
/// [`KILL_GRACE`] if it is still there and still under the session. The
/// session's own process and imece's relay (a program named as imece is,
/// right under the session) are refused.
pub fn end(pid: u32, root: u32, name: &str) -> Result<(), Refusal> {
    if pid == root {
        return Err(Refusal::Session);
    }
    if !child::descends(pid, root) {
        return Err(Refusal::NotUnder);
    }
    if relay(pid, root, name, own_name().as_deref()) {
        return Err(Refusal::Bus);
    }
    let signal = move |sig| {
        // A number past i32 is no process of ours.
        if let Ok(p) = i32::try_from(pid) {
            unsafe { libc::kill(p, sig) };
        }
    };
    signal(libc::SIGTERM);
    std::thread::spawn(move || {
        std::thread::sleep(KILL_GRACE);
        if child::descends(pid, root) {
            signal(libc::SIGKILL);
        }
    });
    Ok(())
}

/// Whether `pid`, named `name`, is imece's relay for session `root`: a
/// program named as imece's own (`own`) right under the session.
fn relay(pid: u32, root: u32, name: &str, own: Option<&str>) -> bool {
    Some(name) == own && child::parent_of(pid) == Some(root)
}

/// The name imece's own program runs under (its relay's too).
fn own_name() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.file_name()?.to_string_lossy().into_owned())
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
    fn a_session_s_process_ends_its_session_and_imece_s_relay_stay() {
        let (root, sleep) = session("sleep 60");
        let r = root.id();
        assert_eq!(end(r, r, "sh"), Err(Refusal::Session));
        assert_eq!(end(sleep, 1, "sleep"), Err(Refusal::NotUnder));
        let (other, other_sleep) = session("sleep 60");
        assert_eq!(end(sleep, other.id(), "sleep"), Err(Refusal::NotUnder));
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
        assert_eq!(end(sleep, r, "sleep"), Ok(()));
        assert!(gone_soon(sleep, r, Duration::from_secs(1)), "TERM ends it");
        stop(root, &[]);
        // One that ignores TERM goes at KILL, after the grace.
        let (root, stubborn) = session("sh -c 'trap \"\" TERM; exec sleep 60'");
        let r = root.id();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(end(stubborn, r, "sleep"), Ok(()));
        assert!(!gone_soon(stubborn, r, KILL_GRACE / 2), "TERM is ignored");
        assert!(
            gone_soon(stubborn, r, KILL_GRACE * 2),
            "KILL after the grace"
        );
        stop(root, &[stubborn]);
    }

    #[test]
    fn ps_reads_each_session_s_tree() {
        assert_eq!(read(&[]), Ok(Vec::new()));
        let refused = ps(&["-p", "x"]).unwrap_err();
        assert!(refused.starts_with("ps ended with "), "{refused}");
        let me = std::env::current_exe().unwrap();
        let me = me.file_name().unwrap().to_string_lossy();
        assert_eq!(own_name().as_deref(), Some(me.as_ref()));
        let (root, sleep) = session("sleep 60");
        let r = root.id();
        let got = read(&[(4, r), (5, u32::MAX - 1)]).unwrap();
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
}
