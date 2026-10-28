//! imece: one terminal for many coding agents from many vendors. This is the
//! screen prototype: the agents are fake (`sim`), the screen is real.

mod app;
mod i18n;
mod model;
mod rng;
mod sim;
mod ui;

use std::time::Instant;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use crate::i18n::Lang;

const USAGE: &str =
    "usage: imece [--projects N] [--agents N] [--researchers N] [--seed N] [--lang en|tr]

  --projects N      projects worked on at once (1-4, default 2)
  --agents N        workers next to the leaders (1-200, default 24)
  --researchers N   researchers that may run at once (1-32, default 4)
  --seed N          replays the same fake scenario (default 7)
  --lang en|tr      the screen's language (default en, or IMECE_LANG)";

struct Args {
    projects: usize,
    agents: usize,
    researchers: usize,
    seed: u64,
    lang: Lang,
}

fn parse_args() -> Result<Args, String> {
    let lang = match std::env::var("IMECE_LANG") {
        Ok(v) => Lang::parse(&v).ok_or_else(|| format!("IMECE_LANG: {v:?} is not en or tr"))?,
        Err(_) => Lang::En,
    };
    let mut args = Args {
        projects: 2,
        agents: 24,
        researchers: 4,
        seed: 7,
        lang,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        if flag == "-h" || flag == "--help" {
            return Err(USAGE.into());
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
            "--agents" => args.agents = number(1, 200)? as usize,
            "--researchers" => args.researchers = number(1, 32)? as usize,
            "--seed" => args.seed = number(0, u64::MAX)?,
            "--lang" => {
                args.lang = Lang::parse(&value)
                    .ok_or_else(|| format!("--lang: {value:?} is not en or tr"))?
            }
            _ => return Err(format!("unknown option {flag}\n\n{USAGE}")),
        }
    }
    Ok(args)
}

fn main() -> std::io::Result<()> {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    };
    let mut app = app::App::new(
        args.projects,
        args.agents,
        args.researchers,
        args.seed,
        args.lang,
    );
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
