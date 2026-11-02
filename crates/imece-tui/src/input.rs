//! Waiting for the terminal's input before crossterm reads it, so that a
//! terminal gone away (its tab closed, `tmux kill-server`) ends imece
//! instead of spinning it.
//!
//! crossterm 0.29 reads the terminal in a loop that ends only on an event
//! or "would block": a read that finds the end (0 bytes) or fails (EIO)
//! goes round again at once, forever, and never comes back to see a
//! signal. imece asks first, with its own `poll` and the number of bytes
//! waiting (`FIONREAD`), and calls crossterm only when bytes are there.
//! In raw mode (no canonical end-of-file key), a terminal that says it
//! can be read with nothing in it has been hung up.

use std::io;
use std::os::fd::RawFd;
use std::time::Duration;

/// What the terminal's input has after a wait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    /// Bytes to read: crossterm reads them without spinning.
    Ready,
    /// Nothing came within the wait.
    Quiet,
    /// The terminal went away: nothing will come again.
    Gone,
}

/// The descriptor crossterm reads: stdin when it is a terminal. `None`
/// otherwise (crossterm opens `/dev/tty` then, which macOS's `poll` can
/// not wait on), and imece waits through crossterm as before.
pub fn terminal() -> Option<RawFd> {
    // SAFETY: isatty reads nothing through its argument.
    (unsafe { libc::isatty(libc::STDIN_FILENO) } == 1).then_some(libc::STDIN_FILENO)
}

/// Waits up to `wait` for `fd` to have something. A signal ends the wait
/// as `Interrupted`, so that the caller looks at what it set.
pub fn wait(fd: RawFd, wait: Duration) -> io::Result<Wait> {
    let mut poll = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    // Rounded up: a wait of 0.4 ms is not a busy return.
    let ms = i32::try_from(wait.as_micros().div_ceil(1000)).unwrap_or(i32::MAX);
    // SAFETY: one pollfd, as declared.
    let n = unsafe { libc::poll(&mut poll, 1, ms) };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(seen(n, poll.revents, || waiting(fd)))
}

/// What `poll`'s answer means: `n` descriptors with `revents`, and the
/// bytes waiting asked only when the input says it can be read.
fn seen(n: libc::c_int, revents: libc::c_short, waiting: impl FnOnce() -> usize) -> Wait {
    if n == 0 {
        return Wait::Quiet;
    }
    if revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
        return Wait::Gone;
    }
    if revents & libc::POLLIN == 0 {
        return Wait::Quiet;
    }
    if waiting() == 0 {
        Wait::Gone
    } else {
        Wait::Ready
    }
}

/// What crossterm may still hold: events it read with others and has not
/// given yet. Asked again only while it may hold some, since asking
/// crossterm on a terminal gone spins.
#[derive(Debug, Default)]
pub struct Guard {
    held: bool,
}

impl Guard {
    /// Whether an event can be read within `timeout`, `None` when the
    /// terminal went away. `wait` waits on the terminal (`wait` above),
    /// `read` asks crossterm without waiting (`event::poll(0)`).
    pub fn ready(
        &mut self,
        timeout: Duration,
        mut wait: impl FnMut(Duration) -> io::Result<Wait>,
        mut read: impl FnMut() -> io::Result<bool>,
    ) -> io::Result<Option<bool>> {
        if self.held {
            if wait(Duration::ZERO)? == Wait::Gone {
                return Ok(None);
            }
            self.held = read()?;
            if self.held {
                return Ok(Some(true));
            }
        }
        match wait(timeout)? {
            Wait::Gone => Ok(None),
            Wait::Quiet => Ok(Some(false)),
            Wait::Ready => {
                self.held = read()?;
                Ok(Some(self.held))
            }
        }
    }
}

/// The bytes waiting to be read on `fd`; 0 when it can not say: a
/// failed ask (EIO on a terminal hung up) leaves the count as it was.
fn waiting(fd: RawFd) -> usize {
    let mut n: libc::c_int = 0;
    // SAFETY: FIONREAD writes one int, and nothing when it fails.
    unsafe { libc::ioctl(fd, libc::FIONREAD, &mut n) };
    usize::try_from(n).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::time::Instant;

    /// A pty's two sides, the slave in raw mode like imece's terminal.
    fn pty() -> (File, OwnedFd) {
        let (mut master, mut slave): (RawFd, RawFd) = (0, 0);
        // SAFETY: openpty writes two descriptors; null means defaults.
        let opened = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(opened, 0);
        // SAFETY: two fresh descriptors nobody else owns.
        let (master, slave) = unsafe { (File::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) };
        // SAFETY: a termios read, made raw and written back.
        unsafe {
            let mut t: libc::termios = std::mem::zeroed();
            assert_eq!(libc::tcgetattr(slave.as_raw_fd(), &mut t), 0);
            libc::cfmakeraw(&mut t);
            assert_eq!(libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &t), 0);
        }
        (master, slave)
    }

    #[test]
    fn a_quiet_terminal_waits_its_time() {
        let (_master, slave) = pty();
        let start = Instant::now();
        let got = wait(slave.as_raw_fd(), Duration::from_millis(60)).unwrap();
        assert_eq!(got, Wait::Quiet);
        assert!(start.elapsed() >= Duration::from_millis(50), "it waited");
    }

    #[test]
    fn a_key_is_ready() {
        let (mut master, slave) = pty();
        master.write_all(b"jk").unwrap();
        let got = wait(slave.as_raw_fd(), Duration::from_secs(2)).unwrap();
        assert_eq!(got, Wait::Ready);
        // Both keys, not just "something".
        let start = Instant::now();
        while waiting(slave.as_raw_fd()) < 2 && start.elapsed() < Duration::from_secs(2) {}
        assert_eq!(waiting(slave.as_raw_fd()), 2);
        assert_eq!(waiting(-1), 0, "a descriptor that is none");
    }

    #[test]
    fn a_terminal_closed_is_gone_at_once() {
        let (master, slave) = pty();
        drop(master);
        let start = Instant::now();
        let got = wait(slave.as_raw_fd(), Duration::from_secs(5)).unwrap();
        assert_eq!(got, Wait::Gone);
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "not the whole wait"
        );
    }

    #[test]
    fn a_terminal_closed_with_keys_unread_is_still_ready_for_them_or_gone() {
        // What was typed before the end may still be read, or the end
        // may have dropped it; either way the wait never spins.
        let (mut master, slave) = pty();
        master.write_all(b"q").unwrap();
        drop(master);
        let got = wait(slave.as_raw_fd(), Duration::from_secs(5)).unwrap();
        assert_ne!(got, Wait::Quiet);
    }

    #[test]
    fn a_short_wait_is_rounded_up_not_down() {
        let (_master, slave) = pty();
        let start = Instant::now();
        let got = wait(slave.as_raw_fd(), Duration::from_micros(1_500)).unwrap();
        assert_eq!(got, Wait::Quiet);
        assert!(
            start.elapsed() >= Duration::from_micros(1_900),
            "2 ms, not 1"
        );
    }

    /// A guard's run: what each wait says, what crossterm says, and the
    /// waits asked for.
    fn run(guard: &mut Guard, waits: &[Wait], reads: &[bool]) -> (Option<bool>, Vec<Duration>) {
        let (mut w, mut r) = (waits.iter(), reads.iter());
        let mut asked = Vec::new();
        let got = guard
            .ready(
                Duration::from_millis(250),
                |t| {
                    asked.push(t);
                    Ok(*w.next().expect("a wait too many"))
                },
                || Ok(*r.next().expect("a read too many")),
            )
            .unwrap();
        assert!(w.next().is_none() && r.next().is_none(), "all asked");
        (got, asked)
    }

    #[test]
    fn a_guard_asks_crossterm_only_when_bytes_came() {
        let wait = Duration::from_millis(250);
        let mut g = Guard::default();
        assert_eq!(run(&mut g, &[Wait::Quiet], &[]), (Some(false), vec![wait]));
        assert_eq!(run(&mut g, &[Wait::Gone], &[]), (None, vec![wait]));
        // Bytes that are not yet an event (half an escape sequence).
        assert_eq!(
            run(&mut g, &[Wait::Ready], &[false]),
            (Some(false), vec![wait])
        );
        assert!(!g.held);
        assert_eq!(
            run(&mut g, &[Wait::Ready], &[true]),
            (Some(true), vec![wait])
        );
        assert!(g.held);
    }

    #[test]
    fn a_guard_gives_what_crossterm_holds_before_waiting() {
        let (zero, wait) = (Duration::ZERO, Duration::from_millis(250));
        let mut g = Guard { held: true };
        assert_eq!(
            run(&mut g, &[Wait::Quiet], &[true]),
            (Some(true), vec![zero])
        );
        assert!(g.held);
        // Held nothing more: then waits as usual.
        assert_eq!(
            run(&mut g, &[Wait::Quiet, Wait::Quiet], &[false]),
            (Some(false), vec![zero, wait])
        );
        assert!(!g.held);
        let mut g = Guard { held: true };
        assert_eq!(
            run(&mut g, &[Wait::Ready, Wait::Ready], &[false, true]),
            (Some(true), vec![zero, wait])
        );
        // Gone while crossterm may hold more: never asked, it would spin.
        let mut g = Guard { held: true };
        assert_eq!(run(&mut g, &[Wait::Gone], &[]), (None, vec![zero]));
    }

    #[test]
    fn what_poll_says_is_read_by_its_flags() {
        let none = || -> usize { panic!("not asked") };
        assert_eq!(seen(0, 0, none), Wait::Quiet);
        assert_eq!(seen(1, libc::POLLHUP, none), Wait::Gone);
        assert_eq!(seen(1, libc::POLLERR, none), Wait::Gone);
        assert_eq!(seen(1, libc::POLLNVAL, none), Wait::Gone);
        assert_eq!(seen(1, libc::POLLIN | libc::POLLHUP, none), Wait::Gone);
        assert_eq!(seen(1, libc::POLLIN, || 3), Wait::Ready);
        assert_eq!(seen(1, libc::POLLIN, || 1), Wait::Ready);
        assert_eq!(seen(1, libc::POLLIN, || 0), Wait::Gone);
        // Something else only (POLLPRI): nothing for crossterm to read.
        assert_eq!(seen(1, libc::POLLPRI, none), Wait::Quiet);
    }
}
