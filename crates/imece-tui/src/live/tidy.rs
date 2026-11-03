//! `:tidy`: the worktrees under imece's state that no agent owns (record
//! 96), looked for on a thread of their own, and removed on the person's
//! word: a clean one when asked, one that holds work only with a yes for
//! it. An agent's worktree is never removed: ownership is read from the
//! world when the look starts, again when it answers and again when a
//! removal is asked, and a start that would make or reuse a worktree
//! being removed waits until it is gone. Before a removal the teardown
//! script of the worktree's checkout runs in it (record 96), on the same
//! terms as the setup script: only bytes the person said yes to, as a
//! private copy; a no removes without it, a failure is said and the
//! removal decides on its own.

use std::sync::{Condvar, Mutex};

use imece_agent::run::Running;

use super::setup::{self, Kind, Script};
use super::*;

/// What a look or a removal sends back.
pub(super) enum Back {
    Found(Vec<Orphan>),
    Tidied(Tidied, Vec<Orphan>),
}

/// The worktrees being removed: a start waits on its own before it
/// makes or reuses one (see [`Removing::wait`]).
#[derive(Clone, Default)]
pub(super) struct Removing(Arc<(Mutex<HashSet<PathBuf>>, Condvar)>);

impl Removing {
    /// `paths` are being removed from now on.
    fn hold(&self, paths: &[PathBuf]) {
        let (set, _) = &*self.0;
        let mut set = set.lock().unwrap_or_else(|e| e.into_inner());
        set.extend(paths.iter().cloned());
    }

    /// `path` is removed, or was not.
    fn free(&self, path: &Path) {
        let (set, gone) = &*self.0;
        let mut set = set.lock().unwrap_or_else(|e| e.into_inner());
        set.remove(path);
        gone.notify_all();
    }

    /// Returns once `path` is not being removed.
    pub(super) fn wait(&self, path: &Path) {
        let (set, gone) = &*self.0;
        let set = set.lock().unwrap_or_else(|e| e.into_inner());
        drop(
            gone.wait_while(set, |s| s.contains(path))
                .unwrap_or_else(|e| e.into_inner()),
        );
    }
}

/// The tidying's channel and the removals under way.
pub(super) struct Tidy {
    tx: Sender<Back>,
    rx: Receiver<Back>,
    pub(super) removing: Removing,
    /// The teardown scripts the person said no to this run, by key.
    refused: HashSet<String>,
    /// The teardown runs under way, killed with imece.
    pub(super) running: Running,
}

impl Default for Tidy {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Tidy {
            tx,
            rx,
            removing: Removing::default(),
            refused: HashSet::new(),
            running: Running::default(),
        }
    }
}

/// What a removal does about a checkout's teardown script, decided
/// before its thread.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Plan {
    /// None there.
    None,
    /// Run this private copy first.
    Run(PathBuf),
    /// Said no to this run.
    Refused,
    /// Could not be read or copied: why.
    Failed(String),
}

/// A run's end as the flash says it.
fn torn(ran: Result<run::Outcome, String>) -> TornDown {
    match ran {
        Ok(o) => {
            let (ok, end) = setup::ended(&o);
            TornDown::Ran {
                ok,
                end,
                last: setup::last_lines(&o.output).pop(),
            }
        }
        Err(why) => TornDown::Failed(clean(&why)),
    }
}

/// The worktrees under `dir` (`state/worktrees`: a directory per
/// checkout, a worktree per agent in it) that are not in `owned`, sorted
/// by path. Links and what is not a worktree added to a checkout are
/// passed over: a Gemini session's private directory, a file.
pub(super) fn orphans_under(dir: &Path, owned: &HashSet<PathBuf>) -> Vec<Orphan> {
    let mut found = Vec::new();
    for root in dirs_in(dir) {
        for path in dirs_in(&root) {
            if owned.contains(&path) {
                continue;
            }
            let Ok((checkout, held)) = worktree::held(&path) else {
                continue;
            };
            found.push(Orphan {
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path,
                checkout,
                changes: held.changes,
                commits: held.commits,
            });
        }
    }
    found
}

/// The directories in `dir`, links left out, sorted.
fn dirs_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect();
    dirs.sort();
    dirs
}

impl Live {
    /// Where every worktree is: one directory per checkout.
    fn worktrees(&self) -> PathBuf {
        self.state.join("worktrees")
    }

    /// The worktree paths agents own: every agent's of every project,
    /// open, closed or archived, as [`Live::tree_of`] names it. A leader's
    /// and a git-less solo's are counted too: owning too much only keeps
    /// a directory.
    pub(super) fn owned(&self, w: &World) -> HashSet<PathBuf> {
        (0..w.agents.len())
            .filter_map(|id| {
                let place = self.places.get(w.agents[id].project)?;
                Some(place.root.join(Self::tree_of(w, id)))
            })
            .collect()
    }

    /// Looks for the worktrees no agent owns, on a thread of its own; the
    /// answer comes to [`Live::take_tidy`]. Nothing while a look or a
    /// removal is under way: its answer is the newer.
    pub fn look_orphans(&mut self, w: &mut World) {
        if w.orphans.busy {
            return;
        }
        w.orphans.busy = true;
        let (dir, owned, tx) = (self.worktrees(), self.owned(w), self.tidy.tx.clone());
        thread::spawn(move || {
            let _ = tx.send(Back::Found(orphans_under(&dir, &owned)));
        });
    }

    /// Removes the worktrees found at `at` in the last look, on a thread
    /// of its own, then looks again; the answer comes to
    /// [`Live::take_tidy`]. Each must be no agent's now; one that holds
    /// work goes only when `force` (the person's yes to it). Refused while
    /// a look or a removal is under way, and when nothing is asked.
    pub fn tidy(&mut self, w: &mut World, at: &[usize], force: bool) -> Result<(), Refusal> {
        if w.orphans.busy {
            return Err(Refusal::Busy);
        }
        let found = w.orphans.found.as_deref().unwrap_or_default();
        let asked: Vec<Orphan> = at.iter().filter_map(|&i| found.get(i).cloned()).collect();
        if asked.is_empty() {
            return Err(Refusal::Nothing);
        }
        let owned = self.owned(w);
        let (mine, theirs): (Vec<Orphan>, Vec<Orphan>) =
            asked.into_iter().partition(|o| !owned.contains(&o.path));
        // Each checkout's teardown script, read once now: one not yet
        // answered is asked about first, and nothing goes meanwhile.
        let mut plans: HashMap<PathBuf, Plan> = HashMap::new();
        for o in &mine {
            if !plans.contains_key(&o.checkout) {
                let plan = self.teardown_plan(w, &o.checkout)?;
                plans.insert(o.checkout.clone(), plan);
            }
        }
        let mut tidied = Tidied {
            kept: (theirs.into_iter()).map(|o| (o.name, None)).collect(),
            ..Tidied::default()
        };
        w.orphans.busy = true;
        let paths: Vec<PathBuf> = mine.iter().map(|o| o.path.clone()).collect();
        let removing = self.tidy.removing.clone();
        removing.hold(&paths);
        let (dir, tx) = (self.worktrees(), self.tidy.tx.clone());
        let running = self.tidy.running.clone();
        thread::spawn(move || {
            for o in mine {
                let ran = match &plans[&o.checkout] {
                    Plan::None => None,
                    Plan::Run(copy) => {
                        Some(torn(setup::run_in(copy, &o.checkout, &o.path, &running)))
                    }
                    Plan::Refused => Some(TornDown::Refused),
                    Plan::Failed(why) => Some(TornDown::Failed(why.clone())),
                };
                if let Some(ran) = ran {
                    tidied.torn.push((o.name.clone(), ran));
                }
                match worktree::remove(&o.path, force) {
                    Ok(()) => tidied.removed.push(o.name),
                    Err(e) => tidied.kept.push((o.name, Some(e.to_string()))),
                }
                removing.free(&o.path);
            }
            let _ = tx.send(Back::Tidied(tidied, orphans_under(&dir, &owned)));
        });
        Ok(())
    }

    /// What a removal does about the teardown script of `checkout`: none
    /// there; run the private copy of bytes the person said yes to;
    /// nothing after a no this run; why it can not be read. One not yet
    /// answered is refused with it, for the person to read.
    fn teardown_plan(&self, w: &World, checkout: &Path) -> Result<Plan, Refusal> {
        let script = match setup::look_for(checkout, Kind::Teardown) {
            Ok(Some(s)) => s,
            Ok(None) => return Ok(Plan::None),
            Err(why) => return Ok(Plan::Failed(clean(&why))),
        };
        let key = script.key();
        if self.tidy.refused.contains(&key) {
            return Ok(Plan::Refused);
        }
        if !w.setup_approved.contains(&key) {
            return Err(Refusal::Asks(Box::new(script)));
        }
        Ok(match setup::copy_of(&self.setup_dir(), &script) {
            Ok(copy) => Plan::Run(copy),
            Err(e) => Plan::Failed(e.to_string()),
        })
    }

    /// The person's answer to a teardown script a removal asked about:
    /// yes is kept with the setup scripts' (in the store), no for the run.
    /// The removal is asked again after.
    pub fn decide_teardown(&mut self, w: &mut World, script: &Script, yes: bool) {
        if yes {
            w.approve_setup(script.key());
        } else {
            self.tidy.refused.insert(script.key());
        }
    }

    /// What a look or a removal answered, put in the world; a worktree an
    /// agent came to own since the look started is left out. Whether one
    /// answered.
    pub(super) fn take_tidy(&mut self, w: &mut World) -> bool {
        let mut any = false;
        while let Ok(back) = self.tidy.rx.try_recv() {
            any = true;
            let owned = self.owned(w);
            let (mut found, tidied) = match back {
                Back::Found(found) => (found, None),
                Back::Tidied(tidied, found) => (found, Some(tidied)),
            };
            found.retain(|o| !owned.contains(&o.path));
            w.orphans = Orphans {
                found: Some(found),
                busy: false,
                tidied: tidied.or(w.orphans.tidied.take()),
            };
        }
        any
    }
}

/// Why a removal was not done.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// A look or a removal is under way.
    Busy,
    /// Nothing found is asked.
    Nothing,
    /// The teardown script of a checkout among them waits for the
    /// person's yes or no: these bytes.
    Asks(Box<Script>),
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    /// A state directory under the temp dir with a checkout `repo` beside
    /// it, and a live team of a leader and `workers` on it whose
    /// worktrees go where live puts them (`root_for`).
    fn state(name: &str, workers: usize) -> (PathBuf, PathBuf, Live, World) {
        let dir = std::env::temp_dir().join(format!("imece-tidy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        let repo = checkout(&format!("tidy-{name}-repo"));
        let state = dir.join("state");
        let root = root_for(&state, &repo);
        let mut live = Live::new(
            workers + 1,
            claude_at(&fake_claude(&dir)),
            repo.clone(),
            root,
        );
        live.state = state;
        (dir, repo, live, team(workers))
    }

    fn root(live: &Live) -> PathBuf {
        live.places[0].root.clone()
    }

    /// A worktree of `repo` named `name` under the team's root.
    fn tree(live: &Live, repo: &Path, name: &str) -> PathBuf {
        worktree::ensure(repo, &root(live), name, None)
            .unwrap()
            .path
    }

    fn answered(live: &mut Live, w: &mut World) {
        wait_for(live, w, |w| !w.orphans.busy);
    }

    fn names(w: &World) -> Vec<String> {
        let found = w.orphans.found.as_deref().unwrap_or_default();
        found.iter().map(|o| o.name.clone()).collect()
    }

    #[test]
    fn every_agent_of_every_project_owns_its_worktree() {
        let (dir, repo, mut live, mut w) = state("owned", 1);
        // A helper's is its worker's; a closed project's are still owned.
        let helper = with_helper(&mut w, 1);
        live.grow(None, false);
        w.projects[0].closed = true;
        let owned = live.owned(&w);
        let r = root(&live);
        assert!(owned.contains(&r.join("claude-1")), "{owned:?}");
        assert!(owned.contains(&r.join("lead")), "{owned:?}");
        assert_eq!(owned.len(), 2, "{owned:?}");
        assert_eq!(Live::tree_of(&w, helper), "claude-1");
        let _ = (dir, repo);
    }

    #[test]
    fn a_look_finds_the_worktrees_no_agent_owns_and_nothing_else() {
        let (dir, repo, mut live, mut w) = state("look", 1);
        let mine = tree(&live, &repo, "claude-1");
        let lost = tree(&live, &repo, "claude-7");
        std::fs::write(lost.join("new.txt"), "n").unwrap();
        let other = tree(&live, &repo, "codex-2");
        // A checkout's worktrees elsewhere under the state, a Gemini
        // private directory, a file and a link are not worktrees to list.
        let far = root_for(&live.state, &dir.join("gone"));
        let far_tree = worktree::ensure(&repo, &far, "solo-3", None).unwrap().path;
        std::fs::create_dir_all(root(&live).join("not-a-tree")).unwrap();
        std::fs::write(root(&live).join("a-file"), "").unwrap();
        std::os::unix::fs::symlink(&other, root(&live).join("a-link")).unwrap();
        let mut gemini = root(&live).into_os_string();
        gemini.push("-gemini");
        std::fs::create_dir_all(PathBuf::from(gemini).join("claude-9")).unwrap();
        assert_eq!(w.orphans, Orphans::default());
        live.look_orphans(&mut w);
        assert!(w.orphans.busy);
        answered(&mut live, &mut w);
        let found = w.orphans.found.clone().unwrap();
        let paths: Vec<&Path> = found.iter().map(|o| o.path.as_path()).collect();
        // Sorted by path: the far checkout's directory comes first.
        let mut want = vec![far_tree.as_path(), lost.as_path(), other.as_path()];
        want.sort();
        assert_eq!(paths, want);
        assert!(!paths.contains(&mine.as_path()));
        let lost = found.iter().find(|o| o.name == "claude-7").unwrap();
        assert_eq!((lost.changes, lost.commits), (1, 0));
        assert_eq!(lost.checkout, repo.canonicalize().unwrap());
        assert!(found.iter().find(|o| o.name == "codex-2").unwrap().clean());
        assert_eq!(w.orphans.tidied, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_look_while_one_is_under_way_waits_for_its_answer() {
        let (dir, _repo, mut live, mut w) = state("busy", 0);
        w.orphans.busy = true;
        live.look_orphans(&mut w);
        thread::sleep(Duration::from_millis(100));
        assert!(!live.take_tidy(&mut w), "a second look was sent");
        w.orphans.busy = false;
        live.look_orphans(&mut w);
        answered(&mut live, &mut w);
        assert_eq!(w.orphans.found, Some(Vec::new()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_worktree_an_agent_came_to_own_is_left_out_of_the_answer() {
        let (dir, repo, mut live, mut w) = state("came", 1);
        tree(&live, &repo, "claude-2");
        live.look_orphans(&mut w);
        // Hired while the look ran: `claude-2` is its now.
        w.agents.push(Agent::new(
            2,
            0,
            "claude-2".into(),
            Vendor::Claude,
            Role::Worker,
            Status::Idle,
            0,
        ));
        live.grow(None, false);
        answered(&mut live, &mut w);
        assert_eq!(names(&w), Vec::<String>::new());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_removal_takes_the_clean_ones_and_what_holds_work_only_when_forced() {
        let (dir, repo, mut live, mut w) = state("remove", 0);
        let clean = tree(&live, &repo, "claude-3");
        let held = tree(&live, &repo, "claude-4");
        std::fs::write(held.join("new.txt"), "n").unwrap();
        // Nothing found yet, nothing asked: refused.
        assert_eq!(live.tidy(&mut w, &[0], false), Err(Refusal::Nothing));
        live.look_orphans(&mut w);
        assert_eq!(live.tidy(&mut w, &[0], false), Err(Refusal::Busy));
        answered(&mut live, &mut w);
        assert_eq!(names(&w), ["claude-3", "claude-4"]);
        assert_eq!(live.tidy(&mut w, &[], false), Err(Refusal::Nothing));
        assert_eq!(live.tidy(&mut w, &[5], false), Err(Refusal::Nothing));
        live.tidy(&mut w, &[0, 1], false).unwrap();
        assert!(w.orphans.busy);
        answered(&mut live, &mut w);
        assert!(!clean.exists());
        assert!(held.join("new.txt").exists(), "work was thrown away");
        let tidied = w.orphans.tidied.clone().unwrap();
        assert_eq!(tidied.removed, ["claude-3"]);
        assert_eq!(tidied.kept.len(), 1);
        assert_eq!(tidied.kept[0].0, "claude-4");
        let why = tidied.kept[0].1.clone().unwrap();
        assert!(why.contains("holds work: 1 files changed"), "{why}");
        // Looked at again after: only what is left.
        assert_eq!(names(&w), ["claude-4"]);
        // The person's yes to it.
        live.tidy(&mut w, &[0], true).unwrap();
        answered(&mut live, &mut w);
        assert!(!held.exists());
        assert_eq!(w.orphans.tidied.clone().unwrap().removed, ["claude-4"]);
        assert_eq!(names(&w), Vec::<String>::new());
        let list = git_in(&repo, &["worktree", "list", "--porcelain"]);
        assert_eq!(list.matches("worktree ").count(), 1, "{list}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_worktree_an_agent_owns_now_is_never_removed() {
        let (dir, repo, mut live, mut w) = state("theirs", 0);
        let path = tree(&live, &repo, "claude-1");
        live.look_orphans(&mut w);
        answered(&mut live, &mut w);
        assert_eq!(names(&w), ["claude-1"]);
        // Hired after the look: the next hire's name is the old tree's.
        w.agents.push(Agent::new(
            1,
            0,
            "claude-1".into(),
            Vendor::Claude,
            Role::Worker,
            Status::Idle,
            0,
        ));
        live.grow(None, false);
        live.tidy(&mut w, &[0], true).unwrap();
        answered(&mut live, &mut w);
        assert!(path.exists(), "an agent's worktree was removed");
        let tidied = w.orphans.tidied.clone().unwrap();
        assert_eq!(tidied.removed, Vec::<String>::new());
        assert_eq!(tidied.kept, [("claude-1".to_string(), None)]);
        assert_eq!(names(&w), Vec::<String>::new());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_wait_returns_once_the_path_is_not_being_removed() {
        let removing = Removing::default();
        let (a, b) = (PathBuf::from("/t/a"), PathBuf::from("/t/b"));
        // Not held: at once.
        removing.wait(&a);
        removing.hold(&[a.clone(), b.clone()]);
        let (tx, rx) = mpsc::channel();
        let r = removing.clone();
        let waited = a.clone();
        thread::spawn(move || {
            r.wait(&waited);
            tx.send(()).unwrap();
        });
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
        // Another path freed: still waits.
        removing.free(&b);
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
        removing.free(&a);
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
    }

    #[test]
    fn a_start_waits_for_its_worktree_s_removal_and_makes_it_anew() {
        let (dir, repo, mut live, mut w) = state("start", 1);
        let path = root(&live).join("claude-1");
        live.tidy.removing.hold(std::slice::from_ref(&path));
        live.told(&mut w, 1, "hello").unwrap();
        thread::sleep(Duration::from_millis(300));
        live.step(&mut w);
        assert!(!path.exists(), "made while it was being removed");
        live.tidy.removing.free(&path);
        wait_for(&mut live, &mut w, |w| w.agents[1].status == Status::Stopped);
        assert!(path.join("args.txt").exists());
        let _ = (std::fs::remove_dir_all(&dir), repo);
    }

    /// Writes `body` as the checkout's teardown script and says yes to it
    /// when `yes`.
    fn teardown(repo: &Path, w: &mut World, body: &str, yes: bool) -> Script {
        std::fs::create_dir_all(repo.join(".imece")).unwrap();
        std::fs::write(repo.join(setup::TEARDOWN_FILE), body).unwrap();
        let s = setup::look_for(repo, Kind::Teardown).unwrap().unwrap();
        if yes {
            w.approve_setup(s.key());
        }
        s
    }

    #[test]
    fn a_teardown_script_not_answered_is_asked_about_and_nothing_goes() {
        let (dir, repo, mut live, mut w) = state("td-ask", 0);
        let path = tree(&live, &repo, "claude-3");
        let s = teardown(&repo, &mut w, "echo bye\n", false);
        assert!(s.key().starts_with("teardown /bin/sh "), "{}", s.key());
        live.look_orphans(&mut w);
        answered(&mut live, &mut w);
        assert_eq!(
            live.tidy(&mut w, &[0], false),
            Err(Refusal::Asks(Box::new(s.clone())))
        );
        assert!(!w.orphans.busy);
        assert!(path.exists());
        // A no holds for the run: removed without it.
        live.decide_teardown(&mut w, &s, false);
        assert!(w.setup_approved.is_empty());
        live.tidy(&mut w, &[0], false).unwrap();
        answered(&mut live, &mut w);
        assert!(!path.exists());
        let tidied = w.orphans.tidied.clone().unwrap();
        assert_eq!(tidied.torn, [("claude-3".to_string(), TornDown::Refused)]);
        assert_eq!(tidied.removed, ["claude-3"]);
        // A yes is kept with the setup scripts'.
        live.decide_teardown(&mut w, &s, true);
        assert_eq!(w.setup_approved, [s.key()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_teardown_said_yes_to_runs_in_each_worktree_before_it_goes_and_a_failure_is_said() {
        let (dir, repo, mut live, mut w) = state("td-run", 0);
        let a = tree(&live, &repo, "claude-3");
        let b = tree(&live, &repo, "claude-4");
        let marks = dir.join("torn.txt");
        let body = format!(
            "printf '%s %s\\n' \"$IMECE_REPO\" \"$IMECE_WORKTREE\" >> {}\n\
             test -e \"$IMECE_WORKTREE/a.txt\" || exit 9\n\
             echo\necho \"dropped $(basename \"$IMECE_WORKTREE\")\"\n\
             case \"$IMECE_WORKTREE\" in *claude-4) exit 3;; esac\n",
            marks.display()
        );
        teardown(&repo, &mut w, &body, true);
        live.look_orphans(&mut w);
        answered(&mut live, &mut w);
        live.tidy(&mut w, &[0, 1], false).unwrap();
        answered(&mut live, &mut w);
        // It ran in each, while it was still there, with the checkout named.
        let checkout = repo.canonicalize().unwrap();
        let said = std::fs::read_to_string(&marks).unwrap();
        assert_eq!(
            said,
            format!(
                "{} {}\n{} {}\n",
                checkout.display(),
                a.display(),
                checkout.display(),
                b.display()
            )
        );
        let tidied = w.orphans.tidied.clone().unwrap();
        // A failed teardown is said; the removal still goes.
        assert_eq!(tidied.removed, ["claude-3", "claude-4"]);
        assert!(!a.exists() && !b.exists());
        let [(n1, t1), (n2, t2)] = tidied.torn.as_slice() else {
            panic!("{:?}", tidied.torn);
        };
        assert_eq!((n1.as_str(), n2.as_str()), ("claude-3", "claude-4"));
        let TornDown::Ran { ok, end, last } = t1 else {
            panic!("{t1:?}");
        };
        assert!(*ok, "{end}");
        assert!(end.starts_with("exit 0, "), "{end}");
        assert_eq!(last.as_deref(), Some("dropped claude-3"));
        let TornDown::Ran { ok, end, last } = t2 else {
            panic!("{t2:?}");
        };
        assert!(!ok);
        assert!(end.starts_with("exit 3, "), "{end}");
        assert_eq!(last.as_deref(), Some("dropped claude-4"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_worktree_a_teardown_left_work_in_stays_and_one_that_can_not_be_read_is_said() {
        let (dir, repo, mut live, mut w) = state("td-left", 0);
        let path = tree(&live, &repo, "claude-3");
        teardown(
            &repo,
            &mut w,
            "echo left > \"$IMECE_WORKTREE/left.txt\"\n",
            true,
        );
        live.look_orphans(&mut w);
        answered(&mut live, &mut w);
        live.tidy(&mut w, &[0], false).unwrap();
        answered(&mut live, &mut w);
        // The removal decides on its own: work is never thrown away.
        assert!(path.join("left.txt").exists());
        let tidied = w.orphans.tidied.clone().unwrap();
        assert_eq!(tidied.removed, Vec::<String>::new());
        assert_eq!(tidied.kept[0].0, "claude-3");
        // A script that is not a plain file: said, and the removal goes.
        std::fs::remove_file(path.join("left.txt")).unwrap();
        std::fs::remove_file(repo.join(setup::TEARDOWN_FILE)).unwrap();
        std::fs::create_dir_all(repo.join(setup::TEARDOWN_FILE)).unwrap();
        live.tidy(&mut w, &[0], false).unwrap();
        answered(&mut live, &mut w);
        assert!(!path.exists());
        let tidied = w.orphans.tidied.clone().unwrap();
        let [(name, TornDown::Failed(why))] = tidied.torn.as_slice() else {
            panic!("{:?}", tidied.torn);
        };
        assert_eq!(name, "claude-3");
        assert_eq!(why, &format!("{}: not a plain file", setup::TEARDOWN_FILE));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_removal_runs_no_teardown_where_the_checkout_has_none() {
        let (dir, repo, mut live, mut w) = state("td-none", 0);
        tree(&live, &repo, "claude-3");
        live.look_orphans(&mut w);
        answered(&mut live, &mut w);
        live.tidy(&mut w, &[0], false).unwrap();
        answered(&mut live, &mut w);
        let tidied = w.orphans.tidied.clone().unwrap();
        assert_eq!(tidied.torn, Vec::new());
        assert_eq!(tidied.removed, ["claude-3"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
