//! imece: one terminal for many coding agents from many vendors. The agents
//! are real coding-agent sessions (`live`), or fake ones with `--demo`
//! (`sim`).

mod app;
mod clip;
mod complete;
mod edit;
mod i18n;
mod live;
mod model;
mod prefs;
mod probe;
mod rng;
mod schemes;
mod screen;
mod shell;
mod sim;
mod store;
mod theme;
mod ui;

use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use imece_agent::{Confine, Tier};
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use crate::i18n::Lang;
use crate::model::{Vendor, parse_dollars};

const USAGE: &str = "usage: imece [--repo DIR] [--agents N] [--helpers N] [--lead VENDOR]
             [--model [V:]NAME]... [--tier [V:]T=NAME]... [--budget USD] [--fresh]
             [--confine standard|strict|open] [--lang en|tr] [--theme NAME|FILE]
       imece --demo [--projects N] [--agents N] [--researchers N] [--seed N]

imece runs coding agents on the checkout DIR is in: a leader that reads it
and talks to you, and workers it hires as the work needs them, each changing
its own git worktree; a worker may hire helpers for parts of its task.
While it runs, :open PATH [NAME] opens another checkout as a project of its
own, with its own leader; :close NAME closes one, :rename NAME renames the
one at hand; ? lists every key and command.

  --repo DIR        the checkout (default: the current directory)
  --agents N        workers the leader may hire, at most (0-16, default 4)
  --helpers N       helpers a worker may have at once, at most (0-8, default 2)
  --lead VENDOR     the leader's coding agent: claude, codex or gemini; ask
                    asks at the start. Without it: your primary vendor (set
                    with p in that question, or with :lead VENDOR), else the
                    only one on PATH, else the question
  --model [V:]NAME  the model agents of vendor V run on (default: the
                    vendor's own); without V, the leader's vendor
  --tier [V:]T=NAME the model a tier stands for, T being cheap, standard or
                    strong (claude: haiku, the vendor's own, opus; codex:
                    an effort on its default model, @low, the default,
                    @high, or MODEL@EFFORT); without V, the leader's vendor
  --budget USD      the most all agents together may spend; one pot, kept
                    between runs (50 the first time); :budget USD changes
                    it while agents run
  --fresh           forget every agent's remembered session: each wakes in a
                    new one, told where it left off, and what the vendors
                    kept of the old ones is removed (:forget @NAME does it
                    for one agent while imece runs)
  --confine LEVEL   how much of its vendor each agent gets: standard (the
                    default) gives workers and helpers the vendor's full
                    coding toolset (read, write, shell, web search and
                    fetch) and the leader the web, with nothing of your own
                    vendor setup loaded; strict gives file tools only, with
                    commands through imece's run tool; open runs the vendor
                    as you have it set up (your settings, hooks, MCP
                    servers, skills, extensions). The leader never writes
                    and the vendor's own sub-agent tool stays off at every
                    level; imece sandboxes nothing, isolation is a
                    container's job
  --run PROG[,..]   programs workers may run with their run tool, next to
                    cargo and git's reading commands (repeatable)
  --run-timeout S   seconds one run may take (10-3600, default 600)
  --lang en|tr      the screen's language (default en, or IMECE_LANG)
  --theme NAME|FILE terminal (default: your terminal's own colours), dark,
                    light, mono, a scheme (dracula, nord, gruvbox-dark-medium,
                    catppuccin-mocha, tokyo-night-dark, solarized-dark, …) or
                    a colour file in Ghostty's or Kitty's format; or
                    IMECE_THEME; the last choice (T, :theme) is remembered

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
    fresh: bool,
    repo: PathBuf,
    lead: Option<Lead>,
    /// Stand-ins for the vendors' own models, by vendor (none: the
    /// leader's) and tier; `--model` is the standard tier.
    tiers: Vec<(Option<Vendor>, Tier, String)>,
    /// Ten-thousandths of a dollar; none keeps the pot as it was left.
    budget: Option<u64>,
    run_programs: Vec<String>,
    run_timeout: Duration,
    confine: Confine,
}

/// What `--lead` asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lead {
    Vendor(Vendor),
    /// Ask at the start, whatever the primary vendor is.
    Ask,
}

/// Who leads, from the flag, the primary vendor `primary` and the vendors
/// on PATH that imece drives, `candidates`: `None` is a question for the
/// person. A flag names a vendor even when it is not here (the honest error
/// comes when the team is built); a primary vendor that is not here is
/// skipped, and the chooser says so.
fn choose_lead(
    flag: Option<Lead>,
    primary: Option<Vendor>,
    candidates: &[Vendor],
) -> Option<Vendor> {
    match flag {
        Some(Lead::Vendor(v)) => return Some(v),
        Some(Lead::Ask) => {}
        None => {
            if let Some(p) = primary.filter(|p| candidates.contains(p)) {
                return Some(p);
            }
        }
    }
    match candidates {
        // Nothing to choose from: the team's own error says what is missing.
        [] => Some(primary.unwrap_or(Vendor::Claude)),
        [only] => Some(*only),
        _ => None,
    }
}

/// Workers a leader may hire, at most.
const LIVE_AGENTS: u64 = 16;
/// Workers a leader may hire unless the person says otherwise.
const DEFAULT_WORKERS: usize = 4;
/// Helpers a worker may have at once, at most, and unless the person says
/// otherwise.
const MAX_HELPERS: u64 = 8;
const DEFAULT_HELPERS: usize = 2;

/// Dollars with at most two decimals, as ten-thousandths of a dollar.
/// Model names as Claude Code takes them: `sonnet`, `claude-opus-5-5`,
/// `opus[1m]`.
fn valid_model(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && !s.starts_with('-')
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._[]@".contains(&b))
}

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
        fresh: false,
        repo: PathBuf::from("."),
        lead: None,
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
        if flag == "--fresh" {
            args.fresh = true;
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
            "--lead" if value == "ask" => args.lead = Some(Lead::Ask),
            "--lead" => {
                args.lead = Some(Lead::Vendor(Vendor::parse(&value).ok_or_else(|| {
                    format!("--lead: {value:?} is not claude, codex, gemini or ask")
                })?))
            }
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

/// `[VENDOR:]REST`: the vendor a `--model` or `--tier` value names, if
/// any, and the rest. A model name holds no colon, so a colon is a vendor.
fn split_vendor<'a>(flag: &str, s: &'a str) -> Result<(Option<Vendor>, &'a str), String> {
    match s.split_once(':') {
        None => Ok((None, s)),
        Some((v, rest)) => Vendor::parse(v)
            .filter(|v| live::KNOWN.iter().any(|k| k.0 == *v && k.2))
            .map(|v| (Some(v), rest))
            .ok_or_else(|| {
                format!("{flag}: {v:?} is not a vendor imece drives (claude, codex, gemini)")
            }),
    }
}

/// `[VENDOR:]TIER=MODEL`, as `--tier` takes it.
fn parse_tier(s: &str) -> Result<(Option<Vendor>, Tier, String), String> {
    let (vendor, s) = split_vendor("--tier", s)?;
    let (tier, model) = s
        .split_once('=')
        .ok_or_else(|| format!("--tier: {s:?} is not TIER=MODEL, like strong=opus"))?;
    let tier = Tier::from_wire(tier)
        .ok_or_else(|| format!("--tier: {tier:?} is not cheap, standard or strong"))?;
    if !valid_model(model) {
        return Err(format!("--tier: {model:?} is not a model name"));
    }
    Ok((vendor, tier, model.to_string()))
}

/// `[VENDOR:]MODEL`, as `--model` takes it: the standard tier.
fn parse_model(s: &str) -> Result<(Option<Vendor>, Tier, String), String> {
    let (vendor, model) = split_vendor("--model", s)?;
    if !valid_model(model) {
        return Err(format!("--model: {model:?} is not a model name"));
    }
    Ok((vendor, Tier::Standard, model.to_string()))
}

fn main() -> std::io::Result<()> {
    // A live agent's CLI starts imece this way as its MCP server: it only
    // carries bytes between the CLI and imece's socket for that agent.
    let mut argv = std::env::args_os().skip(1);
    if argv.next().is_some_and(|a| a == "mcp-relay") {
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
    let mut saved = prefs::load(&state);
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
    let mut terminal = screen::init(said.keys).inspect_err(|_| screen::restore())?;
    let mut app = if args.demo {
        // The simulation's clock says 14:00 at its start, on today's date.
        model::start_clock_at(14 * 3600, live::today());
        app::App::new(
            args.projects,
            args.agents.unwrap_or(24),
            args.researchers,
            args.seed,
            args.lang,
        )
    } else {
        let fail = |msg: String| -> ! {
            screen::restore();
            eprintln!("imece: {msg}");
            std::process::exit(1);
        };
        // Running every vendor's --version takes a moment: say so.
        terminal
            .draw(|f| ui::pick::probing(f, args.lang))
            .unwrap_or_else(|e| fail(e.to_string()));
        let present = live::probe(&args.repo, live::KNOWN).unwrap_or_else(|e| fail(e));
        let candidates = live::leaders(&present);
        let primary = saved.lead.as_deref().and_then(Vendor::parse);
        let lead = match choose_lead(args.lead, primary, &candidates) {
            Some(v) => v,
            None => {
                let mut pick = ui::pick::Pick::new(candidates, primary, args.lang);
                match ask(&mut terminal, &mut pick) {
                    Ok(Some(ui::pick::Picked::Lead { vendor, remember })) => {
                        if remember {
                            saved.lead = Some(vendor.label().to_string());
                            let _ = prefs::save(
                                &state,
                                saved.side.unwrap_or(prefs::SIDE_DEFAULT),
                                &theme::chosen(),
                                saved.lead.as_deref(),
                            );
                        }
                        vendor
                    }
                    Ok(_) => {
                        screen::restore();
                        return Ok(());
                    }
                    Err(e) => fail(e.to_string()),
                }
            }
        };
        let options = live::Options {
            repo: args.repo,
            state: state.clone(),
            max_workers: args.agents.unwrap_or(DEFAULT_WORKERS),
            max_helpers: args.helpers,
            lead,
            tiers: args.tiers,
            budget: args.budget,
            fresh: args.fresh,
            run_programs: args.run_programs,
            run_timeout: args.run_timeout,
            confine: args.confine,
        };
        match live::world(options, &present) {
            Ok((world, live)) => {
                // The screen's clock counts from when the world began,
                // which may be an earlier run.
                let (seconds, day) = live::local_clock(world.started_at);
                model::start_clock_at(seconds, day);
                app::App::with(world, app::Engine::Live(Box::new(live)), args.lang)
            }
            Err(msg) => fail(msg),
        }
    };
    if let Some(side) = saved.side {
        app.side = side;
    }
    app.shift_enter = said.keys;
    app.primary = saved.lead;
    let result = run(&mut terminal, &mut app, &state);
    // The last word before the sessions end: what every agent had in hand,
    // and when this run ended.
    app.persist();
    app.leave();
    screen::restore();
    result
}

/// Asks who leads, until the person answers or quits (also by signal).
fn ask(
    terminal: &mut screen::Screen,
    pick: &mut ui::pick::Pick,
) -> std::io::Result<Option<ui::pick::Picked>> {
    loop {
        if SIGNALLED.load(Ordering::SeqCst) {
            return Ok(None);
        }
        terminal.draw(|f| ui::pick::draw(f, pick))?;
        let ready = match event::poll(std::time::Duration::from_millis(200)) {
            Ok(ready) => ready,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if ready
            && let Event::Key(k) = event::read()?
            && k.kind == KeyEventKind::Press
            && let Some(picked) = pick.on_key(k.code)
        {
            return Ok(Some(picked));
        }
    }
}

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
) -> std::io::Result<()> {
    let mut last = Instant::now();
    let mut title = screen::Title::default();
    while !app.quit && !SIGNALLED.load(Ordering::SeqCst) {
        terminal.draw(|f| ui::draw(f, app))?;
        let waiting = app.world.open_inbox().len();
        title.show(&mut std::io::stdout(), &ui::words::title(app.lang, waiting))?;
        let timeout = app.tick_interval().saturating_sub(last.elapsed());
        let ready = match event::poll(timeout) {
            Ok(ready) => ready,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        let event = if ready { Some(event::read()?) } else { None };
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
            let (side, theme, primary) = (app.side, app.theme(), app.primary.clone());
            app.on_key(k);
            if app.side != side || app.theme() != theme || app.primary != primary {
                // Remembered for the next run; a failure only costs that.
                let _ = prefs::save(state, app.side, &app.theme(), app.primary.as_deref());
            }
            if let Some(text) = app.copy.take() {
                app.copied(clip::copy(&text));
            }
            if let Some(command) = app.shell.take() {
                let cwd = app.cwd();
                let how = shell::run(terminal, &cwd, &command, |ended| app.shell_back(ended));
                app.ran(&command, how);
                // The shell had the terminal, and its title back.
                title.forget();
            }
        }
        if last.elapsed() >= app.tick_interval() {
            if !app.paused {
                app.step();
            }
            last = Instant::now();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(a.lead, None, "who leads is decided at the start");
        assert!(a.tiers.is_empty());
        assert_eq!(a.budget, None, "the pot is kept between runs");
        assert!(!a.fresh, "sessions are remembered unless --fresh says");
        assert!(args(&["--fresh"], &[]).unwrap().fresh);
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
    fn who_leads_is_the_flag_else_the_primary_else_the_only_one_else_a_question() {
        use Vendor::{Claude, Codex, Gemini};
        let both = [Claude, Codex];
        // The flag wins, even over a primary vendor, even when not here.
        assert_eq!(
            choose_lead(Some(Lead::Vendor(Gemini)), Some(Claude), &both),
            Some(Gemini)
        );
        // The primary vendor answers when it is here.
        assert_eq!(choose_lead(None, Some(Codex), &both), Some(Codex));
        // Not here: a question, like no primary at all.
        assert_eq!(choose_lead(None, Some(Gemini), &both), None);
        assert_eq!(choose_lead(None, None, &both), None);
        // One candidate is no question, whatever the primary.
        assert_eq!(choose_lead(None, Some(Gemini), &[Codex]), Some(Codex));
        assert_eq!(
            choose_lead(Some(Lead::Ask), Some(Claude), &[Codex]),
            Some(Codex)
        );
        // ask asks even with a primary vendor here.
        assert_eq!(choose_lead(Some(Lead::Ask), Some(Claude), &both), None);
        // Nobody here: no question; the team's error names what is missing.
        assert_eq!(choose_lead(None, None, &[]), Some(Claude));
        assert_eq!(choose_lead(None, Some(Codex), &[]), Some(Codex));
        assert_eq!(choose_lead(Some(Lead::Ask), None, &[]), Some(Claude));
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
        for cmd in [":open PATH [NAME]", ":close NAME", ":rename NAME"] {
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
    fn the_leader_s_vendor_is_one_imece_knows() {
        assert_eq!(
            args(&["--lead", "codex"], &[]).unwrap().lead,
            Some(Lead::Vendor(Vendor::Codex))
        );
        assert_eq!(
            args(&["--lead", "gemini"], &[]).unwrap().lead,
            Some(Lead::Vendor(Vendor::Gemini))
        );
        assert_eq!(args(&["--lead", "ask"], &[]).unwrap().lead, Some(Lead::Ask));
        let e = args(&["--lead", "qwen"], &[]).unwrap_err();
        assert!(
            e.contains("--lead") && e.contains("claude, codex, gemini or ask"),
            "{e}"
        );
    }

    #[test]
    fn tiers_are_tier_equals_model_and_repeat() {
        let a = args(
            &[
                "--tier",
                "strong=opus[1m]",
                "--tier",
                "codex:cheap=gpt-5.6-luna@low",
                "--model",
                "sonnet",
                "--model",
                "codex:gpt-5.6-terra",
            ],
            &[],
        )
        .unwrap();
        assert_eq!(
            a.tiers,
            [
                (None, Tier::Strong, "opus[1m]".to_string()),
                (
                    Some(Vendor::Codex),
                    Tier::Cheap,
                    "gpt-5.6-luna@low".to_string()
                ),
                (None, Tier::Standard, "sonnet".to_string()),
                (
                    Some(Vendor::Codex),
                    Tier::Standard,
                    "gpt-5.6-terra".to_string()
                ),
            ]
        );
        assert_eq!(
            parse_tier("standard=sonnet"),
            Ok((None, Tier::Standard, "sonnet".to_string()))
        );
        assert_eq!(
            parse_tier("claude:strong=opus"),
            Ok((Some(Vendor::Claude), Tier::Strong, "opus".to_string()))
        );
        for (bad, why) in [
            ("opus", "TIER=MODEL"),
            ("huge=opus", "cheap, standard or strong"),
            ("strong=", "not a model name"),
            ("strong=--dangerously-skip-permissions", "not a model name"),
            ("=opus", "cheap, standard or strong"),
            ("qwen:strong=big", "not a vendor imece drives"),
        ] {
            let e = parse_tier(bad).unwrap_err();
            assert!(e.starts_with("--tier") && e.contains(why), "{bad}: {e}");
        }
        let e = args(&["--tier", "opus"], &[]).unwrap_err();
        assert!(e.contains("TIER=MODEL"), "{e}");
        for (bad, why) in [
            ("", "not a model name"),
            ("--x", "not a model name"),
            ("qwen:pro", "not a vendor imece drives"),
            ("codex:", "not a model name"),
        ] {
            let e = parse_model(bad).unwrap_err();
            assert!(e.starts_with("--model") && e.contains(why), "{bad}: {e}");
        }
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

    #[test]
    fn a_model_name_can_not_pass_as_a_flag() {
        assert!(valid_model("sonnet") && valid_model("opus[1m]") && valid_model("claude-opus-5-5"));
        assert!(valid_model("@low") && valid_model("gpt-5.6-terra@xhigh"));
        for bad in [
            "",
            "--dangerously-skip-permissions",
            "a b",
            "x;y",
            "m/../n",
            "codex:x",
        ] {
            assert!(!valid_model(bad), "{bad}");
        }
    }
}
