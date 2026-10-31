//! Files git ignores that a worker's worktree needs all the same (`.env`,
//! `node_modules`, `target/`), copied from the person's checkout into it.
//! Only what the person listed, path by path: literal paths inside the
//! checkout that git ignores, never a link, never over what the worktree
//! has, and no more bytes than a budget. On macOS a copy is an APFS clone
//! (`cp -c`), elsewhere a reflink where the file system has them.

use std::io;
use std::path::{Component, Path};
use std::process::Stdio;

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
}
