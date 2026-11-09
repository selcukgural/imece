//! The demo on the real `Live` (record 130; `--demo-live` until it does
//! what the simulation does, then `--demo`): imece's stand-in plays Claude
//! Code in sessions `Live` starts as it starts Claude Code's, on git
//! checkouts made for the run, with a state directory of the run's own.
//! All of it is in one directory that goes when the demo ends: the demo
//! keeps nothing and touches none of the person's checkouts or state.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{DEFAULT_RUN_TIMEOUT, Live, Open, Options, StandIn, world as live_world};
use crate::model::World;
use crate::sim::{self, Repo};
use imece_agent::Confine;
use imece_agent::stand_in::{Habits, work};

/// The demo's helpers per worker, at most.
const HELPERS: usize = 2;

/// How long a demo worker takes over each step of its list, in
/// milliseconds, and one assign in how many runs a command that waits
/// for the person's yes first.
const PAUSE: u64 = 1_500;
const ASKS: u64 = 3;

/// How the demo's stand-ins work, their choices seeded by `seed`.
fn habits(seed: u64) -> Habits {
    Habits {
        pause: std::time::Duration::from_millis(PAUSE),
        asks: ASKS,
        seed,
    }
}

/// The demo's directory: removed with everything in it when dropped,
/// after the `Live` that runs in it (its sessions end with it).
#[derive(Debug)]
pub struct Demo {
    pub dir: PathBuf,
}

impl Drop for Demo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A new directory of the demo's own under `parent`, 0700, by its real
/// path: Claude Code keeps a session's transcript by the directory it ran
/// in as the session sees it (`claude::transcript`), so a resume finds it
/// only when `Live` names the same path (macOS's temporary directory is
/// a link).
fn made(parent: &Path) -> Result<Demo, String> {
    use std::os::unix::fs::DirBuilderExt;
    let at = parent.join(format!("imece-demo-{}", std::process::id()));
    // A run of the same number that left its directory behind.
    let _ = std::fs::remove_dir_all(&at);
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&at)
        .map_err(|e| format!("{}: {e}", at.display()))?;
    let dir = at
        .canonicalize()
        .map_err(|e| format!("{}: {e}", at.display()))?;
    Ok(Demo { dir })
}

/// Runs git with `args` in `dir`, the person's hooks and signing kept out.
fn git(dir: &Path, args: &[&str]) -> Result<(), String> {
    let out = Command::new("git")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&out.stderr);
    Err(format!("git {}: {}", args.join(" "), said.trim()))
}

/// What a file of a demo repository holds: a line per symbol in it, as
/// a comment, or a key of a JSON file.
fn text_of(path: &str, symbols: &[&str]) -> String {
    if path.ends_with(".json") {
        let keys: Vec<String> = symbols.iter().map(|s| format!("  \"{s}\": \"\"")).collect();
        return format!("{{\n{}\n}}\n", keys.join(",\n"));
    }
    symbols.iter().map(|s| format!("// {s}\n")).collect()
}

/// A git checkout at `dir` of `repo`'s files, on `main`, one commit.
fn checkout(dir: &Path, repo: Repo) -> Result<(), String> {
    for (path, symbols) in repo {
        let file = dir.join(path);
        let parent = file.parent().unwrap_or(dir);
        std::fs::create_dir_all(parent)
            .and_then(|()| std::fs::write(&file, text_of(path, symbols)))
            .map_err(|e| format!("{}: {e}", file.display()))?;
    }
    git(dir, &["init", "--quiet", "--initial-branch=main"])?;
    git(dir, &["add", "--all"])?;
    git(
        dir,
        &[
            "-c",
            "user.name=imece demo",
            "-c",
            "user.email=demo@imece.invalid",
            "commit",
            "--quiet",
            "--message",
            "The demo's start",
        ],
    )
}

/// The demo's world: `projects` (one at least, the simulation's names, as
/// many as it has) each a checkout of its own with its leader, `agents`
/// workers a leader may hire, every session the stand-in of `imece`
/// (imece's own binary), its choices seeded by `seed`. The demo's
/// directory goes with the `Demo`.
pub fn world(
    projects: usize,
    agents: usize,
    seed: u64,
    imece: &Path,
) -> Result<(World, Live, Demo), String> {
    world_in(
        &std::env::temp_dir(),
        projects,
        agents,
        &habits(seed),
        imece,
    )
}

/// [`world`], the demo's directory under `parent`.
fn world_in(
    parent: &Path,
    projects: usize,
    agents: usize,
    habits: &Habits,
    imece: &Path,
) -> Result<(World, Live, Demo), String> {
    let demo = made(parent)?;
    let dir = &demo.dir;
    let (bin, home, state) = (dir.join("bin"), dir.join("home"), dir.join("state"));
    for d in [&bin, &home, &state] {
        std::fs::create_dir(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let mut repos = Vec::new();
    for (name, repo) in sim::repos().take(projects.max(1)) {
        let at = dir.join("src").join(name);
        checkout(&at, repo)?;
        repos.push(at);
    }
    let program = imece_agent::stand_in::install(&bin, imece, &home, habits)
        .map_err(|e| format!("the stand-in: {e}"))?;
    let options = Options {
        repo: repos[0].clone(),
        state,
        max_workers: agents,
        max_helpers: HELPERS,
        tiers: Vec::new(),
        budget: None,
        fresh: false,
        // The stand-in's test runs are routine; what it asks first is not.
        run_programs: work::TEST_PROGRAMS.map(String::from).into(),
        run_timeout: DEFAULT_RUN_TIMEOUT,
        instructions: None,
        own_notes: None,
        confine: Confine::default(),
        solo: None,
        claude: Some(StandIn {
            program,
            home,
            relay: imece.to_path_buf(),
        }),
    };
    let (mut w, mut live) = live_world(options)?;
    for repo in &repos[1..] {
        live.open_project(&mut w, repo, None, Open::Team)
            .map_err(|e| e.describe())?;
    }
    Ok((w, live, demo))
}

#[cfg(test)]
mod tests {
    use super::super::testing::imece;
    use super::*;
    use crate::model::{Role, Status};
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn a_file_says_its_symbols_in_its_own_kind() {
        assert_eq!(text_of("a/B.cs", &["B", "B.c"]), "// B\n// B.c\n");
        assert_eq!(
            text_of("i18n/tr.json", &["checkout", "cart"]),
            "{\n  \"checkout\": \"\",\n  \"cart\": \"\"\n}\n"
        );
        assert_eq!(text_of("empty.rs", &[]), "");
    }

    #[test]
    fn the_demo_runs_live_on_checkouts_of_its_own_and_leaves_nothing() {
        let parent = std::env::temp_dir().join(format!("imece-demo-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&parent);
        std::fs::create_dir_all(&parent).unwrap();
        let (mut w, mut live, demo) =
            world_in(&parent, 2, 3, &Habits::default(), &imece()).unwrap();
        let dir = demo.dir.clone();
        assert_eq!(
            dir,
            parent
                .canonicalize()
                .unwrap()
                .join(format!("imece-demo-{}", std::process::id()))
        );
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        // Two projects of the simulation's names, each its own checkout
        // with a commit of its files.
        let names: Vec<&str> = w.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["shop-api", "mobile-app"]);
        for (p, (name, repo)) in sim::repos().take(2).enumerate() {
            let at = dir.join("src").join(name);
            assert_eq!(live.repo_of(p), at);
            for (path, _) in repo {
                assert!(at.join(path).is_file(), "{path}");
            }
            let log = Command::new("git")
                .args(["log", "--format=%s %ae"])
                .current_dir(&at)
                .output()
                .unwrap();
            assert_eq!(
                String::from_utf8_lossy(&log.stdout),
                "The demo's start demo@imece.invalid\n"
            );
        }
        assert_eq!(live.max_workers, 3);
        assert_eq!(live.max_helpers, HELPERS);
        assert_eq!(live.claude_home, dir.join("home"));
        assert_eq!(live.state, dir.join("state"));
        assert_eq!(live.run_programs, ["dotnet", "gradle", "npm", "go"]);
        // The leader hires on the stand-in; the worker's change waits.
        live.told(&mut w, 0, "Write a greeting").unwrap();
        let start = Instant::now();
        while !w
            .changes
            .iter()
            .any(|c| w.agents[c.agent].role == Role::Worker)
            || w.agents.iter().any(|a| a.status != Status::Idle)
        {
            assert!(
                start.elapsed().as_secs() < 30,
                "timed out: {:#?}",
                w.journal
            );
            live.step(&mut w);
            thread::sleep(Duration::from_millis(20));
        }
        // Its transcripts are the demo's.
        assert!(dir.join("home").join("projects").is_dir());
        drop(live);
        drop(demo);
        assert!(!dir.exists(), "the demo leaves nothing behind");
        let _ = std::fs::remove_dir_all(&parent);
    }

    #[test]
    fn the_demo_s_workers_take_their_time_and_sometimes_ask() {
        assert_eq!(
            habits(9),
            Habits {
                pause: Duration::from_millis(1_500),
                asks: 3,
                seed: 9
            }
        );
    }

    #[test]
    fn one_project_at_least_and_no_more_than_the_simulation_has() {
        let parent = std::env::temp_dir().join(format!("imece-demo-few-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&parent);
        std::fs::create_dir_all(&parent).unwrap();
        let none = Habits::default();
        let (w, live, demo) = world_in(&parent, 0, 1, &none, &imece()).unwrap();
        assert_eq!(w.projects.len(), 1);
        drop((live, demo));
        let (w, live, demo) = world_in(&parent, 9, 1, &none, &imece()).unwrap();
        assert_eq!(w.projects.len(), sim::MAX_PROJECTS);
        drop((live, demo));
        let _ = std::fs::remove_dir_all(&parent);
    }
}
