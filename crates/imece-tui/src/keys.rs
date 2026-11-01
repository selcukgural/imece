//! The person's own keys: a file of `KEY = KEY` lines that makes a key
//! act as one of imece's (`^n = j`), read at start, on `:keys` and
//! again when it changes ([`stamp`], looked at every [`EVERY`]).
//!
//! A line is turned once, as vim's `noremap`: `j = k` and `k = j` swap
//! the two, and a key the file does not name stays itself. A side is one
//! key or two ([`Keys`]): `gt = G`, `x = gg`; the first of two waits for
//! the second up to [`WAIT`]. The map holds
//! wherever imece reads keys except writing, the help's `/` filter and
//! the hint letters, whose keys are text. A file imece cannot take whole
//! is refused and the keys in use stay; the last good file is kept in the
//! store, so a start with a bad file still has them. See
//! `docs/decisions/76-keys.md`.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::i18n::{Lang, fill};

/// The file's name under `$XDG_CONFIG_HOME/imece`.
pub const NAME: &str = "keys";
/// A bigger file is refused unread.
pub const MAX_BYTES: u64 = 64 * 1024;
/// How often the key file is looked at for a change; back in the
/// window it is looked at once more.
pub const EVERY: Duration = Duration::from_secs(2);
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
    /// Keys on line `n` that line `first` gave already.
    Twice(usize, Keys, usize),
    /// ctrl-c on line `n`: it only ever quits.
    Quits(usize),
    /// More than [`MAX_KEYS`] keys.
    TooMany,
    /// More than [`STROKES`] keys on a side of line `n`.
    Long(usize),
    /// A key on line `n` given alone and as the first of two keys, the
    /// other on line `first`: alone, it would never wait for a second.
    Starts(usize, Key, usize),
}

/// The most keys on a side of a line: `gt = G`, `x = gg`.
pub const STROKES: usize = 2;
/// How long the first key of the person's two waits for the second;
/// then it is let through as itself (vim's `timeoutlen`).
pub const WAIT: Duration = Duration::from_secs(1);

/// One key or two, one after the other: a side of a line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keys(Vec<Key>);

impl Keys {
    /// The keys one after the other.
    pub fn of(keys: &[Key]) -> Keys {
        Keys(keys.to_vec())
    }

    /// The keys as the help writes them: `j`, `gt`, `^x j`. Plain
    /// characters go together unless that reads as another key (`f1`,
    /// `up`); the rest apart.
    pub fn name(&self) -> String {
        let names: Vec<String> = self.0.iter().map(|k| k.name()).collect();
        let joined = names.concat();
        if names.iter().all(|n| n.chars().count() == 1) && Key::parse(&joined).is_none() {
            joined
        } else {
            names.join(" ")
        }
    }

    /// The keys as key events, with the kind and state of `like`.
    fn events(&self, like: KeyEvent) -> Vec<KeyEvent> {
        (self.0.iter())
            .map(|k| KeyEvent {
                code: k.code,
                modifiers: k.mods,
                ..like
            })
            .collect()
    }
}

/// The keys one side of a line names: one word or more (`g enter`), a
/// word a key's name (`^n`, `f1`) or else up to [`STROKES`] plain keys
/// one after the other (`gt`, `zM`). `Err` holds the word that is no key.
fn side(words: &[&str]) -> Result<Vec<Key>, String> {
    let mut keys = Vec::new();
    for &word in words {
        if let Some(k) = Key::parse(word) {
            keys.push(k);
            continue;
        }
        if word.chars().count() > STROKES {
            return Err(word.chars().take(SAID).collect());
        }
        keys.extend(chord(word));
    }
    Ok(keys)
}

/// imece's own keys of two strokes, as the help writes them.
const CHORDS: [&str; 6] = ["gg", "za", "zM", "zR", "qq", "qs"];

/// The keys a file gives: each of the person's keys and the keys of
/// imece's it acts as, in the file's order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Keymap {
    given: Vec<(Keys, Keys)>,
}

impl Keymap {
    /// The keys `text` gives, or the first reason it cannot be taken.
    pub fn read(text: &str) -> Result<Keymap, Why> {
        let mut given: Vec<(Keys, Keys)> = Vec::new();
        let mut lines: Vec<usize> = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let n = i + 1;
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let words: Vec<&str> = line.split_whitespace().collect();
            let mut parts = words.split(|&w| w == "=");
            let (Some(from), Some(to), None) = (parts.next(), parts.next(), parts.next()) else {
                return Err(Why::Shape(n));
            };
            if from.is_empty() || to.is_empty() {
                return Err(Why::Shape(n));
            }
            let not_a_key = |name| Why::NotAKey(n, name);
            let (from, to) = (side(from).map_err(not_a_key)?, side(to).map_err(not_a_key)?);
            if from.iter().chain(&to).any(|k| k.quits()) {
                return Err(Why::Quits(n));
            }
            if from.len() > STROKES || to.len() > STROKES {
                return Err(Why::Long(n));
            }
            if let Some(&k) = to.iter().find(|k| !k.used()) {
                return Err(Why::Unused(n, k));
            }
            let (from, to) = (Keys(from), Keys(to));
            if let Some(at) = given.iter().position(|(f, _)| *f == from) {
                return Err(Why::Twice(n, from, lines[at]));
            }
            // The first key alone on one line and first of two on another.
            let first = from.0[0];
            if let Some(at) = (given.iter())
                .position(|(f, _)| f.0[0] == first && (f.0.len() == 1) != (from.0.len() == 1))
            {
                return Err(Why::Starts(n, first, lines[at]));
            }
            if given.len() == MAX_KEYS {
                return Err(Why::TooMany);
            }
            given.push((from, to));
            lines.push(n);
        }
        Ok(Keymap { given })
    }

    /// The keys a line gives for exactly `keys`.
    fn to(&self, keys: &[Key]) -> Option<&Keys> {
        (self.given.iter())
            .find(|(from, _)| from.0 == keys)
            .map(|(_, to)| to)
    }

    /// `k` by itself as the person means it: the keys it acts as, or
    /// itself. Not looked up again, so `j = k` and `k = j` swap.
    pub fn alone(&self, k: KeyEvent) -> Vec<KeyEvent> {
        match self.to(&[Key::of(&k)]) {
            Some(to) => to.events(k),
            None => vec![k],
        }
    }

    /// The keys `first` and then `k` act as when the file gives the two
    /// as one key of the person's.
    pub fn pair(&self, first: KeyEvent, k: KeyEvent) -> Option<Vec<KeyEvent>> {
        (self.to(&[Key::of(&first), Key::of(&k)])).map(|to| to.events(k))
    }

    /// Whether `k` is the first of two keys the file gives: it waits
    /// for the second before it acts.
    pub fn starts(&self, k: KeyEvent) -> bool {
        let key = Key::of(&k);
        (self.given.iter()).any(|(from, _)| from.0.len() == STROKES && from.0[0] == key)
    }

    /// Each of the person's keys and the keys it acts as.
    pub fn given(&self) -> &[(Keys, Keys)] {
        &self.given
    }

    /// The keys that do what `keys` (one of imece's, or two) does now:
    /// the person's whose line gives exactly that, in the file's order,
    /// then `keys` themselves as the person reaches them: a key unless
    /// the file gave it alone another meaning; two keys each by its
    /// first way, unless the file gives those two another meaning.
    pub fn acting_as(&self, keys: &[Key]) -> Vec<Keys> {
        let mut ways: Vec<Keys> = (self.given.iter())
            .filter(|(_, to)| to.0 == keys)
            .map(|(from, _)| from.clone())
            .collect();
        let own = match keys {
            [key] => (self.to(keys).is_none()).then(|| Keys(vec![*key])),
            _ => {
                let each: Option<Vec<Key>> = (keys.iter()).map(|&k| self.reaching(k)).collect();
                each.filter(|each| self.to(each).is_none()).map(Keys)
            }
        };
        ways.extend(own);
        ways
    }

    /// The first key alone that does what `key` does now.
    fn reaching(&self, key: Key) -> Option<Key> {
        (self.given.iter())
            .find(|(from, to)| from.0.len() == 1 && to.0 == [key])
            .map(|(from, _)| from.0[0])
            .or_else(|| self.to(&[key]).is_none().then_some(key))
    }

    /// imece's keys (alone, or two such as `gg`) the file took a
    /// meaning from without giving it to other keys: nothing does what
    /// they did.
    pub fn lost(&self) -> Vec<Keys> {
        (self.given.iter())
            .map(|(from, _)| from)
            .filter(|from| match &from.0[..] {
                [k] => k.used(),
                _ => CHORDS.iter().any(|c| Keys::of(&chord(c)) == **from),
            })
            .filter(|from| self.acting_as(&from.0).is_empty())
            .cloned()
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
        let keys = match Key::parse(word) {
            Some(key) if key.used() => vec![key],
            Some(_) => return word.to_string(),
            // Two plain keys one after the other (`gg`, `zM`).
            None => {
                let keys = chord(word);
                if keys.len() != STROKES || !keys.iter().all(|k| k.used()) {
                    return word.to_string();
                }
                keys
            }
        };
        let now = self.acting_as(&keys);
        if now == [Keys(keys)] {
            return word.to_string();
        }
        let names: Vec<String> = now.iter().map(Keys::name).collect();
        if names.is_empty() {
            "–".into()
        } else {
            names.join("/")
        }
    }
}

/// The plain keys of `word`, one per character.
fn chord(word: &str) -> Vec<Key> {
    (word.chars())
        .map(|c| Key::new(KeyCode::Char(c), KeyModifiers::NONE))
        .collect()
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

/// What tells one state of the key file from another without reading
/// it: its modified time, its length and its inode (an editor that saves
/// by renaming a new file over it gives a new one). The default is no
/// file, or one that cannot be looked at.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stamp(Option<(Option<SystemTime>, u64, u64)>);

/// The stamp of the file at `path` now, through a link as [`load`] reads
/// it. Taken before the file is read, so a change while it is read is
/// seen the next time.
pub fn stamp(path: &Path) -> Stamp {
    Stamp(
        std::fs::metadata(path)
            .ok()
            .map(|m| (m.modified().ok(), m.len(), m.ino())),
    )
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
        Why::Long(n) => line(*n, fill(t.keys_long, &[&STROKES.to_string()])),
        Why::Starts(n, key, first) => {
            line(*n, fill(t.keys_starts, &[&key.name(), &first.to_string()]))
        }
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
    let lost: Vec<String> = map.lost().iter().map(Keys::name).collect();
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

    /// The keys a side of a line names.
    fn ks(text: &str) -> Keys {
        let words: Vec<&str> = text.split(' ').collect();
        Keys(side(&words).unwrap_or_else(|w| panic!("{w} is no key")))
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
            [(ks("^n"), ks("j")), (ks("^p"), ks("k")), (ks("J"), ks("G"))]
        );
        assert_eq!(map(""), Keymap::default());
        let m = map("gt = G\nx = gg\ng enter = z M\n^t = ^d ^o\nf1 = zR\nu p = j");
        let sides: Vec<(Vec<Key>, Vec<Key>)> = (m.given().iter())
            .map(|(f, t)| (f.0.clone(), t.0.clone()))
            .collect();
        assert_eq!(
            sides,
            [
                (vec![key("g"), key("t")], vec![key("G")]),
                (vec![key("x")], vec![key("g"), key("g")]),
                (vec![key("g"), key("enter")], vec![key("z"), key("M")]),
                (vec![key("^t")], vec![key("^d"), key("^o")]),
                (vec![key("f1")], vec![key("z"), key("R")]),
                (vec![key("u"), key("p")], vec![key("j")]),
            ],
            "a word of two plain keys, or keys apart; a name is one key"
        );
    }

    #[test]
    fn keys_are_named_together_unless_that_reads_as_another_key() {
        assert_eq!(ks("gt").name(), "gt");
        assert_eq!(ks("z M").name(), "zM");
        assert_eq!(ks("j").name(), "j");
        assert_eq!(ks("^n").name(), "^n");
        assert_eq!(ks("g enter").name(), "g enter");
        assert_eq!(ks("^x j").name(), "^x j");
        assert_eq!(ks("u p").name(), "u p", "`up` is ↑");
        assert_eq!(ks("f 1").name(), "f 1", "`f1` is f1");
        for keys in ["gt", "g enter", "u p", "f 1", "^x j", "zM"] {
            let name = ks(keys).name();
            let words: Vec<&str> = name.split(' ').collect();
            assert_eq!(Keys(side(&words).unwrap()), ks(keys), "{keys} read back");
        }
    }

    #[test]
    fn a_file_is_refused_with_its_first_wrong_line() {
        let why = |text: &str| Keymap::read(text).unwrap_err();
        assert_eq!(why("^n = j\n^p k"), Why::Shape(2));
        assert_eq!(why("^n == j"), Why::Shape(1));
        assert_eq!(why("^n=j"), Why::Shape(1));
        assert_eq!(why("^n = j = k"), Why::Shape(1));
        assert_eq!(why("= j"), Why::Shape(1));
        assert_eq!(why("j ="), Why::Shape(1));
        assert_eq!(why("j = k x a"), Why::Long(1));
        assert_eq!(why("j = kx a"), Why::Long(1));
        assert_eq!(why("\na b c = j"), Why::Long(2));
        assert_eq!(why("ab c = j"), Why::Long(1));
        assert_eq!(why("abc = j"), Why::NotAKey(1, "abc".into()));
        assert_eq!(why("x = jd"), Why::Unused(1, key("d")));
        assert_eq!(why("x = dj"), Why::Unused(1, key("d")));
        assert_eq!(why("gt = G\ngt = j"), Why::Twice(2, ks("gt"), 1));
        assert_eq!(why("g = j\nx = k\ngt = G"), Why::Starts(3, key("g"), 1));
        assert_eq!(why("gt = G\ng = j"), Why::Starts(2, key("g"), 1));
        assert_eq!(why("x ^c = j"), Why::Quits(1));
        assert_eq!(why("x = j ^c"), Why::Quits(1));
        assert!(
            Keymap::read("gt = G\ngx = j\ntg = k").is_ok(),
            "two firsts alike"
        );
        assert!(Keymap::read("g = G\ntg = j").is_ok(), "a second alike");
        assert_eq!(why("\n\nnope = j"), Why::NotAKey(3, "nope".into()));
        assert_eq!(why("^n = nope"), Why::NotAKey(1, "nope".into()));
        assert_eq!(
            why(&format!("{} = j", "x".repeat(40))),
            Why::NotAKey(1, "x".repeat(SAID)),
            "a long wrong name is cut"
        );
        assert_eq!(why("^n = d"), Why::Unused(1, key("d")));
        assert_eq!(why("^n = ^a"), Why::Unused(1, key("^a")));
        assert_eq!(why("^n = j\nx = k\n^n = k"), Why::Twice(3, ks("^n"), 1));
        assert_eq!(why("j = k\n# x\nj = G"), Why::Twice(3, ks("j"), 1));
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
            let out = m.alone(KeyEvent {
                code,
                modifiers: mods,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            });
            let [e] = out[..] else {
                panic!("one key: {out:?}")
            };
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
        let held = m.alone(KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: none,
            kind: KeyEventKind::Repeat,
            state: KeyEventState::KEYPAD,
        });
        let held = held[0];
        assert_eq!(
            (held.code, held.kind, held.state),
            (
                KeyCode::Char('k'),
                KeyEventKind::Repeat,
                KeyEventState::KEYPAD
            )
        );
        assert_eq!(
            Keymap::default().alone(event(KeyCode::Char('j'), none)),
            [event(KeyCode::Char('j'), none)]
        );
    }

    fn plain(c: char) -> KeyEvent {
        event(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn two_keys_are_one_and_one_key_two() {
        let m = map("gt = G\nx = gg\n^t = z M\nj = k\ng enter = ^o");
        assert!(m.starts(plain('g')), "g waits for its second");
        assert!(!m.starts(plain('t')), "a second key does not");
        assert!(!m.starts(plain('x')), "nor a key given alone");
        assert!(!m.starts(plain('j')));
        assert!(!Keymap::default().starts(plain('g')));
        assert!(
            !map("^g = j").starts(event(KeyCode::Char('g'), KeyModifiers::CONTROL)),
            "a key alone is not the first of two"
        );
        assert_eq!(m.pair(plain('g'), plain('t')), Some(vec![plain('G')]));
        let enter = event(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(
            m.pair(plain('g'), enter),
            Some(vec![event(KeyCode::Char('o'), KeyModifiers::CONTROL)])
        );
        assert_eq!(m.pair(plain('g'), plain('g')), None, "gg is imece's");
        assert_eq!(m.pair(plain('t'), plain('g')), None, "in its order");
        assert_eq!(m.pair(plain('x'), plain('g')), None);
        assert_eq!(m.alone(plain('x')), [plain('g'), plain('g')]);
        assert_eq!(
            m.alone(event(KeyCode::Char('t'), KeyModifiers::CONTROL)),
            [plain('z'), plain('M')]
        );
        assert_eq!(m.alone(plain('g')), [plain('g')], "g alone is g");
        // The second key's kind and state go with what comes out.
        let second = KeyEvent {
            kind: KeyEventKind::Repeat,
            state: KeyEventState::KEYPAD,
            ..plain('t')
        };
        let out = m.pair(plain('g'), second).unwrap();
        assert_eq!(
            (out[0].kind, out[0].state),
            (KeyEventKind::Repeat, KeyEventState::KEYPAD)
        );
    }

    #[test]
    fn what_acts_as_a_key_and_what_is_lost() {
        let m = map("^n = j\n^j = j\nj = G\nk = j\n^x = k");
        let acting = |keys: &str| m.acting_as(&ks(keys).0);
        assert_eq!(acting("j"), [ks("^n"), ks("^j"), ks("k")]);
        assert_eq!(acting("G"), [ks("j"), ks("G")]);
        assert_eq!(acting("k"), [ks("^x")]);
        assert_eq!(acting("a"), [ks("a")]);
        assert_eq!(acting("gg"), [ks("gg")], "two keys the file left");
        assert_eq!(acting("jg"), [ks("^n g")], "each by its first way");
        assert_eq!(acting("kj"), [ks("^x ^n")]);
        assert_eq!(
            m.lost(),
            Vec::<Keys>::new(),
            "j and k moved to other keys, ^x meant nothing"
        );
        assert_eq!(map("x = j").lost(), [ks("x")], "x chose a hunk");
        let m = map("j = G\nk = G\n^a = k\n^e = G");
        assert_eq!(
            m.lost(),
            [ks("j")],
            "j's meaning went nowhere; ^e meant nothing"
        );
        let m = map("gt = G\nx = gg\nzM = j\nza = k\n^a = za\ngx = q");
        let acting = |keys: &str| m.acting_as(&ks(keys).0);
        assert_eq!(acting("G"), [ks("gt"), ks("G")], "two keys act as one");
        assert_eq!(acting("gg"), [ks("x"), ks("gg")], "one key as two");
        assert_eq!(acting("g"), [ks("g")], "g alone is still g");
        assert_eq!(acting("zM"), Vec::<Keys>::new(), "zM is j now");
        assert_eq!(acting("za"), [ks("^a")], "za is k, ^a is za");
        assert_eq!(acting("q"), [ks("gx"), ks("q")]);
        assert_eq!(
            m.lost(),
            [ks("x"), ks("zM")],
            "nothing does x or zM; ^a does za"
        );
        assert_eq!(
            map("gx = j").lost(),
            Vec::<Keys>::new(),
            "gx is none of imece's"
        );
        assert_eq!(map("qs = j\nqq = k").lost(), [ks("qs"), ks("qq")]);
        assert_eq!(map("zR = j").lost(), [ks("zR")]);
        assert_eq!(map("gg = j").lost(), [ks("gg")]);
        assert_eq!(map("gg = j\n^g = gg").lost(), Vec::<Keys>::new());
        let m = map("g = j\n^g = g");
        assert_eq!(m.acting_as(&ks("gg").0), [ks("^g ^g")]);
        assert_eq!(m.lost(), Vec::<Keys>::new());
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
        // Keys of two.
        let m = map("gt = G\nx = gg\nzM = j\n^t = zR\ng enter = q");
        assert_eq!(m.shown("gg  G"), "x/gg  gt/G");
        assert_eq!(m.shown("zM  zR"), "–  ^t/zR");
        assert_eq!(m.shown("j"), "zM/j");
        assert_eq!(m.shown("q q"), "g enter/q g enter/q");
        assert_eq!(m.shown("za"), "za");
        assert_eq!(m.shown("zd"), "zd", "d is no key of imece's");
        let m = map("^n = j\nj = G\nd = k");
        assert_eq!(m.shown("jjj"), "jjj", "three keys are no chord of imece's");
        assert_eq!(m.shown("d"), "d", "nor is a key it does nothing with");
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
    fn a_stamp_tells_a_changed_file_from_the_same_one() {
        let dir = dir("stamp");
        let path = dir.join(NAME);
        assert_eq!(stamp(&path), Stamp::default(), "no file");
        std::fs::write(&path, "^n = j\n").unwrap();
        let first = stamp(&path);
        assert_ne!(first, Stamp::default(), "a file");
        assert_eq!(stamp(&path), first, "looked at again, unchanged");
        std::fs::write(&path, "^n = j\nj = G\n").unwrap();
        let longer = stamp(&path);
        assert_ne!(longer, first, "a longer text");
        let other = dir.join("new");
        std::fs::write(&other, "^p = k\nj = G\n").unwrap();
        std::fs::rename(&other, &path).unwrap();
        assert_ne!(stamp(&path), longer, "a new file saved over it");
        let link = dir.join("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert_eq!(stamp(&link), stamp(&path), "through a link");
        std::fs::remove_file(&path).unwrap();
        assert_eq!(stamp(&path), Stamp::default(), "gone again");
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
            say(Lang::En, Why::Long(7)),
            "/c/imece/keys: line 7: 2 keys at most on a side; the keys you had stay"
        );
        assert_eq!(
            say(Lang::En, Why::Starts(8, key("g"), 2)),
            "/c/imece/keys: line 8: g is given alone and as the first of two keys (line 2); the keys you had stay"
        );
        assert_eq!(
            say(Lang::Tr, Why::Long(7)),
            "/c/imece/keys: satır 7: bir yanda en çok 2 tuş; elindeki tuşlar kalıyor"
        );
        assert_eq!(
            say(Lang::Tr, Why::Starts(8, key("g"), 2)),
            "/c/imece/keys: satır 8: g hem tek başına hem iki tuşun ilki olarak verilmiş (2. satır); elindeki tuşlar kalıyor"
        );
        assert_eq!(
            say(Lang::En, Why::Twice(9, ks("gt"), 3)),
            "/c/imece/keys: line 9: gt is given on line 3 already; the keys you had stay"
        );
        assert_eq!(
            say(Lang::En, Why::Twice(5, ks("^n"), 1)),
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
            say(Lang::Tr, Why::Twice(5, ks("^n"), 1)),
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
        let m2 = map("gg = j\nx = G");
        assert_eq!(
            taken(Lang::En, path, &m2),
            "/c/imece/keys: keys taken (2); nothing does gg x now"
        );
        assert_eq!(
            taken(Lang::Tr, path, &m),
            "/c/imece/keys: tuşlar alındı (3); artık hiçbir tuş j k işini yapmıyor"
        );
    }
}
