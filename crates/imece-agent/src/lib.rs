//! Drives coding agents from vendor CLIs as child processes and reports what
//! they do as vendor-neutral events. The screen never sees a vendor's wire
//! format; it sees [`Event`]s.

pub mod child;
pub mod claude;
pub mod codex;
pub mod gemini;
pub mod mcp;
pub mod run;
pub mod worktree;

use std::io;

/// The name imece's MCP server has in every session; its tools are named
/// `mcp__imece__<tool>` on the screen, whatever the vendor calls them.
pub const MCP_SERVER: &str = "imece";

/// The name a tool of imece's MCP server has on the screen.
pub fn mcp_tool(tool: &str) -> String {
    format!("mcp__{MCP_SERVER}__{tool}")
}

/// Whether `id` is a UUID as the vendors give their session ids: the only
/// form that goes on a command line, into a path or back to a vendor.
pub fn valid_uuid(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

/// How a session starts imece's MCP server, and which of its tools the
/// agent may call without asking.
#[derive(Clone, Debug, PartialEq)]
pub struct Mcp {
    pub command: String,
    pub args: Vec<String>,
    /// Tool names as the server offers them (`send`).
    pub tools: Vec<String>,
}

/// The tool of imece's MCP server a Claude Code session asks through
/// before a tool call that needs a yes (`--permission-prompt-tool`).
/// Claude Code hides it from the model: it is never among a session's
/// tools, so never in [`Mcp::tools`].
pub const APPROVE_TOOL: &str = "approve";

/// What a session asks the person's yes for, before it does it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    /// A shell command, as the agent wrote it.
    Command { tool: String, command: String },
    /// Any other tool call: the tool and what it would touch.
    Other { tool: String, detail: String },
}

impl Ask {
    /// One line for the person: the tool and what it runs or touches.
    pub fn line(&self) -> String {
        match self {
            Ask::Command { tool, command } => format!("{tool}: {command}"),
            Ask::Other { tool, detail } if detail.is_empty() => tool.clone(),
            Ask::Other { tool, detail } => format!("{tool}: {detail}"),
        }
    }
}

/// A running session of any vendor, as the screen drives it.
pub trait Agent: Send {
    /// Gives the agent a prompt. A busy session takes it after the running
    /// turn.
    fn prompt(&mut self, text: &str) -> io::Result<()>;
    /// Gives the session one line of its vendor's own command line
    /// (`/clear`, `/compact`, `/cost`, whatever the CLI knows), which runs
    /// as a turn of its own and ends in a [`Event::TurnDone`] like a
    /// prompt. A vendor without such a line, or one that does not know
    /// the command, answers [`io::ErrorKind::Unsupported`] and nothing
    /// runs.
    fn command(&mut self, line: &str) -> io::Result<()>;
    /// Asks the session to stop without waiting.
    fn terminate(&mut self);
    /// Answers what the session asked in an [`Event::Asks`] with
    /// `request`: `allow` lets it run, else it is refused and the turn
    /// goes on. An ask already gone is answered by nobody. A vendor that
    /// asks over imece's socket instead (Claude Code) has nothing to
    /// answer here: [`io::ErrorKind::Unsupported`].
    fn decide(&mut self, request: &str, allow: bool) -> io::Result<()> {
        let _ = (request, allow);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "this session asks through imece's socket",
        ))
    }
}

/// How strong a model an agent gets. Agents choose a tier, never a model
/// name: names change with every vendor release, tiers do not. Each vendor
/// maps the tiers to its own models; `Standard` is the vendor's own default
/// unless the person named a model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tier {
    Cheap,
    Standard,
    Strong,
}

impl Tier {
    pub const ALL: [Tier; 3] = [Tier::Cheap, Tier::Standard, Tier::Strong];

    /// The tier's name on the wire and the command line.
    pub fn wire(self) -> &'static str {
        match self {
            Tier::Cheap => "cheap",
            Tier::Standard => "standard",
            Tier::Strong => "strong",
        }
    }

    pub fn from_wire(s: &str) -> Option<Tier> {
        Tier::ALL.into_iter().find(|t| t.wire() == s)
    }
}

/// How much of its vendor's capability an agent's session gets: the
/// confine dial (`--confine`). The leader stays non-writing at every
/// level (a role, not a restriction), and the vendor's own sub-agent tool
/// stays off at every level: a sub-agent spawned inside the CLI would
/// skip the worktree, the budget, the bus and the screen. imece sandboxes
/// nothing; what each level switches on is the vendor's own restrictions,
/// checked fail closed against what the session reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Confine {
    /// The person's own vendor setup, as if they ran the CLI by hand:
    /// their settings, hooks, MCP servers, skills and extensions load.
    /// imece checks only what it still asks for: its own server with its
    /// tools, the role's tools, no sub-agent tool.
    Open,
    /// The vendor's full coding toolset (read, write, shell, web search
    /// and fetch) for workers and helpers, web for the leader; nothing of
    /// the person's setup loads, no MCP server but imece's.
    #[default]
    Standard,
    /// File tools only, commands through imece's `run` tool.
    Strict,
}

impl Confine {
    pub const ALL: [Confine; 3] = [Confine::Open, Confine::Standard, Confine::Strict];

    /// The level's name on the command line and in the journal.
    pub fn wire(self) -> &'static str {
        match self {
            Confine::Open => "open",
            Confine::Standard => "standard",
            Confine::Strict => "strict",
        }
    }

    pub fn from_wire(s: &str) -> Option<Confine> {
        Confine::ALL.into_iter().find(|c| c.wire() == s)
    }

    /// Whether the session gets a shell, web search and fetch: every
    /// level but strict.
    pub fn full(self) -> bool {
        self != Confine::Strict
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_go_over_the_wire_by_name_and_back() {
        assert_eq!(Tier::Cheap.wire(), "cheap");
        assert_eq!(Tier::Standard.wire(), "standard");
        assert_eq!(Tier::Strong.wire(), "strong");
        for t in Tier::ALL {
            assert_eq!(Tier::from_wire(t.wire()), Some(t));
        }
        assert_eq!(Tier::from_wire("huge"), None);
        assert_eq!(Tier::from_wire(""), None);
    }

    #[test]
    fn the_confine_dial_has_three_levels_and_standard_is_the_default() {
        assert_eq!(Confine::default(), Confine::Standard);
        assert_eq!(Confine::Open.wire(), "open");
        assert_eq!(Confine::Standard.wire(), "standard");
        assert_eq!(Confine::Strict.wire(), "strict");
        for c in Confine::ALL {
            assert_eq!(Confine::from_wire(c.wire()), Some(c));
        }
        assert_eq!(Confine::from_wire("loose"), None);
        assert_eq!(Confine::from_wire("Strict"), None);
        assert!(Confine::Open.full());
        assert!(Confine::Standard.full());
        assert!(!Confine::Strict.full());
    }

    #[test]
    fn an_ask_is_one_line_with_its_tool() {
        let run = Ask::Command {
            tool: "Bash".into(),
            command: "cargo publish".into(),
        };
        assert_eq!(run.line(), "Bash: cargo publish");
        let fetch = Ask::Other {
            tool: "WebFetch".into(),
            detail: "https://example.com".into(),
        };
        assert_eq!(fetch.line(), "WebFetch: https://example.com");
        let bare = Ask::Other {
            tool: "Mystery".into(),
            detail: String::new(),
        };
        assert_eq!(bare.line(), "Mystery");
    }

    #[test]
    fn a_session_that_asks_through_the_socket_has_nothing_to_decide() {
        struct Plain;
        impl Agent for Plain {
            fn prompt(&mut self, _: &str) -> io::Result<()> {
                Ok(())
            }
            fn command(&mut self, _: &str) -> io::Result<()> {
                Ok(())
            }
            fn terminate(&mut self) {}
        }
        let e = Plain.decide("1", true).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::Unsupported);
    }
}

/// What a session reports, whatever the vendor.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The vendor confirmed the settings it runs with, and they are the ones
    /// asked for.
    Started { session: String, model: String },
    /// The vendor's own command line, as it reports it (`/name` each),
    /// when the session starts and again whenever it changes; the screen
    /// completes with it, so a vendor's new command needs no release.
    Commands(Vec<String>),
    /// Text the agent wrote during a turn. Display only.
    Text(String),
    /// The agent used a tool: its name and what it touched (a path relative
    /// to the agent's directory, a pattern, or nothing).
    Tool { name: String, target: String },
    /// A tool call came back with an error.
    ToolFailed { name: String },
    /// The session waits for the person's yes before it does `what`;
    /// [`Agent::decide`] with `request` answers it.
    Asks { request: String, what: Ask },
    /// What the session asked with `request` waits no more: its vendor
    /// settled it (the turn ended or was stopped).
    Withdrawn { request: String },
    /// Something the vendor said about the session that the person may
    /// want in the trace: what a turn cost in tokens, whose plan pays.
    Note(String),
    /// A turn ended. `cost` is the session's total so far, in millionths of
    /// a dollar; `denied` counts tool calls the vendor refused.
    TurnDone {
        ok: bool,
        text: String,
        cost: u64,
        denied: usize,
    },
    /// The session is over: the process exited, its settings were not the
    /// ones asked for, or it broke the protocol. Nothing follows.
    Stopped(String),
}
