//! Where imece keeps its state, and the screen choices the person made
//! (the side panel's width, the theme, how they are told in another
//! window). The choices live in the store's `meta` table ([`crate::store::settings`]), read before the store is taken and
//! written from a demo too; a `screen` file from before 2026-10-30 moves
//! into it the first time it is seen.

use std::fs;
use std::path::{Path, PathBuf};

use crate::away::Away;
use crate::health::Health;
use crate::motion::Motion;
use crate::notify::Notify;
use crate::store;

/// `$XDG_STATE_HOME/imece`, or `~/.local/state/imece`.
pub fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".local/state")))
        .unwrap_or_else(std::env::temp_dir)
        .join("imece")
}

/// The settings' keys in the store. (`side`, the salon's panel width,
/// is no longer read: the salon went with record 125.)
const THEME: &str = "theme";
const NOTIFY: &str = "notify";
const MOTION: &str = "motion";
const AWAY: &str = "away";
const STORY: &str = "story";
const REVIEW: &str = "review";
const HEALTH: &str = "health";
/// The newest version whose "what's new" a start showed (or that started
/// first): the next start of a newer one shows what is new since.
const SEEN: &str = "seen";
/// The text of the last key file that was taken whole (`keys.rs`): a
/// start whose file is refused has these keys still.
const KEYS: &str = "keys";

/// The person's choices; a missing or bad value is `None`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Prefs {
    pub theme: Option<String>,
    /// How the person is told in another window (`:notify`).
    pub notify: Option<Notify>,
    /// The version that started last (`0.2.0`).
    pub seen: Option<String>,
    /// How much moves on the screen (`:motion`).
    pub motion: Option<Motion>,
    /// How long away opens the digest by itself (`:away`).
    pub away: Option<Away>,
    /// Whether the side panel tells the story (`J`, `:story`).
    pub story: Option<bool>,
    /// Whether every change gets a review as it comes (`:review`).
    pub review: Option<bool>,
    /// When a leader hears of its team's health (`:health`).
    pub health: Option<Health>,
}

impl Prefs {
    /// The values as kept, checked: nonsense reads as nothing.
    fn checked(theme: Option<String>, notify: Option<String>, seen: Option<String>) -> Prefs {
        Prefs {
            theme: theme.map(|t| t.trim().to_string()).filter(|t| is_theme(t)),
            notify: notify.and_then(|n| Notify::parse(&n)),
            seen: seen.filter(|v| crate::welcome::parse_version(v).is_some()),
            motion: None,
            away: None,
            story: None,
            review: None,
            health: None,
        }
    }

    /// The `screen` file's lines, `key=value`.
    fn parse(text: &str) -> Prefs {
        let value = |key: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(key))
                .map(|v| v.to_string())
        };
        Prefs::checked(value("theme="), None, None)
    }
}

/// A built-in theme's name (`gruvbox-dark-medium`) or a colour file's
/// absolute path.
fn is_theme(t: &str) -> bool {
    let name = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
    (!t.is_empty() && t.chars().all(name)) || t.starts_with('/')
}

/// The store in `dir`.
fn file(dir: &Path) -> PathBuf {
    dir.join(store::FILE)
}

/// The `screen` file imece kept the choices in before the store did.
fn legacy(dir: &Path) -> PathBuf {
    dir.join("screen")
}

/// The choices saved in `dir`. A `screen` file still there moves into the
/// store, and goes once it is in.
pub fn load(dir: &Path) -> Prefs {
    if let Ok(text) = fs::read_to_string(legacy(dir)) {
        let p = Prefs::parse(&text);
        let set: Vec<(&str, &str)> = [(THEME, p.theme.as_deref())]
            .into_iter()
            .filter_map(|(k, v)| v.map(|v| (k, v)))
            .collect();
        if set.is_empty() || store::set_settings(&file(dir), &set, &[]).is_ok() {
            let _ = fs::remove_file(legacy(dir));
        }
    }
    match store::settings(
        &file(dir),
        &[THEME, NOTIFY, SEEN, MOTION, AWAY, STORY, REVIEW, HEALTH],
    ) {
        Ok(mut v) => {
            let health = v.pop().flatten().and_then(|h| Health::parse(&h));
            let mut on_off = || {
                v.pop().flatten().and_then(|s| match s.as_str() {
                    "on" => Some(true),
                    "off" => Some(false),
                    _ => None,
                })
            };
            let review = on_off();
            let story = on_off();
            let away = v.pop().flatten().and_then(|a| Away::parse(&a));
            let motion = v.pop().flatten().and_then(|m| Motion::parse(&m));
            let seen = v.pop().flatten();
            let notify = v.pop().flatten();
            let theme = v.pop().flatten();
            Prefs {
                motion,
                away,
                story,
                review,
                health,
                ..Prefs::checked(theme, notify, seen)
            }
        }
        Err(_) => Prefs::default(),
    }
}

/// Saves the choices in `dir`.
pub fn save(dir: &Path, theme: &str, notify: Notify) -> Result<(), String> {
    let set = [(THEME, theme), (NOTIFY, notify.name())];
    store::set_settings(&file(dir), &set, &[])
}

/// The last good key file's text kept in `dir`, if any.
pub fn keys(dir: &Path) -> Option<String> {
    store::settings(&file(dir), &[KEYS])
        .ok()
        .and_then(|mut v| v.pop().flatten())
}

/// Keeps `text` as the last good key file in `dir`; none forgets it.
pub fn keep_keys(dir: &Path, text: Option<&str>) -> Result<(), String> {
    match text {
        Some(text) => store::set_settings(&file(dir), &[(KEYS, text)], &[]),
        None => store::set_settings(&file(dir), &[], &[KEYS]),
    }
}

/// Keeps how much moves on the screen in `dir`.
pub fn keep_motion(dir: &Path, motion: Motion) -> Result<(), String> {
    store::set_settings(&file(dir), &[(MOTION, motion.name())], &[])
}

/// Keeps whether the side panel tells the story in `dir`.
pub fn keep_story(dir: &Path, story: bool) -> Result<(), String> {
    let on = if story { "on" } else { "off" };
    store::set_settings(&file(dir), &[(STORY, on)], &[])
}

/// Keeps whether every change gets a review as it comes in `dir`.
pub fn keep_review(dir: &Path, review: bool) -> Result<(), String> {
    let on = if review { "on" } else { "off" };
    store::set_settings(&file(dir), &[(REVIEW, on)], &[])
}

/// Keeps how long away opens the digest in `dir`.
pub fn keep_away(dir: &Path, away: Away) -> Result<(), String> {
    store::set_settings(&file(dir), &[(AWAY, &away.name())], &[])
}

/// Keeps when a leader hears of its team's health in `dir`.
pub fn keep_health(dir: &Path, health: Health) -> Result<(), String> {
    store::set_settings(&file(dir), &[(HEALTH, &health.name())], &[])
}

/// Keeps `version` as the one that started last in `dir`.
pub fn seen(dir: &Path, version: &str) -> Result<(), String> {
    store::set_settings(&file(dir), &[(SEEN, version)], &[])
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("imece-prefs-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn the_choices_come_back_and_nonsense_is_ignored() {
        let dir = dir("choices");
        assert_eq!(load(&dir), Prefs::default(), "nothing saved, no file made");
        assert!(!file(&dir).exists());
        save(&dir, "light", Notify::Off).unwrap();
        assert_eq!(
            load(&dir),
            Prefs {
                theme: Some("light".into()),
                notify: Some(Notify::Off),
                seen: None,
                motion: None,
                away: None,
                story: None,
                review: None,
                health: None,
            }
        );
        let mode = fs::metadata(file(&dir)).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "the person's alone");
        save(&dir, "light", Notify::Bell).unwrap();
        assert_eq!(load(&dir).notify, Some(Notify::Bell));
        save(&dir, "light", Notify::Text).unwrap();
        assert_eq!(load(&dir).notify, Some(Notify::Text));
        let set = |pairs: &[(&str, &str)]| store::set_settings(&file(&dir), pairs, &[]).unwrap();
        set(&[("side", "99"), ("theme", "Neon 1"), ("notify", "beep")]);
        assert_eq!(load(&dir), Prefs::default(), "out of range, not a name");
        set(&[
            ("side", "20"),
            ("theme", "gruvbox-dark-medium"),
            ("lead", "codex"), // an older run's choice: read by nobody
            ("notify", "text"),
        ]);
        assert_eq!(
            load(&dir),
            Prefs {
                theme: Some("gruvbox-dark-medium".into()),
                notify: Some(Notify::Text),
                seen: None,
                motion: None,
                away: None,
                story: None,
                review: None,
                health: None,
            }
        );
        set(&[("side", "70"), ("theme", "/home/me/Dracula.conf")]);
        let p = load(&dir);
        assert_eq!(p.theme.as_deref(), Some("/home/me/Dracula.conf"));
        set(&[("side", "71"), ("theme", ""), ("notify", "")]);
        assert_eq!(load(&dir), Prefs::default());
        set(&[("side", "19"), ("side", "x")]);
        // The version that started last, and only a version.
        seen(&dir, "0.2.0").unwrap();
        assert_eq!(load(&dir).seen.as_deref(), Some("0.2.0"));
        save(&dir, "light", Notify::Off).unwrap();
        assert_eq!(load(&dir).seen.as_deref(), Some("0.2.0"), "kept by a save");
        set(&[("seen", "v1")]);
        assert_eq!(load(&dir).seen, None);
        // How much moves, on its own, kept by a save of the rest.
        keep_motion(&dir, Motion::Full).unwrap();
        assert_eq!(load(&dir).motion, Some(Motion::Full));
        save(&dir, "light", Notify::Off).unwrap();
        assert_eq!(load(&dir).motion, Some(Motion::Full), "kept by a save");
        keep_motion(&dir, Motion::Off).unwrap();
        assert_eq!(load(&dir).motion, Some(Motion::Off));
        set(&[("motion", "wild")]);
        assert_eq!(load(&dir).motion, None);
        // Whether the side panel tells the story, on its own too.
        assert_eq!(load(&dir).story, None);
        keep_story(&dir, false).unwrap();
        assert_eq!(load(&dir).story, Some(false));
        save(&dir, "light", Notify::Off).unwrap();
        assert_eq!(load(&dir).story, Some(false), "kept by a save");
        keep_story(&dir, true).unwrap();
        assert_eq!(load(&dir).story, Some(true));
        set(&[("story", "yes")]);
        assert_eq!(load(&dir).story, None);
        // Whether every change gets a review, on its own too, apart from
        // the story read just before it.
        assert_eq!(load(&dir).review, None);
        keep_review(&dir, true).unwrap();
        keep_story(&dir, false).unwrap();
        assert_eq!(load(&dir).review, Some(true));
        assert_eq!(load(&dir).story, Some(false));
        save(&dir, "light", Notify::Off).unwrap();
        assert_eq!(load(&dir).review, Some(true), "kept by a save");
        keep_review(&dir, false).unwrap();
        assert_eq!(load(&dir).review, Some(false));
        assert_eq!(load(&dir).story, Some(false), "nothing else written");
        set(&[("review", "yes")]);
        assert_eq!(load(&dir).review, None);
        // How long away, on its own too, kept by a save of the rest.
        keep_away(&dir, Away::parse("1h").unwrap()).unwrap();
        assert_eq!(load(&dir).away, Away::parse("60m"));
        save(&dir, "light", Notify::Off).unwrap();
        keep_motion(&dir, Motion::Calm).unwrap();
        assert_eq!(load(&dir).away, Away::parse("1h"), "kept by the others");
        keep_away(&dir, Away::parse("off").unwrap()).unwrap();
        assert_eq!(load(&dir).away, Away::parse("off"));
        // When a leader hears of its team's health, the same way.
        let health = Health::parse("stall 20m idle off loop 5").unwrap();
        keep_health(&dir, health).unwrap();
        assert_eq!(load(&dir).health, Some(health));
        assert_eq!(load(&dir).away, Away::parse("off"), "the others kept");
        set(&[("health", "stall 0m")]);
        assert_eq!(load(&dir).health, None, "nonsense reads as nothing");
        assert_eq!(
            load(&dir).motion,
            Some(Motion::Calm),
            "nothing else written"
        );
        set(&[("away", "3s")]);
        assert_eq!(load(&dir).away, None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_screen_file_moves_into_the_store_once() {
        let dir = dir("legacy");
        fs::create_dir_all(&dir).unwrap();
        fs::write(legacy(&dir), "side=52\ntheme=dracula\nlead=gemini\n").unwrap();
        assert_eq!(
            load(&dir),
            Prefs {
                theme: Some("dracula".into()),
                notify: None,
                seen: None,
                motion: None,
                away: None,
                story: None,
                review: None,
                health: None,
            }
        );
        assert!(!legacy(&dir).exists(), "moved, so gone");
        assert!(file(&dir).exists());
        // A file of nonsense moves nothing in and goes too; what the store
        // has stays.
        fs::write(legacy(&dir), "side=x\n").unwrap();
        assert_eq!(
            load(&dir).theme.as_deref(),
            Some("dracula"),
            "what the store has"
        );
        assert!(!legacy(&dir).exists());
        // A partial file brings only what it has.
        fs::write(legacy(&dir), "theme=light\n").unwrap();
        let p = load(&dir);
        assert_eq!(p.theme.as_deref(), Some("light"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_choices_share_the_file_with_the_world_and_neither_disturbs_the_other() {
        let dir = dir("shared");
        save(&dir, "mono", Notify::Off).unwrap();
        // A file the settings made is a new world to the store.
        let mut st = store::Store::open(&file(&dir)).unwrap();
        assert!(st.load(10).unwrap().is_none());
        save(&dir, "mono", Notify::Bell).unwrap();
        // The world's keys are not settings.
        for key in ["schema_version", "started_at", "budget", "saved_at"] {
            let e = store::set_settings(&file(&dir), &[(key, "1")], &[]).unwrap_err();
            assert!(e.contains(key), "{e}");
            let e = store::set_settings(&file(&dir), &[], &[key]).unwrap_err();
            assert!(e.contains(key), "{e}");
        }
        drop(st);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_last_good_key_file_is_kept_and_forgotten() {
        let dir = dir("keys");
        assert_eq!(keys(&dir), None, "nothing kept, no file made");
        assert!(!file(&dir).exists());
        keep_keys(&dir, Some("^n = j\n")).unwrap();
        assert_eq!(keys(&dir).as_deref(), Some("^n = j\n"));
        save(&dir, "dark", Notify::Off).unwrap();
        assert_eq!(
            keys(&dir).as_deref(),
            Some("^n = j\n"),
            "beside the choices"
        );
        keep_keys(&dir, Some("^p = k")).unwrap();
        assert_eq!(keys(&dir).as_deref(), Some("^p = k"));
        keep_keys(&dir, None).unwrap();
        assert_eq!(keys(&dir), None);
        fs::remove_dir_all(&dir).unwrap();
    }
}
