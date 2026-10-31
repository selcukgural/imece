//! `!command`: a command of the person's own, in their own shell and their
//! own terminal, as `:!` does in vim. The screen steps aside (raw mode
//! off, the alternate screen left), the shell runs in the checkout with
//! the terminal to itself, and a key brings the screen back. Nothing here
//! touches an agent: no sandbox, no policy, no budget; it is the person's
//! shell, which they could open next door anyway.

use std::io::{self, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::crossterm::terminal::{self, disable_raw_mode, enable_raw_mode};
use ratatui::crossterm::{cursor, execute};

use crate::model::Ended;
use crate::screen::{self, Quiet, Screen};

/// How a command ran: how it ended and how long it took, from the spawn
/// to its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ran {
    pub ended: Ended,
    pub took: Duration,
}

/// The shell that runs the command: the person's own (`$SHELL`), else
/// `/bin/sh`.
pub fn program(shell: Option<&str>) -> PathBuf {
    match shell {
        Some(s) if !s.trim().is_empty() => PathBuf::from(s),
        _ => PathBuf::from("/bin/sh"),
    }
}

/// How a finished command ended: its exit code, or the signal that
/// stopped it; a status that is neither (a stopped child) counts as -1.
pub fn ended(status: ExitStatus) -> Ended {
    use std::os::unix::process::ExitStatusExt;
    match (status.code(), status.signal()) {
        (Some(code), _) => Ended::Exit(code),
        (None, Some(sig)) => Ended::Signal(sig),
        (None, None) => Ended::Exit(-1),
    }
}

/// Runs `command` through `shell` in `cwd`, with this process's stdin,
/// stdout and stderr, and waits for it. Ctrl-C in the terminal reaches
/// the command (which is what the person means) and not imece: SIGINT is
/// ignored here while the command runs, and set back to the default in
/// the child before it runs. The time is taken around the run.
pub fn run_in(shell: &Path, cwd: &Path, command: &str) -> io::Result<Ran> {
    // SAFETY: `signal` only swaps the handler; the old one is put back
    // below. SIG_IGN is inherited across exec, so the child resets it in
    // `pre_exec`, where only async-signal-safe calls are made.
    let was = unsafe { libc::signal(libc::SIGINT, libc::SIG_IGN) };
    let mut child = Command::new(shell);
    child.arg("-c").arg(command).current_dir(cwd);
    // SAFETY: see above.
    unsafe {
        child.pre_exec(|| {
            libc::signal(libc::SIGINT, libc::SIG_DFL);
            Ok(())
        });
    }
    let start = Instant::now();
    let status = child.status();
    let took = start.elapsed();
    // SAFETY: restoring the handler that was there.
    unsafe { libc::signal(libc::SIGINT, was) };
    status.map(|s| Ran {
        ended: ended(s),
        took,
    })
}

/// The whole round: the screen steps aside, `command` runs in `cwd`,
/// `back` (already worded by the caller from how it ran) is shown and a
/// key brings the screen back. What the command
/// printed stays on the terminal's main screen, where it is seen again
/// when imece quits.
pub fn run(
    terminal: &mut Screen,
    cwd: &Path,
    command: &str,
    back: impl FnOnce(Ran) -> String,
) -> io::Result<Ran> {
    step_aside()?;
    let mut out = io::stdout();
    writeln!(out, "$ {command}")?;
    out.flush()?;
    let ran = run_in(
        &program(std::env::var("SHELL").ok().as_deref()),
        cwd,
        command,
    );
    let ran = match ran {
        Ok(e) => e,
        Err(e) => {
            // Back to the screen whatever happened; the error is reported
            // there.
            come_back(terminal)?;
            return Err(e);
        }
    };
    write!(out, "{}", back(ran))?;
    out.flush()?;
    wait_for_key()?;
    come_back(terminal)?;
    Ok(ran)
}

/// Leaves the alternate screen (auto-wrap on again) and raw mode: the
/// terminal is the shell's.
fn step_aside() -> io::Result<()> {
    disable_raw_mode()?;
    screen::give_back(&mut io::stdout())?;
    execute!(io::stdout(), cursor::Show)
}

/// Raw mode for one key press (so no newline is echoed and ctrl-c is a
/// key, not a signal), then back to cooked mode.
fn wait_for_key() -> io::Result<()> {
    enable_raw_mode()?;
    let pressed = loop {
        match event::read() {
            Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => break Ok(()),
            Ok(_) => {}
            Err(e) => break Err(e),
        }
    };
    disable_raw_mode()?;
    pressed
}

/// Back to the alternate screen and raw mode, with a fresh terminal so
/// the next draw paints everything. (`Terminal::clear` would do, but it
/// asks the terminal where the cursor is and waits up to two seconds for
/// an answer that a plain pty never gives.) The screen is wiped row by
/// row, never with a whole-screen erase: see `screen`.
fn come_back(terminal: &mut Screen) -> io::Result<()> {
    enable_raw_mode()?;
    screen::take_over(&mut io::stdout())?;
    execute!(io::stdout(), cursor::Hide)?;
    let (_, rows) = terminal::size()?;
    screen::wipe(&mut io::stdout(), rows)?;
    *terminal = Terminal::new(Quiet::new(io::stdout()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard};

    use super::*;

    fn sh() -> PathBuf {
        PathBuf::from("/bin/sh")
    }

    /// `run_in` swaps the process's SIGINT handler, so the tests that
    /// call it take turns.
    fn one_at_a_time() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn the_persons_shell_is_used_and_sh_when_there_is_none() {
        assert_eq!(program(Some("/bin/zsh")), PathBuf::from("/bin/zsh"));
        assert_eq!(program(Some("")), PathBuf::from("/bin/sh"));
        assert_eq!(program(Some("  ")), PathBuf::from("/bin/sh"));
        assert_eq!(program(None), PathBuf::from("/bin/sh"));
    }

    #[test]
    fn the_command_runs_in_the_directory_and_its_exit_code_comes_back() {
        let _turn = one_at_a_time();
        let dir = std::env::temp_dir();
        let ended = |command: &str| run_in(&sh(), &dir, command).unwrap().ended;
        assert_eq!(ended("true"), Ended::Exit(0));
        assert_eq!(ended("exit 3"), Ended::Exit(3));
        let here = dir.canonicalize().unwrap();
        let marker = format!("test -d . && [ \"$(pwd -P)\" = '{}' ]", here.display());
        assert_eq!(ended(&marker), Ended::Exit(0));
        assert_eq!(
            ended("cd / && [ \"$(pwd -P)\" = '/' ] && exit 7"),
            Ended::Exit(7)
        );
    }

    #[test]
    fn the_time_the_command_took_comes_back_with_how_it_ended() {
        let _turn = one_at_a_time();
        let dir = std::env::temp_dir();
        let ran = run_in(&sh(), &dir, "sleep 0.3; exit 2").unwrap();
        assert_eq!(ran.ended, Ended::Exit(2));
        assert!(ran.took >= Duration::from_millis(300), "{:?}", ran.took);
        assert!(ran.took < Duration::from_secs(20), "{:?}", ran.took);
    }

    #[test]
    fn a_signal_that_stops_the_command_is_told_apart_from_an_exit() {
        let _turn = one_at_a_time();
        let dir = std::env::temp_dir();
        let ended = run_in(&sh(), &dir, "kill -TERM $$").unwrap().ended;
        assert_eq!(ended, Ended::Signal(libc::SIGTERM));
    }

    #[test]
    fn ctrl_c_reaches_the_command_and_not_imece() {
        // The terminal sends ctrl-c's SIGINT to imece and the command
        // alike; here the shell does: this process must shrug it off (or
        // the whole test binary dies here), the child must die of it.
        let _turn = one_at_a_time();
        let dir = std::env::temp_dir();
        let ended = run_in(&sh(), &dir, "kill -INT $PPID; kill -INT $$")
            .unwrap()
            .ended;
        assert_eq!(ended, Ended::Signal(libc::SIGINT));
        // Afterwards the handler is what it was, not left at ignore.
        // SAFETY: reading the disposition and putting it straight back.
        let now = unsafe { libc::signal(libc::SIGINT, libc::SIG_IGN) };
        unsafe { libc::signal(libc::SIGINT, now) };
        assert_ne!(now, libc::SIG_IGN, "SIGINT stayed ignored after the run");
    }

    #[test]
    fn a_shell_that_is_not_there_is_an_error_not_a_hang() {
        let _turn = one_at_a_time();
        let dir = std::env::temp_dir();
        let e = run_in(Path::new("/nonexistent/shell"), &dir, "true").unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn an_exit_status_without_code_or_signal_is_minus_one() {
        use std::os::unix::process::ExitStatusExt;
        // A stopped-by-signal status: code none, signal none.
        let stopped = ExitStatus::from_raw(0x7f | (libc::SIGSTOP << 8));
        assert_eq!(ended(stopped), Ended::Exit(-1));
        assert_eq!(ended(ExitStatus::from_raw(2 << 8)), Ended::Exit(2));
        assert_eq!(
            ended(ExitStatus::from_raw(libc::SIGKILL)),
            Ended::Signal(libc::SIGKILL)
        );
    }
}
