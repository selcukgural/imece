//! One git worktree per agent: outside the person's checkout, detached at
//! its HEAD, kept between runs so an agent's work is never thrown away. A
//! worktree found clean at an older commit is moved to the checkout's HEAD
//! when the agent starts, so an agent never works on what was merged
//! already; one with changes or commits of its own stays where it is.
//!
//! What an agent changed is read as one patch ([`change`]) for the person
//! to see, applied to their checkout ([`apply`]) when they take it, and
//! then committed in the worktree ([`applied`]) so the next patch holds
//! only what came after. Commits imece makes this way are its own: a
//! worktree whose only commits are imece's still follows the checkout.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::child;

/// The author of the commits imece makes in a worktree for a change the
/// person applied; what tells them from an agent's or the person's own.
const IMECE_EMAIL: &str = "imece@localhost";

/// Runs git in `dir` with `env` set, and gives its stdout as it is. Git
/// speaks English here, so its words can be read.
fn git_bytes(dir: &Path, env: &[(&str, &str)], args: &[&str]) -> io::Result<Vec<u8>> {
    let mut cmd = child::command("git", dir, &[]);
    cmd.args(args).env("LC_ALL", "C");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(io::Error::other(format!(
            "git {}: {}",
            args.join(" "),
            err.lines().next().unwrap_or("failed")
        )));
    }
    Ok(out.stdout)
}

fn git_env(dir: &Path, env: &[(&str, &str)], args: &[&str]) -> io::Result<String> {
    let out = git_bytes(dir, env, args)?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

fn git(dir: &Path, args: &[&str]) -> io::Result<String> {
    git_env(dir, &[], args)
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
/// it; says where it stands otherwise. Commits imece made for applied
/// changes do not hold it back: they are in the checkout already (or the
/// person let them go). While the checkout has not moved on, a worktree
/// ahead of it by such commits only stays where it is.
pub fn place(repo: &Path, path: &Path) -> io::Result<Placed> {
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
    let ancestor = |a: &str, b: &str| git(path, &["merge-base", "--is-ancestor", a, b]).is_ok();
    // Only a worktree whose own commits are all imece's can move without
    // losing work.
    if !ancestor(&at, &head) {
        let own = git(path, &["log", "--format=%ae", &format!("{head}..{at}")])?;
        if own.lines().any(|l| l != IMECE_EMAIL) {
            return Ok(Placed::Kept {
                at: short(&at),
                head: short(&head),
                why: Why::Commits,
            });
        }
        if ancestor(&head, &at) {
            // Applied, not yet committed by the person: nothing to pick up.
            return Ok(Placed::AtHead);
        }
    }
    git(path, &["checkout", "--quiet", "--detach", &head])?;
    Ok(Placed::Moved {
        from: short(&at),
        to: short(&head),
    })
}

/// One file of a [`Change`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    /// Lines added and removed; none for a binary file.
    pub added: u32,
    pub removed: u32,
    pub binary: bool,
}

/// Everything an agent changed in its worktree since the worktree's HEAD,
/// untracked files included, as one patch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    /// The worktree.
    pub path: PathBuf,
    /// The tree the change makes, to commit once it is applied.
    pub tree: String,
    pub files: Vec<FileChange>,
    /// A `git diff` of it all (binary files included), as `git apply`
    /// takes it.
    pub patch: String,
}

impl Change {
    /// Lines added and removed over all files.
    pub fn counts(&self) -> (u32, u32) {
        self.files
            .iter()
            .fold((0, 0), |(a, r), f| (a + f.added, r + f.removed))
    }
}

/// What the worktree at `path` holds against its HEAD, or none when it is
/// clean. It is read through an index file of imece's own, so the agent's
/// index is never touched. A patch over `max_patch` bytes is refused.
pub fn change(path: &Path, max_patch: usize) -> io::Result<Option<Change>> {
    let git_dir = PathBuf::from(git(path, &["rev-parse", "--absolute-git-dir"])?);
    let index = git_dir.join("imece-index");
    let _ = std::fs::remove_file(&index);
    let env = [("GIT_INDEX_FILE", index.to_str().unwrap_or_default())];
    let staged = (|| {
        git_env(path, &env, &["read-tree", "HEAD"])?;
        git_env(path, &env, &["add", "-A", "--", "."])?;
        let tree = git_env(path, &env, &["write-tree"])?;
        if tree == git(path, &["rev-parse", "HEAD^{tree}"])? {
            return Ok(None);
        }
        let stat = git_env(path, &env, &["diff", "--cached", "--numstat", "HEAD"])?;
        let patch = git_bytes(
            path,
            &env,
            &["diff", "--cached", "--binary", "--no-color", "HEAD"],
        )?;
        if patch.len() > max_patch {
            return Err(io::Error::other(format!(
                "the change is {} KiB, more than imece shows ({} KiB)",
                patch.len() >> 10,
                max_patch >> 10
            )));
        }
        let files = stat.lines().filter_map(file_change).collect();
        Ok(Some(Change {
            path: path.to_path_buf(),
            tree,
            files,
            patch: String::from_utf8_lossy(&patch).into_owned(),
        }))
    })();
    let _ = std::fs::remove_file(&index);
    staged
}

/// One `--numstat` line: added, removed (`-` for a binary file), path.
fn file_change(line: &str) -> Option<FileChange> {
    let mut parts = line.splitn(3, '\t');
    let (added, removed, path) = (parts.next()?, parts.next()?, parts.next()?);
    let binary = added == "-";
    Some(FileChange {
        path: path.to_string(),
        added: added.parse().unwrap_or(0),
        removed: removed.parse().unwrap_or(0),
        binary,
    })
}

/// Applies `patch` to the checkout `repo`, staged, so what came from an
/// agent is marked for the person to look over and commit. A patch that
/// does not fit as a whole (the checkout moved on in the same lines, or
/// has edits of its own in the file) is refused with git's words, and
/// the checkout is as it was: nothing is half applied.
pub fn apply(repo: &Path, patch: &str) -> io::Result<()> {
    // A file touched but unchanged looks changed to git until its index
    // is refreshed; refreshing changes nothing else. It reports files
    // with real changes as a failure, which is not one here.
    let _ = git_bytes(repo, &[], &["update-index", "-q", "--refresh"]);
    let mut child = child::command("git", repo, &[])
        .args(["apply", "--index", "-"])
        .env("LC_ALL", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        // A closed pipe is git's refusal to report, not ours.
        let _ = stdin.write_all(patch.as_bytes());
    }
    let out = child.wait_with_output()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        // Git says what failed and why on separate error lines.
        let why: Vec<&str> = err
            .lines()
            .filter_map(|l| l.strip_prefix("error: "))
            .take(3)
            .collect();
        let why = if why.is_empty() {
            err.lines().next().unwrap_or("failed").to_string()
        } else {
            why.join("; ")
        };
        return Err(io::Error::other(why));
    }
    Ok(())
}

/// Commits `change` in its worktree as imece, once the person applied it:
/// HEAD and the index move to the change, the files stay as they are, so
/// the next [`change`] holds only what came after. Gives the commit's
/// short id.
pub fn applied(change: &Change, message: &str) -> io::Result<String> {
    let path = &change.path;
    let parent = git(path, &["rev-parse", "HEAD"])?;
    let who = [
        ("GIT_AUTHOR_NAME", "imece"),
        ("GIT_AUTHOR_EMAIL", IMECE_EMAIL),
        ("GIT_COMMITTER_NAME", "imece"),
        ("GIT_COMMITTER_EMAIL", IMECE_EMAIL),
    ];
    let commit = git_env(
        path,
        &who,
        &["commit-tree", &change.tree, "-p", &parent, "-m", message],
    )?;
    git(path, &["reset", "--quiet", &commit])?;
    Ok(short(&commit))
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
    fn a_change_is_everything_against_the_worktree_s_head_and_touches_no_index() {
        let s = repo("change");
        let r = &s.repo;
        std::fs::write(r.join("gone.txt"), "gone").unwrap();
        std::fs::write(r.join("same.txt"), "same").unwrap();
        commit(r, "more");
        let w = ensure(r, &s.dir.join("wt"), "claude-1").unwrap().path;
        assert_eq!(change(&w, 1 << 20).unwrap(), None, "clean");
        std::fs::write(w.join("a.txt"), "a\nb\nc\n").unwrap();
        std::fs::write(w.join("new.txt"), "new\n").unwrap();
        std::fs::write(w.join("blob.bin"), [0u8, 1, 2, 255, 0, 10]).unwrap();
        std::fs::remove_file(w.join("gone.txt")).unwrap();
        let c = change(&w, 1 << 20).unwrap().unwrap();
        assert_eq!(c.path, w);
        let files: Vec<(&str, u32, u32, bool)> = c
            .files
            .iter()
            .map(|f| (f.path.as_str(), f.added, f.removed, f.binary))
            .collect();
        assert_eq!(
            files,
            [
                ("a.txt", 3, 1, false),
                ("blob.bin", 0, 0, true),
                ("gone.txt", 0, 1, false),
                ("new.txt", 1, 0, false),
            ]
        );
        assert_eq!(c.counts(), (4, 2));
        assert!(
            c.patch.contains("diff --git a/new.txt b/new.txt"),
            "{}",
            c.patch
        );
        assert!(c.patch.contains("GIT binary patch"), "{}", c.patch);
        assert!(c.patch.ends_with('\n'), "a patch ends with its newline");
        assert_ne!(c.tree, git(&w, &["rev-parse", "HEAD^{tree}"]).unwrap());
        // The agent's own index is as it was: nothing staged.
        let status = git(&w, &["status", "--porcelain"]).unwrap();
        assert!(
            status.contains("M a.txt") && status.contains("?? new.txt") && !status.contains("A "),
            "{status}"
        );
        assert!(!w.join(".git").join("imece-index").exists());
        // A change past the cap is refused, not cut.
        let e = change(&w, 10).unwrap_err().to_string();
        assert!(e.contains("more than imece shows"), "{e}");
        // Lines a stat could not parse are not a crash.
        assert_eq!(file_change("x"), None);
        assert_eq!(
            file_change("-\t-\tb.bin"),
            Some(FileChange {
                path: "b.bin".into(),
                added: 0,
                removed: 0,
                binary: true
            })
        );
    }

    #[test]
    fn an_applied_change_is_staged_in_the_checkout_and_committed_in_the_worktree() {
        let s = repo("apply");
        let r = &s.repo;
        let w = ensure(r, &s.dir.join("wt"), "claude-1").unwrap().path;
        std::fs::write(w.join("a.txt"), "a\nb\n").unwrap();
        std::fs::write(w.join("new.txt"), "new\n").unwrap();
        let c = change(&w, 1 << 20).unwrap().unwrap();
        apply(r, &c.patch).unwrap();
        assert_eq!(std::fs::read_to_string(r.join("a.txt")).unwrap(), "a\nb\n");
        assert_eq!(std::fs::read_to_string(r.join("new.txt")).unwrap(), "new\n");
        let staged = git(r, &["diff", "--cached", "--name-only"]).unwrap();
        assert_eq!(staged, "a.txt\nnew.txt");
        // The same patch again does not fit: the checkout is left as it is.
        let e = apply(r, &c.patch).unwrap_err().to_string();
        assert!(e.contains("does not apply"), "{e}");
        assert_eq!(std::fs::read_to_string(r.join("a.txt")).unwrap(), "a\nb\n");
        assert!(apply(r, "not a patch\n").is_err());
        // The person's own unstaged edit in a file is never overwritten.
        std::fs::write(r.join("a.txt"), "theirs\n").unwrap();
        std::fs::write(w.join("a.txt"), "a\nb\nc\n").unwrap();
        let more = change(&w, 1 << 20).unwrap().unwrap();
        let e = apply(r, &more.patch).unwrap_err().to_string();
        assert!(e.contains("does not match index"), "{e}");
        assert_eq!(
            std::fs::read_to_string(r.join("a.txt")).unwrap(),
            "theirs\n"
        );
        std::fs::write(r.join("a.txt"), "a\nb\n").unwrap();
        std::fs::write(w.join("a.txt"), "a\nb\n").unwrap();

        let before = git(&w, &["rev-parse", "HEAD"]).unwrap();
        let id = applied(&c, "applied: fix it").unwrap();
        let head = git(&w, &["rev-parse", "HEAD"]).unwrap();
        assert_ne!(head, before);
        assert_eq!(short(&head), id);
        assert_eq!(
            git(&w, &["log", "-1", "--format=%ae %s"]).unwrap(),
            "imece@localhost applied: fix it"
        );
        assert_eq!(git(&w, &["rev-parse", "HEAD^"]).unwrap(), before);
        assert_eq!(
            git(&w, &["status", "--porcelain"]).unwrap(),
            "",
            "files and index agree with the commit"
        );
        assert_eq!(std::fs::read_to_string(w.join("new.txt")).unwrap(), "new\n");
        assert_eq!(
            change(&w, 1 << 20).unwrap(),
            None,
            "nothing since the commit"
        );
        // What comes after is the next change, alone.
        std::fs::write(w.join("later.txt"), "later\n").unwrap();
        let next = change(&w, 1 << 20).unwrap().unwrap();
        assert_eq!(next.files.len(), 1);
        assert_eq!(next.files[0].path, "later.txt");
    }

    #[test]
    fn a_worktree_whose_own_commits_are_imece_s_follows_the_checkout() {
        let s = repo("applied");
        let r = &s.repo;
        let root = s.dir.join("wt");
        let w = ensure(r, &root, "claude-1").unwrap().path;
        std::fs::write(w.join("new.txt"), "new\n").unwrap();
        let c = change(&w, 1 << 20).unwrap().unwrap();
        apply(r, &c.patch).unwrap();
        applied(&c, "applied").unwrap();
        let at = git(&w, &["rev-parse", "HEAD"]).unwrap();
        // The checkout has not committed: the worktree stays, and says nothing.
        assert_eq!(place(r, &w).unwrap(), Placed::AtHead);
        assert_eq!(git(&w, &["rev-parse", "HEAD"]).unwrap(), at);
        // The person committed (the change, and more): the worktree follows.
        std::fs::write(r.join("b.txt"), "b").unwrap();
        let new = commit(r, "took it");
        assert_eq!(
            ensure(r, &root, "claude-1").unwrap().placed,
            Placed::Moved {
                from: short(&at),
                to: short(&new)
            }
        );
        assert_eq!(git(&w, &["rev-parse", "HEAD"]).unwrap(), new);
        assert_eq!(std::fs::read_to_string(w.join("new.txt")).unwrap(), "new\n");
        assert_eq!(std::fs::read_to_string(w.join("b.txt")).unwrap(), "b");
        // An agent's own commit on top of imece's holds the worktree back.
        std::fs::write(w.join("mine.txt"), "mine").unwrap();
        let mine = commit(&w, "mine");
        std::fs::write(r.join("c.txt"), "c").unwrap();
        let newer = commit(r, "third");
        assert_eq!(
            place(r, &w).unwrap(),
            Placed::Kept {
                at: short(&mine),
                head: short(&newer),
                why: Why::Commits
            }
        );
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
