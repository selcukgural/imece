//! Teams for the tests of `live` and of the screen.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;

/// A team without sessions, to apply events to.
pub(crate) fn team(workers: usize) -> World {
    let mut w = World {
        tick: 3,
        // Begun just now: a step puts the tick at 0.
        started_at: now(),
        run_spent: 0,
        spending: Spending::default(),
        projects: vec![Project {
            id: 0,
            name: "repo".into(),
            leader: 0,
            trunk: None,
            repo: &[],
            closed: false,
        }],
        agents: Vec::new(),
        cards: Vec::new(),
        journal: Vec::new(),
        inbox: Vec::new(),
        budget: 10_000,
        max_researchers: 0,
        research_queue: Default::default(),
        plans: Vec::new(),
        changes: Vec::new(),
    };
    w.agents.push(Agent::new(
        0,
        0,
        "lead".into(),
        Vendor::Claude,
        Role::Leader,
        Status::Idle,
        0,
    ));
    for i in 1..=workers {
        w.agents.push(Agent::new(
            i,
            0,
            format!("claude-{i}"),
            Vendor::Claude,
            Role::Worker,
            Status::Working,
            0,
        ));
    }
    w
}

pub(super) fn claude_at(program: &str) -> Vec<Hireable> {
    vec![Hireable::claude(program, &[])]
}

/// A live team whose sessions never start: what counts is what was
/// queued for them. A start still runs on its own thread and fails at
/// once (there is no `/nonexistent/claude`); its report is cut off
/// here, or it would race the events a test sends in the session's
/// name and end the session under them.
pub(crate) fn idle_live(agents: usize) -> Live {
    idle_live_in(&std::env::temp_dir(), agents)
}

/// [`idle_live`] with `repo` as the first project's checkout.
pub(crate) fn idle_live_in(repo: &Path, agents: usize) -> Live {
    let mut live = Live::new(
        agents,
        claude_at("/nonexistent/claude"),
        repo.to_path_buf(),
        PathBuf::from("/nonexistent"),
    );
    let (cut, _) = mpsc::channel();
    live.ready_tx = cut;
    live
}

/// A fresh git checkout with one commit, under the temp dir.
pub(crate) fn checkout(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("imece-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}");
    };
    git(&["init", "--quiet"]);
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    git(&["add", "."]);
    git(&[
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
    ]);
    dir
}

impl Live {
    /// `agents` places to begin with, all on their vendor's default model,
    /// in one project whose checkout is `repo` and whose worktrees go
    /// under `root`. `vendors` must hold every vendor an agent has; the
    /// first leads.
    pub(super) fn new(agents: usize, vendors: Vec<Hireable>, repo: PathBuf, root: PathBuf) -> Live {
        let lead = vendors.first().map_or(Vendor::Claude, |h| h.vendor);
        let mut live = Live::bare(vendors, lead);
        live.settle = Duration::ZERO;
        live.places.push(Place { repo, root });
        for _ in 0..agents {
            live.grow(None, false);
        }
        live
    }
}

pub(super) fn with_card(w: &mut World, id: AgentId) -> CardId {
    let c = w.cards.len();
    w.cards.push(Card {
        id: c,
        project: 0,
        title: "fix it".into(),
        owner: Some(id),
        column: Column::Working,
        story: Vec::new(),
        symbols: Vec::new(),
        started: 0,
        finished: 0,
    });
    w.agents[id].card = Some(c);
    c
}

/// A turn of agent `id` ended at Claude Code's cap, `usd` dollars spent
/// in the session so far.
pub(super) fn capped(live: &Live, id: AgentId, usd: u64) {
    live.tx
        .send((
            id,
            live.generation[id],
            AgentEvent::TurnDone {
                ok: false,
                text: CAPPED.into(),
                cost: usd * 1_000_000,
                denied: 0,
            },
        ))
        .unwrap();
}

pub(super) fn wait_for(live: &mut Live, w: &mut World, done: impl Fn(&World) -> bool) {
    let start = std::time::Instant::now();
    while !done(w) {
        assert!(start.elapsed().as_secs() < 10, "timed out");
        live.step(w);
        thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// A session that takes prompts and does nothing with them.
pub(super) struct Stub;

impl imece_agent::Agent for Stub {
    fn prompt(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn command(&mut self, line: &str) -> io::Result<()> {
        if line == "/known" {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "no such command",
            ))
        }
    }
    fn terminate(&mut self) {}
}

pub(super) fn argv(line: &str) -> Vec<String> {
    run::split(line).unwrap()
}

/// A session that says when it was ended.
pub(super) struct Ends(pub(super) Arc<AtomicBool>);

impl imece_agent::Agent for Ends {
    fn prompt(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn command(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn terminate(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

pub(super) fn msg(to: &str, kind: &str, body: &str) -> Value {
    json!({ "to": to, "type": kind, "body": body })
}

pub(super) fn queued(live: &Live, id: AgentId) -> Vec<String> {
    match &live.slots[id] {
        Slot::Starting(q) => q.clone(),
        _ => Vec::new(),
    }
}

pub(super) fn finish(live: &mut Live, w: &mut World, id: AgentId, text: &str) {
    live.tx
        .send((
            id,
            0,
            AgentEvent::TurnDone {
                ok: true,
                text: text.into(),
                cost: 0,
                denied: 0,
            },
        ))
        .unwrap();
    live.step(w);
}

pub(super) fn answers(w: &World) -> Vec<(AgentId, String)> {
    w.open_inbox()
        .iter()
        .map(|&i| &w.inbox[i])
        .filter(|i| i.kind == InboxKind::Reply)
        .map(|i| (i.agent, i.detail.clone()))
        .collect()
}

pub(super) fn card(title: &str, body: &str) -> Value {
    json!({ "title": title, "body": body })
}

/// A tool call as the bus delivers it, answered by the next step.
pub(super) fn call(
    live: &mut Live,
    w: &mut World,
    from: AgentId,
    tool: &str,
    args: Value,
) -> Answer {
    let (tx, rx) = std::sync::mpsc::channel();
    live.calls_tx.send((from, tool.into(), args, tx)).unwrap();
    live.step(w);
    rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap()
}

/// A change of worker `id`'s that imece holds as git read it, for tests
/// of the screen; nothing of it is on disk.
pub(crate) fn hold_change(live: &mut Live, id: AgentId) {
    let change = worktree::Change {
        path: PathBuf::from("/nonexistent/tree"),
        tree: String::new(),
        files: Vec::new(),
        patch: String::new(),
    };
    live.changes.insert(id, change);
}

/// Worker `id`'s change as if git were applying it.
pub(crate) fn as_if_applying(live: &mut Live, id: AgentId) {
    live.applying[id] = true;
}

/// What the git thread was given, done and taken in; a bounded wait.
pub(super) fn settle(live: &mut Live, w: &mut World) {
    for _ in 0..500 {
        live.take_git(w);
        if live.reading.iter().all(|&n| n == 0)
            && !live.applying.contains(&true)
            && live.placing.iter().all(Option::is_none)
            && live.standing.iter().all(Option::is_none)
        {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("git did not finish in time");
}

/// What agent `from`'s `send` was answered, once git has read what a
/// done needs.
pub(super) fn sent(live: &mut Live, w: &mut World, from: AgentId, args: &Value) -> Answer {
    let (tx, rx) = std::sync::mpsc::channel();
    live.send(w, from, args, tx);
    settle(live, w);
    rx.try_recv().expect("answered")
}

/// A helper of worker `parent`, as `hire` would add it; its id.
pub(super) fn with_helper(w: &mut World, parent: AgentId) -> AgentId {
    let id = w.agents.len();
    let name = format!(
        "{}.{}",
        w.agents[parent].name,
        w.helpers_of(parent).count() + 1
    );
    let mut a = Agent::new(id, 0, name, Vendor::Claude, Role::Helper, Status::Idle, 0);
    a.parent = Some(parent);
    w.agents.push(a);
    id
}

pub(super) fn two_vendors(dir: &Path) -> Vec<Hireable> {
    vec![
        Hireable::claude(&fake_claude(dir), &[]),
        Hireable::codex(&fake_codex(dir), &[]),
    ]
}

/// A stand-in for `codex` that writes its arguments where it runs and
/// exits, which ends the session.
pub(super) fn fake_codex(dir: &Path) -> String {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("fake-codex");
    std::fs::write(&path, "#!/bin/sh\nprintf '%s\\n' \"$@\" > args.txt\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path.to_string_lossy().into_owned()
}

/// A stand-in for `claude` that writes its arguments, one per line, to
/// `args.txt` in its directory and exits.
pub(super) fn fake_claude(dir: &Path) -> String {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("fake-claude");
    std::fs::write(&path, "#!/bin/sh\nprintf '%s\\n' \"$@\" > args.txt\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path.to_string_lossy().into_owned()
}

/// A fresh git checkout with one commit, under the temp directory.
/// Git in `dir`, its first stdout line.
pub(super) fn git_in(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A fresh directory for the worktrees of one test.
pub(super) fn trees(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("imece-{name}-trees-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    root
}

/// A worker with a session in a worktree of `repo`, as after its
/// first turn; gives the worktree.
pub(super) fn worker_in(
    live: &mut Live,
    w: &mut World,
    id: AgentId,
    repo: &Path,
    root: &Path,
) -> PathBuf {
    let name = w.agents[id].name.clone();
    let tree = worktree::ensure(repo, root, &name).unwrap().path;
    live.ready(w, id, Ok((Box::new(Stub), tree.clone(), None, None)));
    tree
}

pub(super) fn item(kind: InboxKind, agent: AgentId, detail: &str) -> InboxItem {
    InboxItem {
        kind,
        agent,
        detail: detail.into(),
        researcher: None,
        open: true,
        tick: 1,
        card: None,
    }
}

pub(super) const UUID: &str = "686c6d27-d50b-4ae2-ad6a-159bf2984671";

pub(super) fn journaled(w: &World, e: &Event) -> usize {
    w.journal.iter().filter(|j| j.event == *e).count()
}

/// Claude's transcript of session `id` for agent `id`'s directory,
/// written under the live's Claude home.
pub(super) fn transcript_of(live: &Live, w: &World, agent: AgentId, id: &str) -> PathBuf {
    let file = claude::transcript(&live.claude_home, &live.cwd_of(w, agent), id);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "{}\n").unwrap();
    file
}

/// Whether `path` goes within a few seconds: removing is done on a
/// thread of its own.
pub(super) fn gone_soon(path: &Path) -> bool {
    for _ in 0..400 {
        if !path.exists() {
            return true;
        }
        thread::sleep(Duration::from_millis(25));
    }
    false
}

/// Whether agent `id`'s trace gets a line with `needle` within a few
/// seconds of stepping.
pub(super) fn traced_soon(live: &mut Live, w: &mut World, id: AgentId, needle: &str) -> bool {
    for _ in 0..400 {
        live.step(w);
        if w.agents[id].trace.iter().any(|l| l.contains(needle)) {
            return true;
        }
        thread::sleep(Duration::from_millis(25));
    }
    eprintln!("{:?}", w.agents[id].trace);
    false
}
