//! How long the person is away before "while you were away" opens by
//! itself (`:away 30m|off`, record 52): back in the window after that long
//! in another one, or the first key after that long without one. Shorter
//! absences show nothing; `:digest` opens it by hand.

use std::time::Duration;

/// The longest time `:away` takes, in minutes: a day.
pub const MOST: u16 = 24 * 60;

/// What a bare `:away` goes round, in minutes, then off.
pub const ROUND: [u16; 3] = [15, 30, 60];

/// What `:away` offers on its menu.
pub const NAMES: [&str; 4] = ["15m", "30m", "1h", "off"];

/// After how many minutes away the digest opens by itself; none: never.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Away(Option<u16>);

impl Default for Away {
    /// Half an hour: three minutes brought it on almost every return.
    fn default() -> Away {
        Away(Some(30))
    }
}

impl Away {
    /// `off`, minutes (`30`, `30m`) or hours (`1h`), from one minute to a
    /// day.
    pub fn parse(s: &str) -> Option<Away> {
        let s = s.trim().to_ascii_lowercase();
        if s == "off" {
            return Some(Away(None));
        }
        let (number, per) = match s.strip_suffix('h') {
            Some(n) => (n, 60),
            None => (s.strip_suffix('m').unwrap_or(&s), 1),
        };
        let n: u16 = number.parse().ok()?;
        let minutes = n.checked_mul(per)?;
        (1..=MOST).contains(&minutes).then_some(Away(Some(minutes)))
    }

    /// The word `parse` takes for it: whole hours in hours.
    pub fn name(self) -> String {
        match self.0 {
            None => "off".into(),
            Some(m) if m % 60 == 0 => format!("{}h", m / 60),
            Some(m) => format!("{m}m"),
        }
    }

    /// The next one round: 15 min, 30 min, 1 h, off; a time of the
    /// person's own goes to the next longer one.
    pub fn next(self) -> Away {
        match self.0 {
            None => Away(Some(ROUND[0])),
            Some(m) => Away(ROUND.into_iter().find(|&r| r > m)),
        }
    }

    /// How long away opens the digest; none when it never does.
    pub fn time(self) -> Option<Duration> {
        self.0.map(|m| Duration::from_secs(u64::from(m) * 60))
    }

    /// Whether `gone` is long enough.
    pub fn over(self, gone: Duration) -> bool {
        self.time().is_some_and(|t| gone >= t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_hours_and_off_are_read_from_one_minute_to_a_day() {
        let m = |s| Away::parse(s).and_then(|a| a.0);
        assert_eq!(Away::parse("off"), Some(Away(None)));
        assert_eq!(Away::parse(" OFF "), Some(Away(None)));
        assert_eq!(m("30"), Some(30));
        assert_eq!(m("45m"), Some(45));
        assert_eq!(m("2h"), Some(120));
        assert_eq!(m("2H"), Some(120));
        assert_eq!(m("1"), Some(1));
        assert_eq!(m("1440"), Some(1440));
        assert_eq!(m("24h"), Some(1440));
        for bad in [
            "0", "0h", "1441", "25h", "", "m", "h", "x", "-5", "3s", "1.5h", "9999h",
        ] {
            assert_eq!(Away::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn its_name_is_read_back_to_the_same_time() {
        for (minutes, name) in [
            (None, "off"),
            (Some(30), "30m"),
            (Some(60), "1h"),
            (Some(90), "90m"),
            (Some(120), "2h"),
        ] {
            assert_eq!(Away(minutes).name(), name);
            assert_eq!(Away::parse(name), Some(Away(minutes)));
        }
        for name in NAMES {
            assert_eq!(Away::parse(name).unwrap().name(), name);
        }
    }

    #[test]
    fn a_bare_away_goes_round_and_half_an_hour_is_where_it_starts() {
        assert_eq!(Away::default(), Away(Some(30)));
        assert_eq!(Away(None).next(), Away(Some(15)));
        assert_eq!(Away(Some(15)).next(), Away(Some(30)));
        assert_eq!(Away(Some(30)).next(), Away(Some(60)));
        assert_eq!(Away(Some(60)).next(), Away(None));
        // The person's own time: the next longer one.
        assert_eq!(Away(Some(20)).next(), Away(Some(30)));
        assert_eq!(Away(Some(5)).next(), Away(Some(15)));
        assert_eq!(Away(Some(90)).next(), Away(None));
    }

    #[test]
    fn it_is_over_from_the_minute_on_and_never_when_off() {
        let half = Away(Some(30));
        assert_eq!(half.time(), Some(Duration::from_secs(1800)));
        assert!(!half.over(Duration::from_secs(1799)));
        assert!(half.over(Duration::from_secs(1800)));
        assert!(half.over(Duration::from_secs(1801)));
        assert_eq!(Away(Some(2)).time(), Some(Duration::from_secs(120)));
        assert_eq!(Away(None).time(), None);
        assert!(!Away(None).over(Duration::from_secs(u64::MAX / 2)));
    }
}
