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

fn file(dir: &Path) -> PathBuf {
    dir.join("screen")
}

/// The width saved in `dir`, if there is a valid one.
pub fn side(dir: &Path) -> Option<u16> {
    let text = fs::read_to_string(file(dir)).ok()?;
    text.lines()
        .find_map(|l| l.strip_prefix("side="))
        .and_then(|v| v.trim().parse().ok())
        .filter(|v| (SIDE_MIN..=SIDE_MAX).contains(v))
}

/// Saves the width in `dir`; a reader never sees half a file.
pub fn save_side(dir: &Path, side: u16) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let tmp = dir.join("screen.tmp");
    fs::write(&tmp, format!("side={side}\n"))?;
    fs::rename(tmp, file(dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_side_width_comes_back_and_nonsense_is_ignored() {
        let dir = std::env::temp_dir().join(format!("imece-prefs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(side(&dir), None);
        save_side(&dir, 46).unwrap();
        assert_eq!(side(&dir), Some(46));
        fs::write(file(&dir), "side=99\n").unwrap();
        assert_eq!(side(&dir), None, "out of range");
        fs::write(file(&dir), "side=x\n").unwrap();
        assert_eq!(side(&dir), None);
        fs::remove_dir_all(&dir).unwrap();
    }
}
