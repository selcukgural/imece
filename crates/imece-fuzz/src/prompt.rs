//! `prompt`: the line imece writes to a session's stdin to give it a
//! prompt (`claude::prompt_line`). The text is the person's or another
//! agent's.
//!
//! Holds: one line, whatever the text, so a prompt can never write a second
//! message of its own; read back, it is a user message of exactly the text.

use imece_agent::claude::prompt_line;
use serde_json::Value;

pub fn check(data: &[u8]) {
    let text = crate::text(data);
    let line = prompt_line(&text);
    assert!(!line.contains('\n') && !line.contains('\r'), "{line:?}");
    let v: Value = serde_json::from_str(&line).expect("a prompt line is JSON");
    assert_eq!(v["type"], "user");
    assert_eq!(v["message"]["role"], "user");
    assert_eq!(v["message"]["content"].as_str(), Some(text.as_str()));
}
