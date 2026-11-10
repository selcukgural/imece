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
//!
//! The terminal's title says how many items wait for the person (`Title`),
//! so a tab in the background shows it. The shell's title is pushed on the
//! terminal's title stack when imece takes the terminal and popped when it
//! gives it back (`CSI 22;0 t`, `CSI 23;0 t`; xterm, iTerm2, kitty,
//! WezTerm, Ghostty). Before the pop the title is emptied, so a terminal
//! without a stack shows its own default rather than a stale count.
//!
//! While imece has the screen, a paste comes as one event (bracketed
//! paste), so its line ends do not send the line, and a terminal that
//! speaks the kitty keyboard protocol is asked to tell keys apart
//! (`CSI > 1 u`, disambiguate only: shift-enter then comes as itself,
//! not as enter). Both are switched off whenever the terminal is handed
//! back, the protocol's flags popped before the alternate screen is
//! left, so the shell gets the keys it knows.
//!
//! Each frame is drawn as one synchronized update (mode 2026, `synced`),
//! so a terminal that has the mode never shows half of one. While the
//! person types, the terminal's own cursor stands where the text goes, in
//! a shape that says what is typed (`Cursor`, `Look`); the terminal
//! blinks it. Its shape and colour go back to the terminal's own, and it
//! is shown, whenever the terminal is handed back, a panic included.

use std::io::{self, Stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::{Backend, ClearType, CrosstermBackend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::crossterm::cursor::MoveTo;
use ratatui::crossterm::event::{
    DisableBracketedPaste, DisableFocusChange, EnableBracketedPaste, EnableFocusChange,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::terminal::{
    self, Clear, DisableLineWrap, EnableLineWrap, EnterAlternateScreen, LeaveAlternateScreen,
    disable_raw_mode, enable_raw_mode,
};
use ratatui::crossterm::{execute, queue};
use ratatui::layout::{Position, Size};

/// imece's terminal: the alternate screen, raw mode, quiet erases.
pub type Screen = Terminal<Quiet<Stdout>>;

/// Whether the terminal speaks the kitty keyboard protocol, as the probe
/// found: its flags are pushed when the screen is taken and popped when
/// it is given back.
static KEYS: AtomicBool = AtomicBool::new(false);

/// Whether the terminal shows a tab's progress (`shows_progress`): the
/// progress is cleared when the terminal is given back.
static PROGRESS: AtomicBool = AtomicBool::new(false);

/// Whether the terminal takes a tab icon from imece (`shows_icon`): the
/// icon is put on the tab when the screen is taken and given back with it.
static ICON: AtomicBool = AtomicBool::new(false);

/// Takes the terminal (raw mode, alternate screen without auto-wrap, a
/// panic hook that gives it back) and returns the screen to draw on.
/// `keys`: the terminal speaks the kitty keyboard protocol; `progress`:
/// it shows a tab's progress; `icon`: it takes a tab icon (Konsole).
/// `restore` undoes it.
pub fn init(keys: bool, progress: bool, icon: bool) -> io::Result<Screen> {
    KEYS.store(keys, Ordering::SeqCst);
    PROGRESS.store(progress, Ordering::SeqCst);
    ICON.store(icon, Ordering::SeqCst);
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

/// The shell's title saved, then the alternate screen, with auto-wrap off,
/// pastes bracketed and, where the terminal has them, keys told apart.
pub fn take_over(out: &mut impl Write) -> io::Result<()> {
    taking(
        out,
        KEYS.load(Ordering::SeqCst),
        ICON.load(Ordering::SeqCst),
    )
}

/// Keys as the shell knows them, auto-wrap on again, the main screen,
/// then the shell's title back.
pub fn give_back(out: &mut impl Write) -> io::Result<()> {
    giving(
        out,
        KEYS.load(Ordering::SeqCst),
        PROGRESS.load(Ordering::SeqCst),
        ICON.load(Ordering::SeqCst),
    )
}

/// `take_over`, with `keys` saying whether the kitty flags are pushed and
/// `icon` whether imece's icon goes on the tab.
fn taking(out: &mut impl Write, keys: bool, icon: bool) -> io::Result<()> {
    out.write_all(PUSH_TITLE)?;
    if icon {
        out.write_all(ICON_ON)?;
    }
    execute!(
        out,
        EnterAlternateScreen,
        DisableLineWrap,
        EnableBracketedPaste,
        // Coming back to the window is when the terminal's colours are
        // asked again: its theme may have turned light or dark meanwhile.
        EnableFocusChange
    )?;
    if keys {
        let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES;
        execute!(out, PushKeyboardEnhancementFlags(flags))?;
    }
    Ok(())
}

/// `give_back`, with `keys` saying whether the kitty flags are popped and
/// `progress` whether the tab's progress is cleared (never sent to a
/// terminal not known to show it: one that takes OSC 9 for a notice would
/// show `4;0` as one), and `icon` whether the tab's icon goes back.
fn giving(out: &mut impl Write, keys: bool, progress: bool, icon: bool) -> io::Result<()> {
    // A frame cut short (a panic while drawing) no longer holds what
    // follows; the cursor as the terminal's settings have it, shown.
    out.write_all(SYNC_END)?;
    out.write_all(CURSOR_BACK)?;
    if progress {
        out.write_all(Progress::Calm.osc())?;
    }
    if keys {
        execute!(out, PopKeyboardEnhancementFlags)?;
    }
    execute!(
        out,
        DisableFocusChange,
        DisableBracketedPaste,
        EnableLineWrap,
        LeaveAlternateScreen
    )?;
    out.write_all(b"\x1b]2;\x07")?;
    out.write_all(POP_TITLE)?;
    if icon {
        out.write_all(ICON_OFF)?;
    }
    out.flush()
}

/// Konsole's tab icon (`OSC 32`, a name of the icon theme, record 101):
/// imece's mark (`imece`, the `imece.svg` `install.sh` puts in the user's
/// hicolor theme), then `utilities-terminal`, the default profile's icon,
/// which Konsole shows as no icon at all. What a profile of the person's
/// own had cannot be asked for, so one with an icon of its own gets the
/// default back.
const ICON_ON: &[u8] = b"\x1b]32;imece\x07";
const ICON_OFF: &[u8] = b"\x1b]32;utilities-terminal\x07";

/// Whether the terminal takes a tab icon by name (`OSC 32`): Konsole (and
/// what embeds it, Yakuake and Dolphin's panel), by its answer to
/// XTVERSION or, without one, `KONSOLE_VERSION`, which it sets.
/// `IMECE_TAB_ICON=on|off` says it outright. Never inside tmux, which
/// drops an OSC it does not know. No other terminal lets a program set a
/// tab's icon; iTerm2 takes it from its own table (`install.sh`).
pub fn shows_icon(who: Option<&Who>, var: impl Fn(&str) -> Option<String>) -> bool {
    match var("IMECE_TAB_ICON").as_deref() {
        Some("on") => return true,
        Some("off") => return false,
        _ => {}
    }
    if in_tmux(who, &var) {
        return false;
    }
    match who {
        Some(w) => w.name == "konsole",
        None => var("KONSOLE_VERSION").is_some_and(|v| !v.is_empty()),
    }
}

/// xterm's window ops: save the title on the stack, take it back.
const PUSH_TITLE: &[u8] = b"\x1b[22;0t";
const POP_TITLE: &[u8] = b"\x1b[23;0t";

/// The terminal's own cursor shape (`CSI 0 q`), its own colour (`OSC
/// 112`), and shown (`CSI ?25h`).
const CURSOR_BACK: &[u8] = b"\x1b[0 q\x1b]112\x07\x1b[?25h";

/// Synchronized output (mode 2026): the terminal holds what comes between
/// these two and shows it at once, so a frame is never seen half drawn.
/// A terminal without the mode ignores both.
const SYNC_BEGIN: &[u8] = b"\x1b[?2026h";
const SYNC_END: &[u8] = b"\x1b[?2026l";

/// Runs `draw` as one synchronized frame: begun before it, ended after it
/// whatever it returned, so a failed draw never leaves the terminal
/// holding everything after it.
pub fn synced<T>(out: &mut impl Write, draw: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    out.write_all(SYNC_BEGIN)?;
    let drawn = draw();
    out.write_all(SYNC_END)?;
    out.flush()?;
    drawn
}

/// The cursor's shape (DECSCUSR, `CSI n q`); the terminal blinks it, so a
/// blinking cursor costs imece no frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    BlinkingBar,
    SteadyBar,
    BlinkingUnderline,
}

impl Shape {
    /// DECSCUSR's number for it.
    fn code(self) -> u8 {
        match self {
            Shape::BlinkingUnderline => 3,
            Shape::BlinkingBar => 5,
            Shape::SteadyBar => 6,
        }
    }
}

/// How the cursor looks while it is shown: its shape, and a colour of its
/// own (`OSC 12`) or the terminal's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    pub shape: Shape,
    pub colour: Option<[u8; 3]>,
}

/// The cursor's look as last written, so it is written only when it
/// changes.
#[derive(Default)]
pub struct Cursor(Option<Look>);

impl Cursor {
    /// Makes the cursor look like `look`, unless it already does. `None`:
    /// the cursor is hidden, and how it looks does not matter until it is
    /// shown again.
    pub fn show(&mut self, out: &mut impl Write, look: Option<Look>) -> io::Result<()> {
        let Some(look) = look else {
            return Ok(());
        };
        if self.0 == Some(look) {
            return Ok(());
        }
        write!(out, "\x1b[{} q", look.shape.code())?;
        match look.colour {
            Some([r, g, b]) => write!(out, "\x1b]12;#{r:02x}{g:02x}{b:02x}\x07")?,
            None => out.write_all(b"\x1b]112\x07")?,
        }
        out.flush()?;
        self.0 = Some(look);
        Ok(())
    }

    /// The terminal was handed back (its own cursor again): the next
    /// `show` writes whatever it is given.
    pub fn forget(&mut self) {
        self.0 = None;
    }
}

/// What the tab's progress (OSC 9;4, ConEmu's, shown by Windows Terminal,
/// Ghostty and iTerm2 on the tab or the task bar) says: the top of the
/// state ladder of the open projects' agents, so a tab in the background
/// tells whether anything is stuck, waits for the person, or works.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Progress {
    /// Nothing: no bar.
    Calm,
    /// An agent works or starts: a bar that moves by itself.
    Working,
    /// Something waits for the person: a full bar, paused (yellow).
    Waiting,
    /// An agent is stuck: a full bar, in error (red).
    Stuck,
}

impl Progress {
    /// The sequence that shows it.
    fn osc(self) -> &'static [u8] {
        match self {
            Progress::Calm => b"\x1b]9;4;0\x07",
            Progress::Working => b"\x1b]9;4;3\x07",
            Progress::Waiting => b"\x1b]9;4;4;100\x07",
            Progress::Stuck => b"\x1b]9;4;2;100\x07",
        }
    }
}

/// The terminal as it names itself in its answer to XTVERSION (`CSI > 0
/// q`, asked at start by `probe`): `kitty(0.49.2)`, `ghostty 1.4.0`,
/// `iTerm2 3.7.6`, `tmux 3.8`. Its own answer, unlike `TERM_PROGRAM`,
/// which a terminal started from another one inherits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Who {
    /// The name, in lower case.
    pub name: String,
    /// Its version's first three numbers; none when it has none.
    pub version: Option<[u32; 3]>,
}

impl Who {
    /// The answer's text (between `DCS > |` and `ST`) as a name and a
    /// version: the name runs to the first space or parenthesis.
    pub fn parse(text: &str) -> Option<Who> {
        let text = text.trim();
        let end = text.find([' ', '(']).unwrap_or(text.len());
        let name = text[..end].to_ascii_lowercase();
        if name.is_empty() {
            return None;
        }
        let rest = text[end..].trim_start_matches([' ', '(']);
        Some(Who {
            name,
            version: version_of(rest),
        })
    }

    /// Named `name`, at `least` or later.
    pub fn is(&self, name: &str, least: [u32; 3]) -> bool {
        self.name == name && self.version.is_some_and(|v| v >= least)
    }
}

/// Whether imece runs inside tmux: it says so, or `TMUX` does.
pub fn in_tmux(who: Option<&Who>, var: &impl Fn(&str) -> Option<String>) -> bool {
    who.is_some_and(|w| w.name == "tmux") || var("TMUX").is_some()
}

/// Whether the terminal is known to show a tab's progress. OSC 9 is a
/// notice in older terminals, so only an allow-list gets it.
/// `IMECE_PROGRESS=on|off` says it outright. Inside tmux it is sent:
/// tmux 3.8 passes it on to a terminal it knows shows it (Ghostty,
/// iTerm2), and an older tmux drops an OSC it does not know. Else the
/// terminal's own answer (`who`): kitty from 0.39, Ghostty from 1.2,
/// iTerm2 from 3.6.6, and nothing else that answered. Only a terminal
/// that did not answer is taken from the environment: Windows Terminal
/// (`WT_SESSION`), or `TERM_PROGRAM` with its version, or when there is
/// none `LC_TERMINAL` with its (iTerm2 sends these over ssh).
pub fn shows_progress(who: Option<&Who>, var: impl Fn(&str) -> Option<String>) -> bool {
    match var("IMECE_PROGRESS").as_deref() {
        Some("on") => return true,
        Some("off") => return false,
        _ => {}
    }
    if in_tmux(who, &var) {
        return true;
    }
    if let Some(w) = who {
        return w.is("kitty", [0, 39, 0])
            || w.is("ghostty", [1, 2, 0])
            || w.is("iterm2", [3, 6, 6]);
    }
    if var("WT_SESSION").is_some() {
        return true;
    }
    let (program, version) = match var("TERM_PROGRAM") {
        Some(p) => (p, var("TERM_PROGRAM_VERSION")),
        None => match var("LC_TERMINAL") {
            Some(p) => (p, var("LC_TERMINAL_VERSION")),
            None => return false,
        },
    };
    let least = match program.as_str() {
        "ghostty" => [1, 2, 0],
        "iTerm.app" | "iTerm2" => [3, 6, 6],
        _ => return false,
    };
    version
        .as_deref()
        .and_then(version_of)
        .is_some_and(|v| v >= least)
}

/// `3.6.6` (or `1.2.0-main+abc`, `3.6.6beta1`) as three numbers, a part
/// not there as 0; none when it does not start with a number.
fn version_of(text: &str) -> Option<[u32; 3]> {
    let mut parts = text.split('.');
    let mut out = [0; 3];
    for (n, slot) in out.iter_mut().enumerate() {
        let Some(part) = parts.next() else { break };
        let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
        match digits.parse() {
            Ok(v) => *slot = v,
            Err(_) if n == 0 => return None,
            Err(_) => break,
        }
    }
    Some(out)
}

/// The tab's progress as last written, written only when it changes and
/// only to a terminal known to show it.
pub struct Tab {
    shows: bool,
    last: Option<Progress>,
}

impl Tab {
    /// `shows`: the terminal shows a tab's progress (`shows_progress`).
    pub fn new(shows: bool) -> Self {
        Self { shows, last: None }
    }

    /// Makes the tab show `p`, unless it already does.
    pub fn show(&mut self, out: &mut impl Write, p: Progress) -> io::Result<()> {
        if !self.shows || self.last == Some(p) {
            return Ok(());
        }
        out.write_all(p.osc())?;
        out.flush()?;
        self.last = Some(p);
        Ok(())
    }

    /// The terminal was handed back (the progress cleared): the next
    /// `show` writes whatever it is given.
    pub fn forget(&mut self) {
        self.last = None;
    }
}

/// The terminal's title (OSC 2), written only when it changes.
#[derive(Default)]
pub struct Title(Option<String>);

impl Title {
    /// Sets the title to `text`, unless it already says that. Control
    /// characters are left out: none can end the sequence early.
    pub fn show(&mut self, out: &mut impl Write, text: &str) -> io::Result<()> {
        let text: String = text.chars().filter(|c| !c.is_control()).collect();
        if self.0.as_deref() == Some(text.as_str()) {
            return Ok(());
        }
        write!(out, "\x1b]2;{text}\x07")?;
        out.flush()?;
        self.0 = Some(text);
        Ok(())
    }

    /// The terminal was handed back and taken again (its title popped and
    /// pushed): the next `show` writes whatever it is given.
    pub fn forget(&mut self) {
        self.0 = None;
    }
}

/// How long after the terminal's last change of size the whole screen is
/// erased and drawn once more: a terminal that moves its cells about as
/// it resizes (iTerm2 while a window is dragged) may do it after the
/// frame for the new size came, and a frame drawn as a difference from
/// the last never touches cells it believes blank.
pub const SETTLE: Duration = Duration::from_millis(250);

/// The terminal's size as the main loop last saw it, and when the screen
/// is to be drawn whole again once it has settled (#457).
#[derive(Debug, Default)]
pub struct Sizes {
    last: Option<Size>,
    repaint: Option<Instant>,
}

impl Sizes {
    /// Whether `size`, seen at `now`, is another than the one seen last
    /// (the first is none: the first frame is drawn whole anyway). A
    /// change puts the repaint [`SETTLE`] after it, past any before.
    pub fn seen(&mut self, size: Size, now: Instant) -> bool {
        let changed = self.last.is_some_and(|last| last != size);
        self.last = Some(size);
        if changed {
            self.repaint = Some(now + SETTLE);
        }
        changed
    }

    /// Whether the repaint is due at `now`; it is taken, once.
    pub fn repaint_due(&mut self, now: Instant) -> bool {
        let due = self.repaint.is_some_and(|at| at <= now);
        if due {
            self.repaint = None;
        }
        due
    }

    /// `wait`, or less when the repaint comes sooner.
    pub fn wait(&self, now: Instant, wait: Duration) -> Duration {
        match self.repaint {
            Some(at) => wait.min(at.saturating_duration_since(now)),
            None => wait,
        }
    }
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
        assert_eq!(
            String::from_utf8(taken).unwrap(),
            "\x1b[22;0t\x1b[?1049h\x1b[?7l\x1b[?2004h\x1b[?1004h",
            "the shell's title is saved first; pastes bracketed; focus told"
        );
        let mut given = Vec::new();
        give_back(&mut given).unwrap();
        assert_eq!(
            String::from_utf8(given).unwrap(),
            "\x1b[?2026l\x1b[0 q\x1b]112\x07\x1b[?25h\x1b[?1004l\x1b[?2004l\x1b[?7h\x1b[?1049l\x1b]2;\x07\x1b[23;0t",
            "the terminal's own cursor, shown; the title emptied, then the shell's back"
        );
    }

    #[test]
    fn a_terminal_with_the_kitty_protocol_tells_keys_apart_only_while_imece_has_it() {
        let mut taken = Vec::new();
        taking(&mut taken, true, false).unwrap();
        assert_eq!(
            String::from_utf8(taken).unwrap(),
            "\x1b[22;0t\x1b[?1049h\x1b[?7l\x1b[?2004h\x1b[?1004h\x1b[>1u",
            "pushed on the alternate screen, disambiguate only"
        );
        let mut given = Vec::new();
        giving(&mut given, true, false, false).unwrap();
        assert_eq!(
            String::from_utf8(given).unwrap(),
            "\x1b[?2026l\x1b[0 q\x1b]112\x07\x1b[?25h\x1b[<1u\x1b[?1004l\x1b[?2004l\x1b[?7h\x1b[?1049l\x1b]2;\x07\x1b[23;0t",
            "popped before the alternate screen is left"
        );
    }

    #[test]
    fn a_terminal_that_shows_progress_has_it_cleared_when_given_back() {
        let mut given = Vec::new();
        giving(&mut given, false, true, false).unwrap();
        let text = String::from_utf8(given).unwrap();
        assert!(
            text.starts_with("\x1b[?2026l\x1b[0 q\x1b]112\x07\x1b[?25h\x1b]9;4;0\x07\x1b[?1004l"),
            "{text:?}"
        );
        let mut given = Vec::new();
        giving(&mut given, false, false, false).unwrap();
        assert!(!String::from_utf8(given).unwrap().contains("]9;"));
    }

    #[test]
    fn konsole_s_tab_wears_imece_s_icon_while_imece_has_the_screen_and_gets_its_own_back() {
        let mut taken = Vec::new();
        taking(&mut taken, false, true).unwrap();
        let text = String::from_utf8(taken).unwrap();
        assert_eq!(
            text, "\x1b[22;0t\x1b]32;imece\x07\x1b[?1049h\x1b[?7l\x1b[?2004h\x1b[?1004h",
            "with the title saved"
        );
        let mut given = Vec::new();
        giving(&mut given, false, false, true).unwrap();
        let text = String::from_utf8(given).unwrap();
        assert!(
            text.ends_with("\x1b]2;\x07\x1b[23;0t\x1b]32;utilities-terminal\x07"),
            "{text:?}"
        );
        let mut plain = Vec::new();
        taking(&mut plain, false, false).unwrap();
        giving(&mut plain, false, false, false).unwrap();
        assert!(!String::from_utf8(plain).unwrap().contains("]32;"));
    }

    #[test]
    fn only_konsole_is_sent_a_tab_icon_by_its_answer_else_its_variable() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        let who = |t: &str| Who::parse(t).unwrap();
        assert!(shows_icon(Some(&who("Konsole 24.08.3")), env(&[])));
        assert!(
            !shows_icon(
                Some(&who("kitty(0.49.2)")),
                env(&[("KONSOLE_VERSION", "240803")])
            ),
            "the answer decides"
        );
        assert!(shows_icon(None, env(&[("KONSOLE_VERSION", "240803")])));
        assert!(!shows_icon(None, env(&[("KONSOLE_VERSION", "")])));
        assert!(!shows_icon(None, env(&[("TERM_PROGRAM", "iTerm.app")])));
        assert!(!shows_icon(None, env(&[])));
        // tmux drops it.
        assert!(!shows_icon(
            Some(&who("tmux 3.8")),
            env(&[("KONSOLE_VERSION", "240803")])
        ));
        assert!(!shows_icon(
            None,
            env(&[("KONSOLE_VERSION", "240803"), ("TMUX", "/t")])
        ));
        // The person says outright.
        assert!(shows_icon(
            None,
            env(&[("IMECE_TAB_ICON", "on"), ("TMUX", "/t")])
        ));
        assert!(!shows_icon(
            Some(&who("Konsole 24.08.3")),
            env(&[("IMECE_TAB_ICON", "off")])
        ));
        assert!(shows_icon(
            Some(&who("Konsole 24.08.3")),
            env(&[("IMECE_TAB_ICON", "maybe")])
        ));
    }

    #[test]
    fn the_tab_shows_the_ladder_s_top_only_on_a_change_and_only_where_it_is_shown() {
        let mut tab = Tab::new(true);
        let mut out = Vec::new();
        for (p, osc) in [
            (Progress::Calm, "\x1b]9;4;0\x07"),
            (Progress::Working, "\x1b]9;4;3\x07"),
            (Progress::Waiting, "\x1b]9;4;4;100\x07"),
            (Progress::Stuck, "\x1b]9;4;2;100\x07"),
        ] {
            out.clear();
            tab.show(&mut out, p).unwrap();
            assert_eq!(String::from_utf8(out.clone()).unwrap(), osc);
            out.clear();
            tab.show(&mut out, p).unwrap();
            assert!(out.is_empty(), "{p:?} is not written twice");
        }
        tab.forget();
        tab.show(&mut out, Progress::Stuck).unwrap();
        assert_eq!(out, b"\x1b]9;4;2;100\x07", "after the terminal came back");
        let mut never = Tab::new(false);
        let mut out = Vec::new();
        never.show(&mut out, Progress::Stuck).unwrap();
        never.show(&mut out, Progress::Calm).unwrap();
        assert!(out.is_empty(), "not to a terminal not known to show it");
        assert!(Progress::Stuck > Progress::Waiting);
        assert!(Progress::Waiting > Progress::Working);
        assert!(Progress::Working > Progress::Calm);
    }

    #[test]
    fn only_terminals_known_to_show_a_tab_s_progress_are_sent_it() {
        let shows = |pairs: &[(&str, &str)]| {
            let pairs: Vec<(String, String)> = (pairs.iter())
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            shows_progress(None, |k| {
                pairs.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone())
            })
        };
        assert!(!shows(&[]), "nothing known");
        assert!(shows(&[("WT_SESSION", "x")]));
        assert!(
            shows(&[("TMUX", "/tmp/t")]),
            "tmux passes it on or drops it"
        );
        let ghostty = |v: &str| shows(&[("TERM_PROGRAM", "ghostty"), ("TERM_PROGRAM_VERSION", v)]);
        assert!(ghostty("1.2.0"));
        assert!(ghostty("1.2.0-main+abc"));
        assert!(ghostty("1.10"));
        assert!(ghostty("2"));
        assert!(!ghostty("1.1.9"));
        assert!(!ghostty("main"));
        assert!(!shows(&[("TERM_PROGRAM", "ghostty")]), "no version");
        let iterm = |v: &str| shows(&[("TERM_PROGRAM", "iTerm.app"), ("TERM_PROGRAM_VERSION", v)]);
        assert!(iterm("3.6.6"));
        assert!(iterm("3.6.6beta1"));
        assert!(iterm("3.7"));
        assert!(!iterm("3.6.5"));
        assert!(!iterm("3.5.14"));
        // iTerm2 says so over ssh too.
        assert!(shows(&[
            ("LC_TERMINAL", "iTerm2"),
            ("LC_TERMINAL_VERSION", "3.6.7")
        ]));
        assert!(!shows(&[
            ("LC_TERMINAL", "iTerm2"),
            ("LC_TERMINAL_VERSION", "3.4")
        ]));
        // The terminal at hand says first.
        assert!(!shows(&[
            ("TERM_PROGRAM", "WezTerm"),
            ("TERM_PROGRAM_VERSION", "20240203"),
            ("LC_TERMINAL", "iTerm2"),
            ("LC_TERMINAL_VERSION", "3.6.7"),
        ]));
        assert!(!shows(&[
            ("TERM_PROGRAM", "Apple_Terminal"),
            ("TERM_PROGRAM_VERSION", "455")
        ]));
        // The person says outright, over everything.
        assert!(shows(&[("IMECE_PROGRESS", "on")]));
        assert!(!shows(&[("IMECE_PROGRESS", "off"), ("TMUX", "/tmp/t")]));
        assert!(!shows(&[("IMECE_PROGRESS", "off"), ("WT_SESSION", "x")]));
        assert!(shows(&[("IMECE_PROGRESS", "maybe"), ("WT_SESSION", "x")]));
        assert!(!shows(&[("IMECE_PROGRESS", "maybe")]));
    }

    #[test]
    fn the_terminal_s_own_answer_names_it_and_beats_what_it_inherited() {
        let who = |t: &str| Who::parse(t).unwrap();
        assert_eq!(
            who("kitty(0.49.2)"),
            Who {
                name: "kitty".into(),
                version: Some([0, 49, 2])
            }
        );
        assert_eq!(who("ghostty 1.4.0").version, Some([1, 4, 0]));
        assert_eq!(who("iTerm2 3.7.6").name, "iterm2");
        assert_eq!(who(" tmux 3.8 ").version, Some([3, 8, 0]));
        assert_eq!(
            who("WezTerm"),
            Who {
                name: "wezterm".into(),
                version: None
            }
        );
        assert_eq!(Who::parse(""), None);
        assert_eq!(Who::parse(" (1.2)"), None);
        assert!(who("kitty(0.39.0)").is("kitty", [0, 39, 0]));
        assert!(!who("kitty(0.38.9)").is("kitty", [0, 39, 0]));
        assert!(!who("kitty(0.49.2)").is("ghostty", [0, 0, 0]));
        assert!(!who("kitty").is("kitty", [0, 0, 0]), "no version");
        // The answer decides; what kitty inherited from iTerm2 does not.
        let iterm_env = |k: &str| match k {
            "TERM_PROGRAM" => Some("iTerm.app".to_string()),
            "TERM_PROGRAM_VERSION" => Some("3.7.6".to_string()),
            "WT_SESSION" => Some("x".to_string()),
            _ => None,
        };
        let shows = |t: &str| shows_progress(Some(&who(t)), iterm_env);
        assert!(!shows("kitty(0.38.0)"), "inherited iTerm2 env is not it");
        assert!(shows("kitty(0.39.0)"));
        assert!(shows("ghostty 1.2.0"));
        assert!(!shows("ghostty 1.1.3"));
        assert!(shows("iTerm2 3.6.6"));
        assert!(!shows("iTerm2 3.6.5"));
        assert!(!shows("WezTerm 20240203"), "not on the list");
        assert!(shows("tmux 3.8"), "tmux by its answer");
        let none = |_: &str| None;
        assert!(in_tmux(Some(&who("tmux 3.2")), &none));
        assert!(!in_tmux(Some(&who("kitty(0.49.2)")), &none));
        assert!(!in_tmux(None, &none));
        assert!(in_tmux(None, &|k: &str| (k == "TMUX").then(|| "/t".to_string())));
        let off = |k: &str| (k == "IMECE_PROGRESS").then(|| "off".to_string());
        assert!(!shows_progress(Some(&who("ghostty 1.4.0")), off));
    }

    #[test]
    fn a_version_is_three_numbers_from_its_start() {
        assert_eq!(version_of("3.6.6"), Some([3, 6, 6]));
        assert_eq!(version_of("1.2"), Some([1, 2, 0]));
        assert_eq!(version_of("10"), Some([10, 0, 0]));
        assert_eq!(version_of("1.2.0-main+abc"), Some([1, 2, 0]));
        assert_eq!(version_of("1.x.3"), Some([1, 0, 0]), "stops at a word");
        assert_eq!(version_of("1.2.3.4"), Some([1, 2, 3]));
        assert_eq!(version_of("x1"), None);
        assert_eq!(version_of(""), None);
    }

    #[test]
    fn a_frame_is_one_synchronized_update_even_when_drawing_it_fails() {
        let mut out = Vec::new();
        let drawn = synced(&mut out, || Ok(7)).unwrap();
        assert_eq!(drawn, 7, "what the drawing returned");
        assert_eq!(out, b"\x1b[?2026h\x1b[?2026l");
        let mut out = Vec::new();
        let failed = synced(&mut out, || -> io::Result<()> {
            Err(io::Error::other("gone"))
        });
        assert_eq!(failed.unwrap_err().to_string(), "gone");
        assert_eq!(out, b"\x1b[?2026h\x1b[?2026l", "ended all the same");
        // What the drawing writes lands between the two.
        let shared = Shared::default();
        let mut inner = shared.clone();
        synced(&mut shared.clone(), || inner.write_all(b"frame")).unwrap();
        assert_eq!(written(&shared), "\x1b[?2026hframe\x1b[?2026l");
        assert_eq!(shared.0.borrow().1, 1, "flushed once the frame is whole");
    }

    #[test]
    fn the_cursor_s_look_is_written_only_when_it_changes_and_again_after_a_forget() {
        let bar = Look {
            shape: Shape::BlinkingBar,
            colour: None,
        };
        let command = Look {
            shape: Shape::SteadyBar,
            colour: Some([0x1a, 0x2b, 0xff]),
        };
        let find = Look {
            shape: Shape::BlinkingUnderline,
            colour: None,
        };
        let mut cursor = Cursor::default();
        let mut out = Vec::new();
        cursor.show(&mut out, Some(bar)).unwrap();
        assert_eq!(out, b"\x1b[5 q\x1b]112\x07", "the terminal's own colour");
        out.clear();
        cursor.show(&mut out, Some(bar)).unwrap();
        assert!(out.is_empty(), "the same look is not written twice");
        cursor.show(&mut out, None).unwrap();
        assert!(out.is_empty(), "hidden: nothing to write");
        cursor.show(&mut out, Some(bar)).unwrap();
        assert!(out.is_empty(), "a hidden cursor kept its look");
        cursor.show(&mut out, Some(command)).unwrap();
        assert_eq!(
            String::from_utf8(out.clone()).unwrap(),
            "\x1b[6 q\x1b]12;#1a2bff\x07"
        );
        out.clear();
        cursor.show(&mut out, Some(find)).unwrap();
        assert_eq!(out, b"\x1b[3 q\x1b]112\x07");
        out.clear();
        // The same shape in another colour is another look.
        let tinted = Look {
            colour: Some([1, 2, 3]),
            ..find
        };
        cursor.show(&mut out, Some(tinted)).unwrap();
        assert_eq!(out, b"\x1b[3 q\x1b]12;#010203\x07");
        out.clear();
        cursor.forget();
        cursor.show(&mut out, Some(tinted)).unwrap();
        assert_eq!(
            out, b"\x1b[3 q\x1b]12;#010203\x07",
            "after the terminal came back"
        );
    }

    #[test]
    fn the_title_is_written_only_when_it_changes_and_again_after_a_forget() {
        let mut title = Title::default();
        let mut out = Vec::new();
        title.show(&mut out, "imece").unwrap();
        assert_eq!(out, b"\x1b]2;imece\x07");
        out.clear();
        title.show(&mut out, "imece").unwrap();
        assert!(out.is_empty(), "the same title is not written twice");
        title.show(&mut out, "imece · 2 waiting").unwrap();
        assert_eq!(
            String::from_utf8(out.clone()).unwrap(),
            "\x1b]2;imece · 2 waiting\x07"
        );
        out.clear();
        title.forget();
        title.show(&mut out, "imece · 2 waiting").unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "\x1b]2;imece · 2 waiting\x07",
            "after the terminal came back"
        );
    }

    #[test]
    fn a_title_cannot_end_its_sequence_or_start_another() {
        let mut title = Title::default();
        let mut out = Vec::new();
        title.show(&mut out, "a\x07b\x1b]2;c\nd").unwrap();
        assert_eq!(
            String::from_utf8(out.clone()).unwrap(),
            "\x1b]2;ab]2;cd\x07"
        );
        out.clear();
        title.show(&mut out, "ab]2;cd").unwrap();
        assert!(out.is_empty(), "the cleaned title is what is remembered");
    }

    #[test]
    fn a_change_of_size_is_drawn_whole_once_more_when_it_has_settled() {
        let t0 = Instant::now();
        let ms = Duration::from_millis;
        let (small, big) = (Size::new(100, 30), Size::new(80, 45));
        let mut s = Sizes::default();
        assert!(!s.seen(small, t0), "the first size is no change");
        assert!(!s.repaint_due(t0 + ms(10_000)), "nor a repaint");
        assert_eq!(s.wait(t0, ms(5_000)), ms(5_000), "the wait as it was");
        assert!(!s.seen(small, t0), "the same again");
        assert!(s.seen(big, t0), "another");
        assert_eq!(s.wait(t0, ms(5_000)), SETTLE, "woken for the repaint");
        assert_eq!(s.wait(t0, ms(100)), ms(100), "a sooner wait stands");
        assert_eq!(s.wait(t0 + ms(400), ms(5_000)), Duration::ZERO, "late");
        // Still moving: the repaint goes after the last change.
        assert!(s.seen(small, t0 + ms(200)));
        assert!(!s.repaint_due(t0 + ms(249)));
        assert!(!s.repaint_due(t0 + ms(449)), "not yet");
        assert!(s.repaint_due(t0 + ms(450)), "on its moment");
        assert!(!s.repaint_due(t0 + ms(451)), "once");
        assert_eq!(s.wait(t0 + ms(451), ms(5_000)), ms(5_000));
        assert!(!s.seen(small, t0 + ms(500)));
        assert!(!s.repaint_due(t0 + ms(10_000)), "settled");
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
