//! imece: one terminal for many coding agents from many vendors. The agents
//! are fake (`sim`) unless `--live` runs real Claude Code sessions (`live`).

mod app;
mod i18n;
mod live;
mod model;
mod rng;
mod sim;
mod ui;

use std::path::PathBuf;
use std::time::Instant;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use crate::i18n::Lang;

const USAGE: &str =
    "usage: imece [--projects N] [--agents N] [--researchers N] [--seed N] [--lang en|tr]
       imece --live [--repo DIR] [--agents N] [--model NAME] [--budget USD] [--lang en|tr]

  --projects N      projects worked on at once (1-4, default 2)
  --agents N        workers next to the leaders (1-200, default 24; live 1-16, default 2)
  --researchers N   researchers that may run at once (1-32, default 4)
  --seed N          replays the same fake scenario (default 7)
  --lang en|tr      the screen's language (default en, or IMECE_LANG)

  --live            real Claude Code agents on the checkout DIR is in: a leader
                    that reads it, workers that each change their own worktree
  --repo DIR        the checkout (default: the current directory)
  --model NAME      the model every agent uses (default: Claude Code's own)
  --budget USD      the most all agents together may spend (default 5)";

struct Args {
    projects: usize,
    agents: Option<usize>,
    researchers: usize,
    seed: u64,
    lang: Lang,
    live: bool,
    repo: PathBuf,
    model: Option<String>,
    /// Ten-thousandths of a dollar.
    budget: u64,
}

const LIVE_AGENTS: u64 = 16;

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
    let lang = match std::env::var("IMECE_LANG") {
        Ok(v) => Lang::parse(&v).ok_or_else(|| format!("IMECE_LANG: {v:?} is not en or tr"))?,
        Err(_) => Lang::En,
    };
    let mut args = Args {
        projects: 2,
        agents: None,
        researchers: 4,
        seed: 7,
        lang,
        live: false,
        repo: PathBuf::from("."),
        model: None,
        budget: 5 * 10_000,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        if flag == "-h" || flag == "--help" {
            return Err(USAGE.into());
        }
        if flag == "--live" {
            args.live = true;
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
        match flag.as_str() {
            "--projects" => args.projects = number(1, sim::MAX_PROJECTS as u64)? as usize,
            "--agents" => args.agents = Some(number(1, 200)? as usize),
            "--researchers" => args.researchers = number(1, 32)? as usize,
            "--seed" => args.seed = number(0, u64::MAX)?,
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
    if args.live && args.agents.is_some_and(|n| n as u64 > LIVE_AGENTS) {
        return Err(format!("--agents: live runs at most {LIVE_AGENTS}"));
    }
    Ok(args)
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
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    };
    let mut app = if args.live {
        let options = live::Options {
            repo: args.repo,
            workers: args.agents.unwrap_or(2),
            model: args.model,
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
    } else {
        app::App::new(
            args.projects,
            args.agents.unwrap_or(24),
            args.researchers,
            args.seed,
            args.lang,
        )
    };
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal, app: &mut app::App) -> std::io::Result<()> {
    let mut last = Instant::now();
    while !app.quit {
        terminal.draw(|f| ui::draw(f, app))?;
        let timeout = app.tick_interval().saturating_sub(last.elapsed());
        if event::poll(timeout)?
            && let Event::Key(k) = event::read()?
            && k.kind == KeyEventKind::Press
        {
            app.on_key(k);
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
