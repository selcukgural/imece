//! What the first-run window (`:welcome`) knows of the coding agents on
//! this machine, and which versions' "what's new" (`:news`) a start
//! shows. The words are in `i18n.rs`; the asking is `live::look`'s.

use imece_agent::Login;

use crate::model::Vendor;

/// This build's version, as Cargo.toml has it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Versions a "what's new" window shows, at most.
pub const NEWS_SHOWN: usize = 3;

/// What is known of one vendor's CLI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Seen {
    /// Asked; no answer yet.
    Checking,
    /// Not on PATH, or its `--version` failed.
    Missing,
    /// There: its version line (as it said it, not cleaned yet) and
    /// whether it is signed in.
    Here { version: String, login: Login },
}

/// The answers of the vendors asked, one by one, as they come.
pub type Answers = std::sync::mpsc::Receiver<(Vendor, Seen)>;

/// How the vendors named are asked (`live::look` live, a fake in tests).
pub type Look = fn(&[Vendor]) -> Answers;

/// The vendors the window lists, in this order.
pub const LISTED: [Vendor; 3] = [Vendor::Claude, Vendor::Codex, Vendor::Gemini];

/// What the person runs to sign vendor `v`'s CLI in.
pub fn sign_in(v: Vendor) -> &'static str {
    match v {
        Vendor::Codex => "codex login",
        Vendor::Gemini => "gemini",
        _ => "claude auth login",
    }
}

/// A version `MAJOR.MINOR.PATCH`, compared as numbers; anything else
/// (a pre-release suffix too) is none.
pub fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let mut parts = s.trim().split('.').map(|p| p.parse::<u32>().ok());
    let v = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(v)
}

/// The versions of `listed` (newest first, as `i18n` keeps them) whose
/// news a start of version `current` shows, newest first, at most
/// [`NEWS_SHOWN`]: none on the first start ever (`first`) or when
/// `current` was seen already; those after the version `seen` up to
/// `current`; only `current`'s when no version was kept (a run from
/// before the news were kept) or the kept one is not a version.
pub fn news_since<'a>(
    seen: Option<&str>,
    first: bool,
    current: &str,
    listed: &[&'a str],
) -> Vec<&'a str> {
    let Some(now) = parse_version(current) else {
        return Vec::new();
    };
    if first {
        return Vec::new();
    }
    let after = seen.and_then(parse_version);
    listed
        .iter()
        .copied()
        .filter(|v| {
            parse_version(v).is_some_and(|v| match after {
                Some(after) => after < v && v <= now,
                None => v == now,
            })
        })
        .take(NEWS_SHOWN)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_is_three_numbers() {
        assert_eq!(parse_version("0.1.0"), Some((0, 1, 0)));
        assert_eq!(parse_version(" 12.3.45\n"), Some((12, 3, 45)));
        assert_eq!(parse_version("0.1"), None);
        assert_eq!(parse_version("0.1.0.1"), None);
        assert_eq!(parse_version("0.1.0-rc1"), None);
        assert_eq!(parse_version("v0.1.0"), None);
        assert_eq!(parse_version(""), None);
        assert!(parse_version(VERSION).is_some(), "Cargo.toml's version");
    }

    #[test]
    fn the_news_shown_are_the_versions_since_the_one_seen() {
        let listed = ["0.10.0", "0.4.0", "0.3.1", "0.3.0", "0.2.0", "0.1.0"];
        let news = |seen, first, current| news_since(seen, first, current, &listed);
        // The first start ever: the welcome says it all.
        assert!(news(None, true, "0.4.0").is_empty());
        assert!(news(Some("0.1.0"), true, "0.4.0").is_empty());
        // Seen already.
        assert!(news(Some("0.4.0"), false, "0.4.0").is_empty());
        // After the one seen, up to this one, newest first; numbers, not
        // letters (0.10 is after 0.4).
        assert_eq!(news(Some("0.3.0"), false, "0.4.0"), ["0.4.0", "0.3.1"]);
        assert_eq!(news(Some("0.2.0"), false, "0.3.0"), ["0.3.0"]);
        assert_eq!(news(Some("0.4.0"), false, "0.10.0"), ["0.10.0"]);
        // At most three.
        assert_eq!(
            news(Some("0.1.0"), false, "0.10.0"),
            ["0.10.0", "0.4.0", "0.3.1"]
        );
        // Nothing kept, or nonsense: this version's alone.
        assert_eq!(news(None, false, "0.3.1"), ["0.3.1"]);
        assert_eq!(news(Some("x"), false, "0.3.1"), ["0.3.1"]);
        // An older build than the one seen tells nothing.
        assert!(news(Some("0.4.0"), false, "0.3.0").is_empty());
        // A version without news, or no version at all.
        assert!(news(None, false, "0.5.0").is_empty());
        assert!(news(Some("0.1.0"), false, "dev").is_empty());
    }

    #[test]
    fn this_version_has_news_in_both_languages_newest_first() {
        use crate::i18n::{EN, TR};
        let versions = |news: &[(&'static str, &[&str])]| -> Vec<&'static str> {
            news.iter().map(|n| n.0).collect()
        };
        assert_eq!(versions(EN.news), versions(TR.news), "the same versions");
        assert_eq!(
            EN.news[0].0, VERSION,
            "Cargo.toml's version has no news yet"
        );
        for (en, tr) in EN.news.iter().zip(TR.news) {
            assert!(!en.1.is_empty() && en.1.len() == tr.1.len(), "{}", en.0);
        }
        let parsed: Vec<_> = EN
            .news
            .iter()
            .map(|n| parse_version(n.0).unwrap())
            .collect();
        assert!(parsed.windows(2).all(|p| p[0] > p[1]), "newest first");
    }

    #[test]
    fn every_vendor_listed_has_its_own_way_to_sign_in() {
        assert_eq!(sign_in(Vendor::Claude), "claude auth login");
        assert_eq!(sign_in(Vendor::Codex), "codex login");
        assert_eq!(sign_in(Vendor::Gemini), "gemini");
    }
}
