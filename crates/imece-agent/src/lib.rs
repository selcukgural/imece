//! Drives coding agents from vendor CLIs as child processes and reports what
//! they do as vendor-neutral events. The screen never sees a vendor's wire
//! format; it sees [`Event`]s.

pub mod child;
pub mod claude;
pub mod mcp;
pub mod worktree;

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
}

/// What a session reports, whatever the vendor.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The vendor confirmed the settings it runs with, and they are the ones
    /// asked for.
    Started { session: String, model: String },
    /// Text the agent wrote during a turn. Display only.
    Text(String),
    /// The agent used a tool: its name and what it touched (a path relative
    /// to the agent's directory, a pattern, or nothing).
    Tool { name: String, target: String },
    /// A tool call came back with an error.
    ToolFailed { name: String },
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
