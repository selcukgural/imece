//! imece's stand-in for Claude Code through the binary (record 130). It
//! also makes cargo build the binary before the unit tests, which run the
//! real `Live` on it (`live/end_to_end.rs`).

use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn the_binary_runs_the_stand_in() {
    let imece = env!("CARGO_BIN_EXE_imece");
    let out = Command::new(imece)
        .args(["stand-in", "--version"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "0.0.0 (imece stand-in)\n"
    );
    let mut child = Command::new(imece)
        .args([
            "stand-in",
            "-p",
            "--permission-mode",
            "acceptEdits",
            "--tools",
            "Read",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let prompt = r#"{"type":"user","message":{"role":"user","content":"hi"}}"#;
    writeln!(child.stdin.take().unwrap(), "{prompt}").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let lines: Vec<serde_json::Value> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines[0]["subtype"], "init");
    assert_eq!(lines[0]["tools"], serde_json::json!(["Read"]));
    assert_eq!(lines.last().unwrap()["type"], "result");
    let bad = Command::new(imece)
        .args(["stand-in", "--tools"])
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(2), "a command line it can not read");
}
