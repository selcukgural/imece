//! The paths a card owns (record 114): git pathspecs given with an
//! `assign` or a `post`. A worker that writes outside its card's paths
//! strays, and is told so; two open cards of one project may not own the
//! same path unless one says it is `shared`. Pure: what a list of paths
//! means, nothing of where it comes from.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How many paths one card may own.
pub const MAX_PATHS: usize = 20;
/// How long one path may be, in characters.
pub const MAX_PATH: usize = 200;
/// The word after a path that makes it shared: owned by nobody, open to
/// every card that names it.
const SHARED: &str = "shared";

/// One path of a card: a file, a directory (everything under it) or a
/// pattern (`*` stands for anything, `/` included, as git's does).
/// `shared` says the card does not own it: another card may name it too.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Owned {
    pub path: String,
    #[serde(default)]
    pub shared: bool,
}

impl Owned {
    /// A path the card owns.
    pub fn new(path: &str) -> Owned {
        Owned {
            path: path.to_string(),
            shared: false,
        }
    }

    /// A path the card names but shares.
    #[cfg(test)]
    pub fn shared(path: &str) -> Owned {
        Owned {
            path: path.to_string(),
            shared: true,
        }
    }

    /// The path as it is listed: `src/a.rs`, or `docs (shared)`.
    pub fn words(&self) -> String {
        if self.shared {
            format!("{} ({SHARED})", self.path)
        } else {
            self.path.clone()
        }
    }
}

/// The paths `v` (a tool call's `paths`) lists: nothing when it is not
/// given, else an array of at most [`MAX_PATHS`] strings, each a path
/// relative to the repository of at most [`MAX_PATH`] characters, no
/// control character, no `..`; one ending in ` shared` is shared. Each
/// path is kept as git reads it ([`normal`]: no `.` part, no empty part,
/// so `././x/` is `x`). Refused whole with why.
pub fn parse(v: &Value) -> Result<Vec<Owned>, String> {
    if v.is_null() {
        return Ok(Vec::new());
    }
    let Some(list) = v.as_array() else {
        return Err("paths must be an array of strings".into());
    };
    if list.len() > MAX_PATHS {
        return Err(format!("a card owns at most {MAX_PATHS} paths"));
    }
    let mut out: Vec<Owned> = Vec::new();
    for item in list {
        let Some(text) = item.as_str() else {
            return Err("paths must be an array of strings".into());
        };
        let text = text.trim();
        let (path, shared) = match text.strip_suffix(SHARED) {
            Some(before) if before.ends_with(char::is_whitespace) => (before.trim_end(), true),
            _ => (text, false),
        };
        let given = path;
        let path = normal(given);
        if path.is_empty() {
            return Err("a path is empty".into());
        }
        if path.chars().count() > MAX_PATH {
            return Err(format!("a path is longer than {MAX_PATH} characters"));
        }
        if path.chars().any(char::is_control) {
            return Err(format!("{path:?} has a control character"));
        }
        if given.starts_with('/') {
            return Err(format!(
                "paths are relative to the repository; {given:?} starts with /"
            ));
        }
        if path.split('/').any(|part| part == "..") {
            return Err(format!("{path:?} goes outside the repository (..)"));
        }
        if out.iter().any(|o| o.path == path) {
            return Err(format!("paths lists {path:?} twice"));
        }
        out.push(Owned { path, shared });
    }
    Ok(out)
}

/// `path` with its `.` parts and empty ones left out (`./a//b/.` is
/// `a/b`), as git reads a path: one file has one name, so a card that owns
/// `././x` owns the `x` an agent writes. Nothing else changes: `..` and a
/// leading `/` are refused by [`parse`], not read away.
fn normal(path: &str) -> String {
    (path.split('/'))
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
}

/// Whether `pattern` names `target`: the same path, a directory it is
/// under, or a pattern it matches (`*` for anything), whole or as a
/// directory it is under. One pass over the target answers for every
/// directory it is under at once: a written path is the agent's, and one
/// of thousands of directories costs no more than its length times the
/// pattern's.
fn names(pattern: &str, target: &str) -> bool {
    // A card saved before paths were kept [`normal`] may own `./x`.
    let p: Vec<char> = normal(pattern).chars().collect();
    let t: Vec<char> = normal(target).chars().collect();
    let matched = prefixes(&p, &t);
    matched[t.len()] || (t.iter().zip(&matched)).any(|(&c, &m)| c == '/' && m)
}

/// For every `j`, whether `p` matches the whole of `t[..j]`, `*` standing
/// for any run of characters. Pattern character by pattern character over
/// every length of `t` (no backtracking: a pattern of many stars, which an
/// agent may send, costs no more than its length times the target's).
fn prefixes(p: &[char], t: &[char]) -> Vec<bool> {
    // Whether the pattern so far matches the first `j` characters of `t`.
    let mut sofar = vec![false; t.len() + 1];
    sofar[0] = true;
    let mut next = vec![false; t.len() + 1];
    for &c in p {
        if c == '*' {
            // A star takes any run: the pattern with it matches `t[..j]`
            // when the pattern before it matched some `t[..k]`, k ≤ j.
            let mut reached = false;
            for (j, &was) in sofar.iter().enumerate() {
                reached = reached || was;
                next[j] = reached;
            }
        } else {
            next[0] = false;
            for j in 1..=t.len() {
                next[j] = sofar[j - 1] && t[j - 1] == c;
            }
        }
        std::mem::swap(&mut sofar, &mut next);
    }
    sofar
}

/// Whether writing `target` strays from `paths`: the card owns paths
/// (else nothing is out of bounds) and none of them, shared ones
/// included, names it.
pub fn strays(paths: &[Owned], target: &str) -> bool {
    !paths.is_empty() && !paths.iter().any(|o| names(&o.path, target))
}

/// Where two cards' paths clash: an owned path of `a` and one of `b` that
/// name each other either way (`src/a.rs` and `src`, `src/*` and
/// `src/a.rs`). Shared paths clash with nothing. The first pair, in
/// `a`'s order.
pub fn overlap(a: &[Owned], b: &[Owned]) -> Option<(String, String)> {
    a.iter().filter(|x| !x.shared).find_map(|x| {
        b.iter()
            .filter(|y| !y.shared)
            .find(|y| names(&x.path, &y.path) || names(&y.path, &x.path))
            .map(|y| (x.path.clone(), y.path.clone()))
    })
}

/// The paths listed for an agent or the person: `src/a.rs, docs (shared)`.
pub fn listed(paths: &[Owned]) -> String {
    paths
        .iter()
        .map(Owned::words)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn owned(paths: &[&str]) -> Vec<Owned> {
        parse(&json!(paths)).unwrap()
    }

    #[test]
    fn paths_are_strings_relative_to_the_repository_a_shared_one_said_so() {
        assert_eq!(parse(&Value::Null), Ok(Vec::new()));
        assert_eq!(parse(&json!([])), Ok(Vec::new()));
        assert_eq!(
            parse(&json!([
                "src/a.rs",
                " docs/ shared ",
                "tests/*_test.rs",
                "./x"
            ])),
            Ok(vec![
                Owned::new("src/a.rs"),
                Owned::shared("docs"),
                Owned::new("tests/*_test.rs"),
                Owned::new("x"),
            ])
        );
        // The word alone is a path called shared, not a shared nothing.
        assert_eq!(parse(&json!(["shared"])), Ok(vec![Owned::new("shared")]));
        assert_eq!(
            parse(&json!(["unshared"])),
            Ok(vec![Owned::new("unshared")])
        );
        assert_eq!(parse(&json!(["a/b/"])), Ok(vec![Owned::new("a/b")]));
        // Every `.` part and empty one goes, wherever it is.
        assert_eq!(
            parse(&json!(["././x", "a/./b//c/.", ".//d"])),
            Ok(vec![Owned::new("x"), Owned::new("a/b/c"), Owned::new("d")])
        );
        assert_eq!(
            parse(&json!(["a/b/ shared"])),
            Ok(vec![Owned::shared("a/b")])
        );
    }

    #[test]
    fn bad_paths_are_refused_whole_with_why() {
        let refused = |v: Value| parse(&v).unwrap_err();
        assert_eq!(refused(json!("src")), "paths must be an array of strings");
        assert_eq!(refused(json!(7)), "paths must be an array of strings");
        assert_eq!(
            refused(json!(["a", 7])),
            "paths must be an array of strings"
        );
        assert_eq!(refused(json!(["a", ""])), "a path is empty");
        assert_eq!(refused(json!(["./"])), "a path is empty");
        assert_eq!(refused(json!(["/"])), "a path is empty");
        assert_eq!(refused(json!(["."])), "a path is empty");
        assert_eq!(refused(json!(["././"])), "a path is empty");
        assert_eq!(
            refused(json!(["//etc"])),
            "paths are relative to the repository; \"//etc\" starts with /"
        );
        assert_eq!(refused(json!(["x", "./x/"])), "paths lists \"x\" twice");
        assert_eq!(
            refused(json!(["/etc/passwd"])),
            "paths are relative to the repository; \"/etc/passwd\" starts with /"
        );
        assert_eq!(
            refused(json!(["src/../../x"])),
            "\"src/../../x\" goes outside the repository (..)"
        );
        assert_eq!(
            refused(json!([".."])),
            "\"..\" goes outside the repository (..)"
        );
        assert_eq!(
            refused(json!(["a\tb"])),
            "\"a\\tb\" has a control character"
        );
        assert_eq!(refused(json!(["a", "b", "a"])), "paths lists \"a\" twice");
        assert_eq!(
            refused(json!(["a/", "a shared"])),
            "paths lists \"a\" twice"
        );
        // A name with dots in it is a name.
        assert!(parse(&json!(["a..b/...c"])).is_ok());
        let twenty: Vec<String> = (0..20).map(|i| format!("f{i}")).collect();
        assert_eq!(parse(&json!(twenty)).unwrap().len(), 20);
        let more: Vec<String> = (0..21).map(|i| format!("f{i}")).collect();
        assert_eq!(refused(json!(more)), "a card owns at most 20 paths");
        let long = "x".repeat(200);
        assert_eq!(parse(&json!([long])).unwrap()[0].path.len(), 200);
        let longer = "é".repeat(201);
        assert_eq!(
            refused(json!([longer])),
            "a path is longer than 200 characters"
        );
    }

    #[test]
    fn a_path_names_itself_what_is_under_it_and_what_its_star_matches() {
        assert!(names("src/a.rs", "src/a.rs"));
        assert!(names("src/a.rs", "./src/a.rs"));
        assert!(names("src/a.rs", "././src//./a.rs"));
        assert!(names("src", "src/./deep//a.rs"));
        assert!(!names("src/a.rs", "src/a.rs.bak"));
        assert!(!names("src/a.rs", "src/a"));
        assert!(!names("src/a.rs", "lib/src/a.rs"));
        assert!(names("src", "src/a.rs"));
        assert!(names("src", "src/deep/a.rs"));
        assert!(!names("src", "srcs/a.rs"));
        assert!(!names("src/a.rs", "src"));
        assert!(names("*.rs", "a.rs"));
        assert!(names("*.rs", "src/deep/a.rs"), "a star crosses /");
        assert!(names("*.rs", "a.rs/x"), "a.rs may be a directory");
        assert!(
            !names("*.rs", "a.rsx"),
            "the match is whole, or a directory"
        );
        assert!(names("src/*", "src/a.rs"));
        assert!(names("src/*", "src/a/b.rs"));
        assert!(!names("src/*", "src"));
        assert!(
            names("src/*/tests", "src/x/tests/a.rs"),
            "a pattern as a directory"
        );
        assert!(names("a*c", "abc"));
        assert!(names("a*c", "ac"));
        assert!(!names("a*c", "ab"));
        assert!(names("a**c", "abbc"));
        assert!(names("*", "anything/at/all"));
        assert!(names("**", "x"));
        assert!(!names("", "x"));
        assert!(names("", ""));
        assert!(names("x*", "x"));
        assert!(!names("x*y", "x"));
        assert!(names("*a*b", "xaxb"));
        assert!(!names("*a*b", "xbxa"));
        // Many stars cost nothing much.
        let stars = "*".repeat(MAX_PATH);
        let long = "ab".repeat(100);
        assert!(names(&stars, &long));
        assert!(!names(&format!("{stars}c"), &long));
        // Nor does a written path of many directories: each one it is
        // under is answered by the same pass (looked at one by one, this
        // took hours).
        let deep = "a/".repeat(50_000);
        let pattern = format!("{}x", "a*".repeat(MAX_PATH / 2 - 1));
        assert!(!names(&pattern, &deep));
        assert!(names(&format!("{}a", "a/".repeat(9)), &deep));
        assert!(names("a/a/*/a", &deep));
    }

    #[test]
    fn a_write_strays_when_the_card_owns_paths_and_none_names_it() {
        assert!(!strays(&[], "anything.rs"), "no paths, no bounds");
        let paths = owned(&["src/a.rs", "docs shared"]);
        assert!(!strays(&paths, "src/a.rs"));
        assert!(
            !strays(&paths, "docs/guide.md"),
            "a shared path is open to it"
        );
        assert!(strays(&paths, "src/b.rs"));
        assert!(strays(&paths, "Cargo.toml"));
        // A path the card owns, however either side writes it.
        let dotted = owned(&["././x", "a/./b"]);
        for own in ["x", "./x", "././x", "a/b", "a//b/c", "./a/./b/c"] {
            assert!(!strays(&dotted, own), "{own}");
        }
        assert!(strays(&dotted, "y"));
        // A card saved before kept `./x` as it was given.
        assert!(!strays(&[Owned::new("./x")], "x"));
    }

    #[test]
    fn two_cards_clash_on_an_owned_path_either_names_and_not_on_a_shared_one() {
        let a = owned(&["src/a.rs", "src/b.rs", "CHANGELOG.md shared"]);
        assert_eq!(overlap(&a, &owned(&["src/c.rs"])), None);
        assert_eq!(
            overlap(&a, &owned(&["src/b.rs"])),
            Some(("src/b.rs".into(), "src/b.rs".into()))
        );
        assert_eq!(
            overlap(&a, &owned(&["src"])),
            Some(("src/a.rs".into(), "src".into())),
            "the first of a's, in a's order"
        );
        assert_eq!(
            overlap(&owned(&["src"]), &a),
            Some(("src".into(), "src/a.rs".into())),
            "either way round"
        );
        assert_eq!(
            overlap(&a, &owned(&["*.rs"])),
            Some(("src/a.rs".into(), "*.rs".into()))
        );
        assert_eq!(
            overlap(&a, &owned(&["CHANGELOG.md"])),
            None,
            "a shared path clashes with nothing"
        );
        assert_eq!(overlap(&a, &owned(&["CHANGELOG.md shared"])), None);
        assert_eq!(overlap(&owned(&["src shared"]), &a), None);
        assert_eq!(overlap(&a, &[]), None);
        assert_eq!(overlap(&[], &a), None);
    }

    #[test]
    fn paths_are_listed_with_shared_ones_said_so() {
        let paths = owned(&["src/a.rs", "docs/ shared"]);
        assert_eq!(listed(&paths), "src/a.rs, docs (shared)");
        assert_eq!(listed(&[]), "");
        assert_eq!(Owned::new("x").words(), "x");
        assert_eq!(Owned::shared("x").words(), "x (shared)");
    }

    #[test]
    fn a_path_is_kept_as_json_a_plain_one_without_the_flag() {
        let paths = owned(&["src/a.rs", "docs shared"]);
        let json = serde_json::to_string(&paths).unwrap();
        assert_eq!(
            json,
            r#"[{"path":"src/a.rs","shared":false},{"path":"docs","shared":true}]"#
        );
        let back: Vec<Owned> = serde_json::from_str(r#"[{"path":"x"}]"#).unwrap();
        assert_eq!(back, vec![Owned::new("x")]);
    }
}
