//! Where imece keeps its state, and the screen choices the person made
//! (the side panel's width, the theme) and their primary vendor, the one
//! that leads without asking. The choices live in the store's `meta`
//! table ([`crate::store::settings`]), read before the store is taken and
//! written from a demo too; a `screen` file from before 2026-10-30 moves
//! into it the first time it is seen.

use std::fs;
use std::path::{Path, PathBuf};

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

/// The side panel's width in percent of the screen.
pub const SIDE_DEFAULT: u16 = 38;
pub const SIDE_MIN: u16 = 20;
pub const SIDE_MAX: u16 = 70;

/// The settings' keys in the store.
const SIDE: &str = "side";
const THEME: &str = "theme";
const LEAD: &str = "lead";

/// The person's choices; a missing or bad value is `None`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Prefs {
    pub side: Option<u16>,
    pub theme: Option<String>,
    /// The vendor that leads without being asked for (`claude`).
    pub lead: Option<String>,
}

impl Prefs {
    /// The values as kept, checked: nonsense reads as nothing.
    fn checked(side: Option<String>, theme: Option<String>, lead: Option<String>) -> Prefs {
        Prefs {
            side: side
                .and_then(|v| v.trim().parse().ok())
                .filter(|v| (SIDE_MIN..=SIDE_MAX).contains(v)),
            theme: theme.map(|t| t.trim().to_string()).filter(|t| is_theme(t)),
            lead: lead
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_lowercase())),
        }
    }

    /// The `screen` file's lines, `key=value`.
    fn parse(text: &str) -> Prefs {
        let value = |key: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(key))
                .map(|v| v.to_string())
        };
        Prefs::checked(value("side="), value("theme="), value("lead="))
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
        let side = p.side.map(|s| s.to_string());
        let set: Vec<(&str, &str)> = [
            (SIDE, side.as_deref()),
            (THEME, p.theme.as_deref()),
            (LEAD, p.lead.as_deref()),
        ]
        .into_iter()
        .filter_map(|(k, v)| v.map(|v| (k, v)))
        .collect();
        if store::set_settings(&file(dir), &set, &[]).is_ok() {
            let _ = fs::remove_file(legacy(dir));
        }
    }
    match store::settings(&file(dir), &[SIDE, THEME, LEAD]) {
        Ok(mut v) => {
            let lead = v.pop().flatten();
            let theme = v.pop().flatten();
            Prefs::checked(v.pop().flatten(), theme, lead)
        }
        Err(_) => Prefs::default(),
    }
}

/// Saves the choices in `dir`; no primary vendor forgets the saved one.
pub fn save(dir: &Path, side: u16, theme: &str, lead: Option<&str>) -> Result<(), String> {
    let side = side.to_string();
    let mut set = vec![(SIDE, side.as_str()), (THEME, theme)];
    let mut unset = Vec::new();
    match lead {
        Some(l) => set.push((LEAD, l)),
        None => unset.push(LEAD),
    }
    store::set_settings(&file(dir), &set, &unset)
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
        save(&dir, 46, "light", None).unwrap();
        assert_eq!(
            load(&dir),
            Prefs {
                side: Some(46),
                theme: Some("light".into()),
                lead: None,
            }
        );
        let mode = fs::metadata(file(&dir)).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "the person's alone");
        save(&dir, 46, "light", Some("claude")).unwrap();
        assert_eq!(load(&dir).lead.as_deref(), Some("claude"));
        save(&dir, 46, "light", None).unwrap();
        assert_eq!(load(&dir).lead, None, "forgotten when saved without");
        let set = |pairs: &[(&str, &str)]| store::set_settings(&file(&dir), pairs, &[]).unwrap();
        set(&[("side", "99"), ("theme", "Neon 1"), ("lead", "Claude Code")]);
        assert_eq!(load(&dir), Prefs::default(), "out of range, not a name");
        set(&[
            ("side", "20"),
            ("theme", "gruvbox-dark-medium"),
            ("lead", "codex"),
        ]);
        assert_eq!(
            load(&dir),
            Prefs {
                side: Some(20),
                theme: Some("gruvbox-dark-medium".into()),
                lead: Some("codex".into()),
            }
        );
        set(&[("side", "70"), ("theme", "/home/me/Dracula.conf")]);
        let p = load(&dir);
        assert_eq!(p.side, Some(70));
        assert_eq!(p.theme.as_deref(), Some("/home/me/Dracula.conf"));
        set(&[("side", "71"), ("theme", ""), ("lead", "")]);
        assert_eq!(load(&dir), Prefs::default());
        set(&[("side", "19"), ("side", "x")]);
        assert_eq!(load(&dir).side, None);
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
                side: Some(52),
                theme: Some("dracula".into()),
                lead: Some("gemini".into()),
            }
        );
        assert!(!legacy(&dir).exists(), "moved, so gone");
        assert!(file(&dir).exists());
        assert_eq!(load(&dir).side, Some(52), "from the store now");
        // A file of nonsense moves nothing in and goes too; what the store
        // has stays.
        fs::write(legacy(&dir), "side=x\n").unwrap();
        assert_eq!(load(&dir).side, Some(52));
        assert!(!legacy(&dir).exists());
        // A partial file brings only what it has.
        fs::write(legacy(&dir), "theme=light\n").unwrap();
        let p = load(&dir);
        assert_eq!(
            (p.side, p.theme.as_deref(), p.lead.as_deref()),
            (Some(52), Some("light"), Some("gemini"))
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_choices_share_the_file_with_the_world_and_neither_disturbs_the_other() {
        let dir = dir("shared");
        save(&dir, 30, "mono", Some("codex")).unwrap();
        // A file the settings made is a new world to the store.
        let mut st = store::Store::open(&file(&dir)).unwrap();
        assert!(st.load(10).unwrap().is_none());
        assert_eq!(load(&dir).side, Some(30), "while the store holds the file");
        save(&dir, 31, "mono", None).unwrap();
        assert_eq!(load(&dir).side, Some(31), "written beside it");
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
}
