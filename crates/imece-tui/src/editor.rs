//! `^x ^e` and `:edit`: the command line's draft in the person's own
//! editor, as bash and zsh do (record 95). The draft is written to a file
//! of its own (created new, 0600, in a directory of imece's state only the
//! person may read), the screen steps aside as for a `!` command (record
//! 13), `$VISUAL`, else `$EDITOR`, else `vi` runs on it in the person's
//! terminal, and what the file holds then comes back into the command
//! line, never sent. An editor that cannot start, fails (a non-zero exit,
//! as vim's `:cq`), writes more than [`MAX`] or writes what is not text
//! keeps the draft as it was. The file is removed whatever happened.
//! Like `!`, nothing here reaches an agent: the text does only when the
//! person sends it.

use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::model::Ended;
use crate::screen::Screen;
use crate::shell;

/// The most an editor may leave in the file: 1 MiB, far over any message
/// a person writes, and short of a file that would stall the line.
pub const MAX: u64 = 1024 * 1024;

/// The editor when the person named none: `vi`, as git and crontab do.
pub const FALLBACK: &str = "vi";

/// Why the draft came back as it was.
#[derive(Debug)]
pub enum Why {
    /// The editor ended with a code other than 0, or by a signal.
    Ended(Ended),
    /// The file has more than [`MAX`] bytes in it.
    Long,
    /// The file is not UTF-8 text.
    NotText,
    /// The file or the editor could not be had.
    Io(io::Error),
}

impl From<io::Error> for Why {
    fn from(e: io::Error) -> Why {
        Why::Io(e)
    }
}

/// The editor the person chose: `$VISUAL`, else `$EDITOR`, else `vi`; one
/// set to nothing but blanks counts as not set.
pub fn chosen(visual: Option<&str>, editor: Option<&str>) -> String {
    [visual, editor]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|e| !e.is_empty())
        .unwrap_or(FALLBACK)
        .to_string()
}

/// The editor of this environment, as [`chosen`] picks it.
pub fn of_env() -> String {
    let var = |name| std::env::var(name).ok();
    chosen(var("VISUAL").as_deref(), var("EDITOR").as_deref())
}

/// The whole round on the person's terminal: the screen steps aside, the
/// editor runs on the draft in `cwd`, and the text written is returned.
/// When the draft comes back as it was, `back` (worded by the caller
/// from why) is shown under what the editor left on the screen, its own
/// complaint among it, until a key; then the screen comes back, whatever
/// happened. (While writing, the bottom line shows the line, not a
/// flash: said there, why would not be seen.)
pub fn run(
    terminal: &mut Screen,
    state: &Path,
    cwd: &Path,
    editor: &str,
    draft: &str,
    back: impl FnOnce(&Why) -> String,
) -> Result<String, Why> {
    shell::step_aside()?;
    let edited = edit(&state.join(DRAFTS), cwd, editor, draft);
    if let Err(why) = &edited {
        let mut out = io::stdout();
        let shown = write!(out, "\n{}", back(why)).and_then(|()| out.flush());
        let _ = shown.and_then(|()| shell::wait_for_key());
    }
    shell::come_back(terminal)?;
    edited
}

/// The directory of imece's state the drafts are written in.
const DRAFTS: &str = "drafts";

/// Drafts written by this run so far, for a name no other has.
static WRITTEN: AtomicU32 = AtomicU32::new(0);

/// Writes `draft` to a new file in `dir`, runs `editor` on it in `cwd`
/// with the terminal imece has, and reads back what it left. The editor
/// is a line of the person's shell words (`code --wait`, `emacsclient
/// -t`), so it runs through `/bin/sh` with the file as `$1`, as git runs
/// `core.editor`: the file's name is never put into the line. ctrl-c and
/// ctrl-\ are the editor's while it runs, not imece's.
pub fn edit(dir: &Path, cwd: &Path, editor: &str, draft: &str) -> Result<String, Why> {
    let file = Draft::new(dir, draft)?;
    let mut child = Command::new("/bin/sh");
    child
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("imece-edit")
        .arg(&file.0)
        .current_dir(cwd);
    let quiet = [libc::SIGINT, libc::SIGQUIT];
    // SAFETY: `signal` only swaps the handlers, put back below. SIG_IGN is
    // inherited across exec, so the child resets them in `pre_exec`, with
    // `signal` alone, which is async-signal-safe.
    let was = quiet.map(|sig| unsafe { libc::signal(sig, libc::SIG_IGN) });
    unsafe {
        child.pre_exec(move || {
            for sig in quiet {
                libc::signal(sig, libc::SIG_DFL);
            }
            Ok(())
        });
    }
    let status = child.status();
    for (sig, was) in quiet.into_iter().zip(was) {
        // SAFETY: the handler that was there.
        unsafe { libc::signal(sig, was) };
    }
    let status = status?;
    if !status.success() {
        return Err(Why::Ended(shell::ended(status)));
    }
    read_back(&file.0)
}

/// What the editor left in the file: at most [`MAX`] bytes of UTF-8,
/// the line ends an editor puts at the file's end taken off.
fn read_back(path: &Path) -> Result<String, Why> {
    let mut bytes = Vec::new();
    File::open(path)?.take(MAX + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX {
        return Err(Why::Long);
    }
    let text = String::from_utf8(bytes).map_err(|_| Why::NotText)?;
    Ok(text.trim_end_matches(['\n', '\r']).to_string())
}

/// The draft as the file holds it: a line end after text, as editors
/// write a file (vi says `[noeol]` of one without), none on nothing.
fn with_end(draft: &str) -> String {
    if draft.is_empty() || draft.ends_with('\n') {
        draft.to_string()
    } else {
        format!("{draft}\n")
    }
}

/// A draft's file, removed when it goes.
struct Draft(PathBuf);

impl Draft {
    /// A new file in `dir` (made 0700 when it is not there) that only the
    /// person may read, with `draft` in it. `create_new` never opens what
    /// is there already, a link among them.
    fn new(dir: &Path, draft: &str) -> io::Result<Draft> {
        DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
        let n = WRITTEN.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!("draft-{}-{n}.md", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        let file_of = Draft(path);
        file.write_all(with_end(draft).as_bytes())?;
        Ok(file_of)
    }
}

impl Drop for Draft {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;
    use crate::shell::tests::one_at_a_time;

    /// A directory of its own for one test's drafts.
    fn drafts(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("imece-drafts-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn left_in(dir: &Path) -> usize {
        std::fs::read_dir(dir).map_or(0, |d| d.count())
    }

    #[test]
    fn visual_comes_before_editor_and_vi_after_both() {
        assert_eq!(chosen(Some("nano"), Some("vim")), "nano");
        assert_eq!(chosen(None, Some(" code --wait ")), "code --wait");
        assert_eq!(chosen(Some("  "), Some("vim")), "vim");
        assert_eq!(chosen(Some(""), None), "vi");
        assert_eq!(chosen(None, None), FALLBACK);
        assert_eq!(FALLBACK, "vi");
        let var = |name| std::env::var(name).ok();
        let here = chosen(var("VISUAL").as_deref(), var("EDITOR").as_deref());
        assert_eq!(of_env(), here, "this environment's");
    }

    #[test]
    fn a_draft_ends_its_last_line_and_nothing_stays_nothing() {
        assert_eq!(with_end("ask the leader"), "ask the leader\n");
        assert_eq!(with_end("two\nlines\n"), "two\nlines\n");
        assert_eq!(with_end(""), "");
    }

    #[test]
    fn what_the_editor_writes_comes_back_without_its_last_line_ends() {
        let _turn = one_at_a_time();
        let dir = drafts("writes");
        let cwd = std::env::temp_dir();
        let back = edit(&dir, &cwd, "printf 'first\\nsecond\\r\\n\\n' >", "old").unwrap();
        assert_eq!(back, "first\nsecond");
        // An editor that only looks gives the draft back as it was.
        assert_eq!(edit(&dir, &cwd, "true", "as it was").unwrap(), "as it was");
        assert_eq!(edit(&dir, &cwd, "true", "").unwrap(), "");
        assert_eq!(left_in(&dir), 0, "every draft's file is removed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_file_is_new_private_holds_the_draft_and_the_editor_runs_where_asked() {
        let _turn = one_at_a_time();
        let dir = drafts("private");
        let cwd = std::env::temp_dir().canonicalize().unwrap();
        let seen = dir.with_extension("seen");
        let editor = format!(
            "f() {{ [ \"$(ls -l \"$1\" | cut -c1-10)\" = -rw------- ] && grep -qx 'two words' \"$1\" \
             && printf '%s' \"$1\" > '{}' && pwd -P > \"$1\"; }}; f",
            seen.display()
        );
        let back = edit(&dir, &cwd, &editor, "two words").unwrap();
        assert_eq!(back, cwd.display().to_string());
        let path = PathBuf::from(std::fs::read_to_string(&seen).unwrap());
        assert_eq!(path.parent(), Some(dir.as_path()));
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("md"));
        assert!(!path.exists(), "the draft's file is removed");
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        let _ = std::fs::remove_file(&seen);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_editor_that_fails_or_is_stopped_says_how_and_leaves_no_file() {
        let _turn = one_at_a_time();
        let dir = drafts("fails");
        let cwd = std::env::temp_dir();
        let why = edit(&dir, &cwd, "printf 'lost' > \"$1\"; exit 3; :", "kept");
        assert!(matches!(why, Err(Why::Ended(Ended::Exit(3)))), "{why:?}");
        let why = edit(&dir, &cwd, "kill -TERM $$; :", "kept");
        assert!(
            matches!(why, Err(Why::Ended(Ended::Signal(libc::SIGTERM)))),
            "{why:?}"
        );
        let why = edit(&dir, &cwd, "/nonexistent/editor", "kept");
        assert!(matches!(why, Err(Why::Ended(Ended::Exit(127)))), "{why:?}");
        assert_eq!(left_in(&dir), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_draft_the_state_cannot_hold_is_an_error() {
        let _turn = one_at_a_time();
        let file = drafts("not-a-dir");
        std::fs::write(&file, "a file where the directory would be").unwrap();
        let why = edit(&file, &std::env::temp_dir(), "true", "kept");
        assert!(matches!(why, Err(Why::Io(_))), "{why:?}");
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn what_is_too_long_or_not_text_keeps_the_draft() {
        let _turn = one_at_a_time();
        let dir = drafts("long");
        let cwd = std::env::temp_dir();
        let fill = |n: u64| format!("head -c {n} /dev/zero | tr '\\0' a >");
        let back = edit(&dir, &cwd, &fill(MAX), "kept").unwrap();
        assert_eq!(back.len() as u64, MAX, "exactly the most is taken");
        let why = edit(&dir, &cwd, &fill(MAX + 1), "kept");
        assert!(matches!(why, Err(Why::Long)), "{why:?}");
        let why = edit(&dir, &cwd, "printf 'a\\377' >", "kept");
        assert!(matches!(why, Err(Why::NotText)), "{why:?}");
        assert_eq!(MAX, 1_048_576);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ctrl_c_and_ctrl_backslash_reach_the_editor_and_not_imece() {
        // As the terminal would: the signal reaches imece and the editor
        // alike. This process must shrug both off; the editor dies of one.
        let _turn = one_at_a_time();
        let dir = drafts("ctrl-c");
        let cwd = std::env::temp_dir();
        for sig in [libc::SIGINT, libc::SIGQUIT] {
            let editor = format!("kill -{sig} $PPID; kill -{sig} $$; :");
            let why = edit(&dir, &cwd, &editor, "kept");
            assert!(
                matches!(why, Err(Why::Ended(Ended::Signal(s))) if s == sig),
                "{why:?}"
            );
            // SAFETY: reading the disposition and putting it straight back.
            let now = unsafe { libc::signal(sig, libc::SIG_IGN) };
            unsafe { libc::signal(sig, now) };
            assert_ne!(now, libc::SIG_IGN, "signal {sig} stayed ignored");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
