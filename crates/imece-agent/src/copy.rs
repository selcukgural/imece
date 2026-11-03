//! Files git ignores that a worker's worktree needs all the same (`.env`,
//! `node_modules`, `target/`), copied from the person's checkout into it.
//! Only what the person listed, path by path: literal paths inside the
//! checkout that git ignores, never a link, never over what the worktree
//! has, and no more bytes than a budget. On macOS a copy is an APFS clone
//! (`cp -c`), elsewhere a reflink where the file system has them.
//! Beside that list, what the checkout's own [`INCLUDE`] file names.

use std::collections::HashSet;
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Component, Path};
use std::process::Stdio;
use std::time::Duration;

use crate::child;

/// The most files and directories one listed path may hold: walking more
/// to learn its size is already too slow for a session's start.
pub const MAX_ENTRIES: usize = 100_000;

/// The longest path the person may list, in bytes.
pub const MAX_PATH: usize = 200;

/// What became of one listed path at a worktree's start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Copied {
    /// Copied, with its size in bytes.
    Copied { path: String, bytes: u64 },
    /// The worktree has it already: left as it is.
    There { path: String },
    /// Not copied, and why.
    Skipped { path: String, why: String },
}

/// Whether `path` is one the person may list: relative, plain (no `.`,
/// `..`, glob characters, control characters or backslashes), not in
/// `.git`, at most [`MAX_PATH`] bytes. Why not.
pub fn literal(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("no path given".into());
    }
    if path.len() > MAX_PATH {
        return Err(format!("longer than {MAX_PATH} bytes"));
    }
    if path.chars().any(|c| c.is_control() || "*?[]\\".contains(c)) {
        return Err("a literal path only: no glob, backslash or control character".into());
    }
    let p = Path::new(path);
    for c in p.components() {
        match c {
            Component::Normal(n) if n == ".git" => {
                return Err("git's own directory is not copied".into());
            }
            Component::Normal(_) => {}
            _ => return Err("a relative path inside the checkout, without . or ..".into()),
        }
    }
    Ok(())
}

/// Whether `path` may be copied from the checkout `repo`: literal (see
/// [`literal`]), there, not a link, inside the checkout once its
/// directories are resolved, and ignored by git (a tracked file is in
/// the worktree already). Why not.
pub fn checkable(repo: &Path, path: &str) -> Result<(), String> {
    literal(path)?;
    let src = repo.join(path);
    let meta = std::fs::symlink_metadata(&src).map_err(|_| "not in the checkout".to_string())?;
    if meta.file_type().is_symlink() {
        return Err("a link: what it points to is not copied".into());
    }
    let inside = (src.canonicalize().ok())
        .zip(repo.canonicalize().ok())
        .is_some_and(|(s, r)| s.starts_with(&r) && s != r);
    if !inside {
        return Err("outside the checkout".into());
    }
    match ignored(repo, path)? {
        true => Ok(()),
        false => Err("git does not ignore it: a tracked file is in the worktree already".into()),
    }
}

/// Whether git ignores `path` in the checkout `repo`.
fn ignored(repo: &Path, path: &str) -> Result<bool, String> {
    let status = child::command("git", repo, &[])
        .args(["check-ignore", "-q", "--", path])
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("git: {e}"))?;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err("git could not say whether it is ignored".into()),
    }
}

/// Copies each of `paths` from the checkout `repo` into the worktree
/// `tree` where the worktree does not have it, while the bytes copied stay
/// within `budget`; what became of each, in order.
pub fn into(repo: &Path, tree: &Path, paths: &[String], budget: u64) -> Vec<Copied> {
    let mut left = budget;
    let mut out = Vec::new();
    for path in paths {
        let path = path.clone();
        let dst = tree.join(&path);
        if literal(&path).is_ok() && std::fs::symlink_metadata(&dst).is_ok() {
            out.push(Copied::There { path });
            continue;
        }
        let copied = checkable(repo, &path)
            .and_then(|()| size(&repo.join(&path), left, MAX_ENTRIES))
            .and_then(|bytes| {
                clone(&repo.join(&path), &dst).map_err(|e| format!("the copy failed: {e}"))?;
                Ok(bytes)
            });
        out.push(match copied {
            Ok(bytes) => {
                left -= bytes;
                Copied::Copied { path, bytes }
            }
            Err(why) => Copied::Skipped { path, why },
        });
    }
    out
}

/// The file of a checkout that names, in gitignore's syntax, more paths
/// to copy into its worktrees (agent-deck and Claude Code's desktop app
/// read the same file).
pub const INCLUDE: &str = ".worktreeinclude";

/// The largest [`INCLUDE`] file read, in bytes.
pub const INCLUDE_MAX: u64 = 16 << 10;

/// The most paths git ignores that an [`INCLUDE`] file may name: one that
/// names more matches too much, and none of them is copied.
pub const INCLUDE_PATHS: usize = 100;

/// How long git may take to say what an [`INCLUDE`] file names.
const NAMED_WAIT: Duration = Duration::from_secs(10);

/// A part of an [`INCLUDE`] file not followed, and why: one of its lines
/// (its number from 1, its text), or the whole file (`line` none).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    pub line: Option<(usize, String)>,
    pub why: String,
}

/// What a checkout's [`INCLUDE`] file names: the paths git ignores that
/// its patterns match (a directory they match whole as itself), and what
/// of it was refused.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Included {
    pub paths: Vec<String>,
    pub refused: Vec<Refused>,
}

/// What became of the paths copied into a worktree at its start: the
/// listed ones first (`listed` of `copied`), then the ones only the
/// [`INCLUDE`] file names; and what of that file was refused.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Brought {
    pub copied: Vec<Copied>,
    pub listed: usize,
    pub refused: Vec<Refused>,
}

/// The paths the person listed and those the checkout's [`INCLUDE`] file
/// names besides, copied from the checkout `repo` into the worktree
/// `tree` as [`into`] copies, within one `budget`.
pub fn bring(repo: &Path, tree: &Path, listed: &[String], budget: u64) -> Brought {
    let named = included(repo);
    let mut paths = listed.to_vec();
    paths.extend(named.paths.into_iter().filter(|p| !listed.contains(p)));
    Brought {
        copied: into(repo, tree, &paths, budget),
        listed: listed.len(),
        refused: named.refused,
    }
}

/// Whether the checkout `repo` has an [`INCLUDE`] file (a plain one).
pub fn has_include(repo: &Path) -> bool {
    std::fs::symlink_metadata(repo.join(INCLUDE)).is_ok_and(|m| m.is_file())
}

/// What the checkout `repo`'s [`INCLUDE`] file names (nothing, and
/// nothing said, without one).
pub fn included(repo: &Path) -> Included {
    included_at_most(repo, INCLUDE_PATHS)
}

/// [`included`], with at most `most` paths named.
fn included_at_most(repo: &Path, most: usize) -> Included {
    let mut out = Included::default();
    let text = match read_include(repo) {
        Ok(Some(text)) => text,
        Ok(None) => return out,
        Err(why) => {
            out.refused.push(Refused { line: None, why });
            return out;
        }
    };
    let mut patterns = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        match pattern(line) {
            Ok(()) => patterns.push(line),
            Err(why) => out.refused.push(Refused {
                line: Some((i + 1, line.to_string())),
                why,
            }),
        }
    }
    if patterns.is_empty() {
        return out;
    }
    match named(repo, &patterns, most) {
        Ok(paths) => out.paths = paths,
        Err(why) => out.refused.push(Refused { line: None, why }),
    }
    out
}

/// The checkout `repo`'s [`INCLUDE`] file: none when it is not there;
/// refused when it is a link, not a plain file, larger than
/// [`INCLUDE_MAX`] or not text.
fn read_include(repo: &Path) -> Result<Option<String>, String> {
    let opened = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(repo.join(INCLUDE));
    let file = match opened {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) if e.raw_os_error() == Some(libc::ELOOP) => {
            return Err("a link: not followed".into());
        }
        Err(e) => return Err(format!("could not be read: {e}")),
    };
    if !file.metadata().is_ok_and(|m| m.is_file()) {
        return Err("not a plain file".into());
    }
    let mut bytes = Vec::new();
    (file.take(INCLUDE_MAX + 1).read_to_end(&mut bytes))
        .map_err(|e| format!("could not be read: {e}"))?;
    if bytes.len() as u64 > INCLUDE_MAX {
        return Err(format!("larger than {} KiB", INCLUDE_MAX >> 10));
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| "not text".into())
}

/// Whether a line of an [`INCLUDE`] file may be followed: at most
/// [`MAX_PATH`] bytes, no control character, no `..` (git would match
/// nothing outside, but the line says it wants to) and not into `.git`.
fn pattern(line: &str) -> Result<(), String> {
    if line.len() > MAX_PATH {
        return Err(format!("longer than {MAX_PATH} bytes"));
    }
    if line.chars().any(char::is_control) {
        return Err("a control character".into());
    }
    let body = line.trim_end();
    let body = body.strip_prefix('!').unwrap_or(body);
    for part in body.split('/') {
        match part {
            ".." => return Err("reaches outside the checkout".into()),
            ".git" => return Err("names git's own directory".into()),
            _ => {}
        }
    }
    Ok(())
}

/// The paths git ignores in the checkout `repo` that `patterns` match,
/// in git's order; refused when they are more than `most`, or git can
/// not say in time.
fn named(repo: &Path, patterns: &[&str], most: usize) -> Result<Vec<String>, String> {
    let mut cmd = child::command("git", repo, &[]);
    cmd.args(["ls-files", "-z", "--others", "--ignored", "--directory"])
        .args(patterns.iter().map(|p| format!("--exclude={p}")))
        .env("LC_ALL", "C");
    let Some((status, said)) = child::ask(cmd, NAMED_WAIT) else {
        return Err("git did not say in time what it names".into());
    };
    // Cut at what is read of an answer, git ends on a closed pipe.
    if said.len() as u64 >= child::ANSWER_MAX {
        return Err(format!("matches more than {most} paths"));
    }
    if !status.success() {
        return Err("git could not say what it names".into());
    }
    let found = whole(fields(&said));
    let paths = ignored_of(repo, &found)?;
    if paths.len() > most {
        return Err(format!("matches more than {most} paths git ignores"));
    }
    Ok(paths)
}

/// The paths of git's `-z` answer: what follows the last NUL (stderr) is
/// not one.
fn fields(said: &str) -> Vec<String> {
    let mut paths: Vec<String> = said.split('\0').map(String::from).collect();
    paths.pop();
    paths
}

/// `git ls-files --directory --ignored` lists a directory that holds what
/// the patterns match as well as those paths; a directory the patterns
/// match whole is listed alone. So a directory is kept only when nothing
/// under it is listed, without its `/`.
fn whole(mut listed: Vec<String>) -> Vec<String> {
    listed.sort();
    let mut out = Vec::new();
    for (i, path) in listed.iter().enumerate() {
        let holds = path.ends_with('/')
            && (listed.get(i + 1)).is_some_and(|next| next.starts_with(path.as_str()));
        if !holds {
            out.push(path.trim_end_matches('/').to_string());
        }
    }
    out
}

/// Those of `paths` git ignores in the checkout `repo`, in their order.
fn ignored_of(repo: &Path, paths: &[String]) -> Result<Vec<String>, String> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut cmd = child::command("git", repo, &[]);
    cmd.args(["check-ignore", "-z", "--stdin"])
        .env("LC_ALL", "C");
    let input = paths.join("\0").into_bytes();
    let Some((status, said)) = child::ask_fed(cmd, Some(input), NAMED_WAIT) else {
        return Err("git did not say in time what it ignores".into());
    };
    if !matches!(status.code(), Some(0 | 1)) {
        return Err("git could not say what it ignores".into());
    }
    let ignored: HashSet<String> = fields(&said).into_iter().collect();
    Ok(paths
        .iter()
        .filter(|p| ignored.contains(*p))
        .cloned()
        .collect())
}

/// The bytes of the files under `path` (links counted as themselves,
/// never followed), refused once they pass `left` or the entries pass
/// `most`.
fn size(path: &Path, left: u64, most: usize) -> Result<u64, String> {
    let mut bytes = 0u64;
    let mut entries = 0usize;
    let mut todo = vec![path.to_path_buf()];
    while let Some(p) = todo.pop() {
        entries += 1;
        if entries > most {
            return Err(format!("more than {most} files"));
        }
        let meta = std::fs::symlink_metadata(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        if meta.is_dir() {
            let dir = std::fs::read_dir(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            todo.extend(dir.filter_map(Result::ok).map(|e| e.path()));
        } else {
            bytes += meta.len();
        }
        if bytes > left {
            return Err(format!(
                "more than what is left of the copy budget ({left} bytes)"
            ));
        }
    }
    Ok(bytes)
}

/// `src` copied to `dst` (which is not there), its directories made: a
/// clone on macOS (a whole copy where the volume can not clone), a
/// reflink where the file system has them elsewhere; links copied as
/// links. A copy that fails leaves nothing half done.
fn clone(src: &Path, dst: &Path) -> io::Result<()> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tries: &[&[&str]] = if cfg!(target_os = "macos") {
        &[&["-c", "-R", "-P", "-p"], &["-R", "-P", "-p"]]
    } else {
        &[&["-R", "-P", "-p", "--reflink=auto"]]
    };
    let mut why = String::new();
    for flags in tries {
        let out = child::command("cp", Path::new("/"), &[])
            .args(*flags)
            .arg(src)
            .arg(dst)
            .stdin(Stdio::null())
            .output()?;
        if out.status.success() {
            return Ok(());
        }
        why = String::from_utf8_lossy(&out.stderr)
            .lines()
            .next()
            .unwrap_or("cp failed")
            .to_string();
        remove(dst);
    }
    Err(io::Error::other(why))
}

/// Whatever is at `path`, gone; nothing there is fine.
fn remove(path: &Path) {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => {
            let _ = std::fs::remove_dir_all(path);
        }
        Ok(_) => {
            let _ = std::fs::remove_file(path);
        }
        Err(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A checkout that ignores `.env`, `target/`, `link`, `out` and
    /// `big*`, gone when the test ends.
    struct Scratch {
        dir: PathBuf,
        repo: PathBuf,
        tree: PathBuf,
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn git(dir: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?}");
    }

    fn scratch(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("imece-copy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = dir.join("repo");
        let tree = dir.join("tree");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&tree).unwrap();
        let (dir, repo, tree) = (
            dir.canonicalize().unwrap(),
            repo.canonicalize().unwrap(),
            tree.canonicalize().unwrap(),
        );
        git(&repo, &["init", "--quiet"]);
        std::fs::write(repo.join(".gitignore"), ".env\ntarget/\nlink\nout\nbig*\n").unwrap();
        std::fs::write(repo.join("a.txt"), "a").unwrap();
        git(&repo, &["add", "."]);
        Scratch { dir, repo, tree }
    }

    #[test]
    fn only_plain_relative_paths_may_be_listed() {
        for ok in [
            ".env",
            "node_modules",
            "a/b.txt",
            "target/",
            &"x".repeat(200),
        ] {
            assert_eq!(literal(ok), Ok(()), "{ok:?}");
        }
        for (bad, why) in [
            ("", "no path"),
            (&"x".repeat(201) as &str, "longer than 200"),
            ("*.env", "literal"),
            ("a?b", "literal"),
            ("a[b", "literal"),
            ("a]b", "literal"),
            ("a\\b", "literal"),
            ("x\ny", "literal"),
            ("/etc/passwd", "relative"),
            ("../x", "relative"),
            ("a/../b", "relative"),
            ("./x", "relative"),
            (".git", "git's own"),
            ("a/.git/config", "git's own"),
        ] {
            let e = literal(bad).unwrap_err();
            assert!(e.contains(why), "{bad:?}: {e}");
        }
    }

    #[test]
    fn a_path_is_copied_only_when_git_ignores_it_and_it_stays_inside() {
        let s = scratch("check");
        let r = &s.repo;
        std::fs::write(r.join(".env"), "KEY=1").unwrap();
        std::fs::create_dir_all(r.join("target/debug")).unwrap();
        assert_eq!(checkable(r, ".env"), Ok(()));
        assert_eq!(checkable(r, "target"), Ok(()));
        let e = checkable(r, "a.txt").unwrap_err();
        assert!(e.contains("does not ignore"), "{e}");
        assert_eq!(checkable(r, "big.bin"), Err("not in the checkout".into()));
        std::os::unix::fs::symlink(r.join(".env"), r.join("link")).unwrap();
        assert!(checkable(r, "link").unwrap_err().contains("a link"));
        // A directory that leads out of the checkout.
        std::fs::write(s.dir.join("secret"), "s").unwrap();
        std::os::unix::fs::symlink(&s.dir, r.join("out")).unwrap();
        assert_eq!(
            checkable(r, "out/secret"),
            Err("outside the checkout".into())
        );
        assert!(checkable(r, "../secret").unwrap_err().contains("relative"));
        // Not a checkout: git can not say.
        assert!(checkable(&s.tree, "x").is_err());
    }

    #[test]
    fn what_the_worktree_lacks_is_copied_and_what_it_has_is_left() {
        let s = scratch("into");
        let r = &s.repo;
        std::fs::write(r.join(".env"), "KEY=1").unwrap();
        std::fs::create_dir_all(r.join("target/debug")).unwrap();
        std::fs::write(r.join("target/debug/x"), "123").unwrap();
        std::fs::write(r.join("target/y"), "45").unwrap();
        std::fs::write(r.join("big1"), "theirs").unwrap();
        std::fs::write(s.tree.join("big1"), "mine").unwrap();
        let paths: Vec<String> = [".env", "target", "big1", "a.txt", "../tree/big1"]
            .map(String::from)
            .into();
        let got = into(r, &s.tree, &paths, 1 << 20);
        assert_eq!(
            got[0],
            Copied::Copied {
                path: ".env".into(),
                bytes: 5
            }
        );
        assert_eq!(
            got[1],
            Copied::Copied {
                path: "target".into(),
                bytes: 5
            }
        );
        assert_eq!(
            got[2],
            Copied::There {
                path: "big1".into()
            }
        );
        assert!(
            matches!(&got[3], Copied::Skipped { path, why } if path == "a.txt" && why.contains("ignore"))
        );
        // `tree/../tree/big1` is there, but such a path is never looked at.
        assert!(matches!(&got[4], Copied::Skipped { why, .. } if why.contains("relative")));
        assert_eq!(got.len(), 5);
        assert_eq!(
            std::fs::read_to_string(s.tree.join(".env")).unwrap(),
            "KEY=1"
        );
        assert_eq!(
            std::fs::read_to_string(s.tree.join("target/debug/x")).unwrap(),
            "123"
        );
        assert_eq!(
            std::fs::read_to_string(s.tree.join("big1")).unwrap(),
            "mine"
        );
    }

    #[test]
    fn the_budget_is_for_all_the_paths_together() {
        let s = scratch("budget");
        let r = &s.repo;
        std::fs::write(r.join("big1"), "0123456789").unwrap();
        std::fs::write(r.join("big2"), "0123456789").unwrap();
        std::fs::write(r.join("big3"), "0").unwrap();
        let paths: Vec<String> = ["big1", "big2", "big3"].map(String::from).into();
        let got = into(r, &s.tree, &paths, 21);
        assert!(
            got.iter().all(|c| matches!(c, Copied::Copied { .. })),
            "just the budget: {got:?}"
        );
        let tree = s.dir.join("tree2");
        std::fs::create_dir_all(&tree).unwrap();
        let got = into(r, &tree, &paths, 15);
        assert_eq!(
            got[0],
            Copied::Copied {
                path: "big1".into(),
                bytes: 10
            }
        );
        assert_eq!(
            got[1],
            Copied::Skipped {
                path: "big2".into(),
                why: "more than what is left of the copy budget (5 bytes)".into()
            }
        );
        assert_eq!(
            got[2],
            Copied::Copied {
                path: "big3".into(),
                bytes: 1
            }
        );
        assert!(!tree.join("big2").exists());
    }

    #[test]
    fn a_size_counts_files_and_stops_at_the_entries_it_may_walk() {
        let s = scratch("size");
        let d = s.repo.join("target");
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::write(d.join("sub/a"), "abc").unwrap();
        std::os::unix::fs::symlink("/etc/hosts", d.join("l")).unwrap();
        // target, sub, sub/a and the link: four entries.
        let link = std::fs::symlink_metadata(d.join("l")).unwrap().len();
        assert_eq!(size(&d, 1 << 20, 4), Ok(3 + link));
        assert_eq!(size(&d, 1 << 20, 3), Err("more than 3 files".into()));
        assert!(size(&s.repo.join("nothing"), 1 << 20, 4).is_err());
    }

    #[test]
    fn a_copy_that_can_not_be_made_leaves_nothing() {
        let s = scratch("fail");
        let r = &s.repo;
        std::fs::write(r.join(".env"), "KEY=1").unwrap();
        // The worktree has a file where the copy's directory would go.
        std::fs::write(s.tree.join("sub"), "file").unwrap();
        let e = clone(&r.join(".env"), &s.tree.join("sub/.env")).unwrap_err();
        assert!(!e.to_string().is_empty());
        assert!(clone(&r.join("missing"), &s.tree.join("m")).is_err());
        assert!(!s.tree.join("m").exists());
        // What a failed try leaves goes: a file, a directory, nothing.
        std::fs::write(s.tree.join("f"), "x").unwrap();
        std::fs::create_dir_all(s.tree.join("d/e")).unwrap();
        remove(&s.tree.join("f"));
        remove(&s.tree.join("d"));
        remove(&s.tree.join("none"));
        assert!(!s.tree.join("f").exists() && !s.tree.join("d").exists());
    }

    /// An [`INCLUDE`] file in `s`'s checkout, which ignores what the
    /// tests of it name.
    fn include(s: &Scratch, text: &str) {
        std::fs::write(
            s.repo.join(".gitignore"),
            ".env\n.env.*\nnode_modules/\nbuild/\nbig*\nlink\n",
        )
        .unwrap();
        std::fs::write(s.repo.join(INCLUDE), text).unwrap();
    }

    fn refused(line: usize, text: &str, why: &str) -> Refused {
        Refused {
            line: Some((line, text.into())),
            why: why.into(),
        }
    }

    #[test]
    fn an_include_file_names_what_git_ignores_that_its_patterns_match() {
        let s = scratch("include");
        let r = &s.repo;
        for f in [".env", ".env.local", "node_modules/a/x", "build/sub/.env"] {
            let f = r.join(f);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, "x").unwrap();
        }
        std::fs::write(r.join("build/plain.txt"), "p").unwrap();
        std::fs::write(r.join("untracked.txt"), "u").unwrap();
        include(
            &s,
            "# ../old\n\n.env*\r\nnode_modules/\nuntracked.txt\n../shared\na/.git/x\n!a/.. \n   \n",
        );
        let got = included(r);
        // `build/sub/` holds only what `.env*` matches, so git lists it as
        // well as `build/sub/.env`: the file is named, not the directory.
        assert_eq!(
            got.paths,
            [".env", ".env.local", "build/sub/.env", "node_modules"]
        );
        assert_eq!(
            got.refused,
            [
                refused(6, "../shared", "reaches outside the checkout"),
                refused(7, "a/.git/x", "names git's own directory"),
                refused(8, "!a/.. ", "reaches outside the checkout"),
            ]
        );
        // Only refused lines: nothing is asked of git, nothing more said.
        include(&s, "../x\n");
        let got = included(r);
        assert!(got.paths.is_empty());
        assert_eq!(
            got.refused,
            [refused(1, "../x", "reaches outside the checkout")]
        );
    }

    #[test]
    fn an_include_file_that_names_too_much_names_nothing() {
        let s = scratch("include-most");
        for f in ["big1", "big2", "big3"] {
            std::fs::write(s.repo.join(f), "x").unwrap();
        }
        include(&s, "big*\n");
        assert_eq!(included_at_most(&s.repo, 3).paths, ["big1", "big2", "big3"]);
        let got = included_at_most(&s.repo, 2);
        assert!(got.paths.is_empty());
        assert_eq!(
            got.refused,
            [Refused {
                line: None,
                why: "matches more than 2 paths git ignores".into()
            }]
        );
        // So many that git's answer is cut.
        for i in 0..7400 {
            std::fs::write(s.repo.join(format!("big{i:05}")), "").unwrap();
        }
        let got = included(&s.repo);
        assert!(got.paths.is_empty());
        assert_eq!(got.refused[0].why, "matches more than 100 paths");
        // Not a checkout: git can not say.
        std::fs::write(s.tree.join(INCLUDE), "x\n").unwrap();
        let got = included(&s.tree);
        assert_eq!(got.refused[0].why, "git could not say what it names");
    }

    #[test]
    fn an_include_file_is_a_plain_text_file_of_at_most_16_kib() {
        let s = scratch("include-file");
        let r = &s.repo;
        assert_eq!(read_include(r), Ok(None));
        assert!(!has_include(r));
        let most = INCLUDE_MAX as usize;
        std::fs::write(r.join(INCLUDE), "#".repeat(most)).unwrap();
        assert_eq!(read_include(r), Ok(Some("#".repeat(most))));
        assert!(has_include(r));
        std::fs::write(r.join(INCLUDE), "#".repeat(most + 1)).unwrap();
        assert_eq!(read_include(r), Err("larger than 16 KiB".into()));
        std::fs::write(r.join(INCLUDE), [b'#', 0xff]).unwrap();
        assert_eq!(read_include(r), Err("not text".into()));
        std::fs::remove_file(r.join(INCLUDE)).unwrap();
        std::fs::write(s.dir.join("elsewhere"), ".env\n").unwrap();
        std::os::unix::fs::symlink(s.dir.join("elsewhere"), r.join(INCLUDE)).unwrap();
        assert_eq!(read_include(r), Err("a link: not followed".into()));
        assert!(!has_include(r));
        assert_eq!(
            included(r).refused,
            [Refused {
                line: None,
                why: "a link: not followed".into()
            }]
        );
        std::fs::remove_file(r.join(INCLUDE)).unwrap();
        std::fs::create_dir(r.join(INCLUDE)).unwrap();
        assert_eq!(read_include(r), Err("not a plain file".into()));
        assert!(!has_include(r));
    }

    #[test]
    fn a_line_of_an_include_file_is_short_plain_and_stays_inside() {
        assert_eq!(pattern(&"x".repeat(200)), Ok(()));
        assert_eq!(
            pattern(&"x".repeat(201)),
            Err("longer than 200 bytes".into())
        );
        assert_eq!(pattern("a\tb"), Err("a control character".into()));
        for ok in ["!x", "a/b/", "/.env", "**/.env", "..x", ".gitignore"] {
            assert_eq!(pattern(ok), Ok(()), "{ok:?}");
        }
        assert!(pattern("!../x").is_err());
        assert!(pattern(".git/").is_err());
    }

    #[test]
    fn a_directory_is_named_whole_only_when_nothing_under_it_is_listed() {
        let listed = ["b/x", "b/", ".env.local", ".env", "nm/", "c/", "c-d"].map(String::from);
        assert_eq!(
            whole(listed.into()),
            [".env", ".env.local", "b/x", "c-d", "c", "nm"]
        );
        assert_eq!(fields("a\0b\0git: a warning"), ["a", "b"]);
        assert!(fields("").is_empty());
    }

    #[test]
    fn what_the_include_file_names_comes_after_the_list_in_one_budget() {
        let s = scratch("bring");
        let r = &s.repo;
        std::fs::write(r.join(".env"), "KEY=1").unwrap();
        std::fs::write(r.join("big1"), "0123456789").unwrap();
        std::fs::write(r.join("big2"), "0123456789").unwrap();
        include(&s, ".env\nbig*\n../x\n");
        let listed: Vec<String> = ["big2", ".env"].map(String::from).into();
        let got = bring(r, &s.tree, &listed, 16);
        assert_eq!(got.listed, 2);
        assert_eq!(
            got.copied,
            [
                Copied::Copied {
                    path: "big2".into(),
                    bytes: 10
                },
                Copied::Copied {
                    path: ".env".into(),
                    bytes: 5
                },
                Copied::Skipped {
                    path: "big1".into(),
                    why: "more than what is left of the copy budget (1 bytes)".into()
                },
            ]
        );
        assert_eq!(
            got.refused,
            [refused(3, "../x", "reaches outside the checkout")]
        );
        // Without the file: the list alone.
        std::fs::remove_file(r.join(INCLUDE)).unwrap();
        let tree = s.dir.join("tree2");
        std::fs::create_dir_all(&tree).unwrap();
        let got = bring(r, &tree, &listed[1..], 1 << 20);
        assert_eq!(
            got,
            Brought {
                copied: vec![Copied::Copied {
                    path: ".env".into(),
                    bytes: 5
                }],
                listed: 1,
                refused: Vec::new(),
            }
        );
    }
}
