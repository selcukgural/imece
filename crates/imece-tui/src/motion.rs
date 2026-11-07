//! Motion on the screen (`:motion off|calm|full`, record 84), within the
//! rule of record 77: drawn on a change, never on a timer. A frame of
//! something that moves is a change like the clock's second: the screen
//! is drawn when what it shows moves to its next frame
//! (`App::next_motion`), and only while the terminal has the focus and
//! something on it moves. In another window everything stands still.
//!
//! Calm (the default): a working row's glyph breathes (its colour, two
//! tones, never its shape: record 61), `starting` counts its dots, the
//! bottom line spins while imece waits for an answer, a row that came to
//! wait for the person blinks three times, a row whose work is done or
//! whose session started settles. Full adds a spinner by the header's
//! fleet while anyone works, and spins faster. Over SSH full is calm:
//! every frame crosses the network.

use std::time::Duration;

/// How much moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Motion {
    /// Nothing: the screen as it was before motion.
    Off,
    #[default]
    Calm,
    Full,
}

impl Motion {
    pub fn parse(s: &str) -> Option<Motion> {
        match s.trim().to_ascii_lowercase().as_str() {
            "off" => Some(Motion::Off),
            "calm" => Some(Motion::Calm),
            "full" => Some(Motion::Full),
            _ => None,
        }
    }

    /// The word `parse` takes for it, as the `:` menu shows it.
    pub fn name(self) -> &'static str {
        match self {
            Motion::Off => "off",
            Motion::Calm => "calm",
            Motion::Full => "full",
        }
    }

    /// The next one round: off, calm, full, off.
    pub fn next(self) -> Motion {
        match self {
            Motion::Off => Motion::Calm,
            Motion::Calm => Motion::Full,
            Motion::Full => Motion::Off,
        }
    }

    /// What runs of it: over SSH (`ssh`) full is calm.
    pub fn under(self, ssh: bool) -> Motion {
        match self {
            Motion::Full if ssh => Motion::Calm,
            m => m,
        }
    }

    /// How often the spinner on the bottom line turns.
    pub fn spin(self) -> Option<Duration> {
        match self {
            Motion::Off => None,
            Motion::Calm => Some(SPIN_CALM),
            Motion::Full => Some(SPIN_FULL),
        }
    }
}

/// What `:motion` takes.
pub const NAMES: [&str; 3] = ["off", "calm", "full"];

/// Whether imece runs over SSH, by the variables sshd sets.
pub fn over_ssh(var: impl Fn(&str) -> Option<String>) -> bool {
    ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
        .iter()
        .any(|k| var(k).is_some_and(|v| !v.is_empty()))
}

/// The beat every calm frame falls on, counted from the screen's start:
/// what moves at once changes in the same frame, so a few things moving
/// cost no more frames than one.
pub const BEAT: Duration = Duration::from_millis(300);
/// A working row's glyph: one tone, then the other, each this long.
pub const BREATH: Duration = Duration::from_millis(600);
/// How late a frame of what moves may come so that it falls on the
/// tick's wake and drawing instead of one of its own: a third of the
/// beat. Live ticks every 100 ms, so there what moves never wakes the
/// screen by itself.
pub const LATE: Duration = Duration::from_millis(100);
/// `starting` and its dots: each count this long, with the breath.
pub const DOTS: Duration = BREATH;
/// The dots after `starting`, as many columns each.
pub const DOT_FRAMES: [&str; 4] = ["   ", ".  ", ".. ", "..."];
/// The spinner's frames.
pub const SPIN: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
/// A spinner frame, calm: on the beat.
pub const SPIN_CALM: Duration = BEAT;
/// A spinner frame, full: ten a second, three to the beat.
pub const SPIN_FULL: Duration = Duration::from_millis(100);
/// A row that came to wait: reversed, then not, each a beat…
pub const BLINK: Duration = BEAT;
/// … three times.
pub const ARRIVE: Duration = Duration::from_millis(1800);
/// A row that settles: three steps from bright to its own colours, each
/// a beat.
pub const SETTLE_STEP: Duration = BEAT;
pub const SETTLE: Duration = Duration::from_millis(900);

/// The first beat at or after `since`: a glow starts there, so its
/// frames fall on the beat with the rest.
pub fn on_beat(since: Duration) -> Duration {
    match since.as_nanos() % BEAT.as_nanos() {
        0 => since,
        _ => since + next_frame(since, BEAT),
    }
}

/// The frame shown `since` the start of something that moves to its next
/// frame every `period`, `frames` frames round.
pub fn frame(since: Duration, period: Duration, frames: usize) -> usize {
    (since.as_nanos() / period.as_nanos().max(1)) as usize % frames.max(1)
}

/// How long after `since` the next frame of something that moves every
/// `period` starts.
pub fn next_frame(since: Duration, period: Duration) -> Duration {
    let period = period.as_nanos().max(1);
    let left = period - since.as_nanos() % period;
    Duration::from_nanos(u64::try_from(left).unwrap_or(u64::MAX))
}

/// A short light on a row, then nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glow {
    /// Something came to wait for the person: three blinks.
    Arrive,
    /// A turn done, a session started: bright, then calmer, then its own.
    Settle,
}

/// How a row lights at a moment of its glow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lit {
    Reversed,
    /// The settling step, 0 the brightest.
    Settle(u8),
}

impl Glow {
    pub fn lasts(self) -> Duration {
        match self {
            Glow::Arrive => ARRIVE,
            Glow::Settle => SETTLE,
        }
    }

    /// Its frames change this often.
    pub fn period(self) -> Duration {
        match self {
            Glow::Arrive => BLINK,
            Glow::Settle => SETTLE_STEP,
        }
    }

    /// How the row lights `age` into the glow; none between blinks and
    /// once it is over.
    pub fn lit(self, age: Duration) -> Option<Lit> {
        if age >= self.lasts() {
            return None;
        }
        let at = frame(age, self.period(), usize::MAX);
        match self {
            Glow::Arrive => at.is_multiple_of(2).then_some(Lit::Reversed),
            Glow::Settle => Some(Lit::Settle(u8::try_from(at).unwrap_or(u8::MAX))),
        }
    }

    /// How long after `age` the row next looks different: none once it
    /// is over (the frame at its end, back to the row's own look, is the
    /// last).
    pub fn next(self, age: Duration) -> Option<Duration> {
        (age < self.lasts()).then(|| next_frame(age, self.period()))
    }
}

/// What a glow is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlowOn {
    Agent(crate::model::AgentId),
    /// An item of TO DECIDE, by its index in the inbox.
    Item(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn off_calm_and_full_by_word_round_and_over_ssh() {
        for word in NAMES {
            let m = Motion::parse(&format!(" {} ", word.to_uppercase())).unwrap();
            assert_eq!(m.name(), word);
        }
        assert_eq!(
            NAMES.map(|n| Motion::parse(n).unwrap()),
            [Motion::Off, Motion::Calm, Motion::Full]
        );
        assert_eq!(Motion::parse("still"), None);
        assert_eq!(Motion::parse(""), None);
        assert_eq!(Motion::default(), Motion::Calm);
        assert_eq!(Motion::Off.next(), Motion::Calm);
        assert_eq!(Motion::Calm.next(), Motion::Full);
        assert_eq!(Motion::Full.next(), Motion::Off);
        assert_eq!(Motion::Full.under(true), Motion::Calm);
        assert_eq!(Motion::Full.under(false), Motion::Full);
        assert_eq!(Motion::Calm.under(true), Motion::Calm);
        assert_eq!(Motion::Off.under(true), Motion::Off);
        assert_eq!(Motion::Off.spin(), None);
        assert_eq!(Motion::Calm.spin(), Some(ms(300)));
        assert_eq!(Motion::Full.spin(), Some(ms(100)));
    }

    #[test]
    fn ssh_is_known_by_any_of_its_variables_set() {
        let with =
            |k: &'static str, v: &'static str| move |q: &str| (q == k).then(|| v.to_string());
        assert!(over_ssh(with("SSH_CONNECTION", "10.0.0.1 5 10.0.0.2 22")));
        assert!(over_ssh(with("SSH_CLIENT", "10.0.0.1 5 22")));
        assert!(over_ssh(with("SSH_TTY", "/dev/ttys001")));
        assert!(!over_ssh(with("SSH_TTY", "")));
        assert!(!over_ssh(with("TERM", "xterm")));
    }

    #[test]
    fn a_frame_counts_periods_round_and_the_next_starts_at_the_boundary() {
        assert_eq!(frame(ms(0), ms(600), 2), 0);
        assert_eq!(frame(ms(599), ms(600), 2), 0);
        assert_eq!(frame(ms(600), ms(600), 2), 1);
        assert_eq!(frame(ms(1250), ms(600), 2), 0);
        assert_eq!(frame(ms(1750), ms(500), 4), 3);
        assert_eq!(frame(ms(2100), ms(500), 4), 0);
        assert_eq!(frame(ms(7), ms(0), 0), 0, "never a division by nothing");
        assert_eq!(next_frame(ms(0), ms(600)), ms(600));
        assert_eq!(next_frame(ms(250), ms(600)), ms(350));
        assert_eq!(next_frame(ms(600), ms(600)), ms(600));
        assert_eq!(next_frame(ms(1300), ms(500)), ms(200));
    }

    #[test]
    fn an_arrival_blinks_three_times_and_a_settle_steps_down_three() {
        let a = Glow::Arrive;
        let shown: Vec<_> = (0..7).map(|i| a.lit(ms(i * 300 + 10))).collect();
        let r = Some(Lit::Reversed);
        assert_eq!(shown, [r, None, r, None, r, None, None]);
        assert_eq!(a.lit(ms(1499)), r, "the third blink's last moment");
        assert_eq!(a.lit(ms(1500)), None);
        assert_eq!(a.lit(ms(1799)), None);
        assert_eq!(a.lit(ms(1800)), None, "over at its end");
        let s = Glow::Settle;
        assert_eq!(s.lit(ms(0)), Some(Lit::Settle(0)));
        assert_eq!(s.lit(ms(299)), Some(Lit::Settle(0)));
        assert_eq!(s.lit(ms(300)), Some(Lit::Settle(1)));
        assert_eq!(s.lit(ms(899)), Some(Lit::Settle(2)));
        assert_eq!(s.lit(ms(900)), None);
        assert_eq!((a.lasts(), s.lasts()), (ms(1800), ms(900)));
        assert_eq!((a.period(), s.period()), (ms(300), ms(300)));
    }

    #[test]
    fn a_glow_asks_for_its_next_frame_until_its_end_and_not_after() {
        assert_eq!(Glow::Arrive.next(ms(0)), Some(ms(300)));
        assert_eq!(Glow::Arrive.next(ms(1700)), Some(ms(100)), "the end too");
        assert_eq!(Glow::Arrive.next(ms(1800)), None);
        assert_eq!(Glow::Settle.next(ms(700)), Some(ms(200)));
        assert_eq!(Glow::Settle.next(ms(899)), Some(ms(1)));
        assert_eq!(Glow::Settle.next(ms(900)), None);
    }

    #[test]
    fn everything_calm_falls_on_the_beat() {
        assert_eq!(on_beat(ms(0)), ms(0));
        assert_eq!(on_beat(ms(1)), ms(300));
        assert_eq!(on_beat(ms(299)), ms(300));
        assert_eq!(on_beat(ms(300)), ms(300));
        assert_eq!(on_beat(ms(301)), ms(600));
        assert_eq!(on_beat(ms(1000)), ms(1200));
        let beats = |d: Duration| d.as_millis().is_multiple_of(BEAT.as_millis());
        for d in [BREATH, DOTS, SPIN_CALM, BLINK, ARRIVE, SETTLE_STEP, SETTLE] {
            assert!(beats(d), "{d:?}");
        }
        assert_eq!(BEAT.as_millis() % SPIN_FULL.as_millis(), 0);
    }
}
