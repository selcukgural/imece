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
    // The worker's row says what its model and the background's spent of
    // it (record 134), all of it but what does not make a whole cent's
    // ten-thousandth yet.
    let row = &w.ledger[w.agents[1].meter.row.expect("its row")];
    let background = row.models.get(imece_agent::stand_in::BACKGROUND).copied();
    assert!(background.is_some_and(|b| b > 0), "{row:?}");
    assert_eq!(row.models.len(), 2, "{row:?}");
    assert!(row.unsplit() <= 2, "{row:?}");
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

#[test]
fn a_turn_that_fails_at_the_api_is_tried_again_and_the_account_s_waits_for_the_person() {
    use imece_agent::Fault;
    use imece_agent::stand_in::{Habits, NO_CREDIT, OVERLOADED, work};
    let dir = std::env::temp_dir().join(format!("imece-e2e-faults-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let repo = checkout("e2e-faults-repo");
    // Every turn fails but the one after a failure: the leader's first on
    // a passing fault, its worker's first on the account's.
    let habits = |seed| Habits {
        faults: 1,
        seed,
        ..Default::default()
    };
    let first = |seed, name: &str| habits(seed).fault_now(work::roll(seed, name, 0));
    let seed = (0..10_000u64)
        .find(|&s| {
            first(s, "imece lead") == Some(Fault::Passing)
                && first(s, "imece claude-1") == Some(Fault::Account)
        })
        .unwrap();
    let claude =
        imece_agent::stand_in::install(&dir, &imece(), &dir.join("home"), &habits(seed)).unwrap();
    let mut w = team(0);
    let mut live = Live::new(
        1,
        claude_at(&claude.to_string_lossy()),
        repo.clone(),
        dir.join("trees"),
    );
    live.open_bus(&w, imece().to_string_lossy().into_owned())
        .unwrap();
    live.told(&mut w, 0, "Write a greeting").unwrap();
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
    // The leader's turn fails on an overloaded API: imece will try it
    // again, nobody is hired yet.
    until(&mut w, &mut live, &|w| w.agents[0].retry.is_some());
    assert_eq!(w.agents[0].retry.map(|r| r.attempt), Some(1));
    assert_eq!(w.agents.len(), 1);
    assert!(
        w.journal.iter().any(|l| matches!(
            &l.event,
            Event::Retrying { agent: 0, why, .. } if why == OVERLOADED
        )),
        "{:#?}",
        w.journal
    );
    assert!(
        (w.agents[0].trace.iter()).any(|l| l.contains("api error 529 overloaded: retry 3/10")),
        "Claude Code's own tries are traced"
    );
    // The try's time comes (the clock moved, not waited for): the leader
    // does the work of the prompt that failed, and its worker's first
    // turn fails on the account's fault, which waits for the person.
    w.started_at -= RETRY_TICKS[0] * SECONDS_PER_TICK;
    fn fault_of(w: &World, id: AgentId) -> Option<&InboxItem> {
        (w.inbox.iter()).find(|i| i.kind == InboxKind::Fault && i.agent == id && i.open)
    }
    until(&mut w, &mut live, &|w| fault_of(w, 1).is_some());
    assert!(
        fault_of(&w, 1).unwrap().detail.contains(NO_CREDIT),
        "{:?}",
        fault_of(&w, 1)
    );
    assert_eq!(w.agents[1].retry, None, "nothing tries the account's again");
    assert!(w.changes.is_empty());
    // The person's word is the next try: the worker does its assign.
    live.told(&mut w, 1, "Paid it, go on").unwrap();
    w.said(1, "Paid it, go on");
    assert!(fault_of(&w, 1).is_none());
    until(&mut w, &mut live, &|w| {
        w.changes.iter().any(|c| c.agent == 1) && w.agents[1].status == Status::Idle
    });
    let change = w.changes.iter().find(|c| c.agent == 1).unwrap();
    assert_eq!(change.title, "Write a greeting");
    assert!(
        change.patch.contains("+Write a greeting"),
        "{}",
        change.patch
    );
    drop(live);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn the_plan_s_windows_fill_with_every_session_s_turns_and_a_full_context_compacts() {
    use imece_agent::stand_in::{CONTEXT_START, Habits, TURN_COST, WINDOW};
    let dir = std::env::temp_dir().join(format!("imece-e2e-limits-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let repo = checkout("e2e-limits-repo");
    // A turn alone is four percent of five hours, one of the week; a
    // message is a tenth of the window, so a turn's work fills it.
    let habits = Habits {
        limit: TURN_COST / 4,
        fill: WINDOW / 10,
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
    live.told(&mut w, 0, "Write a greeting").unwrap();
    let until = |w: &mut World, live: &mut Live, done: &dyn Fn(&World) -> bool| {
        let start = std::time::Instant::now();
        while !done(w) {
            assert!(
                start.elapsed().as_secs() < 30,
                "timed out: {:#?}\n{:#?}",
                w.journal,
                w.agents.iter().map(|a| &a.trace).collect::<Vec<_>>()
            );
            live.step(w);
            thread::sleep(Duration::from_millis(20));
        }
    };
    until(&mut w, &mut live, &|w| {
        w.changes.iter().any(|c| c.agent == 1)
            && w.agents[0].status == Status::Idle
            && w.agents[1].status == Status::Idle
    });
    // Both windows, as Claude Code's `rate_limit_event` says them: the
    // leader's and the worker's turns in the same ones, the week's a
    // quarter as full, each ending later than now.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let window = |minutes| w.limits.iter().find(|l| l.minutes == minutes).copied();
    let five = window(imece_agent::FIVE_HOURS).expect("five hours");
    let week = window(imece_agent::WEEK).expect("a week");
    assert_eq!(w.limits.len(), 2);
    // The pot counts ten-thousandths of a dollar, the windows millionths.
    let spent = w.spent() * 100;
    assert!(spent >= 3 * TURN_COST, "three turns at least: {spent}");
    let part = TURN_COST / 4;
    assert_eq!(u64::from(five.used), spent / part, "{:?}", w.limits);
    assert_eq!(u64::from(week.used), spent / (4 * part), "{:?}", w.limits);
    assert!(five.used < 100, "{:?}", w.limits);
    assert!(
        five.resets_at
            .is_some_and(|at| at > now && at <= now + 5 * 3600)
    );
    assert!(
        week.resets_at
            .is_some_and(|at| at > now && at <= now + 7 * 24 * 3600)
    );
    // The worker's turn held more than nine tenths of its window: the
    // session compacted it, the trace says so.
    let compacted = |w: &World, id: AgentId, how: &str| {
        (w.agents[id].trace.iter()).any(|l| l.contains(&format!("note context compacted{how}")))
    };
    assert!(
        compacted(&w, 1, " by the session: it held "),
        "{:#?}",
        w.agents[1].trace
    );
    // The person's `/compact` empties the leader's: a turn of its own,
    // the context said again with its answer.
    live.command(&mut w, 0, "/compact").unwrap();
    until(&mut w, &mut live, &|w| {
        compacted(w, 0, ": it held ") && w.agents[0].status == Status::Idle
    });
    assert_eq!(
        w.agents[0].context,
        Some(Fill {
            used: CONTEXT_START + WINDOW / 10,
            size: Some(WINDOW)
        })
    );
    drop(live);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn the_stand_in_reads_imece_s_words_as_imece_writes_them() {
    use imece_agent::stand_in::{lead, work};
    assert!(PLAN_YES.starts_with(lead::PLAN_YES));
    assert!(PLAN_NO.starts_with(lead::PLAN_NO));
    assert_eq!(GO_ON, lead::GO_ON);
    assert_eq!(MAX_TITLE, lead::TITLE_MAX);
    let w = team(0);
    assert!(prompts::board_prompt(&w, &[]).starts_with(work::BOARD));
    let owned = crate::paths::parse(&serde_json::json!(["a.rs", "docs shared"])).unwrap();
    let task = format!("Mend it{}{}", bus::owns(&owned), bus::asks(&["api".into()]));
    assert_eq!(work::owned_of(&task), ["a.rs", "docs"]);
    assert_eq!(work::plain(&task), "Mend it");
    assert_eq!(
        work::plain(&format!("Mend it{}", bus::asks(&["api".into()]))),
        "Mend it"
    );
}

#[test]
fn a_leader_plans_its_cards_hands_them_out_at_the_yes_and_a_worker_takes_one_from_the_board() {
    let dir = std::env::temp_dir().join(format!("imece-e2e-plan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let repo = checkout("e2e-plan-repo");
    std::fs::remove_file(repo.join("a.txt")).unwrap();
    std::fs::create_dir_all(repo.join("src")).unwrap();
    for (file, text) in [
        ("money.rs", "// money\n"),
        ("money_test.rs", "// money test\n"),
        ("rates.rs", "// rates\n"),
        ("taxes.rs", "// taxes\n"),
    ] {
        std::fs::write(repo.join("src").join(file), text).unwrap();
    }
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
    git(&["add", "--all"]);
    git(&["commit", "--quiet", "-m", "code"]);
    // A card per file of code, and every card's worker strays.
    let habits = imece_agent::stand_in::Habits {
        cards: 3,
        strays: 1,
        ..Default::default()
    };
    let claude =
        imece_agent::stand_in::install(&dir, &imece(), &dir.join("home"), &habits).unwrap();
    let mut w = team(0);
    w.projects[0].plan.on = true;
    // Two workers at most: the third card has nobody to be hired for.
    let mut live = Live::new(
        1,
        claude_at(&claude.to_string_lossy()),
        repo.clone(),
        dir.join("trees"),
    );
    live.max_workers = 2;
    live.open_bus(&w, imece().to_string_lossy().into_owned())
        .unwrap();
    live.told(&mut w, 0, "Round half up").unwrap();
    let until = |live: &mut Live, w: &mut World, done: &dyn Fn(&World) -> bool| {
        let start = std::time::Instant::now();
        while !done(w) {
            assert!(
                start.elapsed().as_secs() < 60,
                "timed out: {:#?}\n{:#?}\n{:#?}",
                w.journal,
                w.cards,
                w.agents
                    .iter()
                    .map(|a| (&a.name, &a.status, &a.trace))
                    .collect::<Vec<_>>()
            );
            live.step(w);
            thread::sleep(Duration::from_millis(20));
        }
    };
    // The first worker is hired and has the first card; the second hire
    // waits for a plan, which the leader puts before the person.
    until(&mut live, &mut w, &|w| {
        w.plan_item(0).is_some() && w.agents[0].status == Status::Idle
    });
    let workers = |w: &World| w.agents.iter().filter(|a| a.role == Role::Worker).count();
    assert_eq!(workers(&w), 1);
    let owned = |c: &Card| (c.paths.iter().map(|p| p.words())).collect::<Vec<_>>();
    assert_eq!(w.cards.len(), 1);
    assert_eq!(w.cards[0].title, "src/money.rs: Round half up");
    assert_eq!(owned(&w.cards[0]), ["src/money.rs", "src/money_test.rs"]);
    let plan = w.projects[0].plan.proposal.clone().expect("the plan");
    let planned: Vec<(&str, Vec<String>)> = (plan.cards.iter())
        .map(|c| {
            (
                c.title.as_str(),
                c.paths.iter().map(|p| p.words()).collect(),
            )
        })
        .collect();
    assert_eq!(
        planned,
        [
            (
                "src/rates.rs: Round half up",
                vec!["src/rates.rs".to_string()]
            ),
            (
                "src/taxes.rs: Round half up",
                vec!["src/taxes.rs".to_string()]
            ),
        ]
    );
    assert_eq!(plan.cost, 1_000, "ten cents, in the pot's ten-thousandths");
    // The yes: a second worker for the second card; the team full, the
    // third goes on the board, and a worker that is free takes it.
    let item = w.plan_item(0).unwrap();
    assert!(live.decide_plan(&mut w, item, true));
    until(&mut live, &mut w, &|w| {
        w.cards.len() == 3
            && w.cards.iter().all(|c| c.column == Column::Review)
            && w.agents.iter().all(|a| a.status == Status::Idle)
    });
    assert_eq!(workers(&w), 2);
    assert_eq!(owned(&w.cards[1]), ["src/rates.rs"]);
    assert_eq!(owned(&w.cards[2]), ["src/taxes.rs"]);
    assert_eq!(
        w.cards[1].owner,
        Some(2),
        "assigned to the worker hired for it"
    );
    assert!(w.cards[2].owner.is_some(), "claimed");
    let story =
        |c: &Card, wanted: &dyn Fn(&StoryEvent) -> bool| c.story.iter().any(|s| wanted(&s.event));
    assert!(story(&w.cards[2], &|e| matches!(
        e,
        StoryEvent::Posted { by: 0, .. }
    )));
    assert!(story(&w.cards[2], &|e| matches!(
        e,
        StoryEvent::Took { .. }
    )));
    // Each card's worker changed a file its card does not own, once, and
    // the card's story says which.
    for c in &w.cards {
        let strayed: Vec<&str> = (c.story.iter())
            .filter_map(|s| match &s.event {
                StoryEvent::Strayed(file) => Some(file.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(strayed.len(), 1, "#{}: {strayed:?}", c.id + 1);
        assert!(!owned(c).iter().any(|p| p == strayed[0]), "{strayed:?}");
    }
    drop(live);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&repo);
}
