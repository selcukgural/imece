//! `paths`: the paths a card owns, as the leader names them on an `assign`,
//! a `post` or a plan (`paths::parse`), and whether a write strays from
//! them (`paths::strays`). The leader is an agent: the list is its JSON.
//!
//! Holds: what is taken is at most `MAX_PATHS` paths, each non-empty, at
//! most `MAX_PATH` characters, with no control character, relative, never
//! through `..`, none twice; a write to a path the card owns never strays.

use crate::owned::{MAX_PATH, MAX_PATHS, parse, strays};
use serde_json::Value;

pub fn check(data: &[u8]) {
    let Some((split, rest)) = crate::choice(data) else {
        return;
    };
    // The first byte says where the list ends and a written path begins.
    let at = rest.len().min(usize::from(split) * 4);
    let (list, written) = rest.split_at(at);
    let Ok(v) = serde_json::from_slice::<Value>(list) else {
        return;
    };
    let Ok(paths) = parse(&v) else {
        return;
    };
    assert!(paths.len() <= MAX_PATHS);
    for (i, o) in paths.iter().enumerate() {
        let p = &o.path;
        assert!(!p.is_empty());
        assert!(p.chars().count() <= MAX_PATH, "{p:?}");
        assert!(!p.chars().any(char::is_control), "{p:?}");
        assert!(!p.starts_with('/'), "{p:?}");
        assert!(!p.split('/').any(|s| s == ".."), "{p:?}");
        assert!(!paths[..i].iter().any(|q| q.path == *p), "{p:?} twice");
        // A path the card owns, as it is written; `.` parts are git's to
        // read, not this check's.
        if !p.split('/').any(|s| s == ".") {
            assert!(!strays(&paths, p), "{p:?} strays from its own card");
        }
    }
    strays(&paths, &crate::text(written));
}
