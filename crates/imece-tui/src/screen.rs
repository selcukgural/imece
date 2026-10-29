//! The screen's terminal: ratatui over crossterm, with one change. A
//! whole-screen erase (`CSI 2 J`, which ratatui sends after every change
//! of size) makes iTerm2 move the erased screen into the scrollback
//! ("Save lines to scrollback in alternate screen mode", on by default),
//! so each resize left a copy of imece above the shell's history, and the
//! terminal's reflow of those copies drew staircases of frame bars (seen
//! 2026-10-29; `CSI J` from the top does the same). Erasing line by line
//! (`CSI 2 K`, one per row) leaves nothing behind, in iTerm2 or anywhere
//! else, so that is the only erase this terminal ever sends. The other
//! way a line can leave the alternate screen is a write past the right
//! edge of the bottom row, which wraps and scrolls: it happens when a
//! frame drawn for one width lands in a terminal that has just become
//! narrower (iTerm2 resizes while the window is dragged). ratatui places
//! every cell itself and never needs the wrap, so auto-wrap (DECAWM) is
//! off while the screen is up and on again whenever the terminal is
//! handed back, to the shell at the end or to a `!` command.

use std::io::{self, Stdout, Write};

use ratatui::Terminal;
use ratatui::backend::{Backend, ClearType, CrosstermBackend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::crossterm::cursor::MoveTo;
use ratatui::crossterm::terminal::{
    self, Clear, DisableLineWrap, EnableLineWrap, EnterAlternateScreen, LeaveAlternateScreen,
    disable_raw_mode, enable_raw_mode,
};
use ratatui::crossterm::{execute, queue};
use ratatui::layout::{Position, Size};

/// imece's terminal: the alternate screen, raw mode, quiet erases.
pub type Screen = Terminal<Quiet<Stdout>>;

/// Takes the terminal (raw mode, alternate screen without auto-wrap, a
/// panic hook that gives it back) and returns the screen to draw on.
/// `restore` undoes it.
pub fn init() -> io::Result<Screen> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        hook(info);
    }));
    enable_raw_mode()?;
    take_over(&mut io::stdout())?;
    Terminal::new(Quiet::new(io::stdout()))
}

/// Gives the terminal back: auto-wrap on, the main screen, cooked mode.
/// Nothing to do about a failure this late, so it is only printed.
pub fn restore() {
    if let Err(e) = give_back(&mut io::stdout()).and_then(|()| disable_raw_mode()) {
        eprintln!("imece: the terminal could not be restored: {e}");
    }
}

/// The alternate screen, with auto-wrap off.
pub fn take_over(out: &mut impl Write) -> io::Result<()> {
    execute!(out, EnterAlternateScreen, DisableLineWrap)
}

/// Auto-wrap on again, then the main screen.
pub fn give_back(out: &mut impl Write) -> io::Result<()> {
    execute!(out, EnableLineWrap, LeaveAlternateScreen)
}

/// Erases `rows` rows one by one and leaves the cursor at the top left.
pub fn wipe(out: &mut impl Write, rows: u16) -> io::Result<()> {
    for y in 0..rows {
        queue!(out, MoveTo(0, y), Clear(terminal::ClearType::CurrentLine))?;
    }
    queue!(out, MoveTo(0, 0))?;
    out.flush()
}

/// How many rows the terminal has right now.
fn terminal_rows() -> io::Result<u16> {
    terminal::size().map(|(_, rows)| rows)
}

/// The crossterm backend, erasing the whole screen only row by row.
pub struct Quiet<W: Write> {
    inner: CrosstermBackend<W>,
    rows: fn() -> io::Result<u16>,
}

impl<W: Write> Quiet<W> {
    pub fn new(writer: W) -> Self {
        Self {
            inner: CrosstermBackend::new(writer),
            rows: terminal_rows,
        }
    }

    /// For tests: the row count comes from `rows`, not the terminal.
    #[cfg(test)]
    fn with_rows(writer: W, rows: fn() -> io::Result<u16>) -> Self {
        Self {
            inner: CrosstermBackend::new(writer),
            rows,
        }
    }
}

impl<W: Write> Backend for Quiet<W> {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.inner.draw(content)
    }

    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        self.inner.append_lines(n)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.clear_region(ClearType::All)
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        match clear_type {
            ClearType::All => {
                let rows = (self.rows)()?;
                // The backend is a writer too: it passes bytes on.
                wipe(&mut self.inner, rows)
            }
            other => self.inner.clear_region(other),
        }
    }

    fn size(&self) -> io::Result<Size> {
        self.inner.size()
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> io::Result<()> {
        Backend::flush(&mut self.inner)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    /// A writer whose bytes, and how often it was flushed, can be read
    /// back from outside the backend.
    #[derive(Clone, Default)]
    struct Shared(Rc<RefCell<(Vec<u8>, usize)>>);

    impl Write for Shared {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().0.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            self.0.borrow_mut().1 += 1;
            Ok(())
        }
    }

    fn quiet(rows: fn() -> io::Result<u16>) -> (Quiet<Shared>, Shared) {
        let out = Shared::default();
        (Quiet::with_rows(out.clone(), rows), out)
    }

    fn written(out: &Shared) -> String {
        String::from_utf8(out.0.borrow().0.clone()).unwrap()
    }

    #[test]
    fn a_wipe_erases_every_row_on_its_own_and_parks_the_cursor_at_the_top() {
        let mut out = Vec::new();
        wipe(&mut out, 3).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "\x1b[1;1H\x1b[2K\x1b[2;1H\x1b[2K\x1b[3;1H\x1b[2K\x1b[1;1H"
        );
        let mut none = Vec::new();
        wipe(&mut none, 0).unwrap();
        assert_eq!(String::from_utf8(none).unwrap(), "\x1b[1;1H");
    }

    #[test]
    fn the_screen_is_taken_without_auto_wrap_and_given_back_with_it() {
        let mut taken = Vec::new();
        take_over(&mut taken).unwrap();
        assert_eq!(String::from_utf8(taken).unwrap(), "\x1b[?1049h\x1b[?7l");
        let mut given = Vec::new();
        give_back(&mut given).unwrap();
        assert_eq!(String::from_utf8(given).unwrap(), "\x1b[?7h\x1b[?1049l");
    }

    #[test]
    fn a_wipe_never_sends_a_whole_screen_erase() {
        let mut out = Vec::new();
        wipe(&mut out, 40).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(!text.contains("\x1b[2J"), "{text:?}");
        assert!(!text.contains("\x1b[J"), "{text:?}");
        assert!(!text.contains("\x1b[1J"), "{text:?}");
        assert_eq!(text.matches("\x1b[2K").count(), 40);
    }

    #[test]
    fn clearing_everything_goes_row_by_row_over_the_terminal_rows() {
        let (mut q, out) = quiet(|| Ok(2));
        q.clear_region(ClearType::All).unwrap();
        assert_eq!(written(&out), "\x1b[1;1H\x1b[2K\x1b[2;1H\x1b[2K\x1b[1;1H");
        let (mut c, cleared) = quiet(|| Ok(2));
        c.clear().unwrap();
        assert_eq!(written(&cleared), written(&out));
    }

    #[test]
    fn a_terminal_that_will_not_say_its_rows_is_not_erased_at_all() {
        let (mut q, out) = quiet(|| Err(io::Error::other("no tty")));
        let err = q.clear_region(ClearType::All).unwrap_err();
        assert_eq!(err.to_string(), "no tty");
        assert_eq!(written(&out), "");
    }

    #[test]
    fn the_smaller_erases_still_go_through_as_they_are() {
        for (kind, bytes) in [
            (ClearType::CurrentLine, "\x1b[2K"),
            (ClearType::UntilNewLine, "\x1b[K"),
            (ClearType::AfterCursor, "\x1b[J"),
            (ClearType::BeforeCursor, "\x1b[1J"),
        ] {
            let (mut q, out) = quiet(|| Ok(5));
            q.clear_region(kind).unwrap();
            assert_eq!(written(&out), bytes, "{kind:?}");
        }
    }

    #[test]
    fn drawing_and_the_cursor_reach_the_terminal_untouched() {
        let (mut q, out) = quiet(|| Ok(5));
        let mut cell = Cell::default();
        cell.set_symbol("x");
        q.draw([(2u16, 1u16, &cell)].into_iter()).unwrap();
        assert!(written(&out).contains("\x1b[2;3Hx"), "{:?}", written(&out));
        let before = out.0.borrow().1;
        Backend::flush(&mut q).unwrap();
        assert_eq!(out.0.borrow().1, before + 1, "flush reaches the writer");

        let (mut c, moved) = quiet(|| Ok(5));
        c.set_cursor_position(Position::new(1, 2)).unwrap();
        c.hide_cursor().unwrap();
        c.show_cursor().unwrap();
        c.append_lines(2).unwrap();
        assert_eq!(written(&moved), "\x1b[3;2H\x1b[?25l\x1b[?25h\n\n");
    }
}
