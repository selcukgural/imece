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
    let habits = imece_agent::stand_in::Habits::default();
    let claude =
        imece_agent::stand_in::install(&dir, &imece(), &dir.join("home"), &habits).unwrap();
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
    // the worker added them to the file of its worktree and said done.
    assert_eq!(w.agents[1].name, "claude-1");
    assert_eq!(w.agents[1].role, Role::Worker);
    let change = w.changes.iter().find(|c| c.agent == 1).unwrap();
    assert_eq!(change.title, "Write a greeting");
    let files: Vec<&str> = change.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(files, ["a.txt"]);
    assert!(
        change.patch.contains("+Write a greeting\n+in French"),
        "{}",
        change.patch
    );
    // Its to-do list, as Claude Code's task tools keep it, all done.
    let steps: Vec<(&str, StepState)> = (w.agents[1].plan.iter())
        .map(|s| (s.text.as_str(), s.state))
        .collect();
    let done: Vec<(&str, StepState)> = (imece_agent::stand_in::work::STEPS.iter())
        .map(|s| (*s, StepState::Done))
        .collect();
    assert_eq!(steps, done);
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
    let habits = imece_agent::stand_in::Habits::default();
    let program = imece_agent::stand_in::install(&dir, &imece(), &home, &habits).unwrap();
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
        w.changes[0].patch.contains("+Write a farewell"),
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

#[test]
fn a_worker_s_command_that_is_no_routine_waits_for_the_person_s_yes() {
    let dir = std::env::temp_dir().join(format!("imece-e2e-ask-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let repo = checkout("e2e-ask-repo");
    // Every assign runs a command that asks first.
    let habits = imece_agent::stand_in::Habits {
        asks: 1,
        ..Default::default()
    };
    let claude =
        imece_agent::stand_in::install(&dir, &imece(), &dir.join("home"), &habits).unwrap();
    let mut w = team(0);
    let mut live = Live::new(
        1,
        claude_at(&claude.to_string_lossy()),
        repo.clone(),
        dir.join("trees"),
    );
    live.open_bus(&w, imece().to_string_lossy().into_owned())
        .unwrap();
    live.told(&mut w, 0, "Check the status").unwrap();
    let until = |w: &mut World, live: &mut Live, done: &dyn Fn(&World) -> bool| {
        let start = std::time::Instant::now();
        while !done(w) {
            assert!(
                start.elapsed().as_secs() < 30,
                "timed out: {:#?}",
                w.journal
            );
            live.step(w);
            thread::sleep(Duration::from_millis(20));
        }
    };
    let asked =
        |w: &World| (w.open_inbox().into_iter()).find(|&i| w.inbox[i].kind == InboxKind::Approval);
    until(&mut w, &mut live, &|w| asked(w).is_some());
    let item = asked(&w).unwrap();
    let command = imece_agent::stand_in::work::OTHER.asks[0];
    assert_eq!(w.inbox[item].agent, 1);
    assert!(
        w.inbox[item].detail.contains(command),
        "{:?}",
        w.inbox[item]
    );
    assert!(w.changes.is_empty(), "nothing done before the yes");
    live.decide(&mut w, item, true);
    until(&mut w, &mut live, &|w| {
        w.changes.iter().any(|c| c.agent == 1) && w.agents.iter().all(|a| a.status == Status::Idle)
    });
    // What it heard is in its done, under the change's title.
    assert_eq!(
        w.done_why(1),
        Some(format!("Changed a.txt.\n{command}: Done.").as_str())
    );
    drop(live);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn a_worker_s_question_waits_with_its_answers_and_the_answer_goes_into_its_change() {
    use imece_agent::stand_in::work;
    let dir = std::env::temp_dir().join(format!("imece-e2e-question-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let repo = checkout("e2e-question-repo");
    // Every assign asks the person, under a seed whose question offers
    // answers.
    let seed = (0..1_000u64)
        .find(|&s| {
            !work::question("a.txt", work::roll(s, "claude-1", 1))
                .1
                .is_empty()
        })
        .unwrap();
    let (question, options) = work::question("a.txt", work::roll(seed, "claude-1", 1));
    let habits = imece_agent::stand_in::Habits {
        questions: 1,
        seed,
        ..Default::default()
    };
    let claude =
        imece_agent::stand_in::install(&dir, &imece(), &dir.join("home"), &habits).unwrap();
    let mut w = team(0);
    let mut live = Live::new(
        1,
        claude_at(&claude.to_string_lossy()),
        repo.clone(),
        dir.join("trees"),
    );
    live.open_bus(&w, imece().to_string_lossy().into_owned())
        .unwrap();
    live.told(&mut w, 0, "Say hello").unwrap();
    let until = |w: &mut World, live: &mut Live, done: &dyn Fn(&World) -> bool| {
        let start = std::time::Instant::now();
        while !done(w) {
            assert!(
                start.elapsed().as_secs() < 30,
                "timed out: {:#?}",
                w.journal
            );
            live.step(w);
            thread::sleep(Duration::from_millis(20));
        }
    };
    until(&mut w, &mut live, &|w| {
        w.question_of(1).is_some() && w.agents[1].status == Status::Idle
    });
    // The question waits for the person with its answers; the worker
    // changed nothing and holds its card.
    let q = w.question_of(1).unwrap();
    assert_eq!(q.text, question);
    assert_eq!(q.options, options);
    assert!(w.changes.is_empty(), "nothing done before the answer");
    let card = w.agents[1].card.unwrap();
    assert_eq!(w.cards[card].column, Column::Working);
    // The person answers with the first of them, as a digit on Home does.
    let answer = options[0];
    live.told(&mut w, 1, answer).unwrap();
    w.said(1, answer);
    assert!(w.question_of(1).is_none());
    until(&mut w, &mut live, &|w| {
        w.changes.iter().any(|c| c.agent == 1) && w.agents.iter().all(|a| a.status == Status::Idle)
    });
    // The same card, and the answer in the done under its title.
    assert_eq!(w.agents[1].card, Some(card));
    assert_eq!(
        w.done_why(1),
        Some(format!("Changed a.txt.\nThe person said: {answer}").as_str())
    );
    drop(live);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn a_worker_s_helper_writes_its_tests_says_done_and_leaves() {
    let dir = std::env::temp_dir().join(format!("imece-e2e-helper-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let repo = checkout("e2e-helper-repo");
    // A file and its test, so the worker has tests to hand on.
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src/money.rs"), "// money\n").unwrap();
    std::fs::write(repo.join("src/money_test.rs"), "// money test\n").unwrap();
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(&repo)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}");
    };
    git(&["add", "."]);
    git(&["commit", "--quiet", "-m", "money"]);
    // Every assign hires a helper for its tests.
    let habits = imece_agent::stand_in::Habits {
        helpers: 1,
        ..Default::default()
    };
    let claude =
        imece_agent::stand_in::install(&dir, &imece(), &dir.join("home"), &habits).unwrap();
    let mut w = team(0);
    let mut live = Live::new(
        1,
        claude_at(&claude.to_string_lossy()),
        repo.clone(),
        dir.join("trees"),
    );
    live.open_bus(&w, imece().to_string_lossy().into_owned())
        .unwrap();
    live.told(&mut w, 0, "Round money half up").unwrap();
    let start = std::time::Instant::now();
    while !(w.changes.iter().any(|c| c.agent == 1)
        && w.agents.iter().all(|a| a.gone || a.status == Status::Idle))
    {
        assert!(
            start.elapsed().as_secs() < 30,
            "timed out: {:#?}",
            w.journal
        );
        live.step(&mut w);
        thread::sleep(Duration::from_millis(20));
    }
    // The worker hired its helper in its worktree; the helper wrote the
    // tests, said done to its worker and left.
    let helper = &w.agents[2];
    assert_eq!(
        (helper.name.as_str(), helper.role, helper.parent),
        ("claude-1.1", Role::Helper, Some(1))
    );
    assert!(helper.gone, "{:?}", helper.status);
    // One change of the worker's: its file and the helper's test.
    let change = w.changes.iter().find(|c| c.agent == 1).unwrap();
    let files: Vec<&str> = change.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(files, ["src/money.rs", "src/money_test.rs"]);
    assert!(
        change.patch.contains("+// covers: Round money half up"),
        "{}",
        change.patch
    );
    assert_eq!(
        w.done_why(1),
        Some(
            "Changed src/money.rs, src/money_test.rs.\nclaude-1.1: Wrote the tests in \
             src/money_test.rs.\ncargo test: All 12 tests passed."
        )
    );
    drop(live);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&repo);
}
