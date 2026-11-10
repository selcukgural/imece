//! `approve`: what a session asks the person's yes for, through imece's
//! permission prompt tool (`claude::ask`), and the answer imece gives back
//! (`claude::decision`). The session writes the question; Claude Code takes
//! the answer as the tool's input to run.
//!
//! Holds: a shell command is read as exactly the command the session gave;
//! a yes gives the input back unchanged, as JSON Claude Code reads; a no
//! refuses and carries no input.

use imece_agent::Ask;
use imece_agent::claude::{SHELL_TOOL, ask, decision};
use serde_json::Value;

pub fn check(data: &[u8]) {
    let Ok(args) = serde_json::from_slice::<Value>(data) else {
        return;
    };
    let tool = args["tool_name"].as_str().unwrap_or_default();
    let input = &args["input"];
    match ask(&args) {
        Ask::Command { tool: t, command } => {
            assert_eq!(t, SHELL_TOOL);
            assert_eq!(Some(command.as_str()), input["command"].as_str());
        }
        Ask::Other { tool: t, .. } => {
            assert_eq!(t, tool);
            assert!(tool != SHELL_TOOL || !input["command"].is_string());
        }
    }
    let yes: Value = serde_json::from_str(&decision(true, input)).expect("a yes is JSON");
    assert_eq!(yes["behavior"], "allow");
    assert_eq!(&yes["updatedInput"], input);
    let no: Value = serde_json::from_str(&decision(false, input)).expect("a no is JSON");
    assert_eq!(no["behavior"], "deny");
    assert!(no.get("updatedInput").is_none());
}
