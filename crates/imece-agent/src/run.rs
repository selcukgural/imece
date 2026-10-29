//! One command an agent runs through imece's `run` tool: a program and
//! its arguments, never a shell; in the agent's directory, with a cleared
//! environment; its own process group, killed at the deadline; stdout and
//! stderr read together and capped, then tidied for a model to read.

use std::fs::File;
use std::io::{self, Read};
use std::os::fd::FromRawFd;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::child;

/// Variables a build needs on top of [`child::BASE_ENV`]: where Rust and
/// its caches live, when the person set them.
pub const BUILD_ENV: &[&str] = &[
    "CARGO_HOME",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_JOBS",
];
/// How often the deadline is checked.
const POLL: Duration = Duration::from_millis(50);

/// What a run gave back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The exit code; none when a signal ended it (the deadline too).
    pub code: Option<i32>,
    pub timed_out: bool,
    /// stdout and stderr in the order they came, tidied (see [`tidy`]).
    pub output: String,
    /// Bytes written before tidying, both streams together.
    pub bytes: usize,
    pub took: Duration,
}

impl Outcome {
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }
}

/// Splits a command line into a program and its arguments the way a shell
/// splits words (single and double quotes, backslash escapes), and refuses
/// what a shell would interpret: pipes, redirections, variables, command
/// substitution, several commands on one line.
pub fn split(line: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => word.push(c),
                        None => return Err("a quote is not closed".into()),
                    }
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(c @ ('"' | '\\')) => word.push(c),
                            Some(c) => {
                                word.push('\\');
                                word.push(c);
                            }
                            None => return Err("a quote is not closed".into()),
                        },
                        Some('$') | Some('`') => {
                            return Err("no shell: $ and ` are not expanded".into());
                        }
                        Some(c) => word.push(c),
                        None => return Err("a quote is not closed".into()),
                    }
                }
            }
            '\\' => match chars.next() {
                Some(c) => {
                    in_word = true;
                    word.push(c);
                }
                None => return Err("a backslash ends the line".into()),
            },
            c if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            // A shell expands `~` at the start of a word only; `HEAD~1` is
            // git's own.
            '~' if in_word => {
                word.push(c);
            }
            '|' | '&' | ';' | '<' | '>' | '(' | ')' | '$' | '`' | '#' | '*' | '?' | '~' | '{' => {
                return Err(format!(
                    "no shell: {c:?} is not interpreted; give one program and its arguments \
                     (quote a literal {c:?})"
                ));
            }
            c => {
                in_word = true;
                word.push(c);
            }
        }
    }
    if in_word {
        words.push(word);
    }
    if words.is_empty() {
        return Err("no program named".into());
    }
    Ok(words)
}

/// Runs `argv` in `cwd` and waits at most `timeout`, keeping at most `max`
/// bytes of output (the first and last halves of it). `started` gets the
/// process group's id as soon as it runs, so the caller can kill it if
/// imece closes first.
pub fn run(
    argv: &[String],
    cwd: &Path,
    timeout: Duration,
    max: usize,
    started: impl FnOnce(i32),
) -> io::Result<Outcome> {
    let (program, args) = argv.split_first().expect("split gives a program");
    let mut cmd = child::command(program, cwd, BUILD_ENV);
    cmd.args(args)
        .env("NO_COLOR", "1")
        .env("CARGO_TERM_COLOR", "never")
        .env("TERM", "dumb")
        .stdin(Stdio::null());
    // One pipe for both streams, so their lines keep their order.
    let (reader, writer) = pipe()?;
    cmd.stdout(Stdio::from(writer.try_clone()?))
        .stderr(Stdio::from(writer))
        .process_group(0);
    let start = Instant::now();
    let mut p = cmd.spawn()?;
    drop(cmd);
    let pgid = p.id() as i32;
    started(pgid);
    let collector = thread::spawn(move || collect(reader, max));
    let mut timed_out = false;
    let status = loop {
        if let Some(s) = p.try_wait()? {
            break s;
        }
        if start.elapsed() >= timeout {
            timed_out = true;
            child::kill_group(pgid, libc::SIGKILL);
            break p.wait()?;
        }
        thread::sleep(POLL);
    };
    // Tools the program started may outlive it and hold the pipe open;
    // they go with it.
    child::kill_group(pgid, libc::SIGKILL);
    let (raw, bytes) = collector.join().unwrap_or_default();
    Ok(Outcome {
        code: status.code(),
        timed_out,
        output: tidy(&raw),
        bytes,
        took: start.elapsed(),
    })
}

fn pipe() -> io::Result<(File, File)> {
    let mut fds = [0; 2];
    // SAFETY: pipe fills two descriptors we own from here on.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: both descriptors are open and ours.
    Ok(unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) })
}

/// Reads `r` to its end, keeping the first and last `max / 2` bytes with a
/// mark between them; the count is of everything read.
fn collect(mut r: impl Read, max: usize) -> (String, usize) {
    let half = max / 2;
    let mut head = Vec::new();
    let mut tail = Vec::new();
    let mut total = 0;
    let mut buf = [0u8; 8192];
    loop {
        let n = match r.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        total += n;
        let mut chunk = &buf[..n];
        if head.len() < half {
            let take = chunk.len().min(half - head.len());
            head.extend_from_slice(&chunk[..take]);
            chunk = &chunk[take..];
        }
        tail.extend_from_slice(chunk);
        if tail.len() > half {
            tail.drain(..tail.len() - half);
        }
    }
    let mut out = String::from_utf8_lossy(&head).into_owned();
    if !tail.is_empty() {
        let cut = total - head.len() - tail.len();
        if cut > 0 {
            out.push_str(&format!("\n[... {cut} bytes cut ...]\n"));
        }
        out.push_str(&String::from_utf8_lossy(&tail));
    }
    (out, total)
}

/// Output as a model should read it: terminal colours and cursor codes
/// removed, carriage-return progress lines reduced to what they ended as,
/// lines of tests that passed dropped (the failures and the totals stay),
/// a line repeated in a row kept once with its count, and blank runs
/// squeezed.
pub fn tidy(raw: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut repeats = 0;
    for line in strip_ansi(raw).lines() {
        let line = line.rsplit('\r').next().unwrap_or(line).trim_end();
        if passed_test(line) {
            continue;
        }
        if line.is_empty() && out.last().is_some_and(|l| l.is_empty()) {
            continue;
        }
        if out.last().is_some_and(|l| l == line) && !line.is_empty() {
            repeats += 1;
            continue;
        }
        if repeats > 0 {
            let last = out.pop().unwrap_or_default();
            out.push(format!("{last} (x{})", repeats + 1));
            repeats = 0;
        }
        out.push(line.to_string());
    }
    if repeats > 0 {
        let last = out.pop().unwrap_or_default();
        out.push(format!("{last} (x{})", repeats + 1));
    }
    out.join("\n").trim().to_string()
}

/// A line that says one test passed, in cargo's, pytest's or Go's words.
fn passed_test(line: &str) -> bool {
    let l = line.trim();
    (l.starts_with("test ") && l.ends_with(" ... ok"))
        || (l.starts_with("--- PASS: ") || l == "PASS")
        || (l.ends_with(" PASSED") && l.contains("::"))
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            if c != '\u{7}' && (c >= ' ' || c == '\n' || c == '\t' || c == '\r') {
                out.push(c);
            }
            continue;
        }
        match chars.next() {
            // CSI: parameters, then one final byte in @..~.
            Some('[') => {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            // OSC: up to BEL or ESC \.
            Some(']') => {
                let mut prev = ' ';
                for c in chars.by_ref() {
                    if c == '\u{7}' || (prev == '\u{1b}' && c == '\\') {
                        break;
                    }
                    prev = c;
                }
            }
            _ => {}
        }
    }
    out
}

/// Process groups of the runs under way, so imece's end is theirs too.
#[derive(Clone, Default)]
pub struct Running(Arc<Mutex<Vec<i32>>>);

impl Running {
    pub fn add(&self, pgid: i32) {
        if let Ok(mut v) = self.0.lock() {
            v.push(pgid);
        }
    }

    pub fn remove(&self, pgid: i32) {
        if let Ok(mut v) = self.0.lock() {
            v.retain(|&p| p != pgid);
        }
    }

    pub fn len(&self) -> usize {
        self.0.lock().map_or(0, |v| v.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Kills every run under way.
    pub fn kill_all(&self) {
        if let Ok(mut v) = self.0.lock() {
            for pgid in v.drain(..) {
                child::kill_group(pgid, libc::SIGKILL);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        split(line).unwrap()
    }

    #[test]
    fn a_line_splits_into_words_like_a_shell_without_a_shell() {
        assert_eq!(words("cargo test"), ["cargo", "test"]);
        assert_eq!(
            words("  git  log --oneline -3 "),
            ["git", "log", "--oneline", "-3"]
        );
        assert_eq!(
            words(r#"grep -n "fn main" src/main.rs"#),
            ["grep", "-n", "fn main", "src/main.rs"]
        );
        assert_eq!(words(r"grep 'it''s' a\ b"), ["grep", "its", "a b"]);
        assert_eq!(words(r#"echo "a \"b\" \n""#), ["echo", r#"a "b" \n"#]);
        assert_eq!(words("echo ''"), ["echo", ""]);
        assert_eq!(
            words("git diff HEAD~1 a~b"),
            ["git", "diff", "HEAD~1", "a~b"]
        );
        assert_eq!(
            words(r#"echo '$HOME' "x|y" '*' '#'"#),
            ["echo", "$HOME", "x|y", "*", "#"]
        );
        for bad in [
            "",
            "   ",
            "cargo test | head",
            "a && b",
            "a; b",
            "a > out",
            "a < in",
            "echo $HOME",
            "echo `id`",
            "echo \"$HOME\"",
            "echo \"`id`\"",
            "ls *",
            "ls ?",
            "ls ~",
            "ls ~/x",
            "ls a ~b",
            "ls {a,b}",
            "(a)",
            "a # b",
            "echo 'open",
            "echo \"open",
            "echo a\\",
        ] {
            assert!(split(bad).is_err(), "{bad:?}");
        }
        assert!(split("a | b").unwrap_err().contains("no shell"));
    }

    #[test]
    fn output_is_tidied_for_a_model() {
        let raw = "\u{1b}[32mok\u{1b}[0m\n\
                   running 3 tests\n\
                   test a ... ok\n\
                   test b::c ... FAILED\n\
                   test d ... ok\n\
                   \n\n\n\
                   warning: x\n\
                   warning: x\n\
                   warning: x\n\
                   10%\r50%\r100% done\n\
                   \u{1b}]0;title\u{7}after\n\
                   test result: FAILED. 2 passed; 1 failed\n\n";
        assert_eq!(
            tidy(raw),
            "ok\nrunning 3 tests\ntest b::c ... FAILED\n\nwarning: x (x3)\n100% done\nafter\n\
             test result: FAILED. 2 passed; 1 failed"
        );
        assert_eq!(tidy(""), "");
        assert_eq!(tidy("same\nsame"), "same (x2)");
        assert_eq!(
            tidy("--- PASS: TestX (0.00s)\nPASS\nok  pkg 0.1s"),
            "ok  pkg 0.1s"
        );
        assert_eq!(tidy("tests/t.py::test_a PASSED\n1 passed"), "1 passed");
        assert_eq!(tidy("test x ... ok\n"), "");
    }

    #[test]
    fn the_first_and_last_bytes_are_kept_and_the_middle_is_cut() {
        let (s, n) = collect(&b"0123456789"[..], 6);
        assert_eq!(n, 10);
        assert_eq!(s, "012\n[... 4 bytes cut ...]\n789");
        let (s, n) = collect(&b"012345"[..], 6);
        assert_eq!((s.as_str(), n), ("012345", 6));
        let (s, _) = collect(&b"01234"[..], 6);
        assert_eq!(s, "01234");
        let (s, n) = collect(&b""[..], 6);
        assert_eq!((s.as_str(), n), ("", 0));
        // Many small reads land in the right halves too.
        let big: Vec<u8> = (0..20_000).map(|i| b'a' + (i % 26) as u8).collect();
        let (s, n) = collect(&big[..], 1000);
        assert_eq!(n, 20_000);
        assert!(s.starts_with(std::str::from_utf8(&big[..500]).unwrap()));
        assert!(s.ends_with(std::str::from_utf8(&big[19_500..]).unwrap()));
        assert!(s.contains("[... 19000 bytes cut ...]"));
    }

    fn sh(script: &str, timeout: Duration, max: usize) -> (Outcome, Vec<i32>) {
        let mut pgids = Vec::new();
        let out = run(
            &["sh".into(), "-c".into(), script.into()],
            &std::env::temp_dir(),
            timeout,
            max,
            |p| pgids.push(p),
        )
        .unwrap();
        (out, pgids)
    }

    #[test]
    fn a_run_gives_both_streams_in_order_its_code_and_its_time() {
        let (out, pgids) = sh(
            "echo one; echo two >&2; echo three; echo $FOO; exit 3",
            Duration::from_secs(10),
            1 << 16,
        );
        assert_eq!(out.code, Some(3));
        assert!(!out.ok() && !out.timed_out);
        assert_eq!(out.output, "one\ntwo\nthree");
        assert_eq!(out.bytes, "one\ntwo\nthree\n\n".len());
        assert_eq!(pgids.len(), 1);
        assert!(out.took < Duration::from_secs(5));
        let (out, _) = sh("exit 0", Duration::from_secs(10), 1 << 16);
        assert!(out.ok());
        assert_eq!(out.output, "");
    }

    #[test]
    fn the_environment_is_cleared_and_colours_are_off() {
        unsafe { std::env::set_var("IMECE_RUN_SECRET", "s3cret") };
        let (out, _) = sh(
            "echo \"[$IMECE_RUN_SECRET] [$NO_COLOR] [$CARGO_TERM_COLOR] [$TERM]\"; test -n \"$PATH\"",
            Duration::from_secs(10),
            1 << 16,
        );
        assert!(out.ok(), "{out:?}");
        assert_eq!(out.output, "[] [1] [never] [dumb]");
    }

    #[test]
    fn a_run_past_its_deadline_is_killed_with_its_children() {
        let start = Instant::now();
        let (out, _) = sh(
            "echo started; sleep 30 & sleep 30; echo never",
            Duration::from_millis(300),
            1 << 16,
        );
        assert!(out.timed_out);
        assert_eq!(out.code, None);
        assert_eq!(out.output, "started");
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "{:?}",
            start.elapsed()
        );
    }

    #[test]
    fn a_long_output_is_cut_in_the_middle_without_blocking() {
        let (out, _) = sh(
            "i=0; while [ $i -lt 30000 ]; do echo line$i; i=$((i+1)); done",
            Duration::from_secs(30),
            4000,
        );
        assert!(out.ok(), "{out:?}");
        assert!(out.bytes > 250_000, "{}", out.bytes);
        assert!(out.output.starts_with("line0\nline1\n"));
        assert!(
            out.output.ends_with("line29998\nline29999"),
            "{}",
            &out.output[out.output.len() - 40..]
        );
        assert!(out.output.contains(" bytes cut ...]"));
        assert!(out.output.len() < 4200);
    }

    #[test]
    fn a_program_that_is_not_there_is_an_error() {
        let r = run(
            &["/nonexistent/program".into()],
            &std::env::temp_dir(),
            Duration::from_secs(1),
            100,
            |_| {},
        );
        assert!(r.is_err());
    }

    #[test]
    fn running_groups_are_tracked_and_killed_together() {
        let running = Running::default();
        assert!(running.is_empty());
        let r = running.clone();
        let t = thread::spawn(move || {
            run(
                &["sh".into(), "-c".into(), "sleep 30".into()],
                &std::env::temp_dir(),
                Duration::from_secs(60),
                100,
                |p| r.add(p),
            )
            .unwrap()
        });
        let start = Instant::now();
        while running.is_empty() {
            assert!(start.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(running.len(), 1);
        running.kill_all();
        assert!(running.is_empty());
        let out = t.join().unwrap();
        assert_eq!(out.code, None);
        assert!(!out.timed_out);
        assert!(start.elapsed() < Duration::from_secs(5));
        running.add(7);
        running.remove(7);
        assert!(running.is_empty());
    }
}
