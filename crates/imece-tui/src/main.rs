//! imece: one terminal for a team of Claude Code agents. The agents
//! are real coding-agent sessions (`live`), or fake ones with `--demo`
//! (`sim`).

mod app;
mod archives;
mod awake;
mod away;
mod blocked;
mod checks;
mod clip;
mod closes;
mod complete;
mod doctor;
mod edit;
mod editor;
mod health;
mod i18n;
mod input;
mod keys;
mod ledger;
mod live;
mod model;
mod motion;
mod notify;
mod paths;
mod prefs;
mod probe;
mod procs;
mod push;
mod recall;
mod rng;
mod round;
mod runners;
mod schemes;
mod screen;
mod sha;
mod shell;
mod sim;
mod store;
mod theme;
mod ui;
mod welcome;

use std::io::{ErrorKind, Write as _};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use imece_agent::{Confine, Tier};
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use crate::i18n::Lang;
use crate::model::{parse_dollars, solo_choice, valid_model};

const USAGE: &str = "usage: imece [--repo DIR] [--agents N] [--helpers N]
             [--solo claude[:MODEL][@EFFORT]] [--model NAME]... [--tier T=NAME]...
             [--budget USD] [--fresh] [--awake] [--confine standard|strict|open]
             [--lang en|tr] [--theme NAME|FILE]
       imece --demo [--projects N] [--agents N] [--researchers N] [--seed N]
       imece doctor [--repo DIR] [--verbose] [--json]
       imece --version

imece runs coding agents on the checkout DIR is in: a leader that reads it
and talks to you, and workers it hires as the work needs them, each changing
its own git worktree; a worker may hire helpers for parts of its task. With
--solo, one agent you drive yourself instead, on the model and effort
you name, in a worktree of its own; what it changes waits for you
after every turn. In a folder that is not a git checkout with a commit, a
solo works straight in the folder (no worktree, nothing to review) and a
team does not start. While it runs, :open PATH [NAME] opens another checkout
as a project of its own, with its own leader, and :solo [PATH] [CHOICE]
[NAME] a solo session (any number next to a team); :close NAME closes one,
:rename NAME renames the one at hand; ? lists every key and command.

  --repo DIR        the checkout (default: the current directory)
  --agents N        workers the leader may hire, at most (0-16, default 4)
  --helpers N       helpers a worker may have at once, at most (0-8, default 2)
  --solo claude[:MODEL][@EFFORT]
                    start with a solo session instead of a team: Claude Code
                    on MODEL (default: its own, or --model's) at EFFORT;
                    claude@xhigh, claude:opus@high. An open solo of the same
                    choice on this checkout is gone on with; a team open
                    here stays
  --model NAME      the model agents run on (default: Claude Code's own).
                    NAME is [MODEL][@EFFORT]: opus, opus@xhigh, @high; the
                    efforts are low, medium, high, xhigh, max
  --tier T=NAME     the model a tier stands for, T being cheap, standard or
                    strong (haiku, Claude Code's own, opus)
  --budget USD      the most all agents together may spend; one pot, kept
                    between runs (50 the first time); :budget USD changes
                    it while agents run
  --fresh           forget every agent's remembered session: each wakes in a
                    new one, told where it left off, and the transcripts
                    of the old ones are removed (:forget @NAME does it
                    for one agent while imece runs); an archived project's
                    are kept
  --awake           keep the machine from sleeping while an agent is
                    mid-turn (caffeinate on macOS, systemd-inhibit on
                    Linux); :awake turns it on or off while imece runs
  --confine LEVEL   how much of Claude Code each agent gets: standard (the
                    default) gives workers and helpers its full coding
                    toolset (read, write, shell, web search and fetch) and
                    the leader the web, with nothing of your own setup
                    loaded; strict gives file tools only, with commands
                    through imece's run tool; open runs it as you have it
                    set up (your settings, hooks, MCP servers, skills).
                    The leader never writes and the sub-agent tool stays
                    off at every level; imece sandboxes nothing, isolation
                    is a container's job
  --run PROG[,..]   programs workers may run with their run tool, next to
                    cargo and git's reading commands (repeatable)
  --run-timeout S   seconds one run may take (10-3600, default 600)
  --lang en|tr      the screen's language (default en, or IMECE_LANG)
  --theme NAME|FILE terminal (default: your terminal's own colours), dark,
                    light, mono, a scheme (dracula, nord, gruvbox-dark-medium,
                    catppuccin-mocha, tokyo-night-dark, solarized-dark, …) or
                    a colour file in Ghostty's or Kitty's format; or
                    IMECE_THEME; the last choice (T, :theme) is remembered

  doctor            one report of what a start needs (Claude Code signed in,
                    git, the checkout's tests, Lonca, the terminal, your
                    files), with what to do about what is wrong; --verbose
                    lists every finding, --json is for scripts; it spends
                    nothing and exits 1 when something does not work

  --demo            fake agents instead, to see the screen at work
  --projects N      projects worked on at once (1-4, default 2)
  --agents N        workers next to the leaders (1-200, default 24)
  --researchers N   researchers that may run at once (1-32, default 4)
  --seed N          replays the same fake scenario (default 7)";

#[derive(Debug)]
struct Args {
    projects: usize,
    agents: Option<usize>,
    helpers: usize,
    researchers: usize,
    seed: u64,
    lang: Lang,
    theme: Option<String>,
    demo: bool,
    /// `--demo-live` (hidden, record 130): the demo on the real `Live`
    /// with imece's stand-in for Claude Code, until it does what `--demo`
    /// does and takes its name.
    demo_live: bool,
    fresh: bool,
    awake: bool,
    repo: PathBuf,
    /// `--solo`: its `[MODEL][@EFFORT]` choice, none for Claude Code's own.
    solo: Option<Option<String>>,
    /// Stand-ins for Claude Code's own models, by tier; `--model` is the
    /// standard tier.
    tiers: Vec<(Tier, String)>,
    /// Ten-thousandths of a dollar; none keeps the pot as it was left.
    budget: Option<u64>,
    run_programs: Vec<String>,
    run_timeout: Duration,
    confine: Confine,
}

/// Workers a leader may hire, at most.
const LIVE_AGENTS: u64 = 16;
/// Workers a leader may hire unless the person says otherwise.
const DEFAULT_WORKERS: usize = 4;
/// Helpers a worker may have at once, at most, and unless the person says
/// otherwise.
const MAX_HELPERS: u64 = 8;
const DEFAULT_HELPERS: usize = 2;
/// Workers a leader of `--demo-live` may hire unless the person says
/// otherwise: the scale the screens are designed at (record 125).
const DEMO_LIVE_WORKERS: usize = 3;

fn parse_args() -> Result<Args, String> {
    parse(std::env::args().skip(1), |k| std::env::var(k).ok())
}

/// The command line `argv` (without the program) and the environment.
fn parse(
    argv: impl IntoIterator<Item = String>,
    env: impl Fn(&str) -> Option<String>,
) -> Result<Args, String> {
    let lang = match env("IMECE_LANG") {
        Some(v) => Lang::parse(&v).ok_or_else(|| format!("IMECE_LANG: {v:?} is not en or tr"))?,
        None => Lang::En,
    };
    let mut args = Args {
        projects: 2,
        agents: None,
        helpers: DEFAULT_HELPERS,
        researchers: 4,
        seed: 7,
        lang,
        theme: match env("IMECE_THEME") {
            Some(v) => match theme::choose(&v) {
                Ok(_) => Some(v),
                Err(why) => return Err(format!("IMECE_THEME: {v:?}: {why}")),
            },
            None => None,
        },
        demo: false,
        demo_live: false,
        fresh: false,
        awake: false,
        repo: PathBuf::from("."),
        solo: None,
        tiers: Vec::new(),
        budget: None,
        run_programs: Vec::new(),
        run_timeout: live::DEFAULT_RUN_TIMEOUT,
        confine: Confine::default(),
    };
    // Options of the demo alone, to refuse them without it.
    let mut demo_only: Vec<String> = Vec::new();
    let mut it = argv.into_iter();
    while let Some(flag) = it.next() {
        if flag == "-h" || flag == "--help" {
            return Err(USAGE.into());
        }
        if flag == "--demo" {
            args.demo = true;
            continue;
        }
        if flag == "--demo-live" {
            args.demo_live = true;
            continue;
        }
        if flag == "--fresh" {
            args.fresh = true;
            continue;
        }
        if flag == "--awake" {
            args.awake = true;
            continue;
        }
        if flag == "--live" {
            // What live agents were asked for with before they were the default.
            continue;
        }
        let value = it
            .next()
            .ok_or_else(|| format!("{flag} needs a value\n\n{USAGE}"))?;
        let number = |lo: u64, hi: u64| {
            value
                .parse::<u64>()
                .ok()
                .filter(|n| (lo..=hi).contains(n))
                .ok_or_else(|| format!("{flag}: {value} is not in {lo}-{hi}"))
        };
        if matches!(flag.as_str(), "--projects" | "--researchers" | "--seed") {
            demo_only.push(flag.clone());
        }
        match flag.as_str() {
            "--projects" => args.projects = number(1, sim::MAX_PROJECTS as u64)? as usize,
            "--agents" => args.agents = Some(number(0, 200)? as usize),
            "--helpers" => args.helpers = number(0, MAX_HELPERS)? as usize,
            "--researchers" => args.researchers = number(1, 32)? as usize,
            "--seed" => args.seed = number(0, u64::MAX)?,
            "--solo" => args.solo = Some(parse_solo(&value)?),
            "--tier" => args.tiers.push(parse_tier(&value)?),
            "--theme" => match theme::choose(&value) {
                Ok(_) => args.theme = Some(value),
                Err(why) => return Err(format!("--theme: {value:?}: {why}")),
            },
            "--lang" => {
                args.lang = Lang::parse(&value)
                    .ok_or_else(|| format!("--lang: {value:?} is not en or tr"))?
            }
            "--repo" => args.repo = PathBuf::from(&value),
            "--model" => args.tiers.push(parse_model(&value)?),
            "--budget" => {
                args.budget =
                    Some(parse_dollars(&value).filter(|&b| b >= 100).ok_or_else(|| {
                        format!("--budget: {value} is not dollars like 5 or 2.50")
                    })?)
            }
            "--run" => {
                for p in value.split(',').map(str::trim).filter(|p| !p.is_empty()) {
                    if p.contains('/') {
                        return Err(format!("--run: {p:?} is a path; name a program on PATH"));
                    }
                    if !args.run_programs.iter().any(|q| q == p) {
                        args.run_programs.push(p.to_string());
                    }
                }
            }
            "--run-timeout" => args.run_timeout = Duration::from_secs(number(10, 3600)?),
            "--confine" => {
                args.confine = Confine::from_wire(&value).ok_or_else(|| {
                    format!("--confine: {value:?} is not standard, strict or open")
                })?
            }
            _ => return Err(format!("unknown option {flag}\n\n{USAGE}")),
        }
    }
    if args.demo && args.demo_live {
        return Err("--demo or --demo-live, not both".into());
    }
    // The demo on `Live` has Live's limits, and as many projects as the
    // demo's.
    if args.demo_live {
        demo_only.retain(|f| f != "--projects" && f != "--seed");
    }
    if args.demo {
        if args.agents == Some(0) {
            return Err("--agents: the demo needs at least 1".into());
        }
    } else {
        if let Some(flag) = demo_only.first() {
            return Err(format!("{flag} is for --demo\n\n{USAGE}"));
        }
        if args.agents.is_some_and(|n| n as u64 > LIVE_AGENTS) {
            return Err(format!("--agents: a leader hires at most {LIVE_AGENTS}"));
        }
    }
    Ok(args)
}

/// `claude[:MODEL][@EFFORT]`, as `--solo` takes it: asked only for an
/// effort Claude Code has.
fn parse_solo(s: &str) -> Result<Option<String>, String> {
    let choice = solo_choice(s).ok_or_else(|| {
        format!("--solo: {s:?} is not claude[:MODEL][@EFFORT], like claude:opus@xhigh")
    })?;
    if let Some(c) = &choice {
        live::valid_choice(c).map_err(|why| format!("--solo: {why}"))?;
    }
    Ok(choice)
}

/// `TIER=MODEL`, as `--tier` takes it.
fn parse_tier(s: &str) -> Result<(Tier, String), String> {
    let (tier, model) = s
        .split_once('=')
        .ok_or_else(|| format!("--tier: {s:?} is not TIER=MODEL, like strong=opus"))?;
    let tier = Tier::from_wire(tier)
        .ok_or_else(|| format!("--tier: {tier:?} is not cheap, standard or strong"))?;
    checked("--tier", model)?;
    Ok((tier, model.to_string()))
}

/// `MODEL`, as `--model` takes it: the standard tier.
fn parse_model(s: &str) -> Result<(Tier, String), String> {
    checked("--model", s)?;
    Ok((Tier::Standard, s.to_string()))
}

/// `model` (`[MODEL][@EFFORT]`) as `flag` took it: a model name, asking
/// Claude Code only for an effort it has.
fn checked(flag: &str, model: &str) -> Result<(), String> {
    if !valid_model(model) {
        return Err(format!("{flag}: {model:?} is not a model name"));
    }
    live::valid_choice(model).map_err(|why| format!("{flag}: {why}"))
}

/// What `imece --version` (or `-V`) prints.
fn version_line() -> String {
    format!("imece {}", env!("CARGO_PKG_VERSION"))
}

/// Whether the command line asks for the version: `--version` or `-V`
/// first.
fn asks_version(first: Option<&std::ffi::OsStr>) -> bool {
    first.is_some_and(|a| a == "--version" || a == "-V")
}

fn main() -> std::io::Result<()> {
    if asks_version(std::env::args_os().nth(1).as_deref()) {
        println!("{}", version_line());
        return Ok(());
    }
    // A live agent's CLI starts imece this way as its MCP server: it only
    // carries bytes between the CLI and imece's socket for that agent.
    let mut argv = std::env::args_os().skip(1);
    let first = argv.next();
    // imece's stand-in for Claude Code (record 130): the demo's agents,
    // and the tests that run the real `Live` end to end.
    if first.as_ref().is_some_and(|a| a == "stand-in") {
        let rest: Vec<String> = argv.map(|a| a.to_string_lossy().into_owned()).collect();
        std::process::exit(imece_agent::stand_in::main(&rest));
    }
    // One report of what a start needs (record 138): no screen, no agent.
    if first.as_ref().is_some_and(|a| a == "doctor") {
        let rest: Vec<String> = argv.map(|a| a.to_string_lossy().into_owned()).collect();
        std::process::exit(doctor::main(&rest));
    }
    if first.is_some_and(|a| a == "mcp-relay") {
        let path = argv.next().map(PathBuf::from).unwrap_or_default();
        if let Err(e) = imece_agent::mcp::relay(&path) {
            eprintln!("imece mcp-relay: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }
    catch_signals();
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    };
    let state = prefs::state_dir();
    let saved = prefs::load(&state);
    // The person's own keys, or the last good ones when their file is
    // refused; the lead question already takes them.
    let keys_file = keys::file();
    let kept_keys = prefs::keys(&state);
    // Before it is read: a change meanwhile is read again soon.
    let keys_seen = keys_file.as_deref().map(keys::stamp).unwrap_or_default();
    let keys_start = keys::Start::new(keys_file.as_deref().map(keys::load), kept_keys.clone());
    // A flag or the environment wins over the saved theme.
    if args.theme.is_none()
        && let Some(name) = &saved.theme
    {
        // A file that is gone since leaves the default; nothing to say.
        let _ = theme::choose(name);
    }
    // Before the screen takes the terminal: what colours it has.
    let said = probe::terminal();
    theme::probed(
        said.back,
        theme::truecolor(std::env::var("COLORTERM").ok().as_deref()),
    );
    // Its own name decides how the tab's progress and a notice are sent.
    let var = |k: &str| std::env::var(k).ok();
    let progress = screen::shows_progress(said.who.as_ref(), var);
    let style = notify::Style::of(said.who.as_ref(), var);
    let icon = screen::shows_icon(said.who.as_ref(), var);
    let mut terminal =
        screen::init(said.keys, progress, icon).inspect_err(|_| screen::restore())?;
    let fail = |msg: String| -> ! {
        screen::restore();
        eprintln!("imece: {msg}");
        std::process::exit(1);
    };
    // The demo on `Live`'s directory: it goes after the app (declared
    // first, dropped last), once its sessions have ended.
    let mut demo_dir = None;
    let mut app = if args.demo {
        // The simulation's clock says 14:00 at its start, on today's date.
        model::start_clock_at(14 * 3600, live::today());
        let mut app = app::App::new(
            args.projects,
            args.agents.unwrap_or(24),
            args.researchers,
            args.seed,
            args.lang,
        );
        // The demo signs in through a plan, as most do: `≈` (record 132).
        app.take_plan();
        app
    } else if args.demo_live {
        let imece =
            std::env::current_exe().unwrap_or_else(|e| fail(format!("imece's own path: {e}")));
        let agents = args.agents.unwrap_or(DEMO_LIVE_WORKERS);
        let (world, live, demo) =
            live::demo::world(args.projects, agents, args.seed, &imece).unwrap_or_else(|e| fail(e));
        let (seconds, day) = live::local_clock(world.started_at);
        model::start_clock_at(seconds, day);
        demo_dir = Some(demo);
        app::App::with(world, app::Engine::Live(Box::new(live)), args.lang)
    } else {
        live::probe(&args.repo, imece_agent::claude::PROGRAM).unwrap_or_else(|e| fail(e));
        let options = live::Options {
            repo: args.repo,
            state: state.clone(),
            max_workers: args.agents.unwrap_or(DEFAULT_WORKERS),
            max_helpers: args.helpers,
            tiers: args.tiers,
            budget: args.budget,
            fresh: args.fresh,
            run_programs: args.run_programs,
            run_timeout: args.run_timeout,
            instructions: keys::config_file(live::INSTRUCTIONS),
            own_notes: imece_agent::claude::own_notes(),
            confine: args.confine,
            solo: args.solo,
            claude: None,
        };
        match live::world(options) {
            Ok((world, live)) => {
                // The screen's clock counts from when the world began,
                // which may be an earlier run.
                let (seconds, day) = live::local_clock(world.started_at);
                model::start_clock_at(seconds, day);
                // Nothing remembered: the first start ever.
                let first = live.left_off().is_none();
                let mut app = app::App::with(world, app::Engine::Live(Box::new(live)), args.lang);
                app.look = Some(|| live::look(imece_agent::claude::PROGRAM));
                app.doctor_look = Some(doctor::look);
                app.terminal = Some(doctor::terminal_of(&said, var));
                app.start_welcome(first);
                // What is new since the version that started last, once.
                let versions: Vec<&str> = app.lang.text().news.iter().map(|n| n.0).collect();
                let seen = saved.seen.as_deref();
                app.news = welcome::news_since(seen, first, welcome::VERSION, &versions);
                let _ = prefs::seen(&state, welcome::VERSION);
                app
            }
            Err(msg) => fail(msg),
        }
    };
    app.shift_enter = said.keys;
    app.start_keys(keys_file, keys_seen, keys_start);
    if app.keys_good != kept_keys {
        // Kept for the next start; a failure only costs that.
        let _ = prefs::keep_keys(&state, app.keys_good.as_deref());
    }
    app.notify = saved.notify.unwrap_or_default();
    app.start_push();
    app.start_tidy();
    app.start_ps();
    app.motion = saved.motion.unwrap_or_default();
    app.away = saved.away.unwrap_or_default();
    app.story = saved.story.unwrap_or(true);
    app.review = saved.review.unwrap_or(false);
    app.world.health = saved.health.unwrap_or_default();
    app.ssh = motion::over_ssh(var);
    app.awake.on = args.awake;
    let result = run(&mut terminal, &mut app, &state, progress, style);
    // A :close or :forget still counting down was asked for: it happens.
    app.end_grace();
    // The last word before the sessions end: what every agent had in hand,
    // and when this run ended.
    app.persist();
    // What is left in the terminal once the screen is gone (record 101),
    // read before the sessions end.
    let spent = app.spent_this_run();
    let bye = ui::words::farewell(app.lang, &app.world, spent, app.demo() || args.demo_live);
    app.leave();
    screen::restore();
    // A terminal gone away takes nothing; that is no error of the run's.
    let _ = writeln!(std::io::stdout(), "{bye}");
    // The demo's sessions end with the app, then its directory goes.
    drop(app);
    drop(demo_dir);
    result
}

/// Whether the terminal has an event within `timeout`; `None` when it
/// went away (crossterm alone would spin on it: `input`).
fn poll_terminal(guard: &mut input::Guard, timeout: Duration) -> std::io::Result<Option<bool>> {
    match input::terminal() {
        Some(fd) => guard.ready(
            timeout,
            |t| input::wait(fd, t),
            || event::poll(Duration::ZERO),
        ),
        None => event::poll(timeout).map(Some),
    }
}

/// Keeps `drawn` in `shown` while a frame may be drawn from it
/// (`patchable`), reusing what `shown` holds; else lets it go.
fn keep(
    shown: &mut Option<ratatui::buffer::Buffer>,
    drawn: &ratatui::buffer::Buffer,
    patchable: bool,
) {
    match shown {
        _ if !patchable => *shown = None,
        Some(b) => b.clone_from(drawn),
        None => *shown = Some(drawn.clone()),
    }
}

/// How long a click report's next event may take to come after the one
/// before it: the terminal writes it at once, a person never types it.
const REPORT_WAIT: Duration = Duration::from_millis(20);

/// Set by SIGTERM, SIGHUP (the terminal went away) and SIGINT from outside.
static SIGNALLED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    SIGNALLED.store(true, Ordering::SeqCst);
}

/// Turns the signals that would end imece on the spot into a normal quit,
/// so the sessions are stopped and the bus's sockets removed on the way
/// out.
fn catch_signals() {
    for sig in [libc::SIGTERM, libc::SIGHUP, libc::SIGINT] {
        // SAFETY: the handler only stores to an atomic, which is
        // async-signal-safe.
        unsafe { libc::signal(sig, on_signal as *const () as libc::sighandler_t) };
    }
}

fn run(
    terminal: &mut screen::Screen,
    app: &mut app::App,
    state: &std::path::Path,
    progress: bool,
    style: notify::Style,
) -> std::io::Result<()> {
    let mut last = Instant::now();
    let mut title = screen::Title::default();
    let mut cursor = screen::Cursor::default();
    let mut tab = screen::Tab::new(progress);
    // Events read with a report's start and found to be keys after all.
    let mut queued: std::collections::VecDeque<Event> = std::collections::VecDeque::new();
    // The last drawing as the terminal shows it, while a frame where only
    // the breath changes may be drawn from it.
    let mut shown: Option<ratatui::buffer::Buffer> = None;
    let mut guard = input::Guard::default();
    while !app.quit && !SIGNALLED.load(Ordering::SeqCst) {
        app.looked();
        app.doctored();
        // Drawn when something changed; in another window, less often.
        let now = Instant::now();
        if app.draw_due(now) {
            app.frame = now;
            let patch = app.patch_due(now);
            // One frame, the cursor's look with it.
            screen::synced(&mut std::io::stdout(), || {
                let mut patched = false;
                let done = terminal.draw(|f| {
                    patched = patch && shown.as_ref().is_some_and(|b| ui::patch(f, app, b));
                    if !patched {
                        ui::draw(f, app);
                    }
                })?;
                if !patched {
                    keep(&mut shown, done.buffer, app.patchable());
                }
                cursor.show(&mut std::io::stdout(), ui::look(app))
            })?;
            app.drawn(now);
        }
        // The ladder's top on the tab and in the title.
        let waiting = app.world.open_decisions();
        let ladder = app.progress();
        title.show(
            &mut std::io::stdout(),
            &ui::words::title(app.lang, waiting, ladder, app.all_done()),
        )?;
        tab.show(&mut std::io::stdout(), ladder)?;
        // Woken for the next tick, or sooner for the next frame of what
        // moves on the screen when it is not near the tick.
        let tick = app.tick_interval().saturating_sub(last.elapsed());
        let timeout = app.wait(Instant::now(), tick);
        let event = match queued.pop_front() {
            Some(e) => Some(e),
            None => {
                let ready = match poll_terminal(&mut guard, timeout) {
                    Ok(Some(ready)) => ready,
                    // The terminal went away (its tab closed): a quit,
                    // as SIGHUP would be, the sessions stopped on the
                    // way out.
                    Ok(None) => break,
                    Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                };
                let fresh = if ready { Some(event::read()?) } else { None };
                // kitty's report of a click on a notification comes as
                // keys (alt-], 9, 9, ;, …): what came with it is read, and
                // a report is taken whole, never typed.
                match fresh {
                    Some(e) if style == notify::Style::Kitty && notify::opens_report(&e) => {
                        let mut came = vec![e];
                        while came.len() < notify::REPORT_MAX
                            && notify::report(&came).is_none()
                            && poll_terminal(&mut guard, REPORT_WAIT)? == Some(true)
                        {
                            came.push(event::read()?);
                        }
                        match notify::report(&came) {
                            Some((taken, id)) => {
                                if let Some(id) = id {
                                    app.clicked(id);
                                }
                                app.stirred = true;
                                queued.extend(came.drain(taken..));
                                None
                            }
                            None => {
                                let first = came.remove(0);
                                queued.extend(came);
                                Some(first)
                            }
                        }
                    }
                    other => other,
                }
            }
        };
        app.stirred |= event.is_some();
        if let Some(Event::Paste(text)) = &event {
            app.paste(text);
        }
        // Back in the window: the terminal's theme may have turned light
        // or dark meanwhile, and `terminal` follows it; after a few
        // minutes away, what happened shows at once.
        if let Some(Event::FocusGained) = &event
            && let Some(back) = probe::colours_again()
        {
            theme::follow(back);
        }
        match &event {
            Some(Event::FocusGained) => app.focus(true, Instant::now()),
            Some(Event::FocusLost) => app.focus(false, Instant::now()),
            _ => {}
        }
        if let Some(Event::Key(k)) = event
            && k.kind == KeyEventKind::Press
        {
            let (theme, notify, moves, away, story, review, health) = (
                app.theme(),
                app.notify,
                app.motion,
                app.away,
                app.story,
                app.review,
                app.world.health,
            );
            app.on_key(k);
            if app.theme() != theme || app.notify != notify {
                // Remembered for the next run; a failure only costs that.
                let _ = prefs::save(state, &app.theme(), app.notify);
            }
            if app.motion != moves {
                let _ = prefs::keep_motion(state, app.motion);
            }
            if app.away != away {
                let _ = prefs::keep_away(state, app.away);
            }
            if app.story != story {
                let _ = prefs::keep_story(state, app.story);
            }
            if app.review != review {
                let _ = prefs::keep_review(state, app.review);
            }
            if app.world.health != health {
                let _ = prefs::keep_health(state, app.world.health);
            }
            if let Some(text) = app.copy.take() {
                app.copied(clip::copy(&text));
            }
            if let Some(command) = app.shell.take() {
                let cwd = app.cwd();
                let how = shell::run(terminal, &cwd, &command, |ended| app.shell_back(ended));
                app.ran(&command, how);
                // The shell had the terminal, its title and cursor back.
                title.forget();
                cursor.forget();
                tab.forget();
            }
            if let Some(draft) = app.editor.take() {
                let (cwd, editor) = (app.cwd(), editor::of_env());
                let back = editor::run(terminal, state, &cwd, &editor, &draft, |why| {
                    app.editor_back(&editor, why)
                });
                app.edited(&draft, back);
                // The editor had the terminal, its title and cursor too.
                title.forget();
                cursor.forget();
                tab.forget();
            }
        }
        // The key file changed (`:keys` or an edit): kept for the next start.
        app.watch_keys(Instant::now());
        app.grace_due(Instant::now());
        app.keys_due(Instant::now());
        if std::mem::take(&mut app.keys_new) {
            let _ = prefs::keep_keys(state, app.keys_good.as_deref());
        }
        if last.elapsed() >= app.tick_interval() {
            if !app.paused {
                app.step();
            }
            last = Instant::now();
        }
        // In another window: a word when something came to wait.
        if let Some(word) = app.notices(Instant::now()) {
            notify::ring(&mut std::io::stdout(), app.notify, style, &word)?;
            app.push_word(&word);
        }
        app.pushed();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_drawing_is_kept_only_while_it_may_be_patched() {
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        let mut drawn = Buffer::empty(Rect::new(0, 0, 4, 2));
        drawn.set_string(0, 0, "ab", ratatui::style::Style::new());
        let mut shown = None;
        keep(&mut shown, &drawn, false);
        assert_eq!(shown, None);
        keep(&mut shown, &drawn, true);
        assert_eq!(shown.as_ref(), Some(&drawn), "a copy");
        drawn.set_string(0, 1, "cd", ratatui::style::Style::new());
        keep(&mut shown, &drawn, true);
        assert_eq!(shown.as_ref(), Some(&drawn), "the newest");
        keep(&mut shown, &drawn, false);
        assert_eq!(shown, None, "let go");
    }

    #[test]
    fn the_version_is_asked_first_and_said_with_the_name() {
        use std::ffi::OsStr;
        assert!(asks_version(Some(OsStr::new("--version"))));
        assert!(asks_version(Some(OsStr::new("-V"))));
        assert!(!asks_version(Some(OsStr::new("--demo"))));
        assert!(!asks_version(Some(OsStr::new("-v"))));
        assert!(!asks_version(None));
        assert_eq!(version_line(), concat!("imece ", env!("CARGO_PKG_VERSION")));
        assert!(USAGE.contains("imece --version"));
    }

    #[test]
    fn a_signal_becomes_a_quit() {
        catch_signals();
        // SAFETY: raising a signal whose handler was just installed.
        unsafe { libc::raise(libc::SIGHUP) };
        assert!(SIGNALLED.load(Ordering::SeqCst));
    }

    fn args(argv: &[&str], env: &[(&str, &str)]) -> Result<Args, String> {
        let env: Vec<(String, String)> = env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        parse(argv.iter().map(|a| a.to_string()), |k| {
            env.iter().find(|(e, _)| e == k).map(|(_, v)| v.clone())
        })
    }

    #[test]
    fn the_theme_comes_from_the_flag_or_the_environment_and_a_wrong_name_is_refused() {
        crate::theme::set("terminal");
        let a = args(&["--theme", "light"], &[]).unwrap();
        assert_eq!(a.theme.as_deref(), Some("light"));
        assert_eq!(crate::theme::pal().name, "light");
        let a = args(&[], &[("IMECE_THEME", "mono")]).unwrap();
        assert_eq!(a.theme.as_deref(), Some("mono"));
        assert_eq!(crate::theme::pal().name, "mono");
        let a = args(&["--theme", "dracula"], &[]).unwrap();
        assert_eq!(a.theme.as_deref(), Some("dracula"));
        assert_eq!(crate::theme::pal().name, "dracula");
        crate::theme::set("terminal");
        let e = args(&["--theme", "neon"], &[]).unwrap_err();
        assert!(
            e.contains("--theme") && e.contains("terminal|dark|light|mono|dracula"),
            "{e}"
        );
        let e = args(&[], &[("IMECE_THEME", "neon")]).unwrap_err();
        assert!(
            e.contains("IMECE_THEME") && e.contains("no such file"),
            "{e}"
        );
        let e = args(&["--theme", "/no/such/dir/x.conf"], &[]).unwrap_err();
        assert!(e.contains("--theme") && e.contains("x.conf"), "{e}");
        assert_eq!(
            crate::theme::pal().name,
            "terminal",
            "a refused name changes nothing"
        );
        assert_eq!(args(&[], &[]).unwrap().theme, None);
        let e = args(&["--tehme", "dark"], &[]).unwrap_err();
        assert!(e.starts_with("unknown option --tehme"), "{e}");
    }

    #[test]
    fn live_agents_are_the_default_and_the_demo_is_asked_for() {
        let a = args(&[], &[]).unwrap();
        assert!(!a.demo);
        assert_eq!(
            a.agents, None,
            "the leader's cap comes from DEFAULT_WORKERS"
        );
        assert!(a.tiers.is_empty());
        assert_eq!(a.budget, None, "the pot is kept between runs");
        assert!(!a.fresh, "sessions are remembered unless --fresh says");
        assert!(args(&["--fresh"], &[]).unwrap().fresh);
        assert!(args(&["--awake"], &[]).unwrap().awake);
        assert!(!args(&[], &[]).unwrap().awake);
        assert!(
            args(&["--demo", "--awake"], &[]).unwrap().awake,
            "the demo too"
        );
        assert!(USAGE.contains("[--awake]") && USAGE.contains("  --awake  "));
        assert_eq!(
            args(&["--budget", "2.50"], &[]).unwrap().budget,
            Some(25_000),
            "a flag sets it"
        );
        assert!(
            args(&["--budget", "0"], &[]).is_err(),
            "nothing is not a pot"
        );
        assert_eq!(a.repo, PathBuf::from("."));
        assert_eq!(
            args(&["--repo", "/work/other"], &[]).unwrap().repo,
            PathBuf::from("/work/other")
        );
        // The old way of asking for live agents still works.
        assert!(!args(&["--live"], &[]).unwrap().demo);
        let a = args(&["--demo", "--seed", "3", "--agents", "30"], &[]).unwrap();
        assert!(a.demo);
        assert_eq!((a.seed, a.agents), (3, Some(30)));
        assert_eq!(DEFAULT_WORKERS, 4);
    }

    #[test]
    fn the_confine_dial_is_standard_unless_said_and_takes_only_its_levels() {
        assert_eq!(args(&[], &[]).unwrap().confine, Confine::Standard);
        assert_eq!(
            args(&["--confine", "strict"], &[]).unwrap().confine,
            Confine::Strict
        );
        assert_eq!(
            args(&["--confine", "open"], &[]).unwrap().confine,
            Confine::Open
        );
        assert_eq!(
            args(&["--confine", "standard"], &[]).unwrap().confine,
            Confine::Standard
        );
        let e = args(&["--confine", "loose"], &[]).unwrap_err();
        assert!(
            e.contains("\"loose\" is not standard, strict or open"),
            "{e}"
        );
        assert!(USAGE.contains("--confine LEVEL"));
        assert!(USAGE.contains("[--confine standard|strict|open]"));
    }

    #[test]
    fn the_cap_on_helpers_is_zero_to_eight_with_two_unless_said() {
        assert_eq!(args(&[], &[]).unwrap().helpers, DEFAULT_HELPERS);
        assert_eq!(DEFAULT_HELPERS, 2);
        assert_eq!(args(&["--helpers", "0"], &[]).unwrap().helpers, 0);
        assert_eq!(args(&["--helpers", "8"], &[]).unwrap().helpers, 8);
        let e = args(&["--helpers", "9"], &[]).unwrap_err();
        assert!(e.contains("0-8"), "{e}");
        assert!(USAGE.contains("--helpers N"));
    }

    #[test]
    fn programs_to_run_are_named_without_paths_and_the_deadline_is_bounded() {
        let a = args(&[], &[]).unwrap();
        assert!(a.run_programs.is_empty());
        assert_eq!(a.run_timeout, live::DEFAULT_RUN_TIMEOUT);
        let a = args(
            &["--run", "npm, make", "--run", "npm", "--run-timeout", "90"],
            &[],
        )
        .unwrap();
        assert_eq!(a.run_programs, ["npm", "make"]);
        assert_eq!(a.run_timeout, Duration::from_secs(90));
        let e = args(&["--run", "/usr/bin/sh"], &[]).unwrap_err();
        assert!(e.contains("is a path"), "{e}");
        assert!(args(&["--run-timeout", "9"], &[]).is_err());
        assert!(args(&["--run-timeout", "3601"], &[]).is_err());
        assert!(USAGE.contains("--run PROG") && USAGE.contains("--run-timeout S"));
        for cmd in [
            ":open PATH [NAME]",
            ":close NAME",
            ":rename NAME",
            ":solo [PATH] [CHOICE]",
            "--solo claude[:MODEL][@EFFORT]",
        ] {
            assert!(USAGE.contains(cmd), "{cmd}");
        }
    }

    #[test]
    fn the_cap_on_workers_is_zero_to_sixteen_live_and_one_up_in_the_demo() {
        assert_eq!(args(&["--agents", "0"], &[]).unwrap().agents, Some(0));
        assert_eq!(args(&["--agents", "16"], &[]).unwrap().agents, Some(16));
        let e = args(&["--agents", "17"], &[]).unwrap_err();
        assert!(e.contains("at most 16"), "{e}");
        let e = args(&["--agents", "201"], &[]).unwrap_err();
        assert!(e.contains("0-200"), "{e}");
        let e = args(&["--demo", "--agents", "0"], &[]).unwrap_err();
        assert!(e.contains("at least 1"), "{e}");
        assert_eq!(
            args(&["--demo", "--agents", "200"], &[]).unwrap().agents,
            Some(200)
        );
    }

    #[test]
    fn the_demo_on_live_takes_projects_and_live_s_limits() {
        let a = args(&["--demo-live"], &[]).unwrap();
        assert!(a.demo_live && !a.demo);
        assert_eq!((a.projects, a.agents), (2, None));
        assert_eq!(DEMO_LIVE_WORKERS, 3);
        assert!(!args(&[], &[]).unwrap().demo_live);
        let a = args(&["--demo-live", "--projects", "4", "--agents", "16"], &[]).unwrap();
        assert_eq!((a.projects, a.agents), (4, Some(16)));
        assert_eq!(
            args(&["--demo-live", "--agents", "0"], &[]).unwrap().agents,
            Some(0)
        );
        let e = args(&["--demo-live", "--agents", "17"], &[]).unwrap_err();
        assert_eq!(e, "--agents: a leader hires at most 16");
        let e = args(&["--demo-live", "--researchers", "2"], &[]).unwrap_err();
        assert!(e.starts_with("--researchers is for --demo"), "{e}");
        let a = args(&["--demo-live", "--seed", "11"], &[]).unwrap();
        assert_eq!(a.seed, 11, "the seed of the stand-ins' choices");
        let e = args(&["--projects", "2"], &[]).unwrap_err();
        assert!(e.starts_with("--projects is for --demo"), "{e}");
        let e = args(&["--demo", "--demo-live"], &[]).unwrap_err();
        assert_eq!(e, "--demo or --demo-live, not both");
    }

    #[test]
    fn the_demo_s_options_need_the_demo() {
        for flag in ["--projects", "--researchers", "--seed"] {
            let e = args(&[flag, "2"], &[]).unwrap_err();
            assert!(e.starts_with(&format!("{flag} is for --demo")), "{e}");
            assert!(args(&["--demo", flag, "2"], &[]).is_ok(), "{flag}");
            assert!(
                args(&[flag, "2", "--demo"], &[]).is_ok(),
                "{flag} before --demo"
            );
        }
    }

    #[test]
    fn who_leads_is_no_flag_any_more() {
        let e = args(&["--lead", "claude"], &[]).unwrap_err();
        assert!(e.starts_with("unknown option --lead"), "{e}");
    }

    #[test]
    fn tiers_are_tier_equals_model_and_repeat() {
        let a = args(
            &[
                "--tier",
                "strong=opus[1m]",
                "--tier",
                "cheap=haiku@low",
                "--model",
                "sonnet",
            ],
            &[],
        )
        .unwrap();
        assert_eq!(
            a.tiers,
            [
                (Tier::Strong, "opus[1m]".to_string()),
                (Tier::Cheap, "haiku@low".to_string()),
                (Tier::Standard, "sonnet".to_string()),
            ]
        );
        assert_eq!(
            parse_tier("standard=sonnet"),
            Ok((Tier::Standard, "sonnet".to_string()))
        );
        for (bad, why) in [
            ("opus", "TIER=MODEL"),
            ("huge=opus", "cheap, standard or strong"),
            ("strong=", "not a model name"),
            ("strong=--dangerously-skip-permissions", "not a model name"),
            ("=opus", "cheap, standard or strong"),
            ("claude:strong=big", "cheap, standard or strong"),
        ] {
            let e = parse_tier(bad).unwrap_err();
            assert!(e.starts_with("--tier") && e.contains(why), "{bad}: {e}");
        }
        let e = args(&["--tier", "opus"], &[]).unwrap_err();
        assert!(e.contains("TIER=MODEL"), "{e}");
        for (bad, why) in [
            ("", "not a model name"),
            ("--x", "not a model name"),
            ("claude:opus", "not a model name"),
        ] {
            let e = parse_model(bad).unwrap_err();
            assert!(e.starts_with("--model") && e.contains(why), "{bad}: {e}");
        }
    }

    #[test]
    fn solo_takes_claude_with_a_model_and_or_an_effort_it_has() {
        let some = |s: &str| Some(s.to_string());
        assert_eq!(parse_solo("claude"), Ok(None));
        assert_eq!(parse_solo("claude:opus@xhigh"), Ok(some("opus@xhigh")));
        assert_eq!(parse_solo("claude@high"), Ok(some("@high")));
        assert_eq!(parse_solo("claude:sonnet"), Ok(some("sonnet")));
        for (bad, why) in [
            ("", "not claude[:MODEL][@EFFORT]"),
            ("opus", "not claude[:MODEL][@EFFORT]"),
            ("codex:gpt", "not claude[:MODEL][@EFFORT]"),
            ("claude:-x", "not claude[:MODEL][@EFFORT]"),
            ("claude@bogus", "claude has no effort \"bogus\""),
        ] {
            let e = parse_solo(bad).unwrap_err();
            assert!(e.starts_with("--solo: ") && e.contains(why), "{bad:?}: {e}");
        }
        let a = args(&["--solo", "claude:opus@xhigh"], &[]).unwrap();
        assert_eq!(a.solo, Some(some("opus@xhigh")));
        assert_eq!(args(&[], &[]).unwrap().solo, None);
        let e = args(&["--solo", "x"], &[]).unwrap_err();
        assert!(e.starts_with("--solo:"), "{e}");
    }

    #[test]
    fn an_effort_is_checked_on_the_command_line() {
        let ok = |s: &str| parse_model(s).is_ok();
        assert!(ok("opus@xhigh") && ok("@max") && ok("sonnet"));
        let why = "claude has no effort \"bogus\"; one of low, medium, high, xhigh, max";
        assert_eq!(parse_model("opus@bogus"), Err(format!("--model: {why}")));
        assert_eq!(
            parse_tier("strong=opus@bogus"),
            Err(format!("--tier: {why}"))
        );
        let e = args(&["--model", "@bogus"], &[]).unwrap_err();
        assert!(e.starts_with("--model: claude has no effort"), "{e}");
    }

    #[test]
    fn dollars_are_read_exactly() {
        assert_eq!(parse_dollars("5"), Some(50_000));
        assert_eq!(parse_dollars("2.5"), Some(25_000));
        assert_eq!(parse_dollars("0.07"), Some(700));
        assert_eq!(parse_dollars("1.23"), Some(12_300));
        assert_eq!(
            parse_dollars("9999"),
            Some(99_990_000),
            "four digits is the most"
        );
        for bad in ["", ".5", "1.234", "-1", "1e3", "12345", "nan", "1,5"] {
            assert_eq!(parse_dollars(bad), None, "{bad}");
        }
    }
}
