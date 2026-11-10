//! `bus`: the lines an agent's session writes to imece's MCP server
//! (`mcp::serve`), the socket every bus tool is called through.
//!
//! Holds: every reply is one line of JSON-RPC 2.0 with an id and either a
//! result or an error, never both; there are no more replies than lines
//! read; a tool is called only by a name the server offers, and its answer
//! comes back as text.

use imece_agent::mcp::{Tool, serve};
use serde_json::{Value, json};

/// The tools the server offers here, as imece's bus names some of its own.
const TOOLS: &[&str] = &["send", "whoami", "board"];

pub fn check(data: &[u8]) {
    let tools: Vec<Tool> = TOOLS
        .iter()
        .map(|&name| Tool {
            name: name.into(),
            description: format!("the {name} tool"),
            input: json!({ "type": "object" }),
        })
        .collect();
    let mut called = Vec::new();
    let mut out = Vec::new();
    serve(data, &mut out, &tools, |name, args| {
        called.push(name.to_string());
        if name == "board" {
            Err(format!("refused {args}"))
        } else {
            Ok(args.to_string())
        }
    });
    assert!(
        called.iter().all(|c| TOOLS.contains(&c.as_str())),
        "{called:?}"
    );
    let out = String::from_utf8(out).expect("replies are UTF-8");
    let lines = data.split(|&b| b == b'\n').count();
    let replies: Vec<&str> = out.lines().collect();
    assert!(
        replies.len() <= lines,
        "{} replies to {lines} lines",
        replies.len()
    );
    assert!(out.is_empty() || out.ends_with('\n'));
    for reply in replies {
        let v: Value = serde_json::from_str(reply).expect("a reply is JSON");
        assert_eq!(v["jsonrpc"], "2.0", "{reply}");
        assert!(v.get("id").is_some(), "{reply}");
        assert_ne!(
            v.get("result").is_some(),
            v.get("error").is_some(),
            "{reply}"
        );
        let content = &v["result"]["content"];
        if !content.is_null() {
            assert!(content[0]["text"].is_string(), "{reply}");
            assert!(v["result"]["isError"].is_boolean(), "{reply}");
        }
    }
}
