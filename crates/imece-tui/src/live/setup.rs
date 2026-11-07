//! A worktree setup script (record 92): `.imece/worktree-setup.sh` of the
//! person's checkout, run in each new worktree after `:copy`, before the
//! agent's session starts, once the person approved its exact bytes; and
//! its teardown script, `.imece/worktree-teardown.sh`, run the same way
//! in a worktree before `:tidy` removes it (record 96); and the
//! repository's check, `.imece/check`, run in a worker's worktree on the
//! change just read from it, its result a note on the change for the
//! person, never a gate (record 112).
//!
//! The script is the repository's code and runs as the person, so it
//! never runs unapproved. The approval is bound to what runs: the bytes'
//! SHA-256, the file's real path (links resolved) and how it runs
//! ([`RUNNER`]), as one key ([`Script::key`]) kept in imece's state, never
//! in the repository; a changed byte is a new key and is asked again.
//! What runs is a private copy of the bytes that were hashed (written
//! into imece's state, `0700`/`0600`), never the file again, so nothing
//! can swap it between the ask and the run; with the `run` tool's cleared
//! environment and `IMECE_REPO`/`IMECE_WORKTREE`, in its own process
//! group, for at most [`DEADLINE`], its output kept short.

use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use std::thread;

use imece_agent::run::{self, Outcome, Running};

use super::Live;
use crate::model::*;

/// Where the script is, in the person's checkout.
pub const FILE: &str = ".imece/worktree-setup.sh";

/// Where the teardown script is, in the person's checkout.
pub const TEARDOWN_FILE: &str = ".imece/worktree-teardown.sh";

/// Where the repository's check is, in the person's checkout (record 112).
pub const CHECK_FILE: &str = ".imece/check";

/// Which of the scripts: what it is approved as and where it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Setup,
    Teardown,
    Check,
}

impl Kind {
    /// Where it is, in the person's checkout.
    pub fn file(self) -> &'static str {
        match self {
            Kind::Setup => FILE,
            Kind::Teardown => TEARDOWN_FILE,
            Kind::Check => CHECK_FILE,
        }
    }

    /// The first word of its approval's key.
    fn word(self) -> &'static str {
        match self {
            Kind::Setup => "setup",
            Kind::Teardown => "teardown",
            Kind::Check => "check",
        }
    }

    /// The item an unapproved script of this kind puts in TO DECIDE;
    /// a teardown script asks over the `:tidy` window instead.
    pub fn inbox_kind(self) -> InboxKind {
        match self {
            Kind::Check => InboxKind::Check,
            Kind::Setup | Kind::Teardown => InboxKind::Script,
        }
    }
}

/// How it runs: the bytes given to this shell as a file.
pub const RUNNER: &str = "/bin/sh";

/// The biggest script imece takes, in bytes.
pub const SCRIPT_MAX: u64 = 64 << 10;

/// How long a setup or teardown run may take before its group is killed;
/// a check gets the `run` tool's deadline.
pub const DEADLINE: Duration = Duration::from_secs(60);

/// Trees the check ran on, kept with their results: a change read again
/// with the same tree gets its result without a run. The oldest go.
pub const CHECKED_KEEP: usize = 32;

/// The most output a run keeps (its first and last halves), in bytes.
pub const OUTPUT_MAX: usize = 8 << 10;

/// The lines of its output the journal keeps: the last ones.
pub const OUTPUT_LINES: usize = 6;

/// The script as read once: where it really is, its bytes and their hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Script {
    pub kind: Kind,
    pub real: PathBuf,
    pub bytes: Vec<u8>,
    pub sha: String,
}

impl Script {
    /// What the approval is of: how it runs, the bytes' hash, where.
    pub fn key(&self) -> String {
        let word = self.kind.word();
        format!("{word} {RUNNER} {} {}", self.sha, self.real.display())
    }

    /// Its text as the person reads it before saying yes (bytes that are
    /// not UTF-8 replaced; the drawing cleans each line).
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }
}

/// The setup script of the checkout at `repo`, read once (see
/// [`look_for`]).
pub fn look(repo: &Path) -> Result<Option<Script>, String> {
    look_for(repo, Kind::Setup)
}

/// The script of `kind` of the checkout at `repo`, read once: none when
/// there is none; an error (in words) when it is there but can not be
/// taken: not a plain file once links are followed, bigger than
/// [`SCRIPT_MAX`], or unreadable.
pub fn look_for(repo: &Path, kind: Kind) -> Result<Option<Script>, String> {
    use std::io::Read;
    let file_name = kind.file();
    let path = repo.join(file_name);
    let real = match std::fs::canonicalize(&path) {
        Ok(real) => real,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{file_name}: {e}")),
    };
    // The real path, opened without following a link put there since,
    // and without waiting on a pipe; looked at as opened.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&real)
        .map_err(|e| format!("{file_name}: {e}"))?;
    let meta = file.metadata().map_err(|e| format!("{file_name}: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{file_name}: not a plain file"));
    }
    if meta.len() > SCRIPT_MAX {
        return Err(format!("{file_name}: bigger than {} KiB", SCRIPT_MAX >> 10));
    }
    let mut bytes = Vec::new();
    file.take(SCRIPT_MAX)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("{file_name}: {e}"))?;
    let sha = crate::sha::hex(&bytes);
    Ok(Some(Script {
        kind,
        real,
        bytes,
        sha,
    }))
}

/// Writes the script's bytes, the ones hashed, to a private copy in `dir`
/// (made `0700` if it is not there), named by their hash, `0600`: what
/// runs. A copy there already is written again.
pub fn copy_of(dir: &Path, s: &Script) -> std::io::Result<PathBuf> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    let path = dir.join(format!("{}.sh", s.sha));
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)?;
    f.write_all(&s.bytes)?;
    f.sync_all()?;
    Ok(path)
}

/// Runs the private copy `copy` in `worktree` as [`RUNNER`] does, with
/// `IMECE_REPO` and `IMECE_WORKTREE` set, for at most `deadline`: what it
/// came to, or why it did not start.
pub fn run_in(
    copy: &Path,
    repo: &Path,
    worktree: &Path,
    deadline: Duration,
    running: &Running,
) -> Result<Outcome, String> {
    let argv = [RUNNER.to_string(), copy.to_string_lossy().into_owned()];
    let env = [("IMECE_REPO", repo), ("IMECE_WORKTREE", worktree)];
    run::run_env(&argv, worktree, &env, deadline, OUTPUT_MAX, running).map_err(|e| e.to_string())
}

/// What the journal keeps of a run's output: its last [`OUTPUT_LINES`]
/// lines that say something, each cleaned.
pub fn last_lines(output: &str) -> Vec<String> {
    let lines: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();
    let from = lines.len().saturating_sub(OUTPUT_LINES);
    lines[from..]
        .iter()
        .map(|l| crate::i18n::clean(l))
        .collect()
}

/// How a run ended, in the journal's words: `exit 0, 3.2 s` or `killed
/// after 60 s` (its `deadline`); whether it went well.
pub fn ended(o: &Outcome, deadline: Duration) -> (bool, String) {
    let how = if o.timed_out {
        format!("killed after {} s", deadline.as_secs())
    } else {
        match o.code {
            Some(c) => format!("exit {c}, {:.1} s", o.took.as_secs_f64()),
            None => "killed".to_string(),
        }
    };
    (o.ok() && !o.timed_out, how)
}

/// What a session's start does about the setup script, decided before
/// its thread: run this private copy, ask about this script, say why it
/// can not, or nothing (none there, a helper, a leader, said no).
#[derive(Debug)]
pub enum Plan {
    None,
    Run(PathBuf),
    Ask(Script),
    Failed(String),
}

/// What the start's thread did about it, once the worktree was there: it
/// ran (or did not start), it waits for the person's yes, or nothing (no
/// new worktree, or nothing to do).
#[derive(Debug)]
pub enum Done {
    None,
    Ran(Result<Outcome, String>),
    Ask(Script),
}

impl Plan {
    /// What the start's thread does once the worktree is there: only a
    /// worktree made just now is set up.
    pub fn carry_out(self, made: bool, repo: &Path, worktree: &Path, running: &Running) -> Done {
        match (made, self) {
            (false, _) | (true, Plan::None) => Done::None,
            (true, Plan::Run(copy)) => Done::Ran(run_in(&copy, repo, worktree, DEADLINE, running)),
            (true, Plan::Ask(s)) => Done::Ask(s),
            (true, Plan::Failed(why)) => Done::Ran(Err(why)),
        }
    }
}

/// One ask in TO DECIDE: its item, the bytes it asks about, and the
/// agents whose new worktrees (a setup script) or changes (the check) went
/// without them meanwhile.
#[derive(Debug)]
pub struct Ask {
    pub item: usize,
    pub script: Script,
    pub agents: Vec<AgentId>,
}

impl Live {
    /// What agent `id`'s start does about the setup script: only a
    /// worker's or a solo's own worktree (a helper works in its worker's,
    /// a leader in the checkout), only with git.
    pub(super) fn setup_plan(&self, w: &World, id: AgentId) -> Plan {
        let a = &w.agents[id];
        let place = &self.places[a.project];
        if a.role == Role::Leader || a.parent.is_some() || !place.git {
            return Plan::None;
        }
        let script = match look(&place.repo) {
            Ok(Some(s)) => s,
            Ok(None) => return Plan::None,
            Err(why) => return Plan::Failed(why),
        };
        let key = script.key();
        if self.setup_refused.contains(&key) {
            return Plan::None;
        }
        if !w.setup_approved.contains(&key) {
            return Plan::Ask(script);
        }
        match copy_of(&self.setup_dir(), &script) {
            Ok(copy) => Plan::Run(copy),
            Err(e) => Plan::Failed(e.to_string()),
        }
    }

    /// Where the private copies of approved scripts are written.
    pub(super) fn setup_dir(&self) -> PathBuf {
        self.state.join("scripts")
    }

    /// What agent `id`'s start's thread did about the setup script, told:
    /// a run in the journal, an ask in TO DECIDE (one per script,
    /// the agents that went without it counted on it).
    pub(super) fn setup_done(&mut self, w: &mut World, id: AgentId, done: Done) {
        match done {
            Done::None => {}
            Done::Ran(Ok(o)) => {
                let (ok, end) = ended(&o, DEADLINE);
                let lines = last_lines(&o.output);
                w.agents[id].trace(w.tick, format!("worktree setup: {end}"));
                w.note(Event::SetupRan {
                    agent: id,
                    ok,
                    end,
                    lines,
                });
            }
            Done::Ran(Err(why)) => {
                w.note(Event::SetupFailed {
                    agent: id,
                    why: crate::i18n::clean(&why),
                });
            }
            Done::Ask(script) => self.ask_script(w, id, script),
        }
    }

    /// Asks the person about `script`, which agent `id`'s new worktree (a
    /// setup script) or change (the check) went without: one item per
    /// script; an ask already open for the same bytes counts the agent on
    /// it. Approved meanwhile, it runs at once; refused meanwhile, nothing.
    fn ask_script(&mut self, w: &mut World, id: AgentId, script: Script) {
        let key = script.key();
        if self.setup_refused.contains(&key) {
            return;
        }
        if w.setup_approved.contains(&key) {
            return match script.kind {
                Kind::Check => self.run_checks(w, &script, &[id]),
                Kind::Setup | Kind::Teardown => self.run_setup(w, &script, &[id]),
            };
        }
        if script.kind == Kind::Check {
            w.checked(id, Check::Asked);
        }
        let open = |a: &&mut Ask| a.script.key() == key && w.inbox[a.item].open;
        if let Some(ask) = self.setup_asks.iter_mut().find(open) {
            ask.agents.push(id);
            return;
        }
        let item = w.inbox.len();
        w.inbox.push(InboxItem {
            kind: script.kind.inbox_kind(),
            agent: id,
            detail: script.kind.file().to_string(),
            researcher: None,
            open: true,
            tick: w.tick,
            // About a worktree or a change, not a card.
            card: None,
        });
        let what = match script.kind {
            Kind::Check => "check",
            Kind::Setup | Kind::Teardown => "worktree setup",
        };
        w.agents[id].trace(w.tick, format!("{what} waits for your yes: {}", script.sha));
        self.setup_asks.push(Ask {
            item,
            script,
            agents: vec![id],
        });
    }

    /// The script inbox item `item` asks about, for the window that shows
    /// it.
    pub fn setup_asked(&self, item: usize) -> Option<&Script> {
        let ask = self.setup_asks.iter().find(|a| a.item == item)?;
        Some(&ask.script)
    }

    /// The person's answer to the script ask `item`: yes keeps the
    /// approval (in the store) and runs the bytes asked about in the
    /// worktrees, or on the changes, that went without them; no is kept
    /// for the run. False when nothing is asked there.
    pub fn decide_setup(&mut self, w: &mut World, item: usize, yes: bool) -> bool {
        let Some(at) = (self.setup_asks.iter()).position(|a| a.item == item && w.inbox[item].open)
        else {
            return false;
        };
        let ask = self.setup_asks.remove(at);
        w.inbox[item].open = false;
        let agent = w.inbox[item].agent;
        let check = ask.script.kind == Kind::Check;
        w.note(match check {
            true => Event::CheckAnswered { agent, yes },
            false => Event::SetupAnswered { agent, yes },
        });
        let key = ask.script.key();
        if !yes {
            self.setup_refused.insert(key);
            if check {
                for &id in &ask.agents {
                    w.checked(id, Check::None);
                }
            }
            return true;
        }
        w.approve_setup(key);
        match check {
            true => self.run_checks(w, &ask.script, &ask.agents),
            false => self.run_setup(w, &ask.script, &ask.agents),
        }
        true
    }

    /// The repository's check (record 112) on worker `id`'s change just
    /// read, the one git knows: none when the repository has none; run
    /// when the person said yes to its bytes (a tree checked before gets
    /// its result at once, without a run); asked about when not; nothing
    /// after a no this run. One that can not be read is said on the
    /// change and in the journal.
    pub(super) fn check_change(&mut self, w: &mut World, id: AgentId) {
        let Some(tree) = self.changes.get(&id).map(|c| c.tree.clone()) else {
            return;
        };
        let repo = &self.places[w.agents[id].project].repo;
        let script = match look_for(repo, Kind::Check) {
            Ok(Some(s)) => s,
            Ok(None) => return,
            Err(why) => {
                let why = crate::i18n::clean(&why);
                w.checked(id, Check::Failed(why.clone()));
                w.note(Event::CheckFailed { agent: id, why });
                return;
            }
        };
        if let Some((_, done)) = self.checks.iter().find(|(t, _)| *t == tree) {
            let done = done.clone();
            w.agents[id].trace(
                w.tick,
                format!("check: {} (this tree was checked before)", done.end),
            );
            w.checked(id, Check::Ran(done));
            return;
        }
        // Approved: it runs; else asked about, or nothing after a no.
        self.ask_script(w, id, script);
    }

    /// Runs the check `script`'s private copy on the changes of `agents`
    /// that still wait, each on a thread of its own, for at most the `run`
    /// tool's deadline; their ends come back through [`Live::take_checks`]
    /// with the tree they ran on.
    fn run_checks(&mut self, w: &mut World, script: &Script, agents: &[AgentId]) {
        let copy = match copy_of(&self.setup_dir(), script) {
            Ok(copy) => copy,
            Err(e) => {
                let why = crate::i18n::clean(&e.to_string());
                for &id in agents {
                    if w.checked(id, Check::Failed(why.clone())) {
                        w.note(Event::CheckFailed {
                            agent: id,
                            why: why.clone(),
                        });
                    }
                }
                return;
            }
        };
        for &id in agents {
            // The change went meanwhile: nothing to check.
            let Some(c) = self.changes.get(&id) else {
                continue;
            };
            let (tree, path) = (c.tree.clone(), c.path.clone());
            w.checked(id, Check::Running);
            w.agents[id].trace(w.tick, "check: runs on the change");
            let repo = self.places[w.agents[id].project].repo.clone();
            let (copy, running, tx, deadline) = (
                copy.clone(),
                self.running[id].clone(),
                self.check_tx.clone(),
                self.run_timeout,
            );
            thread::spawn(move || {
                let ran = run_in(&copy, &repo, &path, deadline, &running);
                let _ = tx.send((id, tree, ran));
            });
        }
    }

    /// The checks that ended since the last step: on the change they ran
    /// on when it still waits, in the journal; a tree's result is kept for
    /// a change read again with the same tree.
    pub(super) fn take_checks(&mut self, w: &mut World) -> bool {
        let mut any = false;
        while let Ok((id, tree, ran)) = self.check_rx.try_recv() {
            any = true;
            self.check_done(w, id, tree, ran);
        }
        any
    }

    fn check_done(
        &mut self,
        w: &mut World,
        id: AgentId,
        tree: String,
        ran: Result<Outcome, String>,
    ) {
        // The change it ran on must still be the one waiting; a newer
        // one, or none, hears nothing of it.
        let waits = self.changes.get(&id).is_some_and(|c| c.tree == tree);
        match ran {
            Ok(o) => {
                let (ok, end) = ended(&o, self.run_timeout);
                let done = Checked {
                    ok,
                    end: end.clone(),
                    lines: last_lines(&o.output),
                };
                self.checks.retain(|(t, _)| *t != tree);
                self.checks.push_back((tree, done.clone()));
                if self.checks.len() > CHECKED_KEEP {
                    self.checks.pop_front();
                }
                if !waits {
                    w.agents[id].trace(w.tick, format!("check: {end}, the change is gone"));
                    return;
                }
                w.agents[id].trace(w.tick, format!("check: {end}"));
                w.checked(id, Check::Ran(done.clone()));
                w.note(Event::Checked {
                    agent: id,
                    ok,
                    end,
                    lines: done.lines,
                });
            }
            Err(why) => {
                let why = crate::i18n::clean(&why);
                if !waits {
                    w.agents[id].trace(w.tick, format!("check: {why}, the change is gone"));
                    return;
                }
                w.checked(id, Check::Failed(why.clone()));
                w.note(Event::CheckFailed { agent: id, why });
            }
        }
    }

    /// Runs `script`'s private copy in the worktrees of `agents` that
    /// still have one running, each on a thread of its own; their ends
    /// come back through [`Live::take_setups`].
    fn run_setup(&mut self, w: &mut World, script: &Script, agents: &[AgentId]) {
        let copy = match copy_of(&self.setup_dir(), script) {
            Ok(copy) => copy,
            Err(e) => {
                let why = e.to_string();
                for &agent in agents {
                    let why = why.clone();
                    w.note(Event::SetupFailed { agent, why });
                }
                return;
            }
        };
        for &id in agents {
            let Some(tree) = self.cwds[id].clone().filter(|_| !w.agents[id].gone) else {
                continue;
            };
            let repo = self.places[w.agents[id].project].repo.clone();
            let (copy, running, tx) = (
                copy.clone(),
                self.running[id].clone(),
                self.setup_tx.clone(),
            );
            thread::spawn(move || {
                let _ = tx.send((id, run_in(&copy, &repo, &tree, DEADLINE, &running)));
            });
        }
    }

    /// The setup runs that ended since the last step, in the journal.
    pub(super) fn take_setups(&mut self, w: &mut World) -> bool {
        let mut any = false;
        while let Ok((id, ran)) = self.setup_rx.try_recv() {
            any = true;
            self.setup_done(w, id, Done::Ran(ran));
        }
        any
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("imece-setup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join(".imece")).unwrap();
        d
    }

    use crate::live::testing::{idle_live_in, team};

    /// A live team of `workers` on checkout `repo`, its state in a
    /// directory of its own.
    fn live_on(repo: &Path, workers: usize) -> (World, Live) {
        let w = team(workers);
        let mut live = idle_live_in(repo, workers + 1);
        live.state = repo.join("state");
        (w, live)
    }

    fn outcome(code: Option<i32>, output: &str) -> Outcome {
        Outcome {
            code,
            timed_out: false,
            output: output.into(),
            bytes: output.len(),
            took: Duration::from_millis(1500),
        }
    }

    #[test]
    fn a_start_plans_to_run_ask_or_nothing_by_who_starts_and_what_was_said() {
        let repo = temp("plan");
        let (mut w, mut live) = live_on(&repo, 2);
        assert!(matches!(live.setup_plan(&w, 1), Plan::None), "no script");
        std::fs::write(repo.join(FILE), "echo set\n").unwrap();
        let s = look(&repo).unwrap().unwrap();
        assert!(matches!(live.setup_plan(&w, 1), Plan::Ask(a) if a == s));
        // A leader works in the checkout, a helper in its worker's tree.
        assert!(matches!(live.setup_plan(&w, 0), Plan::None));
        w.agents[2].parent = Some(1);
        assert!(matches!(live.setup_plan(&w, 2), Plan::None));
        w.agents[2].parent = None;
        // Without git there is no worktree.
        live.places[0].git = false;
        assert!(matches!(live.setup_plan(&w, 1), Plan::None));
        live.places[0].git = true;
        // Said yes to: the private copy runs.
        w.approve_setup(s.key());
        let copy = live.setup_dir().join(format!("{}.sh", s.sha));
        assert!(matches!(live.setup_plan(&w, 1), Plan::Run(p) if p == copy));
        assert_eq!(std::fs::read(&copy).unwrap(), b"echo set\n");
        // A byte changed: asked again.
        std::fs::write(repo.join(FILE), "echo set!\n").unwrap();
        assert!(matches!(live.setup_plan(&w, 1), Plan::Ask(_)));
        // Said no to this run: nothing.
        let changed = look(&repo).unwrap().unwrap();
        live.setup_refused.insert(changed.key());
        assert!(matches!(live.setup_plan(&w, 1), Plan::None));
        // Not one to take: said why.
        std::fs::remove_file(repo.join(FILE)).unwrap();
        std::fs::create_dir(repo.join(FILE)).unwrap();
        assert!(
            matches!(live.setup_plan(&w, 1), Plan::Failed(why) if why.ends_with("not a plain file"))
        );
        // Its copy can not be written: said why.
        std::fs::remove_dir(repo.join(FILE)).unwrap();
        std::fs::write(repo.join(FILE), "echo set\n").unwrap();
        std::fs::remove_dir_all(repo.join("state")).unwrap();
        std::fs::write(repo.join("state"), "a file where the directory goes").unwrap();
        assert!(matches!(live.setup_plan(&w, 1), Plan::Failed(_)));
        assert_eq!(live.setup_dir(), repo.join("state").join("scripts"));
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn only_a_worktree_made_now_is_set_up() {
        let repo = temp("carry");
        let tree = repo.join("tree");
        std::fs::create_dir_all(&tree).unwrap();
        std::fs::write(repo.join(FILE), "echo ran\n").unwrap();
        let s = look(&repo).unwrap().unwrap();
        let copy = copy_of(&repo.join("state"), &s).unwrap();
        let r = Running::default();
        let run = |made| Plan::Run(copy.clone()).carry_out(made, &repo, &tree, &r);
        assert!(matches!(run(false), Done::None));
        assert!(matches!(run(true), Done::Ran(Ok(o)) if o.output == "ran"));
        assert!(matches!(
            Plan::Ask(s.clone()).carry_out(false, &repo, &tree, &r),
            Done::None
        ));
        assert!(
            matches!(Plan::Ask(s.clone()).carry_out(true, &repo, &tree, &r), Done::Ask(a) if a == s)
        );
        let failed = Plan::Failed("no".into());
        assert!(matches!(failed.carry_out(true, &repo, &tree, &r), Done::Ran(Err(w)) if w == "no"));
        assert!(matches!(
            Plan::None.carry_out(true, &repo, &tree, &r),
            Done::None
        ));
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn what_a_start_did_is_journaled_and_an_ask_is_one_per_script() {
        let repo = temp("done");
        let (mut w, mut live) = live_on(&repo, 3);
        live.setup_done(&mut w, 1, Done::None);
        assert!(w.journal.is_empty() && w.inbox.is_empty());
        live.setup_done(&mut w, 1, Done::Ran(Ok(outcome(Some(0), "a\nb\n"))));
        assert_eq!(
            w.journal[0].event,
            Event::SetupRan {
                agent: 1,
                ok: true,
                end: "exit 0, 1.5 s".into(),
                lines: vec!["a".into(), "b".into()],
            }
        );
        assert!(
            w.agents[1]
                .trace
                .back()
                .unwrap()
                .ends_with("worktree setup: exit 0, 1.5 s")
        );
        live.setup_done(&mut w, 1, Done::Ran(Ok(outcome(Some(2), ""))));
        assert!(
            matches!(&w.journal[1].event, Event::SetupRan { ok: false, lines, .. } if lines.is_empty())
        );
        live.setup_done(&mut w, 2, Done::Ran(Err("TOKEN=abc gone".into())));
        assert_eq!(
            w.journal[2].event,
            Event::SetupFailed {
                agent: 2,
                why: "[***] gone".into()
            }
        );
        // An ask: one item for the script, the next worker counted on it.
        std::fs::write(repo.join(FILE), "echo hi\n").unwrap();
        let s = look(&repo).unwrap().unwrap();
        live.setup_done(&mut w, 1, Done::Ask(s.clone()));
        live.setup_done(&mut w, 2, Done::Ask(s.clone()));
        assert_eq!(w.inbox.len(), 1);
        let item = &w.inbox[0];
        assert_eq!(
            (
                item.kind,
                item.agent,
                item.detail.as_str(),
                item.card,
                item.open
            ),
            (InboxKind::Script, 1, FILE, None, true)
        );
        assert_eq!(live.setup_asks[0].agents, [1, 2]);
        assert_eq!(live.setup_asked(0), Some(&s));
        assert_eq!(live.setup_asked(1), None);
        // Another script: an ask of its own.
        let other = Script {
            sha: "0".repeat(64),
            ..s.clone()
        };
        live.setup_done(&mut w, 3, Done::Ask(other));
        assert_eq!(w.inbox.len(), 2);
        // Closed (a start): the next one asks anew.
        w.inbox[0].open = false;
        live.setup_done(&mut w, 3, Done::Ask(s.clone()));
        assert_eq!(w.inbox.len(), 3);
        // Said no to: no ask.
        live.setup_refused.insert(s.key());
        live.setup_done(&mut w, 3, Done::Ask(s.clone()));
        assert_eq!(w.inbox.len(), 3);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_yes_keeps_the_approval_and_runs_the_bytes_asked_about_in_the_trees_that_went_without() {
        let repo = temp("yes");
        let (mut w, mut live) = live_on(&repo, 3);
        let trees: Vec<PathBuf> = (1..=2).map(|i| repo.join(format!("t{i}"))).collect();
        for (i, t) in trees.iter().enumerate() {
            std::fs::create_dir_all(t).unwrap();
            live.cwds[i + 1] = Some(t.clone());
        }
        std::fs::write(repo.join(FILE), "basename \"$IMECE_WORKTREE\"\n").unwrap();
        let s = look(&repo).unwrap().unwrap();
        live.setup_done(&mut w, 1, Done::Ask(s.clone()));
        live.setup_done(&mut w, 2, Done::Ask(s.clone()));
        live.setup_done(&mut w, 3, Done::Ask(s.clone()));
        assert!(!live.decide_setup(&mut w, 1, true), "nothing asked there");
        // The file changes after the ask: what was asked about runs.
        std::fs::write(repo.join(FILE), "echo swapped\n").unwrap();
        assert!(live.decide_setup(&mut w, 0, true));
        assert!(!w.inbox[0].open);
        assert_eq!(w.setup_approved, [s.key()]);
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::SetupAnswered {
                agent: 1,
                yes: true
            }
        );
        assert!(!live.decide_setup(&mut w, 0, true), "answered already");
        // Agent 3 has no worktree running: nothing runs for it.
        let (mut ran, mut said) = (Vec::new(), false);
        for _ in 0..400 {
            said |= live.take_setups(&mut w);
            ran = (w.journal.iter())
                .filter_map(|e| match &e.event {
                    Event::SetupRan { agent, lines, .. } => Some((*agent, lines.clone())),
                    _ => None,
                })
                .collect();
            if ran.len() == 2 {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        ran.sort();
        assert_eq!(
            ran,
            [(1, vec!["t1".to_string()]), (2, vec!["t2".to_string()])]
        );
        assert!(said, "a step that took a run says so");
        assert!(!live.take_setups(&mut w), "nothing more");
        // Approved meanwhile: a start's ask runs at once, asks nothing.
        let before = w.inbox.len();
        live.setup_done(&mut w, 2, Done::Ask(s.clone()));
        assert_eq!(w.inbox.len(), before);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_no_holds_for_the_run_and_a_copy_that_can_not_be_written_is_said() {
        let repo = temp("no");
        let (mut w, mut live) = live_on(&repo, 2);
        std::fs::write(repo.join(FILE), "echo hi\n").unwrap();
        let s = look(&repo).unwrap().unwrap();
        live.setup_done(&mut w, 1, Done::Ask(s.clone()));
        assert!(live.decide_setup(&mut w, 0, false));
        assert!(live.setup_refused.contains(&s.key()));
        assert!(w.setup_approved.is_empty());
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::SetupAnswered {
                agent: 1,
                yes: false
            }
        );
        // A yes whose copy can not be written: each agent told.
        live.setup_refused.clear();
        live.setup_done(&mut w, 1, Done::Ask(s.clone()));
        live.setup_done(&mut w, 2, Done::Ask(s.clone()));
        std::fs::write(repo.join("state"), "not a directory").unwrap();
        assert!(live.decide_setup(&mut w, 1, true));
        let failed: Vec<AgentId> = (w.journal.iter())
            .filter_map(|e| match e.event {
                Event::SetupFailed { agent, .. } => Some(agent),
                _ => None,
            })
            .collect();
        assert_eq!(failed, [1, 2]);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_script_is_read_once_by_its_real_path_and_hashed() {
        let repo = temp("look");
        assert_eq!(look(&repo), Ok(None), "none there");
        std::fs::write(repo.join(FILE), "echo hi\n").unwrap();
        let s = look(&repo).unwrap().unwrap();
        assert_eq!(s.real, std::fs::canonicalize(repo.join(FILE)).unwrap());
        assert_eq!(s.bytes, b"echo hi\n");
        assert_eq!(s.sha, crate::sha::hex(b"echo hi\n"));
        assert_eq!(s.text(), "echo hi\n");
        assert_eq!(
            s.key(),
            format!("setup /bin/sh {} {}", s.sha, s.real.display())
        );
        // A link: its target is where it really is, and part of the key.
        let other = repo.join("elsewhere.sh");
        std::fs::write(&other, "echo hi\n").unwrap();
        std::fs::remove_file(repo.join(FILE)).unwrap();
        std::os::unix::fs::symlink(&other, repo.join(FILE)).unwrap();
        let linked = look(&repo).unwrap().unwrap();
        assert_eq!(linked.real, std::fs::canonicalize(&other).unwrap());
        assert_eq!(linked.sha, s.sha);
        assert_ne!(linked.key(), s.key(), "another place, another approval");
        // Not a file, too big: refused, said why.
        std::fs::remove_file(repo.join(FILE)).unwrap();
        std::fs::create_dir(repo.join(FILE)).unwrap();
        assert!(look(&repo).unwrap_err().ends_with("not a plain file"));
        std::fs::remove_dir(repo.join(FILE)).unwrap();
        std::fs::write(repo.join(FILE), vec![b'#'; SCRIPT_MAX as usize + 1]).unwrap();
        assert!(look(&repo).unwrap_err().ends_with("bigger than 64 KiB"));
        std::fs::write(repo.join(FILE), vec![b'#'; SCRIPT_MAX as usize]).unwrap();
        assert_eq!(
            look(&repo).unwrap().unwrap().bytes.len(),
            SCRIPT_MAX as usize
        );
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_script_path_that_can_not_be_resolved_is_said_not_taken_for_none() {
        let repo = temp("notdir");
        std::fs::remove_dir_all(repo.join(".imece")).unwrap();
        // `.imece` a file: the path fails other than by not being there.
        std::fs::write(repo.join(".imece"), "x").unwrap();
        let why = look(&repo).unwrap_err();
        assert!(why.starts_with(&format!("{FILE}: ")), "{why}");
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_pipe_where_the_script_goes_is_refused_without_waiting_for_a_writer() {
        let repo = temp("fifo");
        let path = repo.join(FILE).into_os_string().into_encoded_bytes();
        let path = std::ffi::CString::new(path).unwrap();
        // SAFETY: a valid C string; mkfifo only makes the node.
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        let (tx, rx) = std::sync::mpsc::channel();
        let at = repo.clone();
        std::thread::spawn(move || {
            let _ = tx.send(look(&at));
        });
        let got = (rx.recv_timeout(Duration::from_secs(5))).expect("not stuck on the pipe");
        assert!(got.unwrap_err().ends_with("not a plain file"));
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn the_copy_is_private_and_what_runs_is_it_with_the_two_places_set() {
        let repo = temp("run");
        let state = repo.join("state").join("scripts");
        let tree = repo.join("tree");
        std::fs::create_dir_all(&tree).unwrap();
        std::fs::write(
            repo.join(FILE),
            "echo \"$IMECE_REPO|$IMECE_WORKTREE|$(pwd -P)\"\nexit 3\n",
        )
        .unwrap();
        let s = look(&repo).unwrap().unwrap();
        let copy = copy_of(&state, &s).unwrap();
        assert_eq!(copy, state.join(format!("{}.sh", s.sha)));
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&state), 0o700);
        assert_eq!(mode(&copy), 0o600);
        // The file changes after it was read: the copy runs, as hashed.
        std::fs::write(repo.join(FILE), "echo swapped\n").unwrap();
        let o = run_in(&copy, &repo, &tree, DEADLINE, &Running::default()).unwrap();
        let real_tree = std::fs::canonicalize(&tree).unwrap();
        assert_eq!(
            o.output,
            format!(
                "{}|{}|{}",
                repo.display(),
                tree.display(),
                real_tree.display()
            )
        );
        assert!(!ended(&o, DEADLINE).0);
        assert!(
            ended(&o, DEADLINE).1.starts_with("exit 3, "),
            "{}",
            ended(&o, DEADLINE).1
        );
        // Written again over a copy that is there.
        assert_eq!(copy_of(&state, &s).unwrap(), copy);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_run_ends_well_only_with_exit_0_in_time() {
        let o = |code, timed_out| Outcome {
            code,
            timed_out,
            output: String::new(),
            bytes: 0,
            took: Duration::from_millis(3200),
        };
        assert_eq!(
            ended(&o(Some(0), false), DEADLINE),
            (true, "exit 0, 3.2 s".into())
        );
        assert_eq!(
            ended(&o(Some(1), false), DEADLINE),
            (false, "exit 1, 3.2 s".into())
        );
        assert_eq!(
            ended(&o(None, true), DEADLINE),
            (false, "killed after 60 s".into())
        );
        assert_eq!(
            ended(&o(Some(0), true), DEADLINE),
            (false, "killed after 60 s".into())
        );
        assert_eq!(ended(&o(None, false), DEADLINE), (false, "killed".into()));
        assert_eq!(DEADLINE, Duration::from_secs(60));
    }

    #[test]
    fn the_journal_keeps_the_last_lines_that_say_something_cleaned() {
        let out = "1\n\n2\n3\n4\n  \n5\n6\n7\nTOKEN=abc";
        assert_eq!(last_lines(out), ["3", "4", "5", "6", "7", "[***]"]);
        assert_eq!(last_lines("a\n\nb"), ["a", "b"]);
        assert!(last_lines("").is_empty());
        assert_eq!(OUTPUT_LINES, 6);
    }

    /// A change of worker `id` as git knows it (`tree`, the worktree
    /// `path`) and as the person sees it.
    fn change_at(w: &mut World, live: &mut Live, id: AgentId, tree: &str, path: &Path) {
        live.changes.insert(
            id,
            imece_agent::worktree::Change {
                path: path.to_path_buf(),
                tree: tree.into(),
                files: Vec::new(),
                patch: String::new(),
            },
        );
        w.change_ready(Change {
            agent: id,
            title: tree.into(),
            files: Vec::new(),
            patch: String::new(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
    }

    fn check_of(w: &World, id: AgentId) -> Check {
        w.change_of(id).map(|c| c.check.clone()).unwrap_or_default()
    }

    /// Takes the checks that end until `done` holds, ten seconds at most.
    fn wait_until(w: &mut World, live: &mut Live, done: impl Fn(&World, &Live) -> bool) {
        let until = std::time::Instant::now() + Duration::from_secs(10);
        while !done(w, live) {
            assert!(
                std::time::Instant::now() < until,
                "nothing came in ten seconds"
            );
            live.take_checks(w);
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn ran_of(w: &World, id: AgentId) -> Checked {
        match check_of(w, id) {
            Check::Ran(d) => d,
            other => panic!("not ran: {other:?}"),
        }
    }

    #[test]
    fn a_change_read_is_checked_once_the_person_said_yes_and_a_tree_s_result_is_kept() {
        let repo = temp("check");
        let tree = repo.join("tree");
        std::fs::create_dir_all(&tree).unwrap();
        let (mut w, mut live) = live_on(&repo, 2);
        // No check in the repository: nothing on the change, nothing asked.
        change_at(&mut w, &mut live, 1, "t0", &tree);
        live.check_change(&mut w, 1);
        assert_eq!(check_of(&w, 1), Check::None);
        assert!(w.inbox.iter().all(|i| i.kind != InboxKind::Check));
        assert!(w.journal.len() == 1, "the change alone: {:?}", w.journal);
        // One there the person has not said yes to: asked, said on the
        // change; a second worker's change counts on the same ask.
        std::fs::write(
            repo.join(CHECK_FILE),
            "echo ran >> \"$IMECE_REPO/runs.txt\"\necho tests pass\n",
        )
        .unwrap();
        change_at(&mut w, &mut live, 1, "t1", &tree);
        live.check_change(&mut w, 1);
        assert_eq!(check_of(&w, 1), Check::Asked);
        let asks = |w: &World| -> Vec<usize> {
            (w.inbox.iter().enumerate())
                .filter(|(_, i)| i.kind == InboxKind::Check && i.open)
                .map(|(n, _)| n)
                .collect()
        };
        let item = asks(&w)[0];
        assert_eq!(asks(&w).len(), 1);
        let it = &w.inbox[item];
        assert_eq!(
            (it.agent, it.detail.as_str(), it.card),
            (1, CHECK_FILE, None)
        );
        assert_eq!(CHECK_FILE, ".imece/check");
        let last = |w: &World, id: AgentId| w.agents[id].trace.back().unwrap().clone();
        assert!(
            last(&w, 1).contains("check waits for your yes"),
            "{}",
            last(&w, 1)
        );
        change_at(&mut w, &mut live, 2, "t2", &tree);
        live.check_change(&mut w, 2);
        assert_eq!(check_of(&w, 2), Check::Asked);
        assert_eq!(asks(&w).len(), 1, "one ask per script");
        assert_eq!(live.setup_asks[0].agents, [1, 2]);
        assert_eq!(live.setup_asked(item).unwrap().kind, Kind::Check);
        // Yes: the approval is kept and both changes are checked.
        assert!(live.decide_setup(&mut w, item, true));
        assert!(!w.inbox[item].open);
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::CheckAnswered {
                agent: 1,
                yes: true
            }
        );
        let key = look_for(&repo, Kind::Check).unwrap().unwrap().key();
        assert!(key.starts_with("check /bin/sh "), "{key}");
        assert!(w.setup_approved.contains(&key));
        assert_eq!(
            (check_of(&w, 1), check_of(&w, 2)),
            (Check::Running, Check::Running)
        );
        assert!(
            last(&w, 1).ends_with("check: runs on the change"),
            "{}",
            last(&w, 1)
        );
        let past_running = |w: &World, id| !matches!(check_of(w, id), Check::Running);
        wait_until(&mut w, &mut live, |w, _| {
            past_running(w, 1) && past_running(w, 2)
        });
        let d = ran_of(&w, 1);
        assert!(d.ok && d.end.starts_with("exit 0, "), "{d:?}");
        assert_eq!(d.lines, ["tests pass"]);
        assert!(ran_of(&w, 2).ok);
        assert!(last(&w, 1).contains("check: exit 0, "), "{}", last(&w, 1));
        assert!(w.journal.iter().any(|e| matches!(
            &e.event,
            Event::Checked { agent: 1, ok: true, end, lines }
                if end.starts_with("exit 0, ") && lines == &["tests pass".to_string()]
        )));
        let runs = |repo: &Path| {
            std::fs::read_to_string(repo.join("runs.txt"))
                .unwrap_or_default()
                .lines()
                .count()
        };
        assert_eq!(runs(&repo), 2);
        // The same tree read again gets its result at once, without a run.
        change_at(&mut w, &mut live, 1, "t1", &tree);
        live.check_change(&mut w, 1);
        assert_eq!(ran_of(&w, 1), d);
        assert_eq!(runs(&repo), 2);
        assert!(
            last(&w, 1).ends_with("(this tree was checked before)"),
            "{}",
            last(&w, 1)
        );
        // A change gone before its check ends: the result is kept, said
        // in the trace alone.
        change_at(&mut w, &mut live, 1, "t3", &tree);
        live.check_change(&mut w, 1);
        assert_eq!(check_of(&w, 1), Check::Running);
        live.changes.remove(&1);
        w.change_rejected(1);
        let journaled = w.journal.len();
        wait_until(&mut w, &mut live, |_, live| {
            live.checks.iter().any(|(t, _)| t == "t3")
        });
        assert_eq!(w.journal.len(), journaled, "nothing said of a change gone");
        assert!(
            last(&w, 1).ends_with("the change is gone"),
            "{}",
            last(&w, 1)
        );
        change_at(&mut w, &mut live, 1, "t3", &tree);
        live.check_change(&mut w, 1);
        assert!(ran_of(&w, 1).ok, "from the kept result");
        assert_eq!(runs(&repo), 3);
        // A check that fails: its exit and its last lines, on the change
        // and in the journal.
        std::fs::write(
            repo.join(CHECK_FILE),
            "echo building\necho\necho FAIL a.rs\nexit 2\n",
        )
        .unwrap();
        change_at(&mut w, &mut live, 1, "t4", &tree);
        live.check_change(&mut w, 1);
        assert_eq!(check_of(&w, 1), Check::Asked, "other bytes: asked again");
        let item = asks(&w)[0];
        assert!(live.decide_setup(&mut w, item, true));
        wait_until(&mut w, &mut live, |w, _| past_running(w, 1));
        let d = ran_of(&w, 1);
        assert!(!d.ok && d.end.starts_with("exit 2, "), "{d:?}");
        assert_eq!(d.lines, ["building", "FAIL a.rs"]);
        assert!(w.journal.iter().any(|e| matches!(
            &e.event,
            Event::Checked { agent: 1, ok: false, lines, .. } if lines.len() == 2
        )));
        // One that outlives the run tool's deadline is killed.
        live.run_timeout = Duration::from_millis(200);
        std::fs::write(repo.join(CHECK_FILE), "sleep 5\n").unwrap();
        change_at(&mut w, &mut live, 1, "t5", &tree);
        live.check_change(&mut w, 1);
        let item = asks(&w)[0];
        assert!(live.decide_setup(&mut w, item, true));
        wait_until(&mut w, &mut live, |w, _| past_running(w, 1));
        let d = ran_of(&w, 1);
        assert_eq!((d.ok, d.end.as_str()), (false, "killed after 0 s"));
        // No: nothing on the change, and not asked again this run.
        std::fs::write(repo.join(CHECK_FILE), "exit 0\n").unwrap();
        change_at(&mut w, &mut live, 1, "t6", &tree);
        live.check_change(&mut w, 1);
        assert_eq!(check_of(&w, 1), Check::Asked);
        let item = asks(&w)[0];
        assert!(live.decide_setup(&mut w, item, false));
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::CheckAnswered {
                agent: 1,
                yes: false
            }
        );
        assert_eq!(check_of(&w, 1), Check::None);
        change_at(&mut w, &mut live, 1, "t7", &tree);
        live.check_change(&mut w, 1);
        assert_eq!(check_of(&w, 1), Check::None);
        assert!(asks(&w).is_empty());
        assert!(!live.decide_setup(&mut w, item, true), "answered already");
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_check_that_can_not_be_read_or_copied_is_said_on_the_change() {
        let repo = temp("check-fail");
        let tree = repo.join("tree");
        std::fs::create_dir_all(&tree).unwrap();
        let (mut w, mut live) = live_on(&repo, 1);
        std::fs::create_dir(repo.join(CHECK_FILE)).unwrap();
        change_at(&mut w, &mut live, 1, "t1", &tree);
        live.check_change(&mut w, 1);
        let Check::Failed(why) = check_of(&w, 1) else {
            panic!("{:?}", check_of(&w, 1));
        };
        assert!(why.ends_with("not a plain file"), "{why}");
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::CheckFailed { agent: 1, why }
        );
        // Said yes to, but its private copy can not be written.
        std::fs::remove_dir(repo.join(CHECK_FILE)).unwrap();
        std::fs::write(repo.join(CHECK_FILE), "exit 0\n").unwrap();
        w.approve_setup(look_for(&repo, Kind::Check).unwrap().unwrap().key());
        std::fs::write(repo.join("state"), "a file where the directory goes").unwrap();
        change_at(&mut w, &mut live, 1, "t2", &tree);
        live.check_change(&mut w, 1);
        assert!(matches!(check_of(&w, 1), Check::Failed(why) if !why.is_empty()));
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::CheckFailed { agent: 1, .. }
        ));
        // Nothing to check: a change gone before the yes.
        std::fs::remove_file(repo.join("state")).unwrap();
        live.changes.remove(&1);
        w.change_rejected(1);
        let journaled = w.journal.len();
        let script = look_for(&repo, Kind::Check).unwrap().unwrap();
        live.run_checks(&mut w, &script, &[1]);
        assert_eq!(w.journal.len(), journaled);
        assert!(!live.take_checks(&mut w), "nothing came");
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn the_check_is_its_own_kind_and_the_kept_results_are_bounded() {
        assert_eq!(Kind::Check.file(), CHECK_FILE);
        assert_eq!(Kind::Check.word(), "check");
        assert_eq!(Kind::Check.inbox_kind(), InboxKind::Check);
        assert_eq!(Kind::Setup.inbox_kind(), InboxKind::Script);
        assert_eq!(Kind::Teardown.inbox_kind(), InboxKind::Script);
        assert_eq!(CHECKED_KEEP, 32);
        let repo = temp("check-kept");
        let (mut w, mut live) = live_on(&repo, 1);
        for i in 0..=CHECKED_KEEP {
            live.check_done(&mut w, 1, format!("tree{i}"), Ok(outcome(Some(0), "fine")));
        }
        // The same tree again keeps one entry, the newest.
        live.check_done(&mut w, 1, "tree5".into(), Ok(outcome(Some(1), "")));
        assert_eq!(live.checks.len(), CHECKED_KEEP);
        let trees: Vec<&str> = live.checks.iter().map(|(t, _)| t.as_str()).collect();
        assert!(!trees.contains(&"tree0"), "the oldest went: {trees:?}");
        assert!(trees.contains(&"tree32"));
        assert_eq!(trees.iter().filter(|t| **t == "tree5").count(), 1);
        assert_eq!(trees.last(), Some(&"tree5"));
        assert!(!live.checks[CHECKED_KEEP - 1].1.ok);
        assert!(w.journal.is_empty(), "no change waited: nothing said");
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_run_that_could_not_run_is_said_on_the_change_that_still_waits_alone() {
        let repo = temp("check-run-failed");
        let tree = repo.join("tree");
        let (mut w, mut live) = live_on(&repo, 1);
        let last = |w: &World, id: AgentId| w.agents[id].trace.back().unwrap().clone();
        // The change it ran on still waits: on the change and in the journal.
        change_at(&mut w, &mut live, 1, "t1", &tree);
        live.check_done(&mut w, 1, "t1".into(), Err("no\u{7}shell".into()));
        assert_eq!(check_of(&w, 1), Check::Failed("no shell".into()), "cleaned");
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::CheckFailed {
                agent: 1,
                why: "no shell".into()
            }
        );
        assert!(
            w.agents[1].trace.is_empty(),
            "the journal has it, not the trace"
        );
        // A newer change waits in its place: the trace alone hears of it.
        change_at(&mut w, &mut live, 1, "t2", &tree);
        let journaled = w.journal.len();
        live.check_done(&mut w, 1, "t1".into(), Err("too late".into()));
        assert_eq!(check_of(&w, 1), Check::None, "t2 untouched");
        assert_eq!(w.journal.len(), journaled);
        assert!(
            last(&w, 1).ends_with("check: too late, the change is gone"),
            "{}",
            last(&w, 1)
        );
        // None waits: the same.
        live.changes.remove(&1);
        w.change_rejected(1);
        let journaled = w.journal.len();
        live.check_done(&mut w, 1, "t2".into(), Err("too late".into()));
        assert_eq!(w.journal.len(), journaled);
        assert!(
            last(&w, 1).ends_with("check: too late, the change is gone"),
            "{}",
            last(&w, 1)
        );
        assert!(
            live.checks.is_empty(),
            "a run that did not run keeps no result"
        );
        let _ = std::fs::remove_dir_all(&repo);
    }
}
