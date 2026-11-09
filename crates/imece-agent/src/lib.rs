//! Drives Claude Code sessions as child processes and reports what they do
//! as events. The screen never sees Claude Code's wire format; it sees
//! [`Event`]s.

pub mod child;
pub mod claude;
pub mod copy;
pub mod lonca;
#[cfg(any(test, feature = "fake"))]
pub mod lonca_fake;
pub mod mcp;
pub mod run;
pub mod stand_in;
pub mod worktree;

use std::io;
use std::time::Duration;

/// The name imece's MCP server has in every session; its tools are named
/// `mcp__imece__<tool>` on the screen.
pub const MCP_SERVER: &str = "imece";

/// The name a tool of imece's MCP server has on the screen.
pub fn mcp_tool(tool: &str) -> String {
    format!("mcp__{MCP_SERVER}__{tool}")
}

/// Whether `id` is a UUID, as Claude Code's session ids are: the only form
/// that goes on a command line, into a path or back to it.
pub fn valid_uuid(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

/// A new random (version 4) UUID, from the system's random source: the
/// id imece gives a session it forks ([`claude::Resume::fork`]).
pub fn new_uuid() -> std::io::Result<String> {
    use std::io::Read;
    let mut b = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut b)?;
    Ok(uuid_of(b))
}

/// `b` as a version 4, variant 1 UUID: the version's and the variant's
/// bits put over the random ones (added: they do not overlap).
fn uuid_of(mut b: [u8; 16]) -> String {
    b[6] = 0x40 + (b[6] & 0x0f);
    b[8] = 0x80 + (b[8] & 0x3f);
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

/// Whether Claude Code says it is signed in, asked without the network and
/// without spending ([`claude::login`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Login {
    /// Signed in; `subscription` when through a claude.ai plan (Pro, Max,
    /// Team), whose turns Claude Code prices at API rates though the plan
    /// is what is paid: its figures are API-equivalent (record 132).
    Yes {
        subscription: bool,
    },
    No,
    /// It did not say: its first session tells.
    Unknown,
}

/// How long Claude Code has to answer a question about itself.
#[cfg(not(test))]
pub const ASK_WAIT: Duration = Duration::from_secs(5);
/// The tests' fake CLIs answer at once, but a loaded machine running
/// the whole suite can take seconds to start a shell: a wait that ran
/// out made them flaky. None of them waits for it to run out.
#[cfg(test)]
pub const ASK_WAIT: Duration = Duration::from_secs(60);

/// The longest version line kept.
const VERSION_MAX: usize = 60;

/// What `program --version` says, its first line (at most 60 characters),
/// run outside any checkout with the variables `keep` besides
/// [`child::BASE_ENV`]; none when it is not there or does not answer.
pub fn version(program: &str, keep: &[&str]) -> Option<String> {
    let mut cmd = child::command(program, &std::env::temp_dir(), keep);
    cmd.arg("--version");
    let (status, said) = child::ask(cmd, ASK_WAIT)?;
    status.success().then(|| first_line(&said))
}

/// The first line of `said` that is not blank, trimmed and cut at
/// [`VERSION_MAX`] characters.
fn first_line(said: &str) -> String {
    let line = said.lines().map(str::trim).find(|l| !l.is_empty());
    line.unwrap_or_default().chars().take(VERSION_MAX).collect()
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

/// A running session, as the screen drives it.
pub trait Agent: Send {
    /// Gives the agent a prompt. A busy session takes it after the running
    /// turn.
    fn prompt(&mut self, text: &str) -> io::Result<()>;
    /// Gives the session one line of Claude Code's own command line
    /// (`/clear`, `/compact`, `/cost`, whatever the CLI knows), which runs
    /// as a turn of its own and ends in a [`Event::TurnDone`] like a
    /// prompt. What is no such line answers
    /// [`io::ErrorKind::Unsupported`] and nothing runs.
    fn command(&mut self, line: &str) -> io::Result<()>;
    /// Asks the session to stop without waiting.
    fn terminate(&mut self);
    /// The process imece started for the session now (Claude Code):
    /// it and its descendants are the session's, and imece's socket
    /// takes a tool call from them alone. None when there is none.
    fn pid(&self) -> Option<u32> {
        None
    }
}

/// How strong a model an agent gets. Agents choose a tier, never a model
/// name: names change with every release, tiers do not. Claude Code's
/// aliases stand for them; `Standard` is its own default unless the person
/// named a model.
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

/// What an agent runs on, as the person names it: `[MODEL][@EFFORT]`, each
/// part optional (`opus`, `@xhigh`, `claude-opus-5-5@high`); nothing is
/// Claude Code's own default model at its own default effort. A choice is
/// checked against [`claude::EFFORTS`] before a session starts, never
/// passed on unchecked: Claude Code only warns about an effort it does not
/// know and runs at its default.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Choice {
    pub model: Option<String>,
    pub effort: Option<String>,
}

impl Choice {
    /// `[MODEL][@EFFORT]`: the part before the first `@` is the model, the
    /// part after it the effort; an empty part is none.
    pub fn parse(s: &str) -> Choice {
        let (model, effort) = s.split_once('@').unwrap_or((s, ""));
        let some = |s: &str| (!s.is_empty()).then(|| s.to_string());
        Choice {
            model: some(model),
            effort: some(effort),
        }
    }

    /// The choice as the person writes it: `[MODEL][@EFFORT]`.
    pub fn wire(&self) -> String {
        let mut s = self.model.clone().unwrap_or_default();
        if let Some(e) = &self.effort {
            s.push('@');
            s.push_str(e);
        }
        s
    }

    /// Whether the effort, if any, is one of `efforts`; why not, in words
    /// for the person.
    pub fn check(&self, efforts: &[&str]) -> Result<(), String> {
        let Some(e) = &self.effort else {
            return Ok(());
        };
        if !efforts.contains(&e.as_str()) {
            return Err(format!(
                "claude has no effort {e:?}; one of {}",
                efforts.join(", ")
            ));
        }
        Ok(())
    }
}

/// How much of Claude Code an agent's session gets: the
/// confine dial (`--confine`). The leader stays non-writing at every
/// level (a role, not a restriction), and Claude Code's own sub-agent tool
/// stays off at every level: a sub-agent spawned inside the CLI would
/// skip the worktree, the budget, the bus and the screen. imece sandboxes
/// nothing; what each level switches on is Claude Code's own restrictions,
/// checked fail closed against what the session reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Confine {
    /// The person's own vendor setup, as if they ran the CLI by hand:
    /// their settings, hooks, MCP servers, skills and extensions load.
    /// imece checks only what it still asks for: its own server with its
    /// tools, the role's tools, no sub-agent tool.
    Open,
    /// Claude Code's full coding toolset (read, write, shell, web search
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
    fn a_new_uuid_is_random_version_4_and_valid() {
        assert_eq!(uuid_of([0; 16]), "00000000-0000-4000-8000-000000000000");
        assert_eq!(uuid_of([0xff; 16]), "ffffffff-ffff-4fff-bfff-ffffffffffff");
        let b: [u8; 16] = std::array::from_fn(|i| i as u8 * 17);
        assert_eq!(uuid_of(b), "00112233-4455-4677-8899-aabbccddeeff");
        let (a, c) = (new_uuid().unwrap(), new_uuid().unwrap());
        assert!(valid_uuid(&a) && valid_uuid(&c) && a != c, "{a} {c}");
        assert_eq!(&a[14..15], "4");
    }

    #[test]
    fn an_api_error_is_a_passing_fault_an_account_fault_or_neither_by_status_and_words() {
        let of = Fault::of_status;
        assert_eq!(of(None, "connection refused"), Some(Fault::Passing));
        for s in [401, 402, 403] {
            assert_eq!(of(Some(s), "no"), Some(Fault::Account), "{s}");
        }
        for s in [408, 429, 500, 503, 529, 599] {
            assert_eq!(of(Some(s), "slow down"), Some(Fault::Passing), "{s}");
        }
        for s in [400, 404, 407, 409, 413, 499, 600, 200] {
            assert_eq!(of(Some(s), "bad request"), None, "{s}");
        }
        for w in [
            "You exceeded your current QUOTA",
            "check your billing details",
            "Credit balance is too low",
            "Claude usage limit reached",
            "You've hit your limit · resets 3pm",
            "5-hour limit reached",
        ] {
            assert_eq!(of(Some(429), w), Some(Fault::Account), "{w}");
            assert_eq!(of(Some(400), w), Some(Fault::Account), "{w}");
            assert_eq!(of(Some(500), w), Some(Fault::Passing), "{w}: a 5xx passes");
            assert_eq!(of(Some(404), w), None, "{w}");
        }
    }

    #[test]
    fn a_version_is_the_first_line_said_cut_and_only_when_it_answers() {
        assert_eq!(
            first_line("\n  2.1.3 (Claude Code)  \nmore\n"),
            "2.1.3 (Claude Code)"
        );
        assert_eq!(first_line(""), "");
        assert_eq!(first_line(&"x".repeat(80)).len(), 60);
        assert_eq!(first_line(&"x".repeat(60)).len(), 60);
        let dir = std::env::temp_dir().join(format!("imece-version-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake = |name: &str, script: &str| {
            let path = dir.join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path.display().to_string()
        };
        let good = fake(
            "good",
            r#"[ "$1" = --version ] && echo "2.1.3 (Claude Code)" && exit 0; exit 9"#,
        );
        assert_eq!(version(&good, &[]).as_deref(), Some("2.1.3 (Claude Code)"));
        let bad = fake("bad", "echo 1.0; exit 1");
        assert_eq!(
            version(&bad, &[]),
            None,
            "a failing --version is no version"
        );
        assert_eq!(version(&dir.join("gone").display().to_string(), &[]), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

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
    fn a_choice_is_a_model_an_effort_or_both_and_writes_back_the_same() {
        let some = |s: &str| Some(s.to_string());
        for (s, model, effort) in [
            ("", None, None),
            ("@", None, None),
            ("opus", some("opus"), None),
            ("@xhigh", None, some("xhigh")),
            ("gpt-5.6-terra@high", some("gpt-5.6-terra"), some("high")),
            // The first `@` splits; what follows is one effort, right or not.
            ("a@b@c", some("a"), some("b@c")),
        ] {
            let c = Choice::parse(s);
            assert_eq!(
                (c.model.clone(), c.effort.clone()),
                (model, effort),
                "{s:?}"
            );
            assert_eq!(c.wire(), s.trim_end_matches('@'), "{s:?} back");
        }
        assert_eq!(Choice::default(), Choice::parse(""));
    }

    #[test]
    fn a_choice_s_effort_must_be_one_claude_code_has() {
        let efforts = ["low", "high"];
        assert_eq!(Choice::parse("opus").check(&efforts), Ok(()));
        assert_eq!(Choice::parse("@high").check(&efforts), Ok(()));
        assert_eq!(Choice::parse("opus@low").check(&efforts), Ok(()));
        let e = Choice::parse("opus@max").check(&efforts).unwrap_err();
        assert_eq!(e, "claude has no effort \"max\"; one of low, high");
        let e = Choice::parse("@High").check(&efforts).unwrap_err();
        assert!(e.starts_with("claude has no effort \"High\""), "{e}");
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
        assert_eq!(Plain.pid(), None, "no process said, none trusted");
    }
}

/// One window of the plan's usage limit, as a session reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limit {
    /// The window's length in minutes (300: five hours, 10080: a week).
    pub minutes: u32,
    /// How much of it is used, in percent, at most 100.
    pub used: u8,
    /// When it starts again, in seconds since the Unix epoch, if said.
    pub resets_at: Option<u64>,
}

/// Why a turn failed on Claude Code's side, as far as imece acts on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// Overloaded, a server error, a lost connection, a short rate
    /// limit: the same turn may go through a little later.
    Passing,
    /// The account: signed out, a key refused, a quota or a bill. Only
    /// the person mends it.
    Account,
}

impl Fault {
    /// The fault an API error with HTTP `status` (none: no answer came,
    /// the connection failed) and `text` is, if it is one of the two.
    pub fn of_status(status: Option<u16>, text: &str) -> Option<Fault> {
        let account = || {
            let t = text.to_lowercase();
            ACCOUNT_WORDS.iter().any(|w| t.contains(w))
        };
        match status {
            None => Some(Fault::Passing),
            Some(401..=403) => Some(Fault::Account),
            Some(429) if account() => Some(Fault::Account),
            Some(408 | 429 | 500..=599) => Some(Fault::Passing),
            Some(400) if account() => Some(Fault::Account),
            Some(_) => None,
        }
    }
}

/// Words in an error that make a rate limit or a refused request the
/// account's: a quota used up, a bill unpaid, a plan's limit reached.
const ACCOUNT_WORDS: [&str; 6] = [
    "quota",
    "billing",
    "credit",
    "usage limit",
    "hit your limit",
    "limit reached",
];

/// Minutes in the windows Claude Code names.
pub const FIVE_HOURS: u32 = 5 * 60;
pub const WEEK: u32 = 7 * 24 * 60;

/// One step of the agent's own to-do list, as Claude Code's task tools
/// keep it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub text: String,
    pub state: StepState,
}

/// Where a [`Step`] stands: not begun, in hand, or done.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Todo,
    Doing,
    Done,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// Claude Code confirmed the settings it runs with, and they are the ones
    /// asked for.
    Started { session: String, model: String },
    /// Claude Code's own command line, as it reports it (`/name` each),
    /// when the session starts and again whenever it changes; the screen
    /// completes with it, so a new command needs no release.
    Commands(Vec<String>),
    /// How much of its plan's usage limits Claude Code says is used, one
    /// window each (a five-hour one, a weekly one). A window not named
    /// is as it was: an update may be sparse.
    Limits(Vec<Limit>),
    /// The session said nothing about its command line where Claude Code's
    /// format puts it (a field gone from the start message, a list that
    /// never came), which an empty list is not: the format may have
    /// changed, and only imece's own commands complete. Once a session.
    CommandsMissing,
    /// Text the agent wrote during a turn. Display only.
    Text(String),
    /// The agent used a tool: its name and what it touched (a path relative
    /// to the agent's directory, a pattern, or nothing).
    Tool { name: String, target: String },
    /// A tool call came back with an error.
    ToolFailed { name: String },
    /// The agent's own to-do list as its planning tool now holds it,
    /// whole, in the agent's order; an empty list is a cleared one.
    Plan(Vec<Step>),
    /// How full the session's context window is, as Claude Code last
    /// said: the tokens in it, and its size when the vendor says.
    Context { used: u64, size: Option<u64> },
    /// Something Claude Code said about the session that the person may
    /// want in the trace: what a turn cost in tokens, whose plan pays.
    Note(String),
    /// The turn about to end failed on Claude Code's side, of a kind
    /// imece acts on (after Claude Code's own retries gave up); the
    /// [`Event::TurnDone`] with `ok: false` follows at once. A failure of
    /// another kind (a refused request, a full context) comes without it.
    Failed(Fault),
    /// The turn about to end did not say all it cost (record 132): its
    /// result had no figure, or a model it ran on was priced at nothing
    /// though it used tokens (named here), or an earlier conversation of
    /// the session that was not priced was cleared. The
    /// [`Event::TurnDone`] follows at once, with the last total known.
    Unpriced(Option<String>),
    /// A turn ended. `cost` is the session's total so far, in millionths of
    /// a dollar; `denied` counts tool calls Claude Code refused.
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

/// What a one-shot call ([`claude::once`]) said, and
/// what it cost (millionths of a dollar).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Said {
    pub text: String,
    /// None when Claude Code did not say it ([`Event::Unpriced`]): the
    /// caller keeps what it reserved.
    pub cost: Option<u64>,
}

/// Why a one-shot call said nothing, and what it cost when that is
/// known: nothing when it could not start, Claude Code's total when it
/// ended its turn in an error, unknown when it stopped or ran out of time
/// before reporting one (the caller keeps what it reserved).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unsaid {
    pub why: String,
    pub cost: Option<u64>,
}
