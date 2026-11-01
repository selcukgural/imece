//! `!command`: a command of the person's own, in their own shell, as
//! `:!` does in vim. The screen steps aside (the alternate screen left),
//! the shell runs in the checkout on a terminal of its own (a pty, as
//! `script(1)` does): imece passes the person's keys to it and copies
//! what it prints to the screen as it comes, keeping the last of it
//! ([`KEPT`]) so that `O` can quote it in a message (record 68). A key
//! brings the screen back. Nothing here touches an agent: no sandbox, no
//! policy, no budget; it is the person's shell, which they could open
//! next door anyway, and what it printed reaches an agent only in a
//! message the person sends.

use std::fs::File;
use std::io::{self, Read, Write};
use std::iter::Peekable;
use std::mem::ManuallyDrop;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::str::Chars;
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::crossterm::terminal::{self, disable_raw_mode, enable_raw_mode};
use ratatui::crossterm::{cursor, execute};

use crate::i18n::clean;
use crate::model::Ended;
use crate::screen::{self, Quiet, Screen};

/// How a command ran: how it ended and how long it took, from the spawn
/// to its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ran {
    pub ended: Ended,
    pub took: Duration,
}

/// The most of a command's output kept for a message: its last 64 KiB.
pub const KEPT: usize = 64 * 1024;

/// How long the terminal of a command that ended is still read: for what
/// was on its way, or while a child it left behind holds it open.
const DRAIN: Duration = Duration::from_millis(300);

/// How long one wait for output or a key lasts, in milliseconds: the
/// command's end and a new window size are seen at least this often.
const TICK: libc::c_int = 20;

/// What a command left: how it ran, the end of what it printed (at most
/// [`KEPT`] bytes, as its terminal got them) and how many lines it
/// printed in all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Caught {
    pub ran: Ran,
    pub kept: Vec<u8>,
    pub lines: usize,
}

/// The person's terminal, for a command run in front of them: where the
/// keys come from, how it was set before raw mode (which the command's
/// terminal starts as), and the descriptor whose size the command's
/// terminal follows.
pub struct Keys {
    pub input: RawFd,
    pub like: Option<libc::termios>,
    pub screen: RawFd,
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

/// The size a command's terminal has when there is no screen to follow.
const DEFAULT_SIZE: libc::winsize = libc::winsize {
    ws_row: 24,
    ws_col: 80,
    ws_xpixel: 0,
    ws_ypixel: 0,
};

/// The size of the terminal on `fd`; none when it is no terminal.
fn size_of(fd: RawFd) -> Option<libc::winsize> {
    // SAFETY: TIOCGWINSZ writes one `winsize`, which lives through the call.
    let mut size: libc::winsize = unsafe { std::mem::zeroed() };
    let asked = unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut size) };
    (asked == 0).then_some(size)
}

/// How the terminal on `fd` is set; none when it is no terminal.
pub fn termios_of(fd: RawFd) -> Option<libc::termios> {
    // SAFETY: tcgetattr writes one `termios`, which lives through the call.
    let mut t: libc::termios = unsafe { std::mem::zeroed() };
    let asked = unsafe { libc::tcgetattr(fd, &mut t) };
    (asked == 0).then_some(t)
}

/// What turns a terminal's special character off.
#[cfg(target_os = "linux")]
const VDISABLE: libc::cc_t = 0;
#[cfg(not(target_os = "linux"))]
const VDISABLE: libc::cc_t = 0xff;

/// `t` with ctrl-z (and macOS's delayed ctrl-y) stopping nothing: a
/// stopped command would hold the screen with nothing to bring it back,
/// since imece waits for the command to end, not to stop.
fn no_suspend(mut t: libc::termios) -> libc::termios {
    t.c_cc[libc::VSUSP] = VDISABLE;
    #[cfg(target_os = "macos")]
    {
        t.c_cc[libc::VDSUSP] = VDISABLE;
    }
    t
}

/// A new pty: its master (imece's side) and its slave (the command's
/// terminal), set like `like` without suspend, `size` big. Both are
/// closed on exec, so a child another thread spawns meanwhile holds
/// neither; the command gets the slave as its own before its exec.
fn open_pty(like: Option<&libc::termios>, size: libc::winsize) -> io::Result<(OwnedFd, OwnedFd)> {
    let (mut master, mut slave) = (-1, -1);
    let mut like = like.copied().map(no_suspend);
    let mut size = size;
    let termp = like
        .as_mut()
        .map_or(std::ptr::null_mut(), |t| t as *mut libc::termios);
    // SAFETY: openpty writes two descriptors; every pointer lives through
    // the call, and a null `termp` means the system's defaults.
    let opened = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            termp,
            &mut size,
        )
    };
    if opened != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: two fresh descriptors nobody else owns.
    let pair = unsafe { (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) };
    for fd in [&pair.0, &pair.1] {
        // SAFETY: setting a flag on a descriptor this function owns.
        unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) };
    }
    Ok(pair)
}

/// Runs `command` through `shell` in `cwd` on a terminal of its own,
/// copies what it prints to `out` as it comes and waits for it. With
/// `keys`, the person's keys go to it (their terminal is in raw mode, so
/// ctrl-c is a key the command's terminal turns into its SIGINT) and its
/// terminal starts like theirs and follows their screen's size; without,
/// it gets no keys and 80 by 24. SIGINT is ignored here while the command
/// runs anyway (one sent to imece's group is meant for the command), and
/// set back to the default in the child. The time is taken around the
/// run.
pub fn run_in(
    shell: &Path,
    cwd: &Path,
    command: &str,
    keys: Option<&Keys>,
    out: &mut impl Write,
) -> io::Result<Caught> {
    let size = keys.and_then(|k| size_of(k.screen)).unwrap_or(DEFAULT_SIZE);
    let (master, slave) = open_pty(keys.and_then(|k| k.like.as_ref()), size)?;
    // SAFETY: `signal` only swaps the handler; the old one is put back
    // below. SIG_IGN is inherited across exec, so the child resets it in
    // `pre_exec`, where only async-signal-safe calls are made.
    let was = unsafe { libc::signal(libc::SIGINT, libc::SIG_IGN) };
    let mut child = Command::new(shell);
    child
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let tty = slave.as_raw_fd();
    // SAFETY: see above; `login_tty` starts a session, makes the slave its
    // terminal and the command's stdin, stdout and stderr, with syscalls
    // only.
    unsafe {
        child.pre_exec(move || {
            if libc::login_tty(tty) != 0 {
                return Err(io::Error::last_os_error());
            }
            libc::signal(libc::SIGINT, libc::SIG_DFL);
            Ok(())
        });
    }
    let start = Instant::now();
    let spawned = child.spawn();
    // The command has its own copy: the terminal ends when it lets go.
    drop(slave);
    let caught = spawned.and_then(|mut c| pump(&mut c, File::from(master), keys, start, out));
    // SAFETY: restoring the handler that was there.
    unsafe { libc::signal(libc::SIGINT, was) };
    caught
}

/// What one read from a descriptor brought.
#[derive(Debug, PartialEq, Eq)]
enum Got {
    Bytes(usize),
    /// A signal came first; nothing was read.
    Again,
    /// Nothing more will come.
    End,
}

fn read_from(from: &mut File, buf: &mut [u8]) -> Got {
    got(from.read(buf))
}

/// What a read's result says.
fn got(read: io::Result<usize>) -> Got {
    match read {
        Ok(0) => Got::End,
        Ok(n) => Got::Bytes(n),
        Err(e) if e.kind() == io::ErrorKind::Interrupted => Got::Again,
        // EIO on the master once the command's side is closed.
        Err(_) => Got::End,
    }
}

/// Whether the read of a command's terminal is over: [`DRAIN`] after the
/// command ended at `ended`.
fn drained(ended: Instant, now: Instant) -> bool {
    now.duration_since(ended) >= DRAIN
}

/// `kept` with `more` after it, cut to its last [`KEPT`] bytes.
fn keep(kept: &mut Vec<u8>, more: &[u8]) {
    kept.extend_from_slice(more);
    let over = kept.len().saturating_sub(KEPT);
    kept.drain(..over);
}

/// The loop of a running command: its output to `out` and `kept`, the
/// person's keys to it, the screen's size to its terminal, until it has
/// ended and its terminal has nothing more (or [`DRAIN`] after its end,
/// for a child it left holding the terminal). Two ways out, so that the
/// loop ends even if one is never seen: the command's end, and its
/// terminal's. A terminal that ended first (the command closed it) is
/// closed here too, which hangs the command up, and its end is waited
/// for.
fn pump(
    child: &mut Child,
    mut master: File,
    keys: Option<&Keys>,
    start: Instant,
    out: &mut impl Write,
) -> io::Result<Caught> {
    // SAFETY: the person's input stays open for imece after this; the
    // `File` only borrows it and is never dropped.
    let mut input = keys.map(|k| ManuallyDrop::new(unsafe { File::from_raw_fd(k.input) }));
    let mut size = keys
        .and_then(|k| size_of(k.screen))
        .map(|s| (s.ws_row, s.ws_col));
    let mut kept = Vec::new();
    let mut lines = 0;
    let mut done: Option<(ExitStatus, Duration, Instant)> = None;
    let mut buf = [0u8; 4096];
    loop {
        if done.is_none()
            && let Some(status) = child.try_wait()?
        {
            done = Some((status, start.elapsed(), Instant::now()));
        }
        if done.is_some_and(|(_, _, at)| drained(at, Instant::now())) {
            break;
        }
        let keys_fd = input.as_ref().map_or(-1, |i| i.as_raw_fd());
        let mut fds = [
            libc::pollfd {
                fd: master.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: keys_fd,
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: two `pollfd`s that live through the call; a negative
        // descriptor is skipped.
        unsafe { libc::poll(fds.as_mut_ptr(), 2, TICK) };
        if fds[0].revents != 0 {
            match read_from(&mut master, &mut buf) {
                Got::Bytes(n) => {
                    let got = &buf[..n];
                    out.write_all(got)?;
                    out.flush()?;
                    keep(&mut kept, got);
                    lines += got.iter().filter(|&&b| b == b'\n').count();
                }
                Got::Again => {}
                Got::End => break,
            }
        }
        if fds[1].revents != 0
            && let Some(from) = input.as_mut()
        {
            match read_from(from, &mut buf) {
                Got::Bytes(n) => master.write_all(&buf[..n])?,
                Got::Again => {}
                Got::End => input = None,
            }
        }
        let now = keys.and_then(|k| size_of(k.screen));
        if let Some(s) = now
            && Some((s.ws_row, s.ws_col)) != size
        {
            size = Some((s.ws_row, s.ws_col));
            // SAFETY: TIOCSWINSZ reads one `winsize`; the command's
            // terminal tells it of the change (SIGWINCH).
            unsafe { libc::ioctl(master.as_raw_fd(), libc::TIOCSWINSZ, &s) };
        }
    }
    let (status, took) = match done {
        Some((status, took, _)) => (status, took),
        None => {
            drop(master);
            (child.wait()?, start.elapsed())
        }
    };
    let ran = Ran {
        ended: ended(status),
        took,
    };
    Ok(Caught { ran, kept, lines })
}

/// What an escape sequence does to the line it is in.
#[derive(Debug, PartialEq, Eq)]
enum Esc {
    /// `ESC [ K`, `ESC [ 0 K`: erase from the cursor to the end.
    EraseRight,
    /// `ESC [ 2 K`: erase the whole line.
    EraseLine,
    /// Anything else: colours, cursor moves, titles; nothing here.
    Other,
}

/// Reads one escape sequence, from after its ESC: a CSI (`ESC [`, up to
/// its final byte), a string (OSC `ESC ]`, DCS `ESC P`, APC, PM: up to BEL
/// or `ESC \`), or ESC with its intermediates and one more character.
fn escape(chars: &mut Peekable<Chars>) -> Esc {
    match chars.next() {
        Some('[') => {
            let mut param = String::new();
            for c in chars.by_ref() {
                if ('\u{40}'..='\u{7e}').contains(&c) {
                    return match (c, param.as_str()) {
                        ('K', "" | "0") => Esc::EraseRight,
                        ('K', "2") => Esc::EraseLine,
                        _ => Esc::Other,
                    };
                }
                param.push(c);
            }
            Esc::Other
        }
        Some(']' | 'P' | '_' | '^') => {
            for c in chars.by_ref() {
                if c == '\u{7}' {
                    break;
                }
                if c == '\u{1b}' {
                    chars.next_if_eq(&'\\');
                    break;
                }
            }
            Esc::Other
        }
        Some(c) if (' '..='/').contains(&c) => {
            // `ESC ( B` and its kind: intermediates, then the final.
            while chars.next_if(|c| (' '..='/').contains(c)).is_some() {}
            chars.next();
            Esc::Other
        }
        _ => Esc::Other,
    }
}

/// What `kept` shows on a terminal, line by line, as text for a message:
/// escape sequences (colours, cursor moves, titles) dropped, erase-line
/// honoured, a carriage return going back to the line's start and a
/// backspace one cell back, so what a progress bar wrote over is gone;
/// every line cleaned (`i18n::clean`: control characters, secret-looking
/// words) and its trailing spaces cut, blank lines at the end dropped.
/// Bytes that are no UTF-8 (a cut at the front) become `�`.
pub fn screen_lines(kept: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(kept);
    let mut chars = text.chars().peekable();
    let mut lines = Vec::new();
    let mut line: Vec<char> = Vec::new();
    let mut col: usize = 0;
    let done = |line: &[char]| {
        clean(&line.iter().collect::<String>())
            .trim_end()
            .to_string()
    };
    while let Some(c) = chars.next() {
        match c {
            '\n' => {
                lines.push(done(&line));
                line.clear();
                col = 0;
            }
            '\r' => col = 0,
            '\u{8}' => col = col.saturating_sub(1),
            '\u{1b}' => match escape(&mut chars) {
                Esc::EraseRight => line.truncate(col),
                Esc::EraseLine => line.clear(),
                Esc::Other => {}
            },
            c if c.is_control() && c != '\t' => {}
            c => {
                line.resize(line.len().max(col + 1), ' ');
                line[col] = c;
                col += 1;
            }
        }
    }
    if !line.is_empty() {
        lines.push(done(&line));
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

/// The last `!` command of a project, for `O`: what was typed, how it
/// ran, the last lines it showed (cleaned; as many as `>` quotes of a
/// hunk, `ui::QUOTE_LINES`) and how many came before them. Kept in memory
/// only, never saved: it may hold what the person would not write down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    pub command: String,
    pub ended: Ended,
    pub took: u64,
    pub lines: Vec<String>,
    pub above: usize,
}

impl Output {
    pub fn of(command: &str, caught: &Caught) -> Output {
        let all = screen_lines(&caught.kept);
        let shown = all.len().min(crate::ui::QUOTE_LINES);
        // Lines cut from the front of what was kept count too.
        let total = caught.lines.max(all.len());
        Output {
            command: command.to_string(),
            ended: caught.ran.ended,
            took: u64::try_from(caught.ran.took.as_millis()).unwrap_or(u64::MAX),
            lines: all[all.len() - shown..].to_vec(),
            above: total - shown,
        }
    }
}

/// The whole round: the screen steps aside, `command` runs in `cwd` on a
/// terminal of its own with the person's keys, `back` (already worded by
/// the caller from how it ran) is shown and a key brings the screen
/// back. What the command printed stays on the terminal's main screen,
/// where it is seen again when imece quits, and its end comes back for
/// `O`.
pub fn run(
    terminal: &mut Screen,
    cwd: &Path,
    command: &str,
    back: impl FnOnce(Ran) -> String,
) -> io::Result<Caught> {
    step_aside()?;
    let mut out = io::stdout();
    writeln!(out, "$ {command}")?;
    out.flush()?;
    let keys = Keys {
        input: libc::STDIN_FILENO,
        // Taken before raw mode: the command's terminal starts cooked.
        like: termios_of(libc::STDIN_FILENO),
        screen: libc::STDOUT_FILENO,
    };
    // Keys pass through as they are; the command's terminal cooks them.
    let caught = enable_raw_mode().and_then(|()| {
        run_in(
            &program(std::env::var("SHELL").ok().as_deref()),
            cwd,
            command,
            Some(&keys),
            &mut out,
        )
    });
    disable_raw_mode()?;
    let caught = match caught {
        Ok(c) => c,
        Err(e) => {
            // Back to the screen whatever happened; the error is reported
            // there.
            come_back(terminal)?;
            return Err(e);
        }
    };
    write!(out, "{}", back(caught.ran))?;
    out.flush()?;
    wait_for_key()?;
    come_back(terminal)?;
    Ok(caught)
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

    /// `command` run with no keys, what it printed thrown away.
    fn quiet(shell: &Path, dir: &Path, command: &str) -> io::Result<Caught> {
        run_in(shell, dir, command, None, &mut Vec::new())
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
        let ended = |command: &str| quiet(&sh(), &dir, command).unwrap().ran.ended;
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
        let ran = quiet(&sh(), &dir, "sleep 0.3; exit 2").unwrap().ran;
        assert_eq!(ran.ended, Ended::Exit(2));
        assert!(ran.took >= Duration::from_millis(300), "{:?}", ran.took);
        assert!(ran.took < Duration::from_secs(20), "{:?}", ran.took);
    }

    #[test]
    fn a_signal_that_stops_the_command_is_told_apart_from_an_exit() {
        let _turn = one_at_a_time();
        let dir = std::env::temp_dir();
        let ended = quiet(&sh(), &dir, "kill -TERM $$").unwrap().ran.ended;
        assert_eq!(ended, Ended::Signal(libc::SIGTERM));
    }

    #[test]
    fn ctrl_c_reaches_the_command_and_not_imece() {
        // The terminal sends ctrl-c's SIGINT to imece and the command
        // alike; here the shell does: this process must shrug it off (or
        // the whole test binary dies here), the child must die of it.
        let _turn = one_at_a_time();
        let dir = std::env::temp_dir();
        let ended = quiet(&sh(), &dir, "kill -INT $PPID; kill -INT $$")
            .unwrap()
            .ran
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
        let e = quiet(Path::new("/nonexistent/shell"), &dir, "true").unwrap_err();
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

    /// A pipe with `bytes` written into it: its read end, and its write
    /// end kept open (so the reader sees no end).
    fn pipe_with(bytes: &[u8]) -> (OwnedFd, File) {
        let mut fds = [-1; 2];
        // SAFETY: pipe writes two fresh descriptors.
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        let (r, w) = unsafe { (OwnedFd::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) };
        let mut w = w;
        w.write_all(bytes).unwrap();
        (r, w)
    }

    fn size(rows: u16, cols: u16) -> libc::winsize {
        libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        }
    }

    #[test]
    fn what_the_command_prints_is_copied_out_kept_and_its_lines_counted() {
        let _turn = one_at_a_time();
        let dir = std::env::temp_dir();
        let mut out = Vec::new();
        let c = run_in(
            &sh(),
            &dir,
            "printf 'one\\ntwo\\n'; echo three >&2",
            None,
            &mut out,
        )
        .unwrap();
        assert_eq!(c.ran.ended, Ended::Exit(0));
        // Its terminal turns each newline into CR LF, as a screen wants.
        assert_eq!(out, b"one\r\ntwo\r\nthree\r\n");
        assert_eq!(c.kept, out);
        assert_eq!(c.lines, 3);
        let none = run_in(&sh(), &dir, "printf ''", None, &mut Vec::new()).unwrap();
        assert!(none.kept.is_empty());
        assert_eq!(none.lines, 0);
    }

    // No test here prints more than a pty holds unread (about 1 KiB on
    // macOS): a mutant that stops the reading would leave such a command
    // blocked for ever, and the mutation run fails on a timeout. `keep`
    // is tested on its own for the cut.

    #[test]
    fn a_read_tells_bytes_a_signal_and_the_end_apart() {
        assert_eq!(got(Ok(5)), Got::Bytes(5));
        assert_eq!(got(Ok(0)), Got::End);
        let interrupted = io::Error::from(io::ErrorKind::Interrupted);
        assert_eq!(got(Err(interrupted)), Got::Again);
        // EIO: the command's side of its terminal is closed.
        assert_eq!(got(Err(io::Error::from_raw_os_error(libc::EIO))), Got::End);
    }

    #[test]
    fn the_terminal_is_read_until_the_drain_after_the_end() {
        let ended = Instant::now();
        assert!(!drained(ended, ended));
        assert!(!drained(ended, ended + DRAIN - Duration::from_millis(1)));
        assert!(drained(ended, ended + DRAIN));
        assert!(drained(ended, ended + DRAIN * 2));
    }

    #[test]
    fn keep_cuts_to_the_last_kept_bytes_exactly() {
        let mut kept = vec![b'x'; KEPT - 1];
        keep(&mut kept, b"ab");
        assert_eq!(kept.len(), KEPT);
        assert!(kept.ends_with(b"xab"));
        keep(&mut kept, b"c");
        assert_eq!(kept.len(), KEPT);
        assert!(kept.ends_with(b"xabc"));
        let mut short = b"12".to_vec();
        keep(&mut short, b"3");
        assert_eq!(short, b"123");
        let mut all = Vec::new();
        keep(&mut all, &vec![b'y'; KEPT + 3]);
        assert_eq!(all.len(), KEPT);
    }

    #[test]
    fn the_command_has_a_terminal_of_its_own_80_by_24_without_a_screen() {
        let _turn = one_at_a_time();
        let dir = std::env::temp_dir();
        let tty = quiet(&sh(), &dir, "test -t 0 && test -t 1 && test -t 2").unwrap();
        assert_eq!(tty.ran.ended, Ended::Exit(0));
        let mut out = Vec::new();
        run_in(&sh(), &dir, "stty size", None, &mut out).unwrap();
        assert_eq!(String::from_utf8_lossy(&out).trim(), "24 80");
    }

    #[test]
    fn keys_reach_the_command_and_ctrl_c_stops_it() {
        let _turn = one_at_a_time();
        let dir = std::env::temp_dir();
        let (r, _w) = pipe_with(b"hello\n");
        let keys = Keys {
            input: r.as_raw_fd(),
            like: None,
            screen: -1,
        };
        let mut out = Vec::new();
        let c = run_in(
            &sh(),
            &dir,
            // Bounded: a command that never gets its line is stopped.
            "(sleep 3; kill $$) & read line; echo \"got $line\"",
            Some(&keys),
            &mut out,
        )
        .unwrap();
        assert_eq!(c.ran.ended, Ended::Exit(0));
        assert!(
            String::from_utf8_lossy(&out).contains("got hello"),
            "{out:?}"
        );
        // Ctrl-c is a key in raw mode: the command's terminal makes it the
        // command's SIGINT, and imece goes on.
        let (r, _w) = pipe_with(b"\x03");
        let keys = Keys {
            input: r.as_raw_fd(),
            like: None,
            screen: -1,
        };
        let c = run_in(&sh(), &dir, "sleep 5", Some(&keys), &mut Vec::new()).unwrap();
        assert_eq!(c.ran.ended, Ended::Signal(libc::SIGINT));
        assert!(c.ran.took < Duration::from_secs(4), "{:?}", c.ran.took);
    }

    #[test]
    fn keys_that_end_are_no_longer_read_and_the_command_goes_on() {
        let _turn = one_at_a_time();
        let dir = std::env::temp_dir();
        let (r, w) = pipe_with(b"");
        drop(w);
        let keys = Keys {
            input: r.as_raw_fd(),
            like: None,
            screen: -1,
        };
        let c = run_in(
            &sh(),
            &dir,
            "sleep 0.2; exit 6",
            Some(&keys),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(c.ran.ended, Ended::Exit(6));
    }

    #[test]
    fn the_command_s_terminal_has_the_screen_s_size_and_follows_it() {
        let _turn = one_at_a_time();
        let dir = std::env::temp_dir();
        let (screen_master, screen) = open_pty(None, size(30, 100)).unwrap();
        let (r, _w) = pipe_with(b"");
        let keys = Keys {
            input: r.as_raw_fd(),
            like: None,
            screen: screen.as_raw_fd(),
        };
        let mut out = Vec::new();
        run_in(&sh(), &dir, "stty size", Some(&keys), &mut out).unwrap();
        assert_eq!(String::from_utf8_lossy(&out).trim(), "30 100");
        // The screen grows while the command runs: its terminal does too.
        let to = screen_master.as_raw_fd();
        let grow = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            // SAFETY: TIOCSWINSZ reads one `winsize`.
            unsafe { libc::ioctl(to, libc::TIOCSWINSZ, &size(40, 120)) };
        });
        let mut out = Vec::new();
        run_in(&sh(), &dir, "sleep 1.2; stty size", Some(&keys), &mut out).unwrap();
        grow.join().unwrap();
        assert_eq!(String::from_utf8_lossy(&out).trim(), "40 120");
    }

    #[test]
    fn a_new_terminal_is_set_like_the_persons_without_suspend() {
        let (_m, probe) = open_pty(None, size(10, 20)).unwrap();
        assert_eq!(
            size_of(probe.as_raw_fd()).map(|s| (s.ws_row, s.ws_col)),
            Some((10, 20))
        );
        let mut like = termios_of(probe.as_raw_fd()).unwrap();
        like.c_cc[libc::VINTR] = 0x18;
        like.c_cc[libc::VSUSP] = 0x1a;
        let (_m, slave) = open_pty(Some(&like), size(5, 6)).unwrap();
        let t = termios_of(slave.as_raw_fd()).unwrap();
        assert_eq!(t.c_cc[libc::VINTR], 0x18);
        assert_eq!(t.c_cc[libc::VSUSP], VDISABLE);
        assert_eq!(
            size_of(slave.as_raw_fd()).map(|s| (s.ws_row, s.ws_col)),
            Some((5, 6))
        );
        // Both ends close on exec: no other child holds them.
        for fd in [&_m, &slave] {
            // SAFETY: reading a descriptor's flags.
            let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFD) };
            assert_eq!(flags & libc::FD_CLOEXEC, libc::FD_CLOEXEC);
        }
        // No terminal, no size and no settings.
        let (r, _w) = pipe_with(b"");
        assert!(size_of(r.as_raw_fd()).is_none());
        assert!(termios_of(r.as_raw_fd()).is_none());
    }

    #[test]
    fn no_suspend_turns_ctrl_z_off_and_keeps_the_rest() {
        // SAFETY: an all-zero termios is a valid value.
        let mut t: libc::termios = unsafe { std::mem::zeroed() };
        t.c_cc[libc::VSUSP] = 0x1a;
        t.c_cc[libc::VINTR] = 0x03;
        t.c_lflag = libc::ISIG;
        let n = no_suspend(t);
        assert_eq!(n.c_cc[libc::VSUSP], VDISABLE);
        assert_eq!(n.c_cc[libc::VINTR], 0x03);
        assert_eq!(n.c_lflag, libc::ISIG);
        #[cfg(target_os = "macos")]
        assert_eq!(n.c_cc[libc::VDSUSP], VDISABLE);
    }

    #[test]
    fn a_child_left_holding_the_terminal_does_not_hold_imece() {
        let _turn = one_at_a_time();
        let dir = std::env::temp_dir();
        let start = Instant::now();
        let c = quiet(&sh(), &dir, "(trap '' HUP; sleep 3) & echo left; exit 4").unwrap();
        assert_eq!(c.ran.ended, Ended::Exit(4));
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "{:?}",
            start.elapsed()
        );
        assert!(c.kept.starts_with(b"left"), "{:?}", c.kept);
    }

    fn lines(bytes: &[u8]) -> Vec<String> {
        screen_lines(bytes)
    }

    #[test]
    fn screen_lines_are_what_the_terminal_shows() {
        assert_eq!(lines(b"a\r\nb\r\n"), ["a", "b"]);
        assert_eq!(lines(b"no end"), ["no end"]);
        assert!(lines(b"").is_empty());
        // Colours, cursor moves, a title (BEL or ST), a charset: gone.
        assert_eq!(lines(b"\x1b[1;31mred\x1b[0m \x1b[2Ahere"), ["red here"]);
        assert_eq!(lines(b"\x1b]0;title\x07x\x1b]2;t\x1b\\y"), ["xy"]);
        assert_eq!(lines(b"\x1b(Bab\x1b%Gc\x1b7d"), ["abcd"]);
        assert_eq!(lines(b"\x1bPdcs\x1b\\e\x1b_apc\x07f"), ["ef"]);
        assert_eq!(lines(b"cut \x1b["), ["cut"]);
        assert_eq!(lines(b"cut \x1b]title"), ["cut"]);
        // A carriage return writes over, a backspace goes back one.
        assert_eq!(lines(b"10%\r20%\r100%\r\n"), ["100%"]);
        assert_eq!(lines(b"abcdef\rXY\n"), ["XYcdef"]);
        assert_eq!(lines(b"ab\x08\x08\x08X\n"), ["Xb"]);
        assert_eq!(lines(b"ab\x08c\n"), ["ac"]);
        // Erase to the end of the line, and the whole line.
        assert_eq!(lines(b"abcdef\r\x1b[Kdone\n"), ["done"]);
        assert_eq!(lines(b"abcdef\rab\x1b[0K\n"), ["ab"]);
        assert_eq!(lines(b"abcdef\x1b[2Kz\n"), ["      z"]);
        assert_eq!(lines(b"abc\x1b[1Kd\n"), ["abcd"]);
        // Control characters do not write; a tab is a space.
        assert_eq!(lines(b"a\x07b\tc\n"), ["ab c"]);
        // Trailing spaces and blank lines at the end are cut; blank lines
        // between stay.
        assert_eq!(lines(b"a  \r\n\r\nb\r\n\r\n  \r\n"), ["a", "", "b"]);
        // Secrets are hidden, bytes that are no UTF-8 are marked.
        assert_eq!(lines(b"export TOKEN=abc\n"), ["export [***]"]);
        assert_eq!(lines(b"\xffok\n"), ["\u{fffd}ok"]);
    }

    fn caught(kept: &[u8], lines: usize) -> Caught {
        Caught {
            ran: Ran {
                ended: Ended::Exit(1),
                took: Duration::from_millis(2_500),
            },
            kept: kept.to_vec(),
            lines,
        }
    }

    #[test]
    fn an_output_keeps_the_last_lines_and_counts_the_ones_above() {
        let few = Output::of("make", &caught(b"a\r\nb\r\n", 2));
        assert_eq!(
            few,
            Output {
                command: "make".into(),
                ended: Ended::Exit(1),
                took: 2_500,
                lines: vec!["a".into(), "b".into()],
                above: 0,
            }
        );
        let many: String = (1..=50).map(|i| format!("l{i}\r\n")).collect();
        let o = Output::of("x", &caught(many.as_bytes(), 50));
        assert_eq!(o.lines.len(), crate::ui::QUOTE_LINES);
        assert_eq!(o.lines[0], "l11");
        assert_eq!(o.lines.last().unwrap(), "l50");
        assert_eq!(o.above, 10);
        // More lines went by than were kept: they count too.
        let o = Output::of("x", &caught(many.as_bytes(), 900));
        assert_eq!(o.above, 860);
        // A last line with no newline is a line of its own.
        let o = Output::of("x", &caught(b"a\nb", 1));
        assert_eq!(o.lines, ["a", "b"]);
        assert_eq!(o.above, 0);
    }
}
