//! A child process an agent runs in: argv only (never a shell), a cleared
//! environment plus an allowlist, its own process group so its tools die
//! with it, and a byte cap on every pipe.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{self, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// The longest stdout line read. A longer one ends the session: the reader
/// never holds more than this.
pub const MAX_LINE: usize = 8 << 20;
/// How much of stderr is kept (its last bytes), to say why a process died.
pub const STDERR_TAIL: usize = 4096;
/// How long a process gets to exit after SIGTERM before SIGKILL.
const GRACE: Duration = Duration::from_millis(1500);
const POLL: Duration = Duration::from_millis(20);
/// Starts tried while the program is busy being written, and the wait
/// between them (see [`retry_busy`]).
const BUSY_TRIES: usize = 10;
const BUSY_WAIT: Duration = Duration::from_millis(50);

/// Variables every child gets, when they are set: finding programs, the
/// home directory, the user, locale and temporary files. Everything else
/// must be named by the caller.
pub const BASE_ENV: &[&str] = &[
    "HOME", "USER", "LOGNAME", "PATH", "SHELL", "TMPDIR", "LANG", "LC_ALL", "LC_CTYPE",
];

/// `program` in `cwd` with a cleared environment: only [`BASE_ENV`] and
/// `keep` are copied from ours.
pub fn command(program: &str, cwd: &Path, keep: &[&str]) -> Command {
    let mut c = Command::new(program);
    c.current_dir(cwd).env_clear();
    for k in BASE_ENV.iter().chain(keep) {
        if let Some(v) = std::env::var_os(k) {
            c.env(k, v);
        }
    }
    c
}

/// `start` again, a few times and a moment apart, while it fails because
/// the program is busy: Linux will not run a file someone holds open for
/// writing (`ETXTBSY`), and another thread's fork of a moment ago holds
/// it until its `exec` (#74: tests that write a fake CLI and run it at
/// once), as does a CLI being updated in place. Any other outcome is
/// returned at once; after [`BUSY_TRIES`] the last try's.
fn retry_busy<T>(mut start: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    for _ in 1..BUSY_TRIES {
        match start() {
            Err(e) if busy(&e) => thread::sleep(BUSY_WAIT),
            other => return other,
        }
    }
    start()
}

fn busy(e: &io::Error) -> bool {
    e.raw_os_error() == Some(libc::ETXTBSY)
}

/// What the reader thread hands to the caller.
pub enum Out<'a> {
    /// One stdout line, without its newline.
    Line(&'a str),
    /// The process is gone; why, in a few words. Nothing follows.
    End(String),
}

pub struct Child {
    proc: Arc<Mutex<process::Child>>,
    stdin: Option<ChildStdin>,
    pgid: i32,
}

impl Child {
    /// Starts `cmd` in a new process group with all three pipes. `on` gets
    /// every stdout line on a reader thread, then one `Out::End`; returning
    /// `false` from a line kills the group.
    pub fn spawn(
        mut cmd: Command,
        mut on: impl FnMut(Out) -> bool + Send + 'static,
    ) -> io::Result<Child> {
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let mut p = retry_busy(|| cmd.spawn())?;
        let pgid = p.id() as i32;
        let stdin = p.stdin.take();
        let stdout = p.stdout.take().expect("stdout is piped");
        let stderr = p.stderr.take().expect("stderr is piped");
        let proc = Arc::new(Mutex::new(p));

        let tail = thread::spawn(move || read_tail(stderr, STDERR_TAIL));
        let reader = Arc::clone(&proc);
        thread::spawn(move || {
            let mut why = String::new();
            read_lines(stdout, MAX_LINE, |line| match line {
                Some(l) => {
                    let go_on = on(Out::Line(l));
                    if !go_on {
                        kill_group(pgid, libc::SIGKILL);
                    }
                    go_on
                }
                None => {
                    why = format!("a line longer than {} MiB", MAX_LINE >> 20);
                    kill_group(pgid, libc::SIGKILL);
                    false
                }
            });
            let status = wait(&reader, None);
            // Tools the agent started may outlive it; they go with it.
            kill_group(pgid, libc::SIGKILL);
            let stderr = tail.join().unwrap_or_default();
            if why.is_empty() {
                why = describe(status, &stderr);
            }
            on(Out::End(why));
        });
        Ok(Child { proc, stdin, pgid })
    }

    /// Writes one line to the process's stdin.
    pub fn write_line(&mut self, line: &str) -> io::Result<()> {
        if line.contains('\n') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a line must not contain a newline",
            ));
        }
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "stdin is closed"))?;
        stdin.write_all(line.as_bytes())?;
        stdin.write_all(b"\n")?;
        stdin.flush()
    }

    /// Closes stdin and asks the whole group to stop, without waiting. Drop
    /// waits a moment, then kills what is left.
    pub fn terminate(&mut self) {
        self.stdin = None;
        kill_group(self.pgid, libc::SIGTERM);
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        self.terminate();
        wait(&self.proc, Some(GRACE));
        // Whether or not the leader is gone: what is left of the group, a
        // tool that ignores SIGTERM for example, goes now.
        kill_group(self.pgid, libc::SIGKILL);
        wait(&self.proc, Some(GRACE));
    }
}

pub(crate) fn kill_group(pgid: i32, signal: i32) {
    // SAFETY: killpg only sends a signal; a group that is gone gives ESRCH.
    unsafe {
        libc::killpg(pgid, signal);
    }
}

/// Waits for the process to exit, at most `limit`. Never blocks while
/// holding the lock, so a drop and the reader can wait at the same time.
fn wait(proc: &Mutex<process::Child>, limit: Option<Duration>) -> Option<ExitStatus> {
    let start = Instant::now();
    loop {
        if let Ok(mut p) = proc.lock()
            && let Ok(Some(status)) = p.try_wait()
        {
            return Some(status);
        }
        if limit.is_some_and(|l| start.elapsed() >= l) {
            return None;
        }
        thread::sleep(POLL);
    }
}

fn describe(status: Option<ExitStatus>, stderr: &str) -> String {
    let how = match status {
        Some(s) => match (s.code(), s.signal()) {
            (Some(code), _) => format!("exited with code {code}"),
            (None, Some(sig)) => format!("killed by signal {sig}"),
            _ => "exited".to_string(),
        },
        None => "exited".to_string(),
    };
    match stderr.lines().rev().find(|l| !l.trim().is_empty()) {
        Some(last) => format!("{how}: {}", last.trim()),
        None => how,
    }
}

/// Reads `r` line by line, at most `max` bytes per line, and gives each
/// line to `f` (`None` for a line that is too long). Stops at the end, at a
/// read error, at a line that is too long, or when `f` returns `false`.
pub fn read_lines(r: impl Read, max: usize, mut f: impl FnMut(Option<&str>) -> bool) {
    let mut r = BufReader::new(r);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match r.by_ref().take(max as u64 + 1).read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if buf.last() == Some(&b'\n') {
            buf.pop();
        } else if buf.len() > max {
            f(None);
            return;
        }
        if !f(Some(&String::from_utf8_lossy(&buf))) {
            return;
        }
    }
}

/// The last `keep` bytes of `r`, read to its end.
fn read_tail(mut r: impl Read, keep: usize) -> String {
    let mut tail: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    while let Ok(n) = r.read(&mut chunk) {
        if n == 0 {
            break;
        }
        tail.extend_from_slice(&chunk[..n]);
        if tail.len() > keep {
            tail.drain(..tail.len() - keep);
        }
    }
    String::from_utf8_lossy(&tail).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn lines(input: &[u8], max: usize) -> Vec<Option<String>> {
        let mut out = Vec::new();
        read_lines(input, max, |l| {
            out.push(l.map(str::to_string));
            true
        });
        out
    }

    #[test]
    fn lines_are_split_and_a_last_line_without_newline_counts() {
        assert_eq!(
            lines(b"one\ntwo\nthree", 10),
            [Some("one".into()), Some("two".into()), Some("three".into())]
        );
    }

    #[test]
    fn a_line_over_the_cap_stops_reading() {
        assert_eq!(
            lines(b"short\n0123456789abc\nnever", 10),
            [Some("short".into()), None]
        );
        // Exactly the cap is fine.
        assert_eq!(lines(b"0123456789\n", 10), [Some("0123456789".into())]);
    }

    #[test]
    fn stderr_keeps_only_its_tail() {
        let s = read_tail(&b"abcdefghij"[..], 4);
        assert_eq!(s, "ghij");
    }

    fn run(cmd: Command) -> (Vec<String>, String) {
        let (tx, rx) = mpsc::channel();
        let _child = Child::spawn(cmd, move |o| {
            tx.send(match o {
                Out::Line(l) => Ok(l.to_string()),
                Out::End(why) => Err(why),
            })
            .is_ok()
        })
        .expect("spawn");
        let mut out = Vec::new();
        loop {
            match rx.recv_timeout(Duration::from_secs(10)).expect("in time") {
                Ok(l) => out.push(l),
                Err(why) => return (out, why),
            }
        }
    }

    #[test]
    fn the_child_sees_only_allowed_variables() {
        // SAFETY: tests in this crate do not read these variables elsewhere.
        unsafe {
            std::env::set_var("IMECE_TEST_SECRET", "leak");
            std::env::set_var("IMECE_TEST_KEPT", "ok");
        }
        let cmd = command("env", Path::new("/"), &["IMECE_TEST_KEPT"]);
        let (out, why) = run(cmd);
        assert!(out.iter().any(|l| l == "IMECE_TEST_KEPT=ok"), "{out:?}");
        assert!(!out.iter().any(|l| l.contains("IMECE_TEST_SECRET")));
        assert!(
            out.iter().all(
                |l| l.split('=').next().is_some_and(|k| BASE_ENV.contains(&k)
                    || k == "IMECE_TEST_KEPT"
                    || k == "PWD"
                    || k == "_"
                    || k == "SHLVL")
            ),
            "{out:?}"
        );
        assert_eq!(why, "exited with code 0");
    }

    #[test]
    fn the_end_says_why_with_the_last_stderr_line() {
        let mut cmd = command("sh", Path::new("/"), &[]);
        cmd.args(["-c", "echo first >&2; echo broken auth >&2; exit 3"]);
        let (_, why) = run(cmd);
        assert_eq!(why, "exited with code 3: broken auth");
    }

    #[test]
    fn dropping_the_child_kills_its_whole_group() {
        let dir = std::env::temp_dir().join(format!("imece-pg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("alive");
        // A grandchild that would touch the marker after the drop.
        let mut cmd = command("sh", &dir, &[]);
        cmd.args([
            "-c",
            "(trap '' TERM; sleep 2; touch alive) & echo started; wait",
        ]);
        let (tx, rx) = mpsc::channel();
        let child = Child::spawn(cmd, move |o| {
            if let Out::Line(l) = o {
                let _ = tx.send(l.to_string());
            }
            true
        })
        .unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), "started");
        drop(child);
        thread::sleep(Duration::from_millis(2500));
        assert!(!marker.exists(), "a grandchild outlived the drop");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_callback_that_says_stop_kills_the_whole_group_at_once() {
        let mut cmd = command("sh", Path::new("/"), &[]);
        cmd.args(["-c", "echo one; sleep 30; echo never"]);
        let (tx, rx) = mpsc::channel();
        let started = Instant::now();
        let _child = Child::spawn(cmd, move |o| {
            let _ = tx.send(match o {
                Out::Line(l) => Ok(l.to_string()),
                Out::End(why) => Err(why),
            });
            // Enough after the first line.
            false
        })
        .unwrap();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            Ok("one".into())
        );
        let end = rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(
            matches!(&end, Err(why) if why.contains("killed by signal 9")),
            "{end:?}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "did not wait for sleep"
        );
    }

    #[test]
    fn terminate_closes_stdin_and_asks_the_group_to_stop() {
        let mut cmd = command("sh", Path::new("/"), &[]);
        cmd.args(["-c", "echo ready; cat; echo after"]);
        let (tx, rx) = mpsc::channel();
        let mut child = Child::spawn(cmd, move |o| {
            let _ = tx.send(match o {
                Out::Line(l) => Ok(l.to_string()),
                Out::End(why) => Err(why),
            });
            true
        })
        .unwrap();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            Ok("ready".into())
        );
        child.terminate();
        let end = rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(
            matches!(&end, Err(why) if why.contains("signal 15")),
            "{end:?}"
        );
    }

    #[test]
    fn a_last_line_exactly_at_the_cap_without_newline_is_a_line() {
        assert_eq!(lines(b"0123456789", 10), [Some("0123456789".into())]);
    }

    #[test]
    fn a_busy_program_is_tried_again_a_bounded_number_of_times() {
        let busy_now = || io::Error::from_raw_os_error(libc::ETXTBSY);
        // Busy twice, then it starts.
        let mut calls = 0;
        let started = retry_busy(|| {
            calls += 1;
            if calls < 3 {
                Err(busy_now())
            } else {
                Ok(calls)
            }
        });
        assert_eq!(started.unwrap(), 3);
        // Any other failure comes back at once.
        let mut calls = 0;
        let e = retry_busy::<()>(|| {
            calls += 1;
            Err(io::Error::from(io::ErrorKind::NotFound))
        })
        .unwrap_err();
        assert_eq!((e.kind(), calls), (io::ErrorKind::NotFound, 1));
        let mut calls = 0;
        let e = retry_busy::<()>(|| {
            calls += 1;
            Err(io::Error::from_raw_os_error(libc::EACCES))
        })
        .unwrap_err();
        assert_eq!((e.raw_os_error(), calls), (Some(libc::EACCES), 1));
        // Busy for good: ten tries, and the last one's error.
        let mut calls = 0;
        let e = retry_busy::<()>(|| {
            calls += 1;
            Err(busy_now())
        })
        .unwrap_err();
        assert_eq!((e.raw_os_error(), calls), (Some(libc::ETXTBSY), 10));
    }

    /// The race of #74 itself: Linux refuses to run a file someone holds
    /// open for writing; once it is let go, the program starts. (macOS
    /// runs it anyway, so the test means something only on Linux.)
    #[cfg(target_os = "linux")]
    #[test]
    fn a_program_held_open_for_writing_starts_once_it_is_let_go() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("imece-busy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cli");
        std::fs::write(&path, "#!/bin/sh\necho hi\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let held = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        let letting_go = thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            drop(held);
        });
        let (tx, rx) = mpsc::channel();
        let _child = Child::spawn(Command::new(&path), move |o| {
            if let Out::Line(l) = o {
                let _ = tx.send(l.to_string());
            }
            true
        })
        .unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), "hi");
        letting_go.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
