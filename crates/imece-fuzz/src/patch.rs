//! `patch`: a worker's patch cut to the hunks the person chose
//! (`worktree::subset`, what `x` then `a` applies). The patch is the
//! worker's: any bytes `git diff` may print.
//!
//! Holds: what comes out is lines of the patch, in their order; choosing no
//! hunk gives nothing; and the cut patch, with all its hunks chosen, is
//! itself again.

use imece_agent::worktree::subset;

pub fn check(data: &[u8]) {
    let Some((mask, rest)) = crate::choice(data) else {
        return;
    };
    let mut patch = crate::text(rest);
    // `git diff` ends its patch with a line end; a last line of its own
    // (`\ No newline at end of file`) says when the file does not.
    if !patch.is_empty() && !patch.ends_with('\n') {
        patch.push('\n');
    }
    let starts: Vec<usize> = hunks(&patch);
    // The hunks the first byte's bits choose, going round them.
    let chosen: Vec<usize> = (starts.iter().enumerate())
        .filter(|(n, _)| mask & (1 << (n % 8)) != 0)
        .map(|(_, &i)| i)
        .collect();
    let part = subset(&patch, &chosen);
    let mut lines = patch.lines();
    for l in part.lines() {
        assert!(
            lines.any(|p| p == l),
            "{l:?} is not a line of the patch, in order"
        );
    }
    assert_eq!(subset(&patch, &[]), "");
    assert_eq!(subset(&part, &hunks(&part)), part, "cut twice: {patch:?}");
}

/// The indexes of the lines a hunk starts at.
fn hunks(patch: &str) -> Vec<usize> {
    (patch.lines().enumerate())
        .filter(|(_, l)| l.starts_with("@@"))
        .map(|(i, _)| i)
        .collect()
}
