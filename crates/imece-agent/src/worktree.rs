//! One git worktree per agent: outside the person's checkout, detached at
//! its HEAD, kept between runs so an agent's work is never thrown away. A
//! worktree found clean at an older commit is moved to the checkout's HEAD
//! when the agent starts, so an agent never works on what was merged
//! already; one with changes or commits of its own stays where it is.

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

/// Why a worktree was left at an older commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// It has changes that are not committed.
    Changes,
    /// It has commits the checkout's HEAD does not.
    Commits,
}

/// Where an ensured worktree stands next to the checkout's HEAD. Commits
/// are named by their short ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Placed {
    /// Made just now, at HEAD.
    Fresh,
    /// Found at HEAD already.
    AtHead,
    /// Found clean at an older commit and moved to HEAD.
    Moved { from: String, to: String },
    /// Found with work of its own at `at` and left there.
    Kept { at: String, head: String, why: Why },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub placed: Placed,
}

fn short(sha: &str) -> String {
    sha.chars().take(7).collect()
}

/// The worktree of agent `name` under `root`, created from `repo`'s HEAD
/// the first time and moved to it on a later start when it is clean. A
/// directory that is there but is not a worktree of `repo` is refused,
/// never reused or replaced.
pub fn ensure(repo: &Path, root: &Path, name: &str) -> io::Result<Worktree> {
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
        let placed = place(repo, &path)?;
        return Ok(Worktree { path, placed });
    }
    std::fs::create_dir_all(root)?;
    let p = path.to_string_lossy();
    git(
        repo,
        &["worktree", "add", "--quiet", "--detach", &p, "HEAD"],
    )?;
    Ok(Worktree {
        path: path.canonicalize()?,
        placed: Placed::Fresh,
    })
}

/// Moves the worktree at `path` to `repo`'s HEAD if it is clean and behind
/// it; says where it stands otherwise.
fn place(repo: &Path, path: &Path) -> io::Result<Placed> {
    let head = git(repo, &["rev-parse", "HEAD"])?;
    let at = git(path, &["rev-parse", "HEAD"])?;
    if at == head {
        return Ok(Placed::AtHead);
    }
    if !git(path, &["status", "--porcelain"])?.is_empty() {
        return Ok(Placed::Kept {
            at: short(&at),
            head: short(&head),
            why: Why::Changes,
        });
    }
    // Only a worktree whose commit is behind HEAD can move without losing
    // commits of its own.
    if git(path, &["merge-base", "--is-ancestor", &at, &head]).is_err() {
        return Ok(Placed::Kept {
            at: short(&at),
            head: short(&head),
            why: Why::Commits,
        });
    }
    git(path, &["checkout", "--quiet", "--detach", &head])?;
    Ok(Placed::Moved {
        from: short(&at),
        to: short(&head),
    })
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
        commit(&r, "init");
        Scratch { dir, repo: r }
    }

    /// Commits everything in `dir` (a repository or a worktree).
    fn commit(dir: &Path, message: &str) -> String {
        git(dir, &["add", "."]).unwrap();
        git(
            dir,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                message,
            ],
        )
        .unwrap();
        git(dir, &["rev-parse", "HEAD"]).unwrap()
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
        assert_eq!(w.placed, Placed::Fresh);
        let w = w.path;
        assert_eq!(std::fs::read_to_string(w.join("a.txt")).unwrap(), "a");
        std::fs::write(w.join("work.txt"), "kept").unwrap();
        let again = ensure(r, &root, "claude-1").unwrap();
        assert_eq!(again.path, w);
        assert_eq!(again.placed, Placed::AtHead);
        assert!(w.join("work.txt").exists(), "a reuse threw work away");
    }

    #[test]
    fn a_clean_worktree_moves_to_the_checkout_s_head() {
        let s = repo("move");
        let r = &s.repo;
        let root = s.dir.join("wt");
        let w = ensure(r, &root, "claude-1").unwrap().path;
        let old = git(&w, &["rev-parse", "HEAD"]).unwrap();
        std::fs::write(r.join("b.txt"), "b").unwrap();
        let new = commit(r, "second");
        let again = ensure(r, &root, "claude-1").unwrap();
        assert_eq!(
            again.placed,
            Placed::Moved {
                from: short(&old),
                to: short(&new)
            }
        );
        assert_eq!(git(&w, &["rev-parse", "HEAD"]).unwrap(), new);
        assert_eq!(std::fs::read_to_string(w.join("b.txt")).unwrap(), "b");
        assert_eq!(short(&new).len(), 7);
    }

    #[test]
    fn a_worktree_with_changes_stays_where_it_is() {
        let s = repo("dirty");
        let r = &s.repo;
        let root = s.dir.join("wt");
        let w = ensure(r, &root, "claude-1").unwrap().path;
        let old = git(&w, &["rev-parse", "HEAD"]).unwrap();
        std::fs::write(w.join("a.txt"), "changed").unwrap();
        let new = commit(r, "second");
        let again = ensure(r, &root, "claude-1").unwrap();
        assert_eq!(
            again.placed,
            Placed::Kept {
                at: short(&old),
                head: short(&new),
                why: Why::Changes
            }
        );
        assert_eq!(git(&w, &["rev-parse", "HEAD"]).unwrap(), old);
        assert_eq!(std::fs::read_to_string(w.join("a.txt")).unwrap(), "changed");
        // An untracked file is work too.
        std::fs::write(w.join("a.txt"), "a").unwrap();
        std::fs::write(w.join("new.txt"), "n").unwrap();
        assert!(matches!(
            ensure(r, &root, "claude-1").unwrap().placed,
            Placed::Kept {
                why: Why::Changes,
                ..
            }
        ));
        assert_eq!(git(&w, &["rev-parse", "HEAD"]).unwrap(), old);
    }

    #[test]
    fn a_worktree_with_commits_of_its_own_stays_where_it_is() {
        let s = repo("commits");
        let r = &s.repo;
        let root = s.dir.join("wt");
        let w = ensure(r, &root, "claude-1").unwrap().path;
        std::fs::write(w.join("w.txt"), "w").unwrap();
        let own = commit(&w, "own work");
        let new = commit(r, "second");
        let again = ensure(r, &root, "claude-1").unwrap();
        assert_eq!(
            again.placed,
            Placed::Kept {
                at: short(&own),
                head: short(&new),
                why: Why::Commits
            }
        );
        assert_eq!(git(&w, &["rev-parse", "HEAD"]).unwrap(), own);
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

    #[test]
    fn a_worktree_of_another_repository_is_refused() {
        let ours = repo("foreign");
        let theirs = repo("foreign-other");
        let root = ours.dir.join("wt");
        // A real worktree under our name, but of another repository.
        let w = ensure(&theirs.repo, &root, "claude-1").unwrap().path;
        assert!(ensure(&ours.repo, &root, "claude-1").is_err());
        assert!(w.join("a.txt").exists(), "nothing of theirs was touched");
    }
}
