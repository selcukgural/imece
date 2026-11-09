//! The real `Live` end to end on imece's stand-in for Claude Code (record
//! 130): real sessions as child processes, imece's bus over their MCP
//! servers, a real worktree, a real change.

use super::testing::*;
use super::*;

#[test]
fn a_leader_hires_a_worker_whose_change_waits_for_the_person() {
    let dir = std::env::temp_dir().join(format!("imece-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let repo = checkout("e2e-repo");
    let claude = imece_agent::stand_in::install(&dir, &imece(), &dir.join("home")).unwrap();
    let mut w = team(0);
    let mut live = Live::new(
        1,
        claude_at(&claude.to_string_lossy()),
        repo.clone(),
        dir.join("trees"),
    );
    live.open_bus(&w, imece().to_string_lossy().into_owned())
        .unwrap();
    live.told(&mut w, 0, "Write a greeting\nin French").unwrap();
    let worker_s = |w: &World| w.changes.iter().any(|c| c.agent == 1);
    let start = std::time::Instant::now();
    while !(worker_s(&w)
        && w.agents[0].status == Status::Idle
        && w.agents[1].status == Status::Idle)
    {
        assert!(
            start.elapsed().as_secs() < 30,
            "timed out: {:#?}\n{:#?}",
            w.journal,
            w.agents
                .iter()
                .map(|a| (&a.name, &a.status, &a.trace))
                .collect::<Vec<_>>()
        );
        live.step(&mut w);
        thread::sleep(Duration::from_millis(20));
    }
    // The leader hired through the bus and assigned the person's words;
    // the worker wrote them in its worktree and said done.
    assert_eq!(w.agents[1].name, "claude-1");
    assert_eq!(w.agents[1].role, Role::Worker);
    let change = w.changes.iter().find(|c| c.agent == 1).unwrap();
    assert_eq!(change.title, "wrote stand-in/claude-1.md");
    let files: Vec<&str> = change.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(files, ["stand-in/claude-1.md"]);
    assert!(
        change.patch.contains("+# Write a greeting"),
        "{}",
        change.patch
    );
    assert!(change.patch.contains("+in French"));
    // Each session reported what it spent: the stand-in's turns.
    let cost = imece_agent::stand_in::TURN_COST;
    assert!(w.agents[0].cost > 0 && w.agents[1].cost > 0);
    assert!(w.spent() * 100 >= 3 * cost, "three turns at least");
    drop(live);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn a_session_of_an_earlier_run_resumes_from_its_transcript() {
    let dir = std::env::temp_dir().join(format!("imece-e2e-resume-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Its real path: a session's transcript is kept by the directory it
    // runs in as it sees it.
    let dir = dir.canonicalize().unwrap();
    let repo = checkout("e2e-resume-repo").canonicalize().unwrap();
    let home = dir.join("home");
    let program = imece_agent::stand_in::install(&dir, &imece(), &home).unwrap();
    let options = || Options {
        repo: repo.clone(),
        state: dir.join("state"),
        max_workers: 2,
        max_helpers: 0,
        tiers: Vec::new(),
        budget: None,
        fresh: false,
        run_programs: Vec::new(),
        run_timeout: DEFAULT_RUN_TIMEOUT,
        instructions: None,
        own_notes: None,
        confine: Confine::Standard,
        solo: None,
        claude: Some(StandIn {
            program: program.clone(),
            home: home.clone(),
            relay: imece(),
        }),
    };
    let settle = |w: &mut World, live: &mut Live, changes: usize| {
        let start = std::time::Instant::now();
        while !(w.changes.len() == changes && w.agents.iter().all(|a| a.status == Status::Idle)) {
            assert!(
                start.elapsed().as_secs() < 30,
                "timed out: {:#?}",
                w.journal
            );
            live.step(w);
            thread::sleep(Duration::from_millis(20));
        }
    };
    let (mut w, mut live) = world(options()).unwrap();
    live.told(&mut w, 0, "Write a greeting").unwrap();
    settle(&mut w, &mut live, 1);
    assert_eq!(live.persist(&w), Ok(true));
    drop(live);
    // The next run: the leader's session resumes, remembers the worker it
    // hired and gives it the new work; nobody new is hired.
    let (mut w, mut live) = world(options()).unwrap();
    let spent = w.agents[0].cost;
    w.changes.clear();
    live.told(&mut w, 0, "Write a farewell").unwrap();
    settle(&mut w, &mut live, 1);
    let resumed = |id| {
        w.journal
            .iter()
            .any(|e| e.event == Event::Resumed { agent: id })
    };
    assert!(resumed(0), "{:#?}", w.journal);
    assert_eq!(w.agents.len(), 2, "no second worker");
    assert!(
        w.changes[0].patch.contains("+# Write a farewell"),
        "{}",
        w.changes[0].patch
    );
    // What the resumed session reports is its whole cost; only the new
    // turns are booked, fewer calls than the first run's (no whoami, no
    // hire), so less than the whole would be.
    let booked = w.agents[0].cost - spent;
    assert!(booked > 0 && booked < spent, "{booked} after {spent}");
    drop(live);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&repo);
}
