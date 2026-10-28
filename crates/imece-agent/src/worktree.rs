//! One git worktree per agent: outside the person's checkout, detached at
//! its HEAD, kept between runs so an agent's work is never thrown away.

use std::io;
use std::path::{Path, PathBuf};

use crate::child;

fn git(dir: &Path, args: &[&str]) -> io::Result<String> {
    let out = child::command("git", dir, &[]).args(args).output()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(io::Error::other(format!(
            "git {}: {}",
            args.join(" "),
            err.lines().next().unwrap_or("failed")
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The top directory of the checkout `dir` is in.
pub fn top(dir: &Path) -> io::Result<PathBuf> {
    PathBuf::from(git(dir, &["rev-parse", "--show-toplevel"])?).canonicalize()
}

fn common_dir(dir: &Path) -> io::Result<PathBuf> {
    let d = git(dir, &["rev-parse", "--git-common-dir"])?;
    dir.join(d).canonicalize()
}

/// Agent names become directory names: lowercase letters, digits and
/// dashes, never a path.
pub fn valid_name(name: &str) -> bool {
    (1..=40).contains(&name.len())
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The worktree of agent `name` under `root`, created from `repo`'s HEAD
/// the first time. A directory that is there but is not a worktree of
/// `repo` is refused, never reused or replaced.
pub fn ensure(repo: &Path, root: &Path, name: &str) -> io::Result<PathBuf> {
    if !valid_name(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name:?} can not name a worktree"),
        ));
    }
    let path = root.join(name);
    if path.exists() {
        let path = path.canonicalize()?;
        let same =
            top(&path).is_ok_and(|t| t == path) && common_dir(&path).ok() == common_dir(repo).ok();
        if !same {
            return Err(io::Error::other(format!(
                "{} is there but is not a worktree of {}",
                path.display(),
                repo.display()
            )));
        }
        return Ok(path);
    }
    std::fs::create_dir_all(root)?;
    let p = path.to_string_lossy();
    git(
        repo,
        &["worktree", "add", "--quiet", "--detach", &p, "HEAD"],
    )?;
    path.canonicalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway repo whose directory goes away when the test ends, also
    /// when it fails.
    struct Scratch {
        dir: PathBuf,
        repo: PathBuf,
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn repo(name: &str) -> Scratch {
        let dir = std::env::temp_dir()
            .join(format!("imece-wt-{name}-{}", std::process::id()))
            .canonicalize_or_self();
        let _ = std::fs::remove_dir_all(&dir);
        let r = dir.join("repo");
        std::fs::create_dir_all(&r).unwrap();
        git(&r, &["init", "--quiet"]).unwrap();
        std::fs::write(r.join("a.txt"), "a").unwrap();
        git(&r, &["add", "."]).unwrap();
        git(
            &r,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                "init",
            ],
        )
        .unwrap();
        Scratch { dir, repo: r }
    }

    trait OrSelf {
        fn canonicalize_or_self(self) -> PathBuf;
    }
    impl OrSelf for PathBuf {
        fn canonicalize_or_self(self) -> PathBuf {
            std::fs::create_dir_all(&self).unwrap();
            self.canonicalize().unwrap()
        }
    }

    #[test]
    fn names_never_become_paths() {
        for bad in ["", "../x", "a/b", "-rf", "A", "x y", "é", &"a".repeat(41)] {
            assert!(!valid_name(bad), "{bad:?}");
        }
        assert!(valid_name("claude-1"));
        let s = repo("names");
        let r = &s.repo;
        assert!(ensure(r, &s.dir.join("wt"), "../escape").is_err());
    }

    #[test]
    fn a_worktree_is_made_once_and_reused() {
        let s = repo("reuse");
        let r = &s.repo;
        let root = s.dir.join("wt");
        let w = ensure(r, &root, "claude-1").unwrap();
        assert_eq!(std::fs::read_to_string(w.join("a.txt")).unwrap(), "a");
        std::fs::write(w.join("work.txt"), "kept").unwrap();
        assert_eq!(ensure(r, &root, "claude-1").unwrap(), w);
        assert!(w.join("work.txt").exists(), "a reuse threw work away");
    }

    #[test]
    fn a_stranger_directory_is_refused() {
        let s = repo("stranger");
        let r = &s.repo;
        let root = s.dir.join("wt");
        std::fs::create_dir_all(root.join("claude-1")).unwrap();
        std::fs::write(root.join("claude-1/mine.txt"), "x").unwrap();
        assert!(ensure(r, &root, "claude-1").is_err());
        assert!(root.join("claude-1/mine.txt").exists());
    }
}
