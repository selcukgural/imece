//! Every saved input of every fuzz target, through its check: the seeds
//! (`fuzz/seeds/<target>/`) and each crash the fuzzer found, kept once it
//! was fixed (`fuzz/regressions/<target>/`). So a crash, once fixed, stays
//! fixed under plain `cargo test`, with no fuzzer and on stable.
//!
//! `write_seeds` (ignored) writes the seeds again:
//! `cargo test -p imece-fuzz --test inputs -- --ignored write_seeds`.

use std::path::{Path, PathBuf};

use imece_agent::Confine;
use imece_agent::claude::tools;
use imece_fuzz::TARGETS;
use serde_json::json;

fn fuzz_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz")
}

fn inputs(kind: &str, target: &str) -> Vec<PathBuf> {
    let dir = fuzz_dir().join(kind).join(target);
    let Ok(read) = std::fs::read_dir(&dir) else {
        return vec![];
    };
    let mut files: Vec<PathBuf> = read
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_file() && !p.ends_with(".gitkeep"))
        .collect();
    files.sort();
    files
}

#[test]
fn every_saved_input_passes_its_check() {
    for &(target, check) in TARGETS {
        let seeds = inputs("seeds", target);
        assert!(!seeds.is_empty(), "fuzz/seeds/{target} has no seed");
        for path in seeds.iter().chain(&inputs("regressions", target)) {
            let data = std::fs::read(path).unwrap();
            eprintln!("{target}: {}", path.display());
            check(&data);
        }
    }
}

/// Every fuzz target has a binary in `fuzz/`, and every binary a target.
#[test]
fn every_target_has_a_binary() {
    let manifest = std::fs::read_to_string(fuzz_dir().join("Cargo.toml")).unwrap();
    let mut bins = Vec::new();
    let mut in_bin = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_bin = line == "[[bin]]";
        } else if in_bin && let Some(name) = line.strip_prefix("name = \"") {
            bins.push(name.trim_end_matches('"').to_string());
        }
    }
    let targets: Vec<&str> = TARGETS.iter().map(|(n, _)| *n).collect();
    assert_eq!(bins, targets);
    for name in targets {
        let file = fuzz_dir().join(format!("fuzz_targets/{name}.rs"));
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(
            text.contains(&format!("imece_fuzz::{name}::check")),
            "{}",
            file.display()
        );
        assert!(
            fuzz_dir().join("regressions").join(name).is_dir(),
            "fuzz/regressions/{name}"
        );
    }
}

/// CI runs every target: the list in `fuzz.yml`'s `plan` job is `TARGETS`,
/// sorted.
#[test]
fn ci_runs_every_target() {
    let workflow = fuzz_dir().join("../.github/workflows/fuzz.yml");
    let text = std::fs::read_to_string(&workflow).unwrap();
    let line = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("all=\""))
        .and_then(|l| l.strip_suffix('"'))
        .expect("an all=\"...\" line in fuzz.yml");
    let listed: Vec<&str> = line.split(' ').collect();
    let mut targets: Vec<&str> = TARGETS.iter().map(|(n, _)| *n).collect();
    targets.sort_unstable();
    assert_eq!(listed, targets);
}

fn write(target: &str, name: &str, data: &[u8]) {
    let dir = fuzz_dir().join("seeds").join(target);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(name), data).unwrap();
}

/// `first` and then `rest`: a target's choosing byte before its input.
fn with(first: u8, rest: &str) -> Vec<u8> {
    let mut out = vec![first];
    out.extend_from_slice(rest.as_bytes());
    out
}

/// The session byte `stream` reads (`stream::spec`): its level by `% 3`,
/// a writer by bit 2, the bus by bit 3; bits 0, 1 and 7 choose nothing.
fn session(confine: Confine, write: bool, bus: bool) -> u8 {
    (0u8..=255)
        .find(|&c| {
            Confine::ALL[usize::from(c % 3)] == confine
                && (c & 0x04 != 0) == write
                && (c & 0x08 != 0) == bus
                && c & 0x70 == 0
        })
        .unwrap()
}

fn lines(lines: &[String]) -> String {
    lines.join("\n") + "\n"
}

#[test]
#[ignore = "writes fuzz/seeds; run by hand"]
fn write_seeds() {
    // approve: what Claude Code asks through the permission prompt tool.
    let asks = [
        (
            "bash",
            json!({"tool_name": "Bash", "input": {"command": "cargo test --workspace", "description": "tests"}}),
        ),
        (
            "edit",
            json!({"tool_name": "Edit", "input": {"file_path": "src/a.rs", "old_string": "a", "new_string": "b"}}),
        ),
        (
            "fetch",
            json!({"tool_name": "WebFetch", "input": {"url": "https://example.com/?token=x", "prompt": "read"}}),
        ),
        (
            "other",
            json!({"tool_name": "Task", "input": {"subagent_type": "x", "prompt": "a long prompt ".repeat(30), "n": [1, 2.5, -0.0, null, true]}}),
        ),
        ("no-input", json!({"tool_name": "Bash"})),
    ];
    for (name, v) in asks {
        write("approve", name, v.to_string().as_bytes());
    }

    // bus: a session's JSON-RPC lines to imece's MCP server.
    let rpc = |id: serde_json::Value, method: &str, params: serde_json::Value| {
        json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
    };
    write(
        "bus",
        "session",
        lines(&[
            rpc(
                json!(0),
                "initialize",
                json!({"protocolVersion": "2025-06-18", "capabilities": {}}),
            ),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string(),
            rpc(json!(1), "tools/list", json!({})),
            rpc(
                json!("a"),
                "tools/call",
                json!({"name": "send", "arguments": {"to": "lead", "type": "done", "body": "ok"}}),
            ),
            rpc(json!(2), "tools/call", json!({"name": "board"})),
            rpc(
                json!(3),
                "tools/call",
                json!({"name": "exec", "arguments": {}}),
            ),
            rpc(json!(4), "ping", json!({})),
            rpc(json!(5), "resources/list", json!({})),
        ])
        .as_bytes(),
    );
    write(
        "bus",
        "broken",
        b"{\"id\":1}\nnot json\n\n{\"method\":\"ping\"}\n{\"id\":null,\"method\":\"ping\"}",
    );

    // clean: agent text, with what must not reach the terminal.
    let texts = [
        ("plain", "Done: the tests pass, 3 files changed."),
        (
            "secrets",
            "use sk-abc123 and API_KEY=xyz or ghp_456; Authorization: Bearer eyJhbGciOi.eyJzdWIi.c2ln https://u:pw@host/x?token=abc",
        ),
        (
            "escapes",
            "\x1b[31mred\x1b[0m \x1b]0;title\x07 \x1b]8;;https://x\x1b\\link\x1b]8;;\x1b\\ \r\n\tend\u{9b}2J",
        ),
        ("turkish", "İşçi değişikliği bitirdi: ğüşıöç ĞÜŞİÖÇ {} {{}}"),
    ];
    for (name, t) in texts {
        write("clean", name, t.as_bytes());
    }

    // patch: a worker's patch, the first byte choosing hunks.
    let patch = "diff --git a/src/a.rs b/src/a.rs\n\
        index 1111111..2222222 100644\n\
        --- a/src/a.rs\n\
        +++ b/src/a.rs\n\
        @@ -1,3 +1,3 @@\n fn a() {\n-    1\n+    2\n }\n\
        @@ -10,2 +10,3 @@ fn b() {\n     x\n+    y\n }\n\
        diff --git a/img.png b/img.png\n\
        Binary files a/img.png and b/img.png differ\n\
        diff --git a/old.md b/new.md\n\
        similarity index 90%\n\
        rename from old.md\n\
        rename to new.md\n\
        @@ -1 +1 @@\n-old\n+new\n\\ No newline at end of file\n";
    write("patch", "all", &with(0xff, patch));
    write("patch", "some", &with(0b101, patch));
    write("patch", "crlf", &with(0b10, &patch.replace('\n', "\r\n")));

    // paths: a card's paths (padded to the first byte's place) and a write.
    let paths = |list: serde_json::Value, written: &str| {
        let mut list = list.to_string();
        while !list.len().is_multiple_of(4) {
            list.push(' ');
        }
        let mut out = vec![u8::try_from(list.len() / 4).unwrap()];
        out.extend_from_slice(list.as_bytes());
        out.extend_from_slice(written.as_bytes());
        out
    };
    write(
        "paths",
        "owned",
        &paths(
            json!(["src/a.rs", "./docs/", "tests/*_it.rs", "README.md shared"]),
            "src/b.rs",
        ),
    );
    write(
        "paths",
        "refused",
        &paths(json!(["../etc/passwd", "/abs"]), "x"),
    );
    write(
        "paths",
        "stars",
        &paths(json!(["**/*", "a*b*c*d*e"]), "a/b/c/d/e/f"),
    );

    // prompt: what the person or another agent says to a session.
    let prompts = [
        ("plain", "Add rate limits to the API."),
        ("lines", "two\nlines \"quoted\" \\ and\r\na tab\t"),
        ("slash", "/clear\n{\"type\":\"user\"}"),
    ];
    for (name, t) in prompts {
        write("prompt", name, t.as_bytes());
    }

    // split: the `run` tool's command lines.
    let commands = [
        ("cargo", "cargo test --workspace -- --nocapture"),
        (
            "quotes",
            r#"git log --format='%h %s' "a \"b\" \$x" a\ b HEAD~1"#,
        ),
        ("refused", "ls | wc -l; echo $HOME > out"),
        ("empty", "''  \"\""),
    ];
    for (name, t) in commands {
        write("split", name, t.as_bytes());
    }

    // stream: a session's stdout, the first byte choosing the session.
    let init = |confine: Confine, write: bool, bus: bool| {
        let mut t = tools(write, confine);
        let mut servers = vec![];
        if bus {
            t.extend([
                "mcp__imece__send".to_string(),
                "mcp__imece__whoami".to_string(),
            ]);
            servers.push(json!({"name": "imece", "status": "connected"}));
        }
        json!({"type": "system", "subtype": "init", "session_id": "s1", "model": "m1",
               "tools": t, "permissionMode": "acceptEdits", "mcp_servers": servers, "slash_commands": []})
        .to_string()
    };
    let turn = [
        json!({"type": "assistant", "message": {"usage": {"input_tokens": 10, "cache_read_input_tokens": 2000}, "content": [{"type": "text", "text": "Reading the code."}]}}).to_string(),
        json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "t1", "name": "Edit", "input": {"file_path": "/work/w1/src/a.rs", "old_string": "1", "new_string": "2"}}]}}).to_string(),
        json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "t1", "content": "ok"}]}}).to_string(),
        json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "t2", "name": "TaskCreate", "input": {"subject": "tests"}}]}}).to_string(),
        json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "t2", "content": "Task #1 created successfully: tests"}]}}).to_string(),
        json!({"type": "rate_limit_event", "rate_limit_info": {"status": "allowed", "rateLimitType": "five_hour", "utilization": 0.4, "resetsAt": 1_800_000_000}}).to_string(),
        json!({"type": "result", "subtype": "success", "is_error": false, "result": "done", "total_cost_usd": 0.0123,
               "modelUsage": {"claude-x": {"costUSD": 0.0123, "contextWindow": 200_000}}}).to_string(),
    ];
    for (name, confine, write_, bus) in [
        ("strict-writer", Confine::Strict, true, false),
        ("standard-writer-bus", Confine::Standard, true, true),
        ("open-reader-bus", Confine::Open, false, true),
    ] {
        let mut all = vec![init(confine, write_, bus)];
        // A reader that edits is stopped: its turn has no `Edit`.
        let edits = |i: usize| !write_ && (i == 1 || i == 2);
        all.extend(
            (turn.iter().enumerate())
                .filter(|&(i, _)| !edits(i))
                .map(|(_, l)| l.clone()),
        );
        write(
            "stream",
            name,
            &with(session(confine, write_, bus), &lines(&all)),
        );
    }
    write(
        "stream",
        "refused-init",
        &with(
            0,
            &lines(&[init(Confine::Strict, true, true), turn[0].clone()]),
        ),
    );

    // tidy: what a command printed.
    let outputs = [
        (
            "cargo",
            "\x1b[1m\x1b[32m   Compiling\x1b[0m imece v0.1\r\nrunning 3 tests\ntest a ... ok\ntest b ... FAILED\ntest c ... ok\n\nfailures:\n\ntest result: FAILED. 2 passed; 1 failed\n",
        ),
        (
            "progress",
            "10%\r50%\r100%\ndone\ndone\ndone\n\n\n\nend\x07\x1b]0;t\x07\x1b[2K\x1b",
        ),
    ];
    for (name, t) in outputs {
        write("tidy", name, t.as_bytes());
    }
}

/// The sessions `stream`'s seeds start get past `init`, all but the one
/// meant to be refused, so the fuzzer starts from a session that runs.
#[test]
fn stream_seeds_get_past_init() {
    for path in inputs("seeds", "stream") {
        let data = std::fs::read(&path).unwrap();
        let spec = imece_fuzz::stream::spec(data[0]);
        let read = imece_fuzz::stream::read(&spec, &String::from_utf8_lossy(&data[1..]));
        let refused = path.ends_with("refused-init");
        assert_eq!(
            read.iter().any(Result::is_err),
            refused,
            "{}: {read:?}",
            path.display()
        );
        if !refused {
            assert!(read.len() > 4, "{}", path.display());
        }
    }
}
