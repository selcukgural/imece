//! What imece remembers between runs: where it keeps its state, and the
//! screen choices the person made (for now the side panel's width).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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

/// What the screen file holds; a missing or bad value is `None`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Prefs {
    pub side: Option<u16>,
    pub theme: Option<String>,
}

fn file(dir: &Path) -> PathBuf {
    dir.join("screen")
}

/// The choices saved in `dir`.
pub fn load(dir: &Path) -> Prefs {
    let text = fs::read_to_string(file(dir)).unwrap_or_default();
    let value = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key))
            .map(|v| v.trim().to_string())
    };
    Prefs {
        side: value("side=")
            .and_then(|v| v.parse().ok())
            .filter(|v| (SIDE_MIN..=SIDE_MAX).contains(v)),
        theme: value("theme=").filter(|t| t.chars().all(|c| c.is_ascii_lowercase())),
    }
}

/// Saves the choices in `dir`; a reader never sees half a file.
pub fn save(dir: &Path, side: u16, theme: &str) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let tmp = dir.join("screen.tmp");
    fs::write(&tmp, format!("side={side}\ntheme={theme}\n"))?;
    fs::rename(tmp, file(dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_choices_come_back_and_nonsense_is_ignored() {
        let dir = std::env::temp_dir().join(format!("imece-prefs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(load(&dir), Prefs::default());
        save(&dir, 46, "light").unwrap();
        assert_eq!(
            load(&dir),
            Prefs {
                side: Some(46),
                theme: Some("light".into())
            }
        );
        fs::write(file(&dir), "side=99\ntheme=Neon 1\n").unwrap();
        assert_eq!(load(&dir), Prefs::default(), "out of range, not a name");
        fs::write(file(&dir), "side=x\n").unwrap();
        assert_eq!(load(&dir), Prefs::default());
        fs::remove_dir_all(&dir).unwrap();
    }
}
