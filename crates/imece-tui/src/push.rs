//! A push to the person's phone through ntfy (`:notify push`, record 91):
//! what came to wait while they were away, said as `:notify` says it in
//! the terminal (after [`crate::notify::AFTER`], once each, a count when
//! several), sent to an ntfy topic they follow.
//!
//! What leaves the machine is imece's own words, an agent's name and a
//! project's, cleaned (never a command, a question or a message body).
//! The configuration is a file of the person's (`push` in imece's config
//! directory, [`FILE`]); a token for a server of their own is read from a
//! file only they can read, never written on a command line, in a URL or
//! in a log: `curl` gets it on its stdin ([`config`]). A topic on the
//! public server must not be one to guess ([`Why::Guessable`]). A push
//! that fails is said once and never tried again; the next thing that
//! waits is pushed as its own.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// The configuration file's name in imece's config directory.
pub const FILE: &str = "push";

/// The public ntfy server, and the default.
pub const PUBLIC: &str = "https://ntfy.sh";

/// The most bytes the configuration file and the token file may have.
pub const FILE_MAX: u64 = 4 << 10;

/// A topic on the public server is refused shorter than this, or without
/// a digit: anyone who guesses it reads what is pushed.
pub const TOPIC_MIN: usize = 16;

/// The longest topic ntfy takes.
pub const TOPIC_MAX: usize = 64;

/// How long one push may take, `curl` and the server together.
pub const WAIT: Duration = Duration::from_secs(10);

/// Where pushes go: a server, a topic on it, and the token it wants, if
/// any. Its `Debug` never shows the token.
#[derive(Clone, PartialEq, Eq)]
pub struct Push {
    pub server: String,
    pub topic: String,
    token: Option<String>,
}

impl std::fmt::Debug for Push {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Push")
            .field("server", &self.server)
            .field("topic", &self.topic)
            .field("token", &self.token.as_ref().map(|_| "[***]"))
            .finish()
    }
}

/// Why a configuration is refused, in the words `:notify push` says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Why {
    /// No home to find the file in, or no file there.
    NoFile(String),
    /// Not readable, too big, or not text.
    Unreadable(String),
    /// Line N is not `name = value`, or names nothing known, or twice.
    Line(usize),
    /// No topic.
    NoTopic,
    /// The server is not `https://host[:port][/path]` (`http` only on this
    /// machine), or has a user, a query or a fragment.
    Server(String),
    /// The topic has other characters than letters, digits, `-` and `_`,
    /// or is longer than [`TOPIC_MAX`].
    Topic,
    /// On the public server, shorter than [`TOPIC_MIN`] or without a digit.
    Guessable,
    /// The token file: not a plain file of the person's, readable by others,
    /// too big, empty, or with characters a token does not have.
    Token(String),
}

/// The configuration file, `$XDG_CONFIG_HOME/imece/push`.
pub fn file() -> Option<PathBuf> {
    crate::keys::config_file(FILE)
}

/// Reads the configuration at `path` (none: no home), and the token file
/// it names.
pub fn read(path: Option<&Path>) -> Result<Push, Why> {
    let path = path.ok_or_else(|| Why::NoFile("~/.config/imece/push".into()))?;
    let shown = path.display().to_string();
    let text = small_file(path).map_err(|e| match e {
        Small::Missing => Why::NoFile(shown.clone()),
        Small::Bad(why) => Why::Unreadable(why),
    })?;
    let home = std::env::var_os("HOME").map(PathBuf::from);
    parse(&text, |p| token(&tilde(p, home.as_deref())))
}

/// `p` with a leading `~/` as `home`.
fn tilde(p: &str, home: Option<&Path>) -> PathBuf {
    match (p.strip_prefix("~/"), home) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(p),
    }
}

/// The configuration in `text`: `name = value` lines (`server`, `topic`,
/// `token`, the last a file's path), `#` comments and blank lines; the
/// token read by `token`. Everything checked; the first fault refuses it
/// whole.
pub fn parse(text: &str, token: impl Fn(&str) -> Result<String, Why>) -> Result<Push, Why> {
    let mut server = None;
    let mut topic = None;
    let mut file = None;
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (name, value) = line.split_once('=').ok_or(Why::Line(n + 1))?;
        let slot = match name.trim() {
            "server" => &mut server,
            "topic" => &mut topic,
            "token" => &mut file,
            _ => return Err(Why::Line(n + 1)),
        };
        let value = value.trim();
        if slot.is_some() || value.is_empty() {
            return Err(Why::Line(n + 1));
        }
        *slot = Some(value.to_string());
    }
    let server = server.unwrap_or_else(|| PUBLIC.to_string());
    let server = server_ok(&server)?;
    let topic = topic.ok_or(Why::NoTopic)?;
    topic_ok(&server, &topic)?;
    let token = file.map(|f| token(&f)).transpose()?;
    Ok(Push {
        server,
        topic,
        token,
    })
}

/// The server as pushes go to it (no `/` at its end), or why not.
fn server_ok(s: &str) -> Result<String, Why> {
    let bad = || Why::Server(crate::i18n::clean(s));
    let (secure, rest) = match (s.strip_prefix("https://"), s.strip_prefix("http://")) {
        (Some(rest), _) => (true, rest),
        (None, Some(rest)) => (false, rest),
        (None, None) => return Err(bad()),
    };
    let ok = |c: char| c.is_ascii_alphanumeric() || "-._~:/[]%".contains(c);
    if !rest.chars().all(ok) {
        return Err(bad());
    }
    let host_end = rest.find('/').unwrap_or(rest.len());
    let host = &rest[..host_end];
    let name = match host.strip_prefix('[') {
        Some(_) => host.split_inclusive(']').next().unwrap_or(host),
        None => host.split(':').next().unwrap_or(host),
    };
    if !name.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '[') {
        return Err(bad());
    }
    let loopback = matches!(name, "localhost" | "127.0.0.1" | "[::1]");
    if !secure && !loopback {
        return Err(bad());
    }
    Ok(s.trim_end_matches('/').to_string())
}

/// Whether `topic` is one to push to on `server`.
fn topic_ok(server: &str, topic: &str) -> Result<(), Why> {
    let ok = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    if topic.len() > TOPIC_MAX || !topic.chars().all(ok) {
        return Err(Why::Topic);
    }
    let public = server == PUBLIC;
    let guessable = topic.len() < TOPIC_MIN || !topic.chars().any(|c| c.is_ascii_digit());
    if public && guessable {
        return Err(Why::Guessable);
    }
    Ok(())
}

/// What reading a small file came to.
#[derive(Debug, PartialEq, Eq)]
enum Small {
    Missing,
    Bad(String),
}

/// A small text file read whole: opened without following a link at its
/// end and without waiting (a pipe would wait for a writer), looked at
/// as opened, at most [`FILE_MAX`] bytes.
fn small_file(path: &Path) -> Result<String, Small> {
    let (text, _) = small_file_meta(path)?;
    Ok(text)
}

fn small_file_meta(path: &Path) -> Result<(String, std::fs::Metadata), Small> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Small::Missing,
            _ => Small::Bad(e.to_string()),
        })?;
    let meta = file.metadata().map_err(|e| Small::Bad(e.to_string()))?;
    if !meta.is_file() {
        return Err(Small::Bad("not a plain file".into()));
    }
    if meta.len() > FILE_MAX {
        return Err(Small::Bad(format!("bigger than {FILE_MAX} bytes")));
    }
    let mut bytes = Vec::new();
    file.take(FILE_MAX)
        .read_to_end(&mut bytes)
        .map_err(|e| Small::Bad(e.to_string()))?;
    let text = String::from_utf8(bytes).map_err(|_| Small::Bad("not text".into()))?;
    Ok((text, meta))
}

/// The token in the file at `path`: the person's own plain file, which
/// no one else may read or write (`0600` or less), one word of visible
/// ASCII without quotes or backslashes.
pub fn token(path: &Path) -> Result<String, Why> {
    use std::os::unix::fs::MetadataExt;
    let shown = path.display().to_string();
    let (text, meta) = small_file_meta(path).map_err(|e| match e {
        Small::Missing => Why::Token(format!("{shown}: no such file")),
        Small::Bad(why) => Why::Token(format!("{shown}: {why}")),
    })?;
    // SAFETY: getuid has no preconditions and cannot fail.
    let me = unsafe { libc::getuid() };
    if meta.uid() != me {
        return Err(Why::Token(format!("{shown}: not your file")));
    }
    if meta.mode() & 0o077 != 0 {
        return Err(Why::Token(format!(
            "{shown}: others may read it (chmod 600)"
        )));
    }
    let word = text.trim();
    let fits = |c: char| c.is_ascii_graphic() && c != '"' && c != '\\';
    if word.is_empty() || !word.chars().all(fits) {
        return Err(Why::Token(format!("{shown}: not one token")));
    }
    Ok(word.to_string())
}

/// `s` as a string in curl's config file: in quotes, `\` and `"` escaped.
/// `s` has no control character (the words are cleaned, the rest
/// checked).
fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// What `curl` reads on its stdin for one push of `words`: where, the
/// words, a title, the priority (`high` when `loud`), the token as a
/// header. Nothing of it is on the command line.
pub fn config(p: &Push, words: &str, loud: bool) -> String {
    let words: String = words.chars().filter(|c| !c.is_control()).collect();
    let mut lines = vec![
        format!("url = {}", quoted(&format!("{}/{}", p.server, p.topic))),
        format!("data-raw = {}", quoted(&words)),
        format!("header = {}", quoted("Title: imece")),
        format!(
            "header = {}",
            quoted(if loud {
                "Priority: high"
            } else {
                "Priority: default"
            })
        ),
    ];
    if let Some(token) = &p.token {
        lines.push(format!(
            "header = {}",
            quoted(&format!("Authorization: Bearer {token}"))
        ));
    }
    lines.join("\n") + "\n"
}

/// The command a push runs: `curl` with nothing of the person's own
/// configuration (`-q`), the config on stdin, no redirect followed, only
/// the server's own protocol, at most [`WAIT`], quiet unless it fails.
pub fn command(p: &Push) -> Command {
    let proto = if p.server.starts_with("https://") {
        "=https"
    } else {
        "=http"
    };
    let mut cmd = Command::new("curl");
    cmd.args([
        "-q",
        "--config",
        "-",
        "--silent",
        "--show-error",
        "--fail",
        "--max-redirs",
        "0",
        "--proto",
        proto,
        "--max-time",
        &WAIT.as_secs().to_string(),
        "--output",
        "/dev/null",
    ]);
    cmd
}

/// Sends one push and waits for it: none when it went, else why not (the
/// first line `curl` said, cleaned, which never has the token: curl does
/// not echo headers).
pub fn send(p: &Push, words: &str, loud: bool) -> Result<(), String> {
    let input = config(p, words, loud).into_bytes();
    match imece_agent::child::ask_fed(command(p), Some(input), ask_wait()) {
        None => Err("curl did not start or did not end in time".into()),
        Some((status, _)) if status.success() => Ok(()),
        Some((status, said)) => Err(failed(status.code(), &said)),
    }
}

/// How long imece waits for `curl`: its own [`WAIT`] and a little more,
/// so curl says why it gave up rather than being killed.
fn ask_wait() -> Duration {
    WAIT + Duration::from_secs(2)
}

/// Why a push failed, from curl's exit code (none: a signal) and what it
/// said: its first line, else the code; cleaned.
fn failed(code: Option<i32>, said: &str) -> String {
    let first = said.lines().next().unwrap_or_default().trim();
    let why = match (first, code) {
        ("", Some(code)) => format!("curl exit {code}"),
        ("", None) => "curl was stopped".to_string(),
        (said, _) => said.to_string(),
    };
    crate::i18n::clean(&why)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOPIC: &str = "imece-alerts-7f3k9q2m";

    fn no_token(_: &str) -> Result<String, Why> {
        panic!("no token named")
    }

    #[test]
    fn a_configuration_is_lines_of_names_and_values_with_the_public_server_by_default() {
        let p = parse(&format!("# mine\n\n  topic = {TOPIC}  \n"), no_token).unwrap();
        assert_eq!((p.server.as_str(), p.topic.as_str()), (PUBLIC, TOPIC));
        assert_eq!(p.token, None);
        let text = "server = https://push.example.org/\ntopic = ab\ntoken = ~/t\n";
        let p = parse(text, |f| {
            assert_eq!(f, "~/t");
            Ok("tk_1".into())
        })
        .unwrap();
        assert_eq!(p.server, "https://push.example.org", "its last / off");
        assert_eq!(p.token.as_deref(), Some("tk_1"));
        // The token never shows in what is printed of it.
        assert!(!format!("{p:?}").contains("tk_1"), "{p:?}");
        assert!(format!("{p:?}").contains("[***]"));
        for (text, why) in [
            ("topic", Why::Line(1)),
            ("\ncolour = red", Why::Line(2)),
            (
                &format!("topic = {TOPIC}\ntopic = {TOPIC}") as &str,
                Why::Line(2),
            ),
            ("topic =", Why::Line(1)),
            ("", Why::NoTopic),
            ("# only\nserver = https://a.b", Why::NoTopic),
        ] {
            assert_eq!(parse(text, no_token), Err(why), "{text}");
        }
        let failed = parse(&format!("topic = {TOPIC}\ntoken = /x"), |_| {
            Err(Why::Token("x".into()))
        });
        assert_eq!(failed, Err(Why::Token("x".into())));
    }

    #[test]
    fn the_server_is_https_or_http_on_this_machine_without_a_user_or_a_query() {
        for ok in [
            "https://ntfy.sh",
            "https://push.example.org:8443/ntfy",
            "http://localhost:8080",
            "http://127.0.0.1",
            "http://[::1]:80/",
            "https://[2001:db8::1]",
        ] {
            assert!(server_ok(ok).is_ok(), "{ok}");
        }
        for bad in [
            "http://push.example.org",
            "http://localhost.evil.com",
            "ftp://a.b",
            "ntfy.sh",
            "https://user@a.b",
            "https://a.b/?x=1",
            "https://a.b/#x",
            "https://",
            "https://-a.b",
            "https://a.b/\"x",
            "https://a .b",
        ] {
            assert_eq!(server_ok(bad), Err(Why::Server(bad.to_string())), "{bad}");
        }
    }

    #[test]
    fn a_topic_on_the_public_server_must_not_be_one_to_guess() {
        assert_eq!(topic_ok(PUBLIC, TOPIC), Ok(()));
        let fifteen = "abcdefghijklmn1";
        assert_eq!(fifteen.len(), TOPIC_MIN - 1);
        assert_eq!(topic_ok(PUBLIC, fifteen), Err(Why::Guessable));
        assert_eq!(topic_ok(PUBLIC, "abcdefghijklmno1"), Ok(()));
        assert_eq!(
            topic_ok(PUBLIC, "abcdefghijklmnopqrstu"),
            Err(Why::Guessable),
            "no digit"
        );
        // A server of one's own takes any well-formed topic.
        assert_eq!(topic_ok("https://a.b", "x"), Ok(()));
        for bad in ["a/b", "a b", "ä1234567890123456", "a?b"] {
            assert_eq!(topic_ok("https://a.b", bad), Err(Why::Topic), "{bad}");
        }
        let longest = "a".repeat(TOPIC_MAX);
        assert_eq!(topic_ok("https://a.b", &longest), Ok(()));
        assert_eq!(
            topic_ok("https://a.b", &format!("{longest}a")),
            Err(Why::Topic)
        );
        assert_eq!((TOPIC_MIN, TOPIC_MAX), (16, 64));
    }

    #[test]
    fn curl_reads_where_the_words_and_the_token_from_its_stdin() {
        let p = Push {
            server: "https://a.b".into(),
            topic: "t1".into(),
            token: Some("tk_9".into()),
        };
        assert_eq!(
            config(&p, "imece · claude-1 \"asks\" \\ you\x07", true),
            "url = \"https://a.b/t1\"\n\
             data-raw = \"imece · claude-1 \\\"asks\\\" \\\\ you\"\n\
             header = \"Title: imece\"\n\
             header = \"Priority: high\"\n\
             header = \"Authorization: Bearer tk_9\"\n"
        );
        let open = Push { token: None, ..p };
        let text = config(&open, "w", false);
        assert!(text.contains("header = \"Priority: default\"\n"), "{text}");
        assert!(!text.contains("Authorization"), "{text}");
        // Nothing of it on the command line.
        let args: Vec<String> = command(&open)
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args.join(" "),
            "-q --config - --silent --show-error --fail --max-redirs 0 --proto =https \
             --max-time 10 --output /dev/null"
        );
        let local = Push {
            server: "http://localhost:8080".into(),
            ..open
        };
        let args: Vec<String> = command(&local)
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.join(" ").contains("--proto =http "), "{args:?}");
        assert_eq!(command(&local).get_program(), "curl");
    }

    #[test]
    fn a_token_file_is_the_person_s_own_readable_by_no_one_else() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("imece-push-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let at = |name: &str, text: &str, mode: u32| {
            let p = dir.join(name);
            std::fs::write(&p, text).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
            p
        };
        let good = at("good", "  tk_abc123\n", 0o600);
        assert_eq!(token(&good), Ok("tk_abc123".into()));
        assert_eq!(token(&at("ro", "tk_1", 0o400)), Ok("tk_1".into()));
        for (name, text, mode, said) in [
            ("group", "tk_1", 0o640, "others may read it"),
            ("other", "tk_1", 0o604, "others may read it"),
            ("gw", "tk_1", 0o620, "others may read it"),
            ("empty", " \n", 0o600, "not one token"),
            ("two", "tk 1", 0o600, "not one token"),
            ("quote", "tk\"1", 0o600, "not one token"),
            ("slash", "tk\\1", 0o600, "not one token"),
        ] {
            match token(&at(name, text, mode)) {
                Err(Why::Token(why)) => assert!(why.contains(said), "{name}: {why}"),
                other => panic!("{name}: {other:?}"),
            }
        }
        let big = at("big", &"a".repeat(FILE_MAX as usize + 1), 0o600);
        assert!(matches!(token(&big), Err(Why::Token(w)) if w.contains("bigger")));
        let fits = at("fits", &"a".repeat(FILE_MAX as usize), 0o600);
        assert_eq!(token(&fits).map(|t| t.len()), Ok(FILE_MAX as usize));
        // A link is not followed, a directory is no file.
        let link = dir.join("link");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&good, &link).unwrap();
        assert!(matches!(token(&link), Err(Why::Token(_))));
        assert!(matches!(token(&dir), Err(Why::Token(w)) if w.contains("not a plain file")));
        assert!(
            matches!(token(&dir.join("none")), Err(Why::Token(w)) if w.ends_with("no such file"))
        );
        // The whole file, read through `read`.
        let conf = at(
            "push",
            &format!("topic = {TOPIC}\ntoken = {}\n", good.display()),
            0o644,
        );
        let p = read(Some(&conf)).unwrap();
        assert_eq!(p.token.as_deref(), Some("tk_abc123"));
        assert_eq!(read(None), Err(Why::NoFile("~/.config/imece/push".into())));
        let none = dir.join("missing");
        assert_eq!(
            read(Some(&none)),
            Err(Why::NoFile(none.display().to_string()))
        );
        assert!(matches!(read(Some(&dir)), Err(Why::Unreadable(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failure_says_curl_s_first_line_else_its_code_and_curl_gives_up_first() {
        assert_eq!(
            failed(Some(7), "curl: (7) refused\nmore\n"),
            "curl: (7) refused"
        );
        assert_eq!(failed(Some(22), "  \n"), "curl exit 22");
        assert_eq!(failed(None, ""), "curl was stopped");
        assert_eq!(failed(Some(6), "TOKEN=abc"), "[***]", "cleaned");
        assert_eq!(WAIT, Duration::from_secs(10));
        assert_eq!(ask_wait(), Duration::from_secs(12));
    }

    #[test]
    fn a_tilde_is_the_home() {
        let home = Path::new("/h");
        assert_eq!(tilde("~/t", Some(home)), PathBuf::from("/h/t"));
        assert_eq!(tilde("~/t", None), PathBuf::from("~/t"));
        assert_eq!(tilde("/a/~/t", Some(home)), PathBuf::from("/a/~/t"));
    }

    #[test]
    fn a_push_that_fails_says_why_and_one_that_goes_says_nothing() {
        // A server that is not there: curl fails at once, within its wait.
        let p = Push {
            server: "http://127.0.0.1:9".into(),
            topic: "t".into(),
            token: Some("tk_secret_1".into()),
        };
        let why = send(&p, "imece · x", false).unwrap_err();
        assert!(!why.is_empty());
        assert!(!why.contains("tk_secret_1"), "{why}");
    }
}
