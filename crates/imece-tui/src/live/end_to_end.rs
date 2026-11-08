//! The real `Live` end to end on imece's stand-in for Claude Code (record
//! 130): real sessions as child processes, imece's bus over their MCP
//! servers, a real worktree, a real change.

use super::testing::*;
use super::*;

/// imece's own binary, which the unit tests do not run from: they run
/// from `target/<profile>/deps`, and the binary is one directory up,
/// built by cargo for the integration tests (`tests/stand_in.rs`).
fn imece() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let bin = exe.parent().unwrap().parent().unwrap().join("imece");
    assert!(
        bin.is_file(),
        "{} is not built: run the crate's tests whole (cargo test -p imece-tui)",
        bin.display()
    );
    bin
}

#[test]
fn a_leader_hires_a_worker_whose_change_waits_for_the_person() {
    let dir = std::env::temp_dir().join(format!("imece-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let repo = checkout("e2e-repo");
    let claude = imece_agent::stand_in::install(&dir, &imece()).unwrap();
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
            "timed out: {:#?}",
            w.journal
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
