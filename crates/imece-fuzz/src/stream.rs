//! `stream`: what a Claude Code session prints on stdout, line by line
//! (`claude::Parser`), which imece turns into its own events. The session
//! is the agent's: its text, its tool calls and what Claude Code reports
//! about it all come through here, and `init` is where a session that
//! runs with less than was asked is stopped.
//!
//! Holds: any line is read or refused without a panic, and read the same
//! way twice: two parsers of one session give the same events and stop at
//! the same line. The first byte chooses the session: its level, whether
//! it writes, has the bus, is on Lonca, resumes another.

use std::path::PathBuf;

use imece_agent::claude::{Parser, Resume, Spec, Store, tools};
use imece_agent::{Confine, Event, Mcp};

pub fn check(data: &[u8]) {
    let Some((c, rest)) = crate::choice(data) else {
        return;
    };
    let spec = spec(c);
    let text = crate::text(rest);
    assert_eq!(read(&spec, &text), read(&spec, &text), "{text:?}");
}

/// Every line through one parser, as a session reads them: up to the first
/// it refuses, which ends the session.
pub fn read(spec: &Spec, text: &str) -> Vec<Result<Vec<Event>, String>> {
    let mut p = Parser::new(spec);
    let mut out = Vec::new();
    for line in text.lines() {
        let got = p.line(line);
        let stop = got.is_err();
        out.push(got);
        if stop {
            break;
        }
    }
    out
}

/// The session `c`'s bits choose.
pub fn spec(c: u8) -> Spec {
    let confine = Confine::ALL[usize::from(c % 3)];
    let write = c & 0x04 != 0;
    let bit = |n: u8| c & (1 << n) != 0;
    Spec {
        program: "claude".into(),
        cwd: PathBuf::from("/work/w1"),
        name: "w1".into(),
        tools: tools(write, confine),
        confine,
        write,
        model: None,
        effort: None,
        budget: 1_000_000,
        system: String::new(),
        mcp: bit(3).then(|| Mcp {
            command: "/opt/imece".into(),
            args: vec!["mcp-relay".into(), "/tmp/i/w1.sock".into()],
            tools: vec!["send".into(), "whoami".into()],
        }),
        resume: bit(4).then(|| Resume {
            session: "00000000-0000-4000-8000-000000000001".into(),
            spent: 50_000,
            fork: bit(6).then(|| "00000000-0000-4000-8000-000000000002".into()),
            models: None,
        }),
        scratch: None,
        notes: None,
        store: bit(5).then(|| Store {
            program: "/opt/lonca".into(),
            socket: PathBuf::from("/run/lonca.sock"),
        }),
    }
}
