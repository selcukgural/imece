//! A word to the person who is in another window: the terminal's bell, or
//! a desktop notification (OSC 9: iTerm2, WezTerm, Ghostty; a terminal
//! that does not know it ignores it), when something came to wait for
//! them while the terminal did not have the focus. Opt-in (`:notify`),
//! remembered in the store.
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

use crate::model::AgentId;

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

/// Says `body` to the person the way `how` says: nothing when off.
/// Control characters are left out, so nothing can end the sequence
/// early.
pub fn ring(out: &mut impl Write, how: Notify, body: &str) -> io::Result<()> {
    match how {
        Notify::Off => return Ok(()),
        Notify::Bell => out.write_all(b"\x07")?,
        Notify::Text => {
            let body: String = body.chars().filter(|c| !c.is_control()).collect();
            write!(out, "\x1b]9;{body}\x07")?;
        }
    }
    out.flush()
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

    #[test]
    fn a_ring_is_a_bell_or_a_notification_with_the_words_and_nothing_when_off() {
        let mut out = Vec::new();
        ring(&mut out, Notify::Off, "x").unwrap();
        assert!(out.is_empty());
        ring(&mut out, Notify::Bell, "x").unwrap();
        assert_eq!(out, b"\x07");
        let mut out = Vec::new();
        ring(&mut out, Notify::Text, "imece · a change waits").unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "\x1b]9;imece · a change waits\x07"
        );
        // No control character gets into the sequence.
        let mut out = Vec::new();
        ring(&mut out, Notify::Text, "a\x07b\x1b]2;c\nd").unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "\x1b]9;ab]2;cd\x07");
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
