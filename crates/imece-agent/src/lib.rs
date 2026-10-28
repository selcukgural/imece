//! Drives coding agents from vendor CLIs as child processes and reports what
//! they do as vendor-neutral events. The screen never sees a vendor's wire
//! format; it sees [`Event`]s.

pub mod child;
pub mod claude;
pub mod mcp;
pub mod worktree;

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
