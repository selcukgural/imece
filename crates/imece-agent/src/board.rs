//! The outside board (record 142): the issues of the checkout's own
//! repository on GitHub, read and closed through the person's `gh`.
//!
//! What crosses here is one number, as digits in an argument of its own:
//! never a repository, an address or a line for a shell. `gh` finds the
//! repository from the checkout it runs in, so an item of another
//! repository cannot be named. `gh issue` answers for a pull request's
//! number too, so nothing is closed that a look did not find to be an
//! issue, and open.

use crate::child;
use std::path::Path;
use std::time::Duration;

/// The program asked.
pub const PROGRAM: &str = "gh";

/// How long `gh` has to answer.
pub const WAIT: Duration = Duration::from_secs(20);

/// The most characters kept of an item's title.
pub const TITLE_MAX: usize = 120;

/// What of the person's environment `gh` gets beside [`child::BASE_ENV`]:
/// where its sign-in is kept or given, and the proxy. Not `GH_REPO`, which
/// would name another repository than the checkout's.
const KEEP: &[&str] = &[
    "XDG_CONFIG_HOME",
    "GH_CONFIG_DIR",
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_HOST",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
    "HTTPS_PROXY",
    "https_proxy",
    "NO_PROXY",
    "no_proxy",
    "SSL_CERT_FILE",
];

/// What a look at an item found.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Seen {
    /// An open issue, with its title.
    Open(String),
    /// An issue closed already, with its title.
    Closed(String),
    /// A pull request has that number: never closed from here.
    Pull,
    /// The repository has no issue of that number.
    NoSuch,
    /// It could not be read.
    Unread(Why),
}

/// Why the board did not answer.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Why {
    /// No `gh` on the PATH.
    NoProgram,
    /// `gh` is not signed in.
    SignedOut,
    /// It did not answer in time.
    NoAnswer,
    /// What it said last, cut.
    Said(String),
}

/// What became of closing an item.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Closed {
    /// Closed now, with its title.
    Now(String),
    /// It was closed already.
    Already(String),
    /// Not closed: what the look found instead, or why closing failed.
    Not(Seen),
}

/// Whether `program` is there to run: a path that is a file, or a name a
/// directory of `path` (a `PATH`) holds.
fn found(program: &str, path: Option<&std::ffi::OsStr>) -> bool {
    if program.contains('/') {
        return Path::new(program).is_file();
    }
    path.is_some_and(|p| std::env::split_paths(p).any(|d| d.join(program).is_file()))
}

/// `gh`'s answer to `args` in `checkout`: its output when it ended well,
/// else why not.
fn ask(program: &str, checkout: &Path, args: &[&str]) -> Result<String, Why> {
    if !found(program, std::env::var_os("PATH").as_deref()) {
        return Err(Why::NoProgram);
    }
    let mut cmd = child::command(program, checkout, KEEP);
    cmd.args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .env("NO_COLOR", "1");
    match child::ask(cmd, WAIT) {
        None => Err(Why::NoAnswer),
        Some((status, out)) if status.success() => Ok(out),
        Some((_, out)) => Err(why_of(&out)),
    }
}

/// Why `gh` failed, from what it wrote.
fn why_of(out: &str) -> Why {
    if out.contains("gh auth login") || out.contains("GH_TOKEN") {
        return Why::SignedOut;
    }
    let last = out.lines().rev().find(|l| !l.trim().is_empty());
    Why::Said(cut(
        last.unwrap_or("gh failed and said nothing").trim(),
        200,
    ))
}

/// The first `most` characters of `s`.
fn cut(s: &str, most: usize) -> String {
    s.chars().take(most).collect()
}

/// What `gh issue view N --json title,state,url` answered with: an issue
/// only when its address ends `/issues/N`, since a pull request answers
/// too.
fn seen(item: u32, out: &str) -> Seen {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(out.trim()) else {
        return Seen::Unread(Why::Said(cut(out.trim(), 200)));
    };
    let field = |k: &str| v[k].as_str().unwrap_or_default();
    let url = field("url");
    if url.ends_with(&format!("/pull/{item}")) {
        return Seen::Pull;
    }
    if !url.ends_with(&format!("/issues/{item}")) {
        return Seen::Unread(Why::Said(cut(url, 200)));
    }
    let title = cut(field("title"), TITLE_MAX);
    match field("state") {
        "OPEN" => Seen::Open(title),
        "CLOSED" => Seen::Closed(title),
        other => Seen::Unread(Why::Said(cut(other, 200))),
    }
}

/// Looks at item `item` of `checkout`'s repository.
pub fn look(program: &str, checkout: &Path, item: u32) -> Seen {
    let n = item.to_string();
    match ask(
        program,
        checkout,
        &["issue", "view", &n, "--json", "title,state,url"],
    ) {
        Ok(out) => seen(item, &out),
        Err(Why::Said(said)) if said.contains("Could not resolve to an issue") => Seen::NoSuch,
        Err(why) => Seen::Unread(why),
    }
}

/// Closes item `item` of `checkout`'s repository: only an issue a look
/// just found open.
pub fn close(program: &str, checkout: &Path, item: u32) -> Closed {
    let title = match look(program, checkout, item) {
        Seen::Open(title) => title,
        Seen::Closed(title) => return Closed::Already(title),
        other => return Closed::Not(other),
    };
    match ask(program, checkout, &["issue", "close", &item.to_string()]) {
        Ok(_) => Closed::Now(title),
        Err(why) => Closed::Not(Seen::Unread(why)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    /// A `gh` that answers `view` with `answer` (a shell snippet), logs
    /// each call's arguments and what it sees of the environment.
    fn fake(name: &str, view: &str, close: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("imece-board-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (gh, log) = (dir.join("gh"), dir.join("log"));
        let script = format!(
            "#!/bin/sh\necho \"$* repo=${{GH_REPO:-none}} prompt=$GH_PROMPT_DISABLED\" >> '{}'\n\
             case \"$2\" in\nview) {view} ;;\nclose) {close} ;;\nesac\n",
            log.display()
        );
        std::fs::write(&gh, script).unwrap();
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        (gh, log)
    }

    fn json(state: &str, kind: &str, n: u32) -> String {
        format!(
            "echo '{{\"state\":\"{state}\",\"title\":\"A 529 stops it\",\
             \"url\":\"https://github.com/o/r/{kind}/{n}\"}}'"
        )
    }

    fn calls(log: &Path) -> Vec<String> {
        let text = std::fs::read_to_string(log).unwrap_or_default();
        text.lines().map(str::to_string).collect()
    }

    #[test]
    fn a_look_tells_an_open_issue_a_closed_one_a_pull_request_and_none() {
        let (gh, log) = fake("open", &json("OPEN", "issues", 58), "exit 0");
        let dir = gh.parent().unwrap();
        let gh = gh.to_str().unwrap();
        assert_eq!(look(gh, dir, 58), Seen::Open("A 529 stops it".into()));
        assert_eq!(
            calls(&log),
            ["issue view 58 --json title,state,url repo=none prompt=1"]
        );
        // The answer is for another number: not this item.
        assert!(matches!(look(gh, dir, 5), Seen::Unread(Why::Said(_))));
        let (gh, _) = fake("closed", &json("CLOSED", "issues", 58), "exit 0");
        let gh = gh.to_str().unwrap();
        assert_eq!(look(gh, dir, 58), Seen::Closed("A 529 stops it".into()));
        let (gh, _) = fake("pull", &json("MERGED", "pull", 58), "exit 0");
        assert_eq!(look(gh.to_str().unwrap(), dir, 58), Seen::Pull);
        let (gh, _) = fake("odd", &json("LOCKED", "issues", 58), "exit 0");
        assert_eq!(
            look(gh.to_str().unwrap(), dir, 58),
            Seen::Unread(Why::Said("LOCKED".into()))
        );
        let none = "echo 'GraphQL: Could not resolve to an issue or pull request with the \
                    number of 58. (repository.issue)' >&2; exit 1";
        let (gh, _) = fake("none", none, "exit 0");
        assert_eq!(look(gh.to_str().unwrap(), dir, 58), Seen::NoSuch);
        let (gh, _) = fake("words", "echo not json", "exit 0");
        assert_eq!(
            look(gh.to_str().unwrap(), dir, 58),
            Seen::Unread(Why::Said("not json".into()))
        );
    }

    #[test]
    fn why_it_did_not_answer_is_told_apart() {
        let dir = std::env::temp_dir();
        assert_eq!(
            look("/nonexistent/imece/gh", &dir, 58),
            Seen::Unread(Why::NoProgram)
        );
        let out = "echo 'To get started with GitHub CLI, please run:  gh auth login' >&2; exit 4";
        let (gh, _) = fake("auth", out, "exit 0");
        assert_eq!(
            look(gh.to_str().unwrap(), &dir, 58),
            Seen::Unread(Why::SignedOut)
        );
        let (gh, _) = fake(
            "said",
            "echo one >&2; echo 'HTTP 502' >&2; echo >&2; exit 1",
            "",
        );
        assert_eq!(
            look(gh.to_str().unwrap(), &dir, 58),
            Seen::Unread(Why::Said("HTTP 502".into()))
        );
        let (gh, _) = fake("mute", "exit 1", "");
        assert_eq!(
            look(gh.to_str().unwrap(), &dir, 58),
            Seen::Unread(Why::Said("gh failed and said nothing".into()))
        );
        assert_eq!(why_of("set GH_TOKEN to use gh"), Why::SignedOut);
    }

    #[test]
    fn a_program_is_found_by_its_path_or_on_the_path() {
        let (gh, _) = fake("found", "exit 0", "exit 0");
        let dir = gh.parent().unwrap();
        assert!(found(gh.to_str().unwrap(), None));
        assert!(!found(dir.join("hg").to_str().unwrap(), None));
        assert!(!found(dir.to_str().unwrap(), None), "a directory");
        let path = std::env::join_paths([Path::new("/nonexistent/imece"), dir]).unwrap();
        assert!(found("gh", Some(&path)));
        assert!(!found("hg", Some(&path)));
        assert!(!found("gh", None));
    }

    #[test]
    fn only_an_issue_a_look_found_open_is_closed() {
        let (gh, log) = fake("close", &json("OPEN", "issues", 58), "exit 0");
        let dir = gh.parent().unwrap().to_path_buf();
        assert_eq!(
            close(gh.to_str().unwrap(), &dir, 58),
            Closed::Now("A 529 stops it".into())
        );
        assert_eq!(
            calls(&log)[1..],
            ["issue close 58 repo=none prompt=1"],
            "the number alone, after the look"
        );
        // A pull request, an issue closed already, none, an unread one:
        // nothing is closed.
        for (name, view, want) in [
            ("p", json("OPEN", "pull", 58), Closed::Not(Seen::Pull)),
            (
                "c",
                json("CLOSED", "issues", 58),
                Closed::Already("A 529 stops it".into()),
            ),
            (
                "u",
                "exit 1".to_string(),
                Closed::Not(Seen::Unread(Why::Said("gh failed and said nothing".into()))),
            ),
        ] {
            let (gh, log) = fake(name, &view, "exit 0");
            assert_eq!(close(gh.to_str().unwrap(), &dir, 58), want, "{name}");
            assert_eq!(calls(&log).len(), 1, "{name}: only the look");
        }
        // Closing itself failed: said, not closed.
        let (gh, _) = fake(
            "f",
            &json("OPEN", "issues", 58),
            "echo 'HTTP 403' >&2; exit 1",
        );
        assert_eq!(
            close(gh.to_str().unwrap(), &dir, 58),
            Closed::Not(Seen::Unread(Why::Said("HTTP 403".into())))
        );
    }

    #[test]
    fn a_title_is_cut_and_another_repository_is_never_named() {
        let long = "x".repeat(TITLE_MAX + 30);
        let out = format!(
            "{{\"state\":\"OPEN\",\"title\":\"{long}\",\"url\":\"https://h/o/r/issues/7\"}}"
        );
        assert_eq!(seen(7, &out), Seen::Open("x".repeat(TITLE_MAX)));
        assert!(!KEEP.contains(&"GH_REPO"));
        assert_eq!(cut("abc", 2), "ab");
    }
}
