//! `tidy`: what a command run by the `run` tool printed, tidied for the
//! model to read (`run::tidy`). The command is the agent's, its output
//! anything.
//!
//! Holds: no escape, bell or carriage return comes out, and no other
//! control character than a line break or a tab.

use imece_agent::run::tidy;

pub fn check(data: &[u8]) {
    let out = tidy(&crate::text(data));
    assert!(
        !out.chars().any(|c| c < ' ' && c != '\n' && c != '\t'),
        "{out:?}"
    );
}
