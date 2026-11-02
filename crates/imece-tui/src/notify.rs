//! A word to the person who is in another window: the terminal's bell, or
//! a desktop notification, when something came to wait for them while the
//! terminal did not have the focus. Opt-in (`:notify`), remembered in the
//! store.
//!
//! The notification is sent the way the terminal shows it best ([`Style`],
//! from its XTVERSION answer): kitty's own (OSC 99: a title, urgency, a
//! sound, and a report when it is clicked, which opens what it was about);
//! Ghostty's OSC 777 (a title); inside tmux, OSC 9 through tmux's
//! passthrough (tmux drops it unless `allow-passthrough` is on); else
//! OSC 9 (iTerm2, WezTerm; a terminal that does not know it ignores it).
//!
//! What rings is what came while away and is still there [`AFTER`]
//! later: a new item in WAITING FOR YOU, an agent stuck. Each rings once
//! ([`Notices`] keeps what rang); what the person saw before leaving, or
//! what came and went within the wait, never does; back in the window,
//! what was about to ring is dropped, since it is on screen now. The
//! sequences carry only imece's own words and an agent's name, never an
//! agent's text, and no control character.

use std::io::{self, Write};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::model::AgentId;
use crate::screen::{Who, in_tmux};

/// How long something must still be waiting before it is said: a change
/// applied by the leader at once, a question answered by its own next
/// line, are not worth a word.
pub const AFTER: Duration = Duration::from_secs(5);

/// How the person is told.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Notify {
    #[default]
    Off,
    /// The terminal's bell (BEL): every terminal has a bell setting, a
    /// multiplexer passes it on.
    Bell,
    /// A desktop notification with the words (OSC 9).
    Text,
}

impl Notify {
    pub fn parse(s: &str) -> Option<Notify> {
        match s.trim().to_ascii_lowercase().as_str() {
            "off" => Some(Notify::Off),
            "bell" => Some(Notify::Bell),
            "text" => Some(Notify::Text),
            _ => None,
        }
    }

    /// The word `parse` takes for it, as the `:` menu shows it.
    pub fn name(self) -> &'static str {
        match self {
            Notify::Off => "off",
            Notify::Bell => "bell",
            Notify::Text => "text",
        }
    }

    /// The next one round: off, bell, text, off.
    pub fn next(self) -> Notify {
        match self {
            Notify::Off => Notify::Bell,
            Notify::Bell => Notify::Text,
            Notify::Text => Notify::Off,
        }
    }
}

/// What `:notify` takes.
pub const NAMES: [&str; 3] = ["off", "bell", "text"];

/// How a notification is sent, by the terminal it goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// OSC 9, the words only.
    Osc9,
    /// OSC 777 `notify`: a title and the words (Ghostty).
    Osc777,
    /// kitty's OSC 99 (0.36 on): a title, the words, an urgency, a sound,
    /// and a report back when it is clicked.
    Kitty,
    /// Inside tmux: OSC 9 wrapped for tmux's passthrough, since tmux drops
    /// an OSC it does not know; the terminal outside is not known.
    Tmux,
}

impl Style {
    /// The style for the terminal that answered `who` (none: it did not),
    /// `var` being the environment.
    pub fn of(who: Option<&Who>, var: impl Fn(&str) -> Option<String>) -> Style {
        if in_tmux(who, &var) {
            Style::Tmux
        } else if who.is_some_and(|w| w.is("kitty", [0, 36, 0])) {
            Style::Kitty
        } else if who.is_some_and(|w| w.name == "ghostty") {
            Style::Osc777
        } else {
            Style::Osc9
        }
    }
}

/// One notification: its words, its number (kitty reports it back when it
/// is clicked), and whether something is stuck or failed (louder).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    pub body: String,
    pub id: u64,
    pub loud: bool,
}

/// The words of every notice start with this; a style with a title of
/// its own leaves it out of the body.
const TITLE: &str = "imece";

/// kitty's sound names, base64 as its `s` key takes them: `error` for
/// something stuck, `question` for something waiting.
const SOUND_ERROR: &str = "ZXJyb3I=";
const SOUND_QUESTION: &str = "cXVlc3Rpb24=";

/// Says `word` to the person the way `how` says, in `style`: nothing when
/// off. Control characters are left out, so nothing can end a sequence
/// early; in OSC 777, whose fields `;` divides, a `;` becomes `,`.
pub fn ring(out: &mut impl Write, how: Notify, style: Style, word: &Word) -> io::Result<()> {
    let body: String = word.body.chars().filter(|c| !c.is_control()).collect();
    // The words without their `imece · `, under a title of their own.
    let rest = body
        .strip_prefix(TITLE)
        .and_then(|r| r.strip_prefix(" · "))
        .unwrap_or(&body);
    match (how, style) {
        (Notify::Off, _) => return Ok(()),
        (Notify::Bell, _) => out.write_all(b"\x07")?,
        (Notify::Text, Style::Osc9) => write!(out, "\x1b]9;{body}\x07")?,
        (Notify::Text, Style::Tmux) => {
            let inner = format!("\x1b]9;{body}\x07");
            write!(out, "\x1bPtmux;{}\x1b\\", inner.replace('\x1b', "\x1b\x1b"))?;
        }
        (Notify::Text, Style::Osc777) => {
            write!(
                out,
                "\x1b]777;notify;{TITLE};{}\x07",
                rest.replace(';', ",")
            )?;
        }
        (Notify::Text, Style::Kitty) => {
            let (urgency, sound) = if word.loud {
                (2, SOUND_ERROR)
            } else {
                (1, SOUND_QUESTION)
            };
            let id = word.id;
            write!(
                out,
                "\x1b]99;i=imece-{id}:d=0:u={urgency}:s={sound}:a=focus,report:p=title;{TITLE}\x1b\\"
            )?;
            write!(out, "\x1b]99;i=imece-{id}:p=body;{rest}\x1b\\")?;
        }
    }
    out.flush()
}

/// A key press as its code and modifiers; none for anything else.
fn key(e: &Event) -> Option<(KeyCode, KeyModifiers)> {
    match e {
        Event::Key(k) if k.kind == KeyEventKind::Press => Some((k.code, k.modifiers)),
        _ => None,
    }
}

/// Whether `e` is how a report starts as crossterm reads it: alt-`]`
/// (`ESC ]`).
pub fn opens_report(e: &Event) -> bool {
    key(e) == Some((KeyCode::Char(']'), KeyModifiers::ALT))
}

/// The most events a click report takes as crossterm reads it: `]`, `99;`,
/// `i=imece-` and a number, `;`, a button's number, the end.
pub const REPORT_MAX: usize = 64;

/// What kitty sends when a notification is clicked, `ESC ] 99 ; i=ID ;
/// ESC \` (or BEL at the end; a button's number after the second `;`),
/// as crossterm reads it: alt-`]`, then each character as a key, then
/// alt-`\` (or ctrl-g). When `events` start with such a report: how many
/// events it takes, and the notification's number when it is one of
/// imece's (`i=imece-N`, clicked as a whole, not a button). None when they
/// do not start with one, or it does not end within them: they are keys.
pub fn report(events: &[Event]) -> Option<(usize, Option<u64>)> {
    if !opens_report(events.first()?) {
        return None;
    }
    let mut text = String::new();
    for (n, e) in events.iter().enumerate().take(REPORT_MAX).skip(1) {
        match key(e)? {
            (KeyCode::Char('\\'), KeyModifiers::ALT)
            | (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
                let rest = text.strip_prefix("99;")?;
                let ours = rest
                    .strip_suffix(';')
                    .and_then(|m| m.strip_prefix("i=imece-"))
                    .and_then(|id| id.parse().ok());
                return Some((n + 1, ours));
            }
            // Two checks, not one of a union: the flags are distinct bits,
            // so `|` and `^` would make the same mask (a mutant no test
            // could catch).
            (KeyCode::Char(c), m)
                if !m.contains(KeyModifiers::ALT) && !m.contains(KeyModifiers::CONTROL) =>
            {
                text.push(c);
            }
            _ => return None,
        }
    }
    None
}

/// One thing that can ring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Note {
    /// The inbox item at this index.
    Item(usize),
    /// This agent is blocked, or its session stopped.
    Stuck(AgentId),
}

/// Which notes the person knows of (seen in the window, or rung) and
/// which wait for their [`AFTER`].
#[derive(Debug, Default)]
pub struct Notices {
    /// Seen while away, and when: ring when still there after [`AFTER`].
    pending: Vec<(Note, Instant)>,
    /// On screen while the person was in the window, or rung already.
    known: Vec<Note>,
}

impl Notices {
    /// `waiting`: everything that waits for the person right now;
    /// `away`: the terminal has lost the focus. The notes due now, each
    /// once, in `waiting`'s order; none when the person is in the window.
    pub fn due(&mut self, waiting: &[Note], away: bool, now: Instant) -> Vec<Note> {
        // Gone from the list: closed, or no longer stuck; a note that
        // comes back is new again.
        self.known.retain(|n| waiting.contains(n));
        self.pending.retain(|(n, _)| waiting.contains(n));
        if !away {
            // On screen now: seen, not worth a word.
            self.pending.clear();
            self.known = waiting.to_vec();
            return Vec::new();
        }
        for n in waiting {
            let known = self.known.contains(n) || self.pending.iter().any(|(p, _)| p == n);
            if !known {
                self.pending.push((*n, now));
            }
        }
        let mut due = Vec::new();
        self.pending.retain(|(n, since)| {
            if now.duration_since(*since) >= AFTER {
                due.push(*n);
                false
            } else {
                true
            }
        });
        // In `waiting`'s order, whatever order they were seen in.
        due.sort_by_key(|n| waiting.iter().position(|w| w == n));
        self.known.extend(due.iter().copied());
        due
    }

    /// Nothing waits to ring and nothing is known: nothing came while
    /// away, or everything went.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty() && self.known.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ways_parse_and_name_themselves_and_go_round() {
        for (word, how) in [
            ("off", Notify::Off),
            ("bell", Notify::Bell),
            ("text", Notify::Text),
        ] {
            assert_eq!(Notify::parse(word), Some(how));
            assert_eq!(
                Notify::parse(&format!(" {} ", word.to_uppercase())),
                Some(how)
            );
            assert_eq!(how.name(), word);
        }
        assert_eq!(Notify::parse("beep"), None);
        assert_eq!(Notify::parse(""), None);
        assert_eq!(Notify::default(), Notify::Off);
        assert_eq!(Notify::Off.next(), Notify::Bell);
        assert_eq!(Notify::Bell.next(), Notify::Text);
        assert_eq!(Notify::Text.next(), Notify::Off);
        assert_eq!(
            NAMES.map(|n| Notify::parse(n).unwrap()).to_vec(),
            [Notify::Off, Notify::Bell, Notify::Text]
        );
    }

    fn word(body: &str, id: u64, loud: bool) -> Word {
        Word {
            body: body.into(),
            id,
            loud,
        }
    }

    fn rung(how: Notify, style: Style, w: &Word) -> String {
        let mut out = Vec::new();
        ring(&mut out, how, style, w).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn a_ring_is_a_bell_or_a_notification_with_the_words_and_nothing_when_off() {
        let w = word("imece · a change waits", 3, false);
        for style in [Style::Osc9, Style::Osc777, Style::Kitty, Style::Tmux] {
            assert_eq!(rung(Notify::Off, style, &w), "", "{style:?}");
            assert_eq!(rung(Notify::Bell, style, &w), "\x07", "{style:?}");
        }
        assert_eq!(
            rung(Notify::Text, Style::Osc9, &w),
            "\x1b]9;imece · a change waits\x07"
        );
        // No control character gets into the sequence.
        let bad = word("a\x07b\x1b]2;c\nd", 1, false);
        assert_eq!(rung(Notify::Text, Style::Osc9, &bad), "\x1b]9;ab]2;cd\x07");
        assert_eq!(
            rung(Notify::Text, Style::Kitty, &bad),
            "\x1b]99;i=imece-1:d=0:u=1:s=cXVlc3Rpb24=:a=focus,report:p=title;imece\x1b\\\
             \x1b]99;i=imece-1:p=body;ab]2;cd\x1b\\"
        );
    }

    #[test]
    fn each_terminal_gets_the_notification_it_shows_best() {
        let quiet = word("imece · claude-1 asks you", 7, false);
        let loud = word("imece · claude-1 is stuck", 8, true);
        // Ghostty: a title, the words under it, no `;` to split them.
        assert_eq!(
            rung(Notify::Text, Style::Osc777, &quiet),
            "\x1b]777;notify;imece;claude-1 asks you\x07"
        );
        assert_eq!(
            rung(Notify::Text, Style::Osc777, &word("imece · a; b", 1, false)),
            "\x1b]777;notify;imece;a, b\x07"
        );
        // kitty: numbered, urgency and sound by what it is, a report back
        // when clicked; the title once, the words under it.
        assert_eq!(
            rung(Notify::Text, Style::Kitty, &quiet),
            "\x1b]99;i=imece-7:d=0:u=1:s=cXVlc3Rpb24=:a=focus,report:p=title;imece\x1b\\\
             \x1b]99;i=imece-7:p=body;claude-1 asks you\x1b\\"
        );
        assert_eq!(
            rung(Notify::Text, Style::Kitty, &loud),
            "\x1b]99;i=imece-8:d=0:u=2:s=ZXJyb3I=:a=focus,report:p=title;imece\x1b\\\
             \x1b]99;i=imece-8:p=body;claude-1 is stuck\x1b\\"
        );
        // Words without the title in front are kept whole.
        assert_eq!(
            rung(Notify::Text, Style::Osc777, &word("other words", 1, false)),
            "\x1b]777;notify;imece;other words\x07"
        );
        assert_eq!(
            rung(Notify::Text, Style::Osc777, &word("imece·x", 1, false)),
            "\x1b]777;notify;imece;imece·x\x07"
        );
        // tmux: OSC 9 inside its passthrough, every ESC doubled.
        assert_eq!(
            rung(Notify::Text, Style::Tmux, &quiet),
            "\x1bPtmux;\x1b\x1b]9;imece · claude-1 asks you\x07\x1b\\"
        );
    }

    #[test]
    fn the_style_follows_the_terminal_s_own_answer() {
        let who = |t: &str| Who::parse(t);
        let none = |_: &str| None;
        let style = |t: &str| Style::of(who(t).as_ref(), none);
        assert_eq!(style("kitty(0.49.2)"), Style::Kitty);
        assert_eq!(style("kitty(0.36.0)"), Style::Kitty);
        assert_eq!(style("kitty(0.35.2)"), Style::Osc9, "no sound before 0.36");
        assert_eq!(style("ghostty 1.4.0"), Style::Osc777);
        assert_eq!(style("iTerm2 3.7.6"), Style::Osc9);
        assert_eq!(style("WezTerm 20240203"), Style::Osc9);
        assert_eq!(style("tmux 3.8"), Style::Tmux);
        assert_eq!(Style::of(None, none), Style::Osc9, "no answer");
        let tmux = |k: &str| (k == "TMUX").then(|| "/t".to_string());
        assert_eq!(Style::of(who("kitty(0.49.2)").as_ref(), tmux), Style::Tmux);
    }

    /// Bytes as crossterm reads them: `ESC x` as alt-x, BEL as ctrl-g,
    /// each other character as itself.
    fn read_as_keys(bytes: &str) -> Vec<Event> {
        use ratatui::crossterm::event::KeyEvent;
        let mut out = Vec::new();
        let mut chars = bytes.chars();
        while let Some(c) = chars.next() {
            let k = match c {
                '\x1b' => KeyEvent::new(KeyCode::Char(chars.next().unwrap()), KeyModifiers::ALT),
                '\x07' => KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
                c if c.is_ascii_uppercase() => KeyEvent::new(KeyCode::Char(c), KeyModifiers::SHIFT),
                c => KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE),
            };
            out.push(Event::Key(k));
        }
        out
    }

    #[test]
    fn a_click_report_is_taken_whole_and_only_imece_s_own_number_acts() {
        let report_of = |b: &str| report(&read_as_keys(b));
        // kitty's report, as its documentation gives it, both ends.
        assert_eq!(report_of("\x1b]99;i=imece-7;\x1b\\"), Some((15, Some(7))));
        assert_eq!(report_of("\x1b]99;i=imece-12;\x07"), Some((16, Some(12))));
        // What follows is not taken.
        assert_eq!(report_of("\x1b]99;i=imece-7;\x1b\\jk"), Some((15, Some(7))));
        // Not imece's, a button, or more than an id: taken, nothing done.
        assert_eq!(report_of("\x1b]99;i=0;\x1b\\"), Some((9, None)));
        assert_eq!(report_of("\x1b]99;i=imece-7;1\x1b\\"), Some((16, None)));
        assert_eq!(
            report_of("\x1b]99;i=imece-7:p=close;untracked\x1b\\").map(|r| r.1),
            Some(None)
        );
        assert_eq!(
            report_of("\x1b]99;i=imece-x;\x1b\\").map(|r| r.1),
            Some(None)
        );
        assert_eq!(
            report_of("\x1b]99;i=imece-7;;\x1b\\").map(|r| r.1),
            Some(None)
        );
        assert_eq!(
            report_of("\x1b]99;I=IMECE-7;\x1b\\").map(|r| r.1),
            Some(None)
        );
        // Keys, not a report: another start, another code, not ended,
        // a key with ctrl or alt inside, or nothing at all.
        assert_eq!(report_of("]99;i=imece-7;\x1b\\"), None);
        assert_eq!(report_of("\x1b[99;i=imece-7;\x1b\\"), None);
        assert_eq!(report_of("\x1b]9;i=imece-7;\x1b\\"), None);
        assert_eq!(report_of("\x1b]98;i=1;\x1b\\"), None);
        assert_eq!(report_of("\x1b]99;i=imece-7;"), None);
        assert_eq!(report_of("\x1b]99;i=\x1bximece-7;\x1b\\"), None);
        let mut events = read_as_keys("\x1b]99;i=imece-7;\x1b\\");
        events[5] = Event::Key(ratatui::crossterm::event::KeyEvent::new(
            KeyCode::Char('a'),
            KeyModifiers::CONTROL,
        ));
        assert_eq!(report(&events), None, "ctrl-a inside");
        events[5] = Event::Key(ratatui::crossterm::event::KeyEvent::from(KeyCode::Enter));
        assert_eq!(report(&events), None, "no character");
        assert_eq!(report(&[]), None);
        // An event that is no key ends it as keys.
        let mut events = read_as_keys("\x1b]99;i=imece-7;\x1b\\");
        events.insert(3, Event::FocusGained);
        assert_eq!(report(&events), None);
        // A key let go is no key pressed.
        let mut events = read_as_keys("\x1b]99;i=imece-7;\x1b\\");
        if let Event::Key(k) = &mut events[0] {
            k.kind = KeyEventKind::Release;
        }
        assert!(!opens_report(&events[0]));
        assert_eq!(report(&events), None);
        // Longer than a report can be: keys.
        let long = format!("\x1b]99;i={};\x1b\\", "a".repeat(REPORT_MAX));
        assert_eq!(report_of(&long), None);
        let fits = format!("\x1b]99;i={};\x1b\\", "a".repeat(REPORT_MAX - 8));
        assert_eq!(report_of(&fits), Some((REPORT_MAX, None)), "the longest");
        let over = format!("\x1b]99;i={};\x1b\\", "a".repeat(REPORT_MAX - 7));
        assert_eq!(report_of(&over), None, "one more");
        assert!(opens_report(&read_as_keys("\x1b]")[0]));
        assert!(!opens_report(&read_as_keys("]")[0]));
        assert!(!opens_report(&Event::FocusLost));
    }

    #[test]
    fn a_note_rings_once_after_the_wait_while_away_and_only_then() {
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let mut n = Notices::default();
        let item = Note::Item(0);
        // In the window: nothing, and nothing came while away yet.
        assert!(n.due(&[], false, at(0)).is_empty());
        assert!(n.is_empty());
        // Away: seen, not yet due.
        assert!(n.due(&[item], true, at(0)).is_empty());
        assert!(n.due(&[item], true, at(4)).is_empty(), "under the wait");
        assert_eq!(n.due(&[item], true, at(5)), vec![item], "the wait is over");
        assert!(n.due(&[item], true, at(60)).is_empty(), "once");
        // Still there when the person is back and leaves again: rang.
        assert!(n.due(&[item], false, at(70)).is_empty());
        assert!(n.due(&[item], true, at(100)).is_empty());
        // Gone and back: new again.
        assert!(n.due(&[], true, at(101)).is_empty());
        assert!(n.is_empty());
        assert!(n.due(&[item], true, at(102)).is_empty());
        assert_eq!(n.due(&[item], true, at(107)), vec![item]);
    }

    #[test]
    fn what_goes_within_the_wait_or_before_the_person_leaves_never_rings() {
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let mut n = Notices::default();
        let (a, b) = (Note::Item(1), Note::Stuck(2));
        // Seen while in the window, then away: on screen before leaving,
        // never rung.
        assert!(n.due(&[a], false, at(0)).is_empty());
        assert!(!n.is_empty(), "known");
        assert!(n.due(&[a], true, at(1)).is_empty());
        assert!(n.due(&[a], true, at(60)).is_empty());
        // `b` comes while away and goes within the wait: nothing; back
        // again, its wait starts over.
        assert!(n.due(&[a, b], true, at(10)).is_empty());
        assert!(n.due(&[a], true, at(12)).is_empty());
        assert!(
            n.due(&[a, b], true, at(13)).is_empty(),
            "new again, not due"
        );
        assert!(
            n.due(&[a, b], true, at(17)).is_empty(),
            "the wait starts over"
        );
        assert_eq!(n.due(&[a, b], true, at(18)), vec![b]);
        // `c` comes while away, and the person is back before its wait is
        // over: on screen, dropped, never rung.
        let c = Note::Item(4);
        assert!(n.due(&[a, b, c], true, at(30)).is_empty());
        assert!(n.due(&[a, b, c], false, at(31)).is_empty());
        assert!(n.due(&[a, b, c], true, at(32)).is_empty());
        assert!(n.due(&[a, b, c], true, at(40)).is_empty());
    }

    #[test]
    fn several_due_at_once_come_in_the_waiting_order() {
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let mut n = Notices::default();
        let (a, b, c) = (Note::Item(3), Note::Stuck(0), Note::Item(1));
        assert!(n.due(&[b], true, at(0)).is_empty());
        assert!(n.due(&[a, b, c], true, at(2)).is_empty());
        // `b` is due first, but all three are said in the list's order.
        assert_eq!(n.due(&[a, b, c], true, at(7)), vec![a, b, c]);
        assert!(n.due(&[a, b, c], true, at(8)).is_empty());
    }
}
