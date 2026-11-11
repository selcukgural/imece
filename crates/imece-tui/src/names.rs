//! The names in a checkout's code, for the word list (record 149): read on
//! the git thread from the files git lists, ranked by how often they stand
//! there, and handed to `pool::picks` as its `code`.
//!
//! What is read is capped: a file larger than [`FILE_MOST`] is left out
//! whole (it is generated or data more often than code), the files read
//! stop at [`READ_MOST`] in all, and [`KEPT`] names are kept. A file is
//! read only as a plain file inside the checkout, never through a link
//! and never waiting on a pipe; one with a NUL byte or that is not UTF-8
//! is binary and left out, so are lock files, minified or generated code
//! (`ui::diff::generated`) and prose (`PROSE`). Every text is cleaned
//! (`i18n::clean`) before its words are read, so a secret the code holds
//! is never offered.

use std::collections::HashMap;
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use crate::i18n::clean;
use crate::pool::words;
use crate::ui::generated;

/// A file larger than this is not read: it is generated or data more
/// often than code a person names.
pub const FILE_MOST: u64 = 1 << 20;

/// The bytes read in all from one checkout; the files after are not read.
pub const READ_MOST: u64 = 8 << 20;

/// The names kept, the most used first.
pub const KEPT: usize = 10_000;

/// Files of prose or data by their extension: their words are said, not
/// named in code.
const PROSE: &[&str] = &[
    "md", "markdown", "mdx", "txt", "rst", "adoc", "org", "csv", "tsv", "svg",
];

/// Whether `path` may hold code worth reading names from, by its name.
fn readable(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let prose = (name.rsplit_once('.'))
        .is_some_and(|(_, ext)| PROSE.iter().any(|p| p.eq_ignore_ascii_case(ext)));
    !prose && !generated(path)
}

/// The names in the code of the checkout at `repo`, from `files` (paths
/// in it, as git lists them, read in that order), the most used first,
/// at most [`KEPT`].
pub fn of(repo: &Path, files: &[String]) -> Vec<String> {
    let Ok(top) = repo.canonicalize() else {
        return Vec::new();
    };
    let mut counts: HashMap<String, u32> = HashMap::new();
    let mut left = READ_MOST;
    for path in files.iter().filter(|p| readable(p)) {
        if left == 0 {
            break;
        }
        let Some(text) = read(&top, path, left) else {
            continue;
        };
        left = left.saturating_sub(text.len() as u64);
        count(&text, &mut counts);
    }
    ranked(counts)
}

/// File `path` of the checkout at `top` (canonical) as text: none when it
/// is not a plain file inside it, larger than [`FILE_MOST`] or than
/// `room`, or binary.
fn read(top: &Path, path: &str, room: u64) -> Option<String> {
    // The real path, inside the checkout; opened without following a
    // link put there since, and without waiting on a pipe; looked at as
    // opened.
    let real = top.join(path).canonicalize().ok()?;
    if !real.starts_with(top) {
        return None;
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&real)
        .ok()?;
    let meta = file.metadata().ok()?;
    let most = FILE_MOST.min(room);
    if !meta.is_file() || meta.len() > most {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(most + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > most || bytes.contains(&0) {
        return None;
    }
    String::from_utf8(bytes).ok()
}

/// Counts the words of `text` (cleaned first) into `counts`.
fn count(text: &str, counts: &mut HashMap<String, u32>) {
    let cleaned = clean(text);
    for w in words(&cleaned) {
        match counts.get_mut(w) {
            Some(n) => *n = n.saturating_add(1),
            None => {
                counts.insert(w.to_string(), 1);
            }
        }
    }
}

/// The names counted, the most used first, then by name; at most
/// [`KEPT`].
fn ranked(counts: HashMap<String, u32>) -> Vec<String> {
    let mut all: Vec<(String, u32)> = counts.into_iter().collect();
    all.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    all.truncate(KEPT);
    all.into_iter().map(|(w, _)| w).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("imece-names-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn list(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn names_come_most_used_first_then_by_name() {
        let dir = scratch("ranked");
        fs::write(dir.join("a.rs"), "fn retry_policy() { retry_policy(); }\n").unwrap();
        fs::write(dir.join("b.py"), "class PaymentClient: retry_policy = None").unwrap();
        let got = of(&dir, &list(&["a.rs", "b.py"]));
        assert_eq!(got, ["retry_policy", "None", "PaymentClient", "class"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn prose_generated_binary_and_other_text_are_left_out() {
        let dir = scratch("left-out");
        fs::write(dir.join("notes.md"), "prosaic words").unwrap();
        fs::write(dir.join("Cargo.lock"), "lockname").unwrap();
        fs::write(dir.join("app.min.js"), "minified").unwrap();
        fs::write(dir.join("blob.bin"), b"binary\0name").unwrap();
        fs::write(dir.join("latin.c"), b"caf\xe9 latinword").unwrap();
        fs::write(dir.join("code.rs"), "fn kept_name() {}").unwrap();
        let files = list(&[
            "notes.md",
            "Cargo.lock",
            "app.min.js",
            "blob.bin",
            "latin.c",
            "code.rs",
            "missing.rs",
        ]);
        assert_eq!(of(&dir, &files), ["kept_name"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_too_large_is_not_read_and_reading_stops_at_the_cap() {
        let dir = scratch("caps");
        let big = format!("bigname {}", "x ".repeat(FILE_MOST as usize));
        fs::write(dir.join("big.rs"), big).unwrap();
        fs::write(dir.join("small.rs"), "smallname").unwrap();
        assert_eq!(of(&dir, &list(&["big.rs", "small.rs"])), ["smallname"]);
        // A file exactly at the cap is read.
        let at = format!("atcap{}", " ".repeat(FILE_MOST as usize - 5));
        fs::write(dir.join("at.rs"), at).unwrap();
        assert_eq!(of(&dir, &list(&["at.rs"])), ["atcap"]);
        // What is left to read is a file's cap too: past it, nothing more.
        assert!(read(&dir.canonicalize().unwrap(), "small.rs", 8).is_none());
        assert!(read(&dir.canonicalize().unwrap(), "small.rs", 9).is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_link_or_a_path_outside_the_checkout_is_never_read() {
        let root = scratch("links");
        let dir = root.join("repo");
        let outside = root.join("outside");
        fs::create_dir_all(&dir).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.rs"), "outsidename").unwrap();
        fs::write(root.join("secret.rs"), "parentname").unwrap();
        std::os::unix::fs::symlink(outside.join("secret.rs"), dir.join("link.rs")).unwrap();
        std::os::unix::fs::symlink(&outside, dir.join("sub")).unwrap();
        let files = list(&["link.rs", "sub/secret.rs", "../secret.rs"]);
        assert!(of(&dir, &files).is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_pipe_is_not_waited_on() {
        let dir = scratch("pipe");
        let fifo = std::ffi::CString::new(dir.join("p.rs").as_os_str().as_encoded_bytes()).unwrap();
        // SAFETY: a valid C path; mkfifo only makes the node.
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        fs::write(dir.join("after.rs"), "aftername").unwrap();
        assert_eq!(of(&dir, &list(&["p.rs", "after.rs"])), ["aftername"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_secret_in_the_code_is_never_a_name() {
        let dir = scratch("secret");
        let token = format!("ghp_{}", "a1B2".repeat(9));
        fs::write(dir.join("c.rs"), format!("let t = \"{token}\"; plainname")).unwrap();
        assert_eq!(of(&dir, &list(&["c.rs"])), ["plainname"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn at_most_the_kept_names() {
        let mut counts = HashMap::new();
        for i in 0..KEPT + 3 {
            counts.insert(format!("name{i:05}"), 1);
        }
        counts.insert("zzzz".to_string(), 2);
        let got = ranked(counts);
        assert_eq!(got.len(), KEPT);
        assert_eq!(got[0], "zzzz");
    }
}
