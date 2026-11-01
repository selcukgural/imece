//! The person's own keys: a file of `KEY = KEY` lines that makes a key
//! act as one of imece's (`^n = j`), read at start and on `:keys`.
//!
//! A line is turned once, as vim's `noremap`: `j = k` and `k = j` swap
//! the two, and a key the file does not name stays itself. The map holds
//! wherever imece reads keys except writing, the help's `/` filter and
//! the hint letters, whose keys are text. A file imece cannot take whole
//! is refused and the keys in use stay; the last good file is kept in the
//! store, so a start with a bad file still has them. See
//! `docs/decisions/76-keys.md`.

use std::path::{Path, PathBuf};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::i18n::{Lang, fill};

/// The file's name under `$XDG_CONFIG_HOME/imece`.
pub const NAME: &str = "keys";
/// A bigger file is refused unread.
pub const MAX_BYTES: u64 = 64 * 1024;
/// A file with more lines that give a key is refused.
pub const MAX_KEYS: usize = 200;
/// The longest part of a wrong name a refusal repeats.
const SAID: usize = 24;

/// The plain keys imece does something with somewhere outside writing:
/// the salon, a conversation, a change, the windows and the pickers.
const USED: &str = "abcfghijklmnpqrstvwxyzCFGKLMORSTWY123456789!#%&'+,-./:<=>?@[] ";
/// The keys with ctrl imece does something with outside writing.
const USED_CTRL: &str = "bdforu";

/// `$XDG_CONFIG_HOME/imece/keys`, or `~/.config/imece/keys`; none
/// without a home.
pub fn file() -> Option<PathBuf> {
    file_in(
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    )
}

fn file_in(config: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    config
        .filter(|p| p.is_absolute())
        .or_else(|| home.map(|h| h.join(".config")))
        .map(|d| d.join("imece").join(NAME))
}

/// The modifiers imece tells keys apart by. Joined with `union`, not
/// `|`: `^` gives the same flags, so a mutant of `|` could not be told
/// apart.
const KEPT: KeyModifiers = KeyModifiers::CONTROL
    .union(KeyModifiers::ALT)
    .union(KeyModifiers::SHIFT);

/// One key as imece tells keys apart: its code and ctrl, alt and shift.
/// A character carries its shift in itself (`G`), so its shift is not
/// kept, nor ⇧tab's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    code: KeyCode,
    mods: KeyModifiers,
}

impl Key {
    /// The key `k` is, as imece tells keys apart.
    pub fn of(k: &KeyEvent) -> Key {
        Key::new(k.code, k.modifiers.intersection(KEPT))
    }

    fn new(code: KeyCode, mut mods: KeyModifiers) -> Key {
        let code = match code {
            KeyCode::Char(c) => {
                mods.remove(KeyModifiers::SHIFT);
                // ctrl-N is the same key as ctrl-n.
                if mods.contains(KeyModifiers::CONTROL) {
                    KeyCode::Char(c.to_ascii_lowercase())
                } else {
                    KeyCode::Char(c)
                }
            }
            KeyCode::BackTab => {
                mods.remove(KeyModifiers::SHIFT);
                KeyCode::BackTab
            }
            other => other,
        };
        Key { code, mods }
    }

    /// The key a name in the file or the help means: `j`, `G`, `^n` or
    /// `ctrl-n`, `⌥b` or `alt-b`, `⇧tab` or `shift-tab`, `enter`, `esc`,
    /// `tab`, `space`, `hash` (`#`), `backspace`, `del`, `home`, `end`,
    /// `pgup`, `pgdn`, `↑ ↓ ← →` or `up down left right`, `f1`–`f12`.
    pub fn parse(name: &str) -> Option<Key> {
        let mut mods = KeyModifiers::NONE;
        let mut rest = name;
        loop {
            let (flag, len) = match rest {
                r if starts(r, "ctrl-") => (KeyModifiers::CONTROL, 5),
                r if starts(r, "alt-") => (KeyModifiers::ALT, 4),
                r if starts(r, "shift-") => (KeyModifiers::SHIFT, 6),
                r if r.starts_with('^') => (KeyModifiers::CONTROL, 1),
                r if r.starts_with('⌥') => (KeyModifiers::ALT, '⌥'.len_utf8()),
                r if r.starts_with('⇧') => (KeyModifiers::SHIFT, '⇧'.len_utf8()),
                _ => break,
            };
            // `^` and `-` alone are keys of their own.
            if rest.len() == len {
                break;
            }
            mods |= flag;
            rest = &rest[len..];
        }
        let code = base(rest)?;
        if !mods.contains(KeyModifiers::SHIFT) {
            return Some(Key::new(code, mods));
        }
        // `Key::new` drops the shift of a character and of ⇧tab.
        match code {
            // `shift-g` is `G`; a shift on any other character is a key
            // the terminal says otherwise (`shift-1` is `!` here, not
            // everywhere).
            KeyCode::Char(c) if c.is_ascii_lowercase() => {
                Some(Key::new(KeyCode::Char(c.to_ascii_uppercase()), mods))
            }
            KeyCode::Char(c) if !c.is_ascii_uppercase() => None,
            KeyCode::Tab => Some(Key::new(KeyCode::BackTab, mods)),
            _ => Some(Key::new(code, mods)),
        }
    }

    /// The key's name as the help writes it: `j`, `^n`, `⌥b`, `⇧tab`,
    /// `enter`, `↑`.
    pub fn name(self) -> String {
        let mut out = String::new();
        for (flag, mark) in [
            (KeyModifiers::CONTROL, '^'),
            (KeyModifiers::ALT, '⌥'),
            (KeyModifiers::SHIFT, '⇧'),
        ] {
            if self.mods.contains(flag) {
                out.push(mark);
            }
        }
        let base = match self.code {
            KeyCode::Char(' ') => "space".to_string(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::BackTab => "⇧tab".into(),
            KeyCode::F(n) => format!("f{n}"),
            code => NAMES
                .iter()
                .find(|(_, c)| *c == code)
                .map_or("?", |&(n, _)| n)
                .to_string(),
        };
        out.push_str(&base);
        out
    }

    /// Whether imece does something with the key outside writing.
    pub fn used(self) -> bool {
        match self.code {
            KeyCode::Char(c) if self.mods == KeyModifiers::CONTROL => USED_CTRL.contains(c),
            _ if !self.mods.is_empty() => false,
            KeyCode::Char(c) => USED.contains(c),
            code => USED_NAMED.contains(&code),
        }
    }

    /// The key that only ever quits.
    fn quits(self) -> bool {
        self == Key::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
    }
}

/// Whether `s` starts with `prefix`, whatever the case of its letters.
fn starts(s: &str, prefix: &str) -> bool {
    s.get(..prefix.len())
        .is_some_and(|p| p.eq_ignore_ascii_case(prefix))
}

/// The named keys, as the help writes them first.
const NAMES: [(&str, KeyCode); 15] = [
    ("enter", KeyCode::Enter),
    ("esc", KeyCode::Esc),
    ("tab", KeyCode::Tab),
    ("backspace", KeyCode::Backspace),
    ("del", KeyCode::Delete),
    ("home", KeyCode::Home),
    ("end", KeyCode::End),
    ("pgup", KeyCode::PageUp),
    ("pgdn", KeyCode::PageDown),
    ("↑", KeyCode::Up),
    ("↓", KeyCode::Down),
    ("←", KeyCode::Left),
    ("→", KeyCode::Right),
    ("insert", KeyCode::Insert),
    ("backtab", KeyCode::BackTab),
];

/// Other words for the named keys.
const ALSO: [(&str, KeyCode); 9] = [
    ("return", KeyCode::Enter),
    ("escape", KeyCode::Esc),
    ("delete", KeyCode::Delete),
    ("pageup", KeyCode::PageUp),
    ("pagedown", KeyCode::PageDown),
    ("up", KeyCode::Up),
    ("down", KeyCode::Down),
    ("left", KeyCode::Left),
    ("right", KeyCode::Right),
];

/// The named keys imece does something with outside writing.
const USED_NAMED: [KeyCode; 12] = [
    KeyCode::Enter,
    KeyCode::Esc,
    KeyCode::Tab,
    KeyCode::BackTab,
    KeyCode::Home,
    KeyCode::End,
    KeyCode::PageUp,
    KeyCode::PageDown,
    KeyCode::Up,
    KeyCode::Down,
    KeyCode::Left,
    KeyCode::Right,
];

/// The key a name without ctrl, alt or shift means.
fn base(name: &str) -> Option<KeyCode> {
    if let Some(&(_, code)) =
        (NAMES.iter().chain(&ALSO)).find(|(n, _)| n.eq_ignore_ascii_case(name))
    {
        return Some(code);
    }
    if name.eq_ignore_ascii_case("space") {
        return Some(KeyCode::Char(' '));
    }
    if name.eq_ignore_ascii_case("hash") {
        return Some(KeyCode::Char('#'));
    }
    if let Some(n) = name
        .strip_prefix(['f', 'F'])
        .and_then(|n| n.parse::<u8>().ok())
        .filter(|n| (1..=12).contains(n))
    {
        return Some(KeyCode::F(n));
    }
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(KeyCode::Char(c)),
        _ => None,
    }
}

/// Why a key file was not taken.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Why {
    /// The file is there but could not be read as text.
    Unreadable,
    /// Bigger than [`MAX_BYTES`].
    TooBig,
    /// A line `n` that is not `KEY = KEY`.
    Shape(usize),
    /// A name on line `n` that is no key.
    NotAKey(usize, String),
    /// A key on line `n` that imece does nothing with.
    Unused(usize, Key),
    /// A key on line `n` that line `first` gave already.
    Twice(usize, Key, usize),
    /// ctrl-c on line `n`: it only ever quits.
    Quits(usize),
    /// More than [`MAX_KEYS`] keys.
    TooMany,
}

/// The keys a file gives: each key of the person's and the key of
/// imece's it acts as, in the file's order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Keymap {
    given: Vec<(Key, Key)>,
}

impl Keymap {
    /// The keys `text` gives, or the first reason it cannot be taken.
    pub fn read(text: &str) -> Result<Keymap, Why> {
        let mut given: Vec<(Key, Key)> = Vec::new();
        let mut lines: Vec<usize> = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let n = i + 1;
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let words: Vec<&str> = line.split_whitespace().collect();
            let [from, "=", to] = words[..] else {
                return Err(Why::Shape(n));
            };
            let key = |name: &str| {
                Key::parse(name).ok_or_else(|| Why::NotAKey(n, name.chars().take(SAID).collect()))
            };
            let (from, to) = (key(from)?, key(to)?);
            if from.quits() || to.quits() {
                return Err(Why::Quits(n));
            }
            if !to.used() {
                return Err(Why::Unused(n, to));
            }
            if let Some(at) = given.iter().position(|&(f, _)| f == from) {
                return Err(Why::Twice(n, from, lines[at]));
            }
            if given.len() == MAX_KEYS {
                return Err(Why::TooMany);
            }
            given.push((from, to));
            lines.push(n);
        }
        Ok(Keymap { given })
    }

    /// `k` as the person means it: the key it acts as, or itself.
    pub fn turn(&self, k: KeyEvent) -> KeyEvent {
        let key = Key::of(&k);
        match self.given.iter().find(|&&(from, _)| from == key) {
            Some(&(_, to)) => KeyEvent {
                code: to.code,
                modifiers: to.mods,
                ..k
            },
            None => k,
        }
    }

    /// Each key of the person's and the one it acts as.
    pub fn given(&self) -> &[(Key, Key)] {
        &self.given
    }

    /// The keys that act as `key` now: the person's, in the file's
    /// order, then `key` itself unless the file gave it another meaning.
    pub fn acting_as(&self, key: Key) -> Vec<Key> {
        let mut keys: Vec<Key> = (self.given.iter())
            .filter(|&&(_, to)| to == key)
            .map(|&(from, _)| from)
            .collect();
        if !self.given.iter().any(|&(from, _)| from == key) {
            keys.push(key);
        }
        keys
    }

    /// imece's keys the file took a meaning from without giving it to
    /// another: nothing does what they did.
    pub fn lost(&self) -> Vec<Key> {
        (self.given.iter())
            .map(|&(from, _)| from)
            .filter(|&k| k.used() && self.acting_as(k).is_empty())
            .collect()
    }

    /// The keys of a help line (`↑ ↓  j k`, `gg  G`) as they are now: a
    /// key the file moved is written as the person's key(s), `–` when
    /// nothing does it now; the rest as it was.
    pub fn shown(&self, keys: &str) -> String {
        if self.given.is_empty() {
            return keys.to_string();
        }
        let words: Vec<String> = keys.split(' ').map(|w| self.shown_word(w)).collect();
        words.join(" ")
    }

    fn shown_word(&self, word: &str) -> String {
        if let Some(key) = Key::parse(word).filter(|k| k.used()) {
            let now = self.acting_as(key);
            if now == [key] {
                return word.to_string();
            }
            let names: Vec<String> = now.iter().map(|k| k.name()).collect();
            return if names.is_empty() {
                "–".into()
            } else {
                names.join("/")
            };
        }
        // Two keys one after the other (`gg`, `zM`): each the person's
        // first key for it.
        let mut chars = word.chars();
        let (Some(a), Some(b), None) = (chars.next(), chars.next(), chars.next()) else {
            return word.to_string();
        };
        let keys = [a, b].map(|c| Key::new(KeyCode::Char(c), KeyModifiers::NONE));
        if !keys.iter().all(|k| k.used()) {
            return word.to_string();
        }
        let firsts: Vec<Option<Key>> = keys
            .iter()
            .map(|&k| self.acting_as(k).first().copied())
            .collect();
        if firsts == keys.map(Some) {
            return word.to_string();
        }
        match firsts[..] {
            [Some(a), Some(b)] => format!("{} {}", a.name(), b.name()),
            _ => "–".into(),
        }
    }
}

/// What reading the key file gave.
#[derive(Debug, PartialEq, Eq)]
pub enum Read {
    /// No file: imece's own keys.
    Missing,
    /// The file's keys, and its text to keep as the last good one.
    Good(Keymap, String),
    /// A file that cannot be taken, and why.
    Bad(Why),
}

/// Reads the key file at `path`.
pub fn load(path: &Path) -> Read {
    let size = match std::fs::metadata(path) {
        Ok(m) if m.is_file() => m.len(),
        Ok(_) => return Read::Bad(Why::Unreadable),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Read::Missing,
        Err(_) => return Read::Bad(Why::Unreadable),
    };
    if size > MAX_BYTES {
        return Read::Bad(Why::TooBig);
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return Read::Bad(Why::Unreadable);
    };
    match Keymap::read(&text) {
        Ok(map) => Read::Good(map, text),
        Err(why) => Read::Bad(why),
    }
}

/// The keys a start uses: the file's when it is good, imece's own when
/// there is none, else the ones of the last good file (`kept`).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Start {
    pub map: Keymap,
    /// The text of the good file in use, to keep for the next start.
    pub good: Option<String>,
    /// Why the file was not taken, to say.
    pub refused: Option<Why>,
}

impl Start {
    pub fn new(read: Option<Read>, kept: Option<String>) -> Start {
        match read {
            Some(Read::Missing) => Start::default(),
            Some(Read::Good(map, text)) => Start {
                map,
                good: Some(text),
                refused: None,
            },
            other => {
                let map = kept.as_deref().map(Keymap::read);
                let (map, good) = match map {
                    Some(Ok(map)) => (map, kept),
                    _ => (Keymap::default(), None),
                };
                Start {
                    map,
                    good,
                    refused: match other {
                        Some(Read::Bad(why)) => Some(why),
                        _ => None,
                    },
                }
            }
        }
    }
}

/// Why the key file at `path` was not taken, in `lang`.
pub fn refusal(lang: Lang, path: &Path, why: &Why) -> String {
    let t = lang.text();
    let line = |n: usize, what: String| fill(t.keys_line, &[&n.to_string(), &what]);
    let what = match why {
        Why::Unreadable => t.keys_unreadable.to_string(),
        Why::TooBig => fill(t.keys_too_big, &[&(MAX_BYTES / 1024).to_string()]),
        Why::Shape(n) => line(*n, t.keys_shape.to_string()),
        Why::NotAKey(n, name) => line(*n, fill(t.keys_not_a_key, &[name])),
        Why::Unused(n, key) => line(*n, fill(t.keys_unused, &[&key.name()])),
        Why::Twice(n, key, first) => {
            line(*n, fill(t.keys_twice, &[&key.name(), &first.to_string()]))
        }
        Why::Quits(n) => line(*n, t.keys_quits.to_string()),
        Why::TooMany => fill(t.keys_too_many, &[&MAX_KEYS.to_string()]),
    };
    fill(t.keys_refused, &[&path.display().to_string(), &what])
}

/// What `:keys` says of a good file at `path`: how many keys it gives,
/// and the keys of imece's nothing does now.
pub fn taken(lang: Lang, path: &Path, map: &Keymap) -> String {
    let t = lang.text();
    let mut said = fill(
        t.keys_taken,
        &[&path.display().to_string(), &map.given().len().to_string()],
    );
    let lost: Vec<String> = map.lost().iter().map(|k| k.name()).collect();
    if !lost.is_empty() {
        said.push_str(&fill(t.keys_lost, &[&lost.join(" ")]));
    }
    said
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyEventKind, KeyEventState};

    use super::*;

    fn key(name: &str) -> Key {
        Key::parse(name).unwrap_or_else(|| panic!("{name} is a key"))
    }

    fn event(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    fn map(text: &str) -> Keymap {
        Keymap::read(text).unwrap_or_else(|e| panic!("{text}: {e:?}"))
    }

    #[test]
    fn the_file_is_under_the_config_home() {
        let abs = PathBuf::from("/cfg");
        let home = PathBuf::from("/home/me");
        assert_eq!(
            file_in(Some(abs), Some(home.clone())),
            Some(PathBuf::from("/cfg/imece/keys"))
        );
        assert_eq!(
            file_in(Some(PathBuf::from("rel")), Some(home.clone())),
            Some(PathBuf::from("/home/me/.config/imece/keys")),
            "a relative XDG_CONFIG_HOME is ignored, as the spec says"
        );
        assert_eq!(
            file_in(None, Some(home)),
            Some(PathBuf::from("/home/me/.config/imece/keys"))
        );
        assert_eq!(file_in(None, None), None);
    }

    #[test]
    fn the_file_is_found_from_the_environment() {
        let env = |name| std::env::var_os(name).map(PathBuf::from);
        assert_eq!(file(), file_in(env("XDG_CONFIG_HOME"), env("HOME")));
        assert!(
            file().is_some() || env("HOME").is_none(),
            "a home has a key file"
        );
    }

    #[test]
    fn names_mean_keys() {
        let k = |code, mods| Key { code, mods };
        let none = KeyModifiers::NONE;
        let ctrl = KeyModifiers::CONTROL;
        let alt = KeyModifiers::ALT;
        let shift = KeyModifiers::SHIFT;
        for (name, want) in [
            ("j", k(KeyCode::Char('j'), none)),
            ("G", k(KeyCode::Char('G'), none)),
            ("shift-g", k(KeyCode::Char('G'), none)),
            ("⇧G", k(KeyCode::Char('G'), none)),
            ("^n", k(KeyCode::Char('n'), ctrl)),
            ("ctrl-n", k(KeyCode::Char('n'), ctrl)),
            ("CTRL-N", k(KeyCode::Char('n'), ctrl)),
            ("^", k(KeyCode::Char('^'), none)),
            ("-", k(KeyCode::Char('-'), none)),
            ("ctrl--", k(KeyCode::Char('-'), ctrl)),
            ("^^", k(KeyCode::Char('^'), ctrl)),
            ("ctrl-^n", k(KeyCode::Char('n'), ctrl)),
            ("shift-backtab", k(KeyCode::BackTab, none)),
            ("⌥b", k(KeyCode::Char('b'), alt)),
            ("alt-b", k(KeyCode::Char('b'), alt)),
            ("^⌥x", k(KeyCode::Char('x'), ctrl | alt)),
            ("⇧tab", k(KeyCode::BackTab, none)),
            ("shift-tab", k(KeyCode::BackTab, none)),
            ("backtab", k(KeyCode::BackTab, none)),
            ("shift-enter", k(KeyCode::Enter, shift)),
            ("⇧↑", k(KeyCode::Up, shift)),
            ("enter", k(KeyCode::Enter, none)),
            ("Return", k(KeyCode::Enter, none)),
            ("esc", k(KeyCode::Esc, none)),
            ("escape", k(KeyCode::Esc, none)),
            ("tab", k(KeyCode::Tab, none)),
            ("space", k(KeyCode::Char(' '), none)),
            ("hash", k(KeyCode::Char('#'), none)),
            ("#", k(KeyCode::Char('#'), none)),
            ("backspace", k(KeyCode::Backspace, none)),
            ("del", k(KeyCode::Delete, none)),
            ("delete", k(KeyCode::Delete, none)),
            ("insert", k(KeyCode::Insert, none)),
            ("home", k(KeyCode::Home, none)),
            ("end", k(KeyCode::End, none)),
            ("pgup", k(KeyCode::PageUp, none)),
            ("pageup", k(KeyCode::PageUp, none)),
            ("pgdn", k(KeyCode::PageDown, none)),
            ("pagedown", k(KeyCode::PageDown, none)),
            ("↑", k(KeyCode::Up, none)),
            ("up", k(KeyCode::Up, none)),
            ("↓", k(KeyCode::Down, none)),
            ("down", k(KeyCode::Down, none)),
            ("←", k(KeyCode::Left, none)),
            ("left", k(KeyCode::Left, none)),
            ("→", k(KeyCode::Right, none)),
            ("right", k(KeyCode::Right, none)),
            ("f1", k(KeyCode::F(1), none)),
            ("F12", k(KeyCode::F(12), none)),
        ] {
            assert_eq!(Key::parse(name), Some(want), "{name}");
        }
        for wrong in [
            "", "jj", "ctrl-", "shift-1", "⇧!", "f0", "f13", "fx", "enterx", "gg",
        ] {
            assert_eq!(Key::parse(wrong), None, "{wrong:?}");
        }
    }

    #[test]
    fn a_key_is_named_as_the_help_writes_it_and_read_back() {
        for (name, shown) in [
            ("j", "j"),
            ("G", "G"),
            ("ctrl-n", "^n"),
            ("alt-b", "⌥b"),
            ("^⌥x", "^⌥x"),
            ("shift-tab", "⇧tab"),
            ("shift-enter", "⇧enter"),
            ("space", "space"),
            ("hash", "#"),
            ("return", "enter"),
            ("escape", "esc"),
            ("tab", "tab"),
            ("backspace", "backspace"),
            ("delete", "del"),
            ("insert", "insert"),
            ("home", "home"),
            ("end", "end"),
            ("pageup", "pgup"),
            ("pagedown", "pgdn"),
            ("up", "↑"),
            ("down", "↓"),
            ("left", "←"),
            ("right", "→"),
            ("f5", "f5"),
        ] {
            assert_eq!(key(name).name(), shown, "{name}");
            assert_eq!(key(shown), key(name), "{shown} reads back");
        }
    }

    #[test]
    fn a_key_from_the_terminal_is_told_apart_as_in_the_file() {
        let of = |code, mods| Key::of(&event(code, mods));
        assert_eq!(
            of(KeyCode::Char('G'), KeyModifiers::SHIFT),
            key("G"),
            "the shift is in the letter"
        );
        assert_eq!(
            of(
                KeyCode::Char('N'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT
            ),
            key("^n")
        );
        assert_eq!(of(KeyCode::BackTab, KeyModifiers::SHIFT), key("⇧tab"));
        assert_eq!(
            of(KeyCode::Enter, KeyModifiers::SHIFT),
            key("shift-enter"),
            "a named key keeps its shift"
        );
        assert_ne!(of(KeyCode::Enter, KeyModifiers::SHIFT), key("enter"));
        assert_eq!(
            of(KeyCode::Char('j'), KeyModifiers::SUPER),
            key("j"),
            "only ctrl, alt and shift count"
        );
        assert_eq!(of(KeyCode::Char('b'), KeyModifiers::ALT), key("⌥b"));
    }

    #[test]
    fn what_imece_uses_outside_writing() {
        for used in [
            "a", "j", "G", "Y", "1", "9", "?", "'", "#", "space", "^d", "^u", "^f", "^b", "^o",
            "^r", "enter", "esc", "tab", "⇧tab", "home", "end", "pgup", "pgdn", "↑", "↓", "←", "→",
        ] {
            assert!(key(used).used(), "{used}");
        }
        for unused in [
            "d",
            "e",
            "o",
            "u",
            "A",
            "N",
            "0",
            "$",
            "^a",
            "^e",
            "^w",
            "^k",
            "^y",
            "^j",
            "⌥b",
            "⌥j",
            "⇧enter",
            "backspace",
            "del",
            "insert",
            "f1",
            "^⌥d",
        ] {
            assert!(!key(unused).used(), "{unused}");
        }
    }

    #[test]
    fn a_file_gives_keys_in_its_order() {
        let m = map("# mine\n\n  ^n = j \n^p = k\nJ = G\n# the end");
        assert_eq!(
            m.given(),
            [
                (key("^n"), key("j")),
                (key("^p"), key("k")),
                (key("J"), key("G"))
            ]
        );
        assert_eq!(map(""), Keymap::default());
    }

    #[test]
    fn a_file_is_refused_with_its_first_wrong_line() {
        let why = |text: &str| Keymap::read(text).unwrap_err();
        assert_eq!(why("^n = j\n^p k"), Why::Shape(2));
        assert_eq!(why("^n == j"), Why::Shape(1));
        assert_eq!(why("^n=j"), Why::Shape(1));
        assert_eq!(why("^n = j k"), Why::Shape(1));
        assert_eq!(why("^n = j = k"), Why::Shape(1));
        assert_eq!(why("\n\nnope = j"), Why::NotAKey(3, "nope".into()));
        assert_eq!(why("^n = nope"), Why::NotAKey(1, "nope".into()));
        assert_eq!(
            why(&format!("{} = j", "x".repeat(40))),
            Why::NotAKey(1, "x".repeat(SAID)),
            "a long wrong name is cut"
        );
        assert_eq!(why("^n = d"), Why::Unused(1, key("d")));
        assert_eq!(why("^n = ^a"), Why::Unused(1, key("^a")));
        assert_eq!(why("^n = j\nx = k\n^n = k"), Why::Twice(3, key("^n"), 1));
        assert_eq!(why("j = k\n# x\nj = G"), Why::Twice(3, key("j"), 1));
        assert_eq!(why("^c = j"), Why::Quits(1));
        assert_eq!(why("j = ^c"), Why::Quits(1));
        assert_eq!(why("x = ctrl-C"), Why::Quits(1));
    }

    #[test]
    fn a_file_gives_no_more_than_its_cap() {
        // MAX_KEYS distinct keys of the person's, all to `j`.
        let froms: Vec<String> = (0..=MAX_KEYS)
            .map(|i| match i {
                0..=25 => format!("⌥{}", char::from(b'a' + i as u8)),
                26..=51 => format!("^⌥{}", char::from(b'a' + (i - 26) as u8)),
                _ => format!("⌥{}", char::from_u32(0x100 + i as u32).unwrap()),
            })
            .collect();
        let text = |n: usize| -> String {
            froms[..n]
                .iter()
                .map(|f| format!("{f} = j\n"))
                .collect::<String>()
        };
        assert_eq!(map(&text(MAX_KEYS)).given().len(), MAX_KEYS);
        assert_eq!(Keymap::read(&text(MAX_KEYS + 1)), Err(Why::TooMany));
    }

    #[test]
    fn a_key_is_turned_once() {
        let m = map("j = k\nk = j\n^n = G\nx = ^d");
        let turn = |code, mods| {
            let e = m.turn(KeyEvent {
                code,
                modifiers: mods,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            });
            (e.code, e.modifiers, e.kind)
        };
        let none = KeyModifiers::NONE;
        let press = KeyEventKind::Press;
        assert_eq!(
            turn(KeyCode::Char('j'), none),
            (KeyCode::Char('k'), none, press),
            "swapped, not turned twice"
        );
        assert_eq!(
            turn(KeyCode::Char('k'), none),
            (KeyCode::Char('j'), none, press)
        );
        assert_eq!(
            turn(KeyCode::Char('n'), KeyModifiers::CONTROL),
            (KeyCode::Char('G'), none, press),
            "ctrl goes with the key"
        );
        assert_eq!(
            turn(KeyCode::Char('x'), none),
            (KeyCode::Char('d'), KeyModifiers::CONTROL, press),
            "and comes with it"
        );
        assert_eq!(
            turn(KeyCode::Char('G'), KeyModifiers::SHIFT),
            (KeyCode::Char('G'), KeyModifiers::SHIFT, press),
            "a key the file does not name is left as it came"
        );
        let held = m.turn(KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: none,
            kind: KeyEventKind::Repeat,
            state: KeyEventState::KEYPAD,
        });
        assert_eq!(
            (held.code, held.kind, held.state),
            (
                KeyCode::Char('k'),
                KeyEventKind::Repeat,
                KeyEventState::KEYPAD
            )
        );
        assert_eq!(
            Keymap::default().turn(event(KeyCode::Char('j'), none)).code,
            KeyCode::Char('j')
        );
    }

    #[test]
    fn what_acts_as_a_key_and_what_is_lost() {
        let m = map("^n = j\n^j = j\nj = G\nk = j\n^x = k");
        assert_eq!(m.acting_as(key("j")), [key("^n"), key("^j"), key("k")]);
        assert_eq!(m.acting_as(key("G")), [key("j"), key("G")]);
        assert_eq!(m.acting_as(key("k")), [key("^x")]);
        assert_eq!(m.acting_as(key("a")), [key("a")]);
        assert_eq!(
            m.lost(),
            Vec::<Key>::new(),
            "j and k moved to other keys, ^x meant nothing"
        );
        assert_eq!(map("x = j").lost(), [key("x")], "x chose a hunk");
        let m = map("j = G\nk = G\n^a = k\n^e = G");
        assert_eq!(
            m.lost(),
            [key("j")],
            "j's meaning went nowhere; ^e meant nothing"
        );
    }

    #[test]
    fn a_help_line_shows_the_keys_as_they_are_now() {
        let m = map("^n = j\nj = G\nk = j\nJ = k\n^g = g");
        assert_eq!(
            m.shown("↑ ↓  j k"),
            "↑ ↓  ^n/k J",
            "the spacing stays, each key the person's"
        );
        assert_eq!(m.shown("gg  G"), "^g ^g  j/G");
        assert_eq!(m.shown("zM"), "zM", "unmoved keys one after the other");
        assert_eq!(m.shown("za  q q  :quit"), "za  q q  :quit");
        assert_eq!(m.shown("@name text"), "@name text");
        assert_eq!(m.shown("#N"), "#N", "N is no key of imece's");
        assert_eq!(m.shown("$"), "$");
        let m = map("g = G\nq = j");
        assert_eq!(m.shown("gg  G  q"), "–  g/G  –", "nothing does g or q now");
        assert_eq!(m.shown("zg"), "–");
        assert_eq!(m.shown("gz"), "–");
        // The example of the keyboard page.
        let m = map("j = k\nk = j\n^n = j\n^p = k\nJ = G");
        assert_eq!(m.shown("↑ ↓  j k"), "↑ ↓  k/^n j/^p");
        assert_eq!(
            Keymap::default().shown("↑ ↓  j k"),
            "↑ ↓  j k",
            "no file, no change"
        );
    }

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("imece-keys-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_file_is_read_from_disk() {
        let dir = dir("load");
        let path = dir.join(NAME);
        assert_eq!(load(&path), Read::Missing);
        std::fs::write(&path, "^n = j\n").unwrap();
        assert_eq!(load(&path), Read::Good(map("^n = j"), "^n = j\n".into()));
        std::fs::write(&path, "^n j\n").unwrap();
        assert_eq!(load(&path), Read::Bad(Why::Shape(1)));
        let big = "#".repeat(MAX_BYTES as usize + 1);
        std::fs::write(&path, &big).unwrap();
        assert_eq!(load(&path), Read::Bad(Why::TooBig));
        std::fs::write(&path, &big[1..]).unwrap();
        assert_eq!(
            load(&path),
            Read::Good(Keymap::default(), big[1..].into()),
            "the cap itself is fine"
        );
        std::fs::write(&path, [0xff, 0xfe]).unwrap();
        assert_eq!(load(&path), Read::Bad(Why::Unreadable), "not text");
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert_eq!(load(&path), Read::Bad(Why::Unreadable), "a folder");
        assert_eq!(
            load(Path::new("/dev/null")),
            Read::Bad(Why::Unreadable),
            "not a file, though it reads as empty"
        );
        assert_eq!(
            load(&dir.join("gone/keys")),
            Read::Missing,
            "no folder, no file"
        );
        std::fs::write(dir.join("plain"), "").unwrap();
        assert_eq!(
            load(&dir.join("plain/keys")),
            Read::Bad(Why::Unreadable),
            "a file in the way is not a missing file"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_start_takes_the_good_file_or_keeps_the_last_good_one() {
        let good = || Some(Read::Good(map("^n = j"), "^n = j".into()));
        let kept = || Some("^p = k".to_string());
        assert_eq!(
            Start::new(good(), kept()),
            Start {
                map: map("^n = j"),
                good: Some("^n = j".into()),
                refused: None
            }
        );
        assert_eq!(
            Start::new(Some(Read::Missing), kept()),
            Start::default(),
            "no file: imece's own keys, the kept ones forgotten"
        );
        assert_eq!(
            Start::new(Some(Read::Bad(Why::Shape(2))), kept()),
            Start {
                map: map("^p = k"),
                good: kept(),
                refused: Some(Why::Shape(2))
            }
        );
        assert_eq!(
            Start::new(Some(Read::Bad(Why::TooBig)), Some("nonsense".into())),
            Start {
                map: Keymap::default(),
                good: None,
                refused: Some(Why::TooBig)
            },
            "kept text that no longer reads is dropped"
        );
        assert_eq!(
            Start::new(Some(Read::Bad(Why::TooBig)), None),
            Start {
                refused: Some(Why::TooBig),
                ..Start::default()
            }
        );
        assert_eq!(
            Start::new(None, kept()),
            Start {
                map: map("^p = k"),
                good: kept(),
                refused: None
            },
            "no home to look in: the kept keys"
        );
    }

    #[test]
    fn refusals_and_readings_are_said_in_both_languages() {
        let path = Path::new("/c/imece/keys");
        let say = |lang, why| refusal(lang, path, &why);
        assert_eq!(
            say(Lang::En, Why::Shape(3)),
            "/c/imece/keys: line 3: write it as KEY = KEY; the keys you had stay"
        );
        assert_eq!(
            say(Lang::En, Why::NotAKey(2, "nope".into())),
            "/c/imece/keys: line 2: `nope` is not a key; the keys you had stay"
        );
        assert_eq!(
            say(Lang::En, Why::Unused(4, key("d"))),
            "/c/imece/keys: line 4: imece does nothing with d; the keys you had stay"
        );
        assert_eq!(
            say(Lang::En, Why::Twice(5, key("^n"), 1)),
            "/c/imece/keys: line 5: ^n is given on line 1 already; the keys you had stay"
        );
        assert_eq!(
            say(Lang::En, Why::Quits(6)),
            "/c/imece/keys: line 6: ctrl-c always quits; the keys you had stay"
        );
        assert_eq!(
            say(Lang::En, Why::TooMany),
            "/c/imece/keys: more than 200 keys; the keys you had stay"
        );
        assert_eq!(
            say(Lang::En, Why::TooBig),
            "/c/imece/keys: bigger than 64 KiB; the keys you had stay"
        );
        assert_eq!(
            say(Lang::En, Why::Unreadable),
            "/c/imece/keys: cannot be read as text; the keys you had stay"
        );
        assert_eq!(
            say(Lang::Tr, Why::Twice(5, key("^n"), 1)),
            "/c/imece/keys: satır 5: ^n 1. satırda zaten verilmiş; elindeki tuşlar kalıyor"
        );
        assert_eq!(
            say(Lang::Tr, Why::Unused(4, key("d"))),
            "/c/imece/keys: satır 4: imece d ile bir şey yapmıyor; elindeki tuşlar kalıyor"
        );
        assert_eq!(
            say(Lang::Tr, Why::TooMany),
            "/c/imece/keys: 200 tuştan fazla; elindeki tuşlar kalıyor"
        );
        assert_eq!(
            say(Lang::En, Why::NotAKey(1, "\u{1b}[2J".into())),
            "/c/imece/keys: line 1: ` [2J` is not a key; the keys you had stay",
            "what the file says is cleaned"
        );
        let m = map("^n = j\nj = G");
        assert_eq!(taken(Lang::En, path, &m), "/c/imece/keys: keys taken (2)");
        let m = map("^n = G\nj = G\nk = G");
        assert_eq!(
            taken(Lang::En, path, &m),
            "/c/imece/keys: keys taken (3); nothing does j k now"
        );
        assert_eq!(
            taken(Lang::Tr, path, &m),
            "/c/imece/keys: tuşlar alındı (3); artık hiçbir tuş j k işini yapmıyor"
        );
    }
}
