//! imece: one terminal for many coding agents from many vendors. The agents
//! are real coding-agent sessions (`live`), or fake ones with `--demo`
//! (`sim`).

mod app;
mod clip;
mod i18n;
mod live;
mod model;
mod prefs;
mod rng;
mod sim;
mod theme;
mod ui;

use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use imece_agent::Tier;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use crate::i18n::Lang;
use crate::model::Vendor;

const USAGE: &str = "usage: imece [--repo DIR] [--agents N] [--lead VENDOR] [--model NAME]
             [--tier T=NAME]... [--budget USD] [--lang en|tr] [--theme NAME]
       imece --demo [--projects N] [--agents N] [--researchers N] [--seed N]

imece runs coding agents on the checkout DIR is in: a leader that reads it
and talks to you, and workers it hires as the work needs them, each changing
its own git worktree.

  --repo DIR        the checkout (default: the current directory)
  --agents N        workers the leader may hire, at most (0-16, default 4)
  --lead VENDOR     the leader's coding agent: claude (default), codex, gemini
  --model NAME      the model every agent runs on (default: the vendor's own)
  --tier T=NAME     the model a tier stands for, T being cheap, standard or
                    strong (claude: haiku, the vendor's own, opus)
  --budget USD      the most all agents together may spend (default 50)
  --lang en|tr      the screen's language (default en, or IMECE_LANG)
  --theme NAME      dark (default), light or mono; or IMECE_THEME; the last
                    choice made with T is remembered

  --demo            fake agents instead, to see the screen at work
  --projects N      projects worked on at once (1-4, default 2)
  --agents N        workers next to the leaders (1-200, default 24)
  --researchers N   researchers that may run at once (1-32, default 4)
  --seed N          replays the same fake scenario (default 7)";

#[derive(Debug)]
struct Args {
    projects: usize,
    agents: Option<usize>,
    researchers: usize,
    seed: u64,
    lang: Lang,
    theme: Option<String>,
    demo: bool,
    repo: PathBuf,
    lead: Vendor,
    model: Option<String>,
    tiers: Vec<(Tier, String)>,
    /// Ten-thousandths of a dollar.
    budget: u64,
}

/// Workers a leader may hire, at most.
const LIVE_AGENTS: u64 = 16;
/// Workers a leader may hire unless the person says otherwise.
const DEFAULT_WORKERS: usize = 4;

/// Dollars with at most two decimals, as ten-thousandths of a dollar.
fn parse_dollars(s: &str) -> Option<u64> {
    let (whole, cents) = s.split_once('.').unwrap_or((s, ""));
    if whole.is_empty() || whole.len() > 4 || cents.len() > 2 {
        return None;
    }
    if !(whole.bytes().chain(cents.bytes())).all(|b| b.is_ascii_digit()) {
        return None;
    }
    let cents: u64 = format!("{cents:0<2}").parse().ok()?;
    Some(whole.parse::<u64>().ok()? * 10_000 + cents * 100)
}

/// Model names as Claude Code takes them: `sonnet`, `claude-opus-5-5`,
/// `opus[1m]`.
fn valid_model(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && !s.starts_with('-')
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._[]".contains(&b))
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
        researchers: 4,
        seed: 7,
        lang,
        theme: match env("IMECE_THEME") {
            Some(v) if theme::set(&v) => Some(v),
            Some(v) => return Err(format!("IMECE_THEME: {v:?} is not {}", theme::names())),
            None => None,
        },
        demo: false,
        repo: PathBuf::from("."),
        lead: Vendor::Claude,
        model: None,
        tiers: Vec::new(),
        budget: 50 * 10_000,
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
            "--researchers" => args.researchers = number(1, 32)? as usize,
            "--seed" => args.seed = number(0, u64::MAX)?,
            "--lead" => {
                args.lead = Vendor::parse(&value)
                    .ok_or_else(|| format!("--lead: {value:?} is not claude, codex or gemini"))?
            }
            "--tier" => args.tiers.push(parse_tier(&value)?),
            "--theme" if theme::set(&value) => args.theme = Some(value),
            "--theme" => {
                return Err(format!("--theme: {value:?} is not {}", theme::names()));
            }
            "--lang" => {
                args.lang = Lang::parse(&value)
                    .ok_or_else(|| format!("--lang: {value:?} is not en or tr"))?
            }
            "--repo" => args.repo = PathBuf::from(&value),
            "--model" if valid_model(&value) => args.model = Some(value),
            "--model" => return Err(format!("--model: {value:?} is not a model name")),
            "--budget" => {
                args.budget = parse_dollars(&value)
                    .filter(|&b| b >= 100)
                    .ok_or_else(|| format!("--budget: {value} is not dollars like 5 or 2.50"))?
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

/// `TIER=MODEL`, as `--tier` takes it.
fn parse_tier(s: &str) -> Result<(Tier, String), String> {
    let (tier, model) = s
        .split_once('=')
        .ok_or_else(|| format!("--tier: {s:?} is not TIER=MODEL, like strong=opus"))?;
    let tier = Tier::from_wire(tier)
        .ok_or_else(|| format!("--tier: {tier:?} is not cheap, standard or strong"))?;
    if !valid_model(model) {
        return Err(format!("--tier: {model:?} is not a model name"));
    }
    Ok((tier, model.to_string()))
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
    let mut app = if args.demo {
        app::App::new(
            args.projects,
            args.agents.unwrap_or(24),
            args.researchers,
            args.seed,
            args.lang,
        )
    } else {
        let options = live::Options {
            repo: args.repo,
            max_workers: args.agents.unwrap_or(DEFAULT_WORKERS),
            lead: args.lead,
            model: args.model,
            tiers: args.tiers,
            budget: args.budget,
        };
        match live::world(options) {
            Ok((world, live)) => {
                app::App::with(world, app::Engine::Live(Box::new(live)), args.lang)
            }
            Err(msg) => {
                eprintln!("imece: {msg}");
                std::process::exit(1);
            }
        }
    };
    let state = prefs::state_dir();
    let saved = prefs::load(&state);
    if let Some(side) = saved.side {
        app.side = side;
    }
    // A flag or the environment wins over the saved theme.
    if args.theme.is_none()
        && let Some(name) = &saved.theme
    {
        theme::set(name);
    }
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app, &state);
    ratatui::restore();
    result
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
    terminal: &mut DefaultTerminal,
    app: &mut app::App,
    state: &std::path::Path,
) -> std::io::Result<()> {
    let mut last = Instant::now();
    while !app.quit && !SIGNALLED.load(Ordering::SeqCst) {
        terminal.draw(|f| ui::draw(f, app))?;
        let timeout = app.tick_interval().saturating_sub(last.elapsed());
        let ready = match event::poll(timeout) {
            Ok(ready) => ready,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if ready
            && let Event::Key(k) = event::read()?
            && k.kind == KeyEventKind::Press
        {
            let (side, theme) = (app.side, theme::pal().name);
            app.on_key(k);
            if app.side != side || theme::pal().name != theme {
                // Remembered for the next run; a failure only costs that.
                let _ = prefs::save(state, app.side, theme::pal().name);
            }
            if let Some(text) = app.copy.take() {
                app.copied(clip::copy(&text));
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
        crate::theme::set("dark");
        let a = args(&["--theme", "light"], &[]).unwrap();
        assert_eq!(a.theme.as_deref(), Some("light"));
        assert_eq!(crate::theme::pal().name, "light");
        let a = args(&[], &[("IMECE_THEME", "mono")]).unwrap();
        assert_eq!(a.theme.as_deref(), Some("mono"));
        assert_eq!(crate::theme::pal().name, "mono");
        crate::theme::set("dark");
        let e = args(&["--theme", "neon"], &[]).unwrap_err();
        assert!(
            e.contains("--theme") && e.contains("dark|light|mono"),
            "{e}"
        );
        let e = args(&[], &[("IMECE_THEME", "neon")]).unwrap_err();
        assert!(
            e.contains("IMECE_THEME") && e.contains("dark|light|mono"),
            "{e}"
        );
        assert_eq!(
            crate::theme::pal().name,
            "dark",
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
        assert_eq!(a.lead, Vendor::Claude);
        assert_eq!(a.model, None);
        assert!(a.tiers.is_empty());
        assert_eq!(a.budget, 500_000);
        assert_eq!(a.repo, PathBuf::from("."));
        // The old way of asking for live agents still works.
        assert!(!args(&["--live"], &[]).unwrap().demo);
        let a = args(&["--demo", "--seed", "3", "--agents", "30"], &[]).unwrap();
        assert!(a.demo);
        assert_eq!((a.seed, a.agents), (3, Some(30)));
        assert_eq!(DEFAULT_WORKERS, 4);
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
        assert_eq!(args(&["--lead", "codex"], &[]).unwrap().lead, Vendor::Codex);
        assert_eq!(
            args(&["--lead", "gemini"], &[]).unwrap().lead,
            Vendor::Gemini
        );
        let e = args(&["--lead", "qwen"], &[]).unwrap_err();
        assert!(
            e.contains("--lead") && e.contains("claude, codex or gemini"),
            "{e}"
        );
    }

    #[test]
    fn tiers_are_tier_equals_model_and_repeat() {
        let a = args(
            &["--tier", "strong=opus[1m]", "--tier", "cheap=sonnet"],
            &[],
        )
        .unwrap();
        assert_eq!(
            a.tiers,
            [
                (Tier::Strong, "opus[1m]".to_string()),
                (Tier::Cheap, "sonnet".to_string())
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
        ] {
            let e = parse_tier(bad).unwrap_err();
            assert!(e.starts_with("--tier") && e.contains(why), "{bad}: {e}");
        }
        let e = args(&["--tier", "opus"], &[]).unwrap_err();
        assert!(e.contains("TIER=MODEL"), "{e}");
    }

    #[test]
    fn dollars_are_read_exactly() {
        assert_eq!(parse_dollars("5"), Some(50_000));
        assert_eq!(parse_dollars("2.5"), Some(25_000));
        assert_eq!(parse_dollars("0.07"), Some(700));
        for bad in ["", ".5", "1.234", "-1", "1e3", "12345", "nan", "1,5"] {
            assert_eq!(parse_dollars(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_model_name_can_not_pass_as_a_flag() {
        assert!(valid_model("sonnet") && valid_model("opus[1m]") && valid_model("claude-opus-5-5"));
        for bad in ["", "--dangerously-skip-permissions", "a b", "x;y", "m/../n"] {
            assert!(!valid_model(bad), "{bad}");
        }
    }
}
