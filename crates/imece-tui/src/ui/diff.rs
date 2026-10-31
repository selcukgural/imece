//! A worker's change, whole: the patch as git wrote it, file by file,
//! for the person to read before they apply it to their checkout, under
//! the runs that ran on exactly it.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::*;
use crate::app::{App, DiffNote};

/// Line offsets of the file headers (`diff --git`) in `patch`, for
/// jumping from file to file.
pub fn file_starts(patch: &str) -> Vec<usize> {
    patch
        .lines()
        .enumerate()
        .filter(|(_, l)| l.starts_with("diff --git "))
        .map(|(i, _)| i)
        .collect()
}

/// The most lines of a hunk `>` quotes; the rest is counted after them.
pub const QUOTE_LINES: usize = 40;

/// The hunk the diff view has at `top` (its first row) in `patch`, as a
/// quote for a message: in a `diff` fence, under its file's `---` and
/// `+++` lines, at most [`QUOTE_LINES`] of it, every line cleaned (the
/// patch is the worker's text, and the line it goes into is drawn). The
/// hunk is the one `top` is in, else, on a file's header, the first one
/// after it; none past the last hunk or in a patch without one (binary
/// files only).
pub fn quote(patch: &str, top: usize) -> Option<String> {
    let lines: Vec<&str> = patch.lines().collect();
    let file = |i: &usize| lines[*i].starts_with("diff --git ");
    let hunk = |i: &usize| lines[*i].starts_with("@@");
    let top = top.min(lines.len().checked_sub(1)?);
    let before = (0..=top).rev().find(|i| hunk(i) || file(i));
    let start = match before.filter(hunk) {
        Some(at) => at,
        None => (top..lines.len()).find(hunk)?,
    };
    let end = (start + 1..lines.len())
        .find(|i| hunk(i) || file(i))
        .unwrap_or(lines.len());
    // The file's own `---` and `+++`: between its header and its first
    // hunk, where no line of a hunk can pass for one.
    let header = (0..start).rev().find(file).unwrap_or(0);
    let first = (header..start).find(hunk).unwrap_or(start);
    let names = (header..first)
        .map(|i| lines[i])
        .filter(|l| l.starts_with("--- ") || l.starts_with("+++ "));
    let body = &lines[start..end];
    let shown = body.iter().take(QUOTE_LINES).copied();
    let mut out = String::from("```diff\n");
    for l in names.chain(shown) {
        out.push_str(&clean(l));
        out.push('\n');
    }
    if body.len() > QUOTE_LINES {
        out.push_str(&format!("… +{}\n", body.len() - QUOTE_LINES));
    }
    out.push_str("```");
    Some(out)
}

/// Where line `i` of `patch` is, for a note on it: its file (the new
/// name) and its line there, or, for a removed line, its line in the old
/// file with a minus sign (`src/p.rs:12`, `src/p.rs:−7`). None for a line
/// that is no line of a hunk (a header, a `@@` line, past the end).
pub fn place(patch: &str, i: usize) -> Option<String> {
    let lines: Vec<&str> = patch.lines().collect();
    let line = *lines.get(i)?;
    // The hunk it is in: the nearest `@@` above, with no file header
    // between (a file's `---`/`+++` come before its first `@@`, so a
    // removed `-- x`, which reads `--- x`, is a line of a hunk).
    let at = (0..i)
        .rev()
        .find(|&n| lines[n].starts_with("@@") || lines[n].starts_with("diff --git "))?;
    if !lines[at].starts_with("@@") || !line.starts_with([' ', '+', '-']) {
        return None;
    }
    let (old, new) = starts(lines[at])?;
    let path = (0..at)
        .rev()
        .find(|&n| lines[n].starts_with("diff --git "))
        .and_then(|n| lines[n].rsplit_once(" b/"))
        .map(|(_, p)| p)?;
    let (mut o, mut n) = (old, new);
    // From the `@@` line on, which moves neither side.
    for l in &lines[at..i] {
        match l.chars().next() {
            Some('+') => n += 1,
            Some('-') => o += 1,
            Some(' ') => (o, n) = (o + 1, n + 1),
            _ => {}
        }
    }
    match line.starts_with('-') {
        true => Some(format!("{}:−{o}", clean(path))),
        false => Some(format!("{}:{n}", clean(path))),
    }
}

/// The old and new start lines a `@@ -a,b +c,d @@` header names.
fn starts(header: &str) -> Option<(u32, u32)> {
    let mut words = header.split(' ').skip(1);
    let start = |w: Option<&str>, sign: char| -> Option<u32> {
        let w = w?.strip_prefix(sign)?;
        w.split(',').next()?.parse().ok()
    };
    Some((start(words.next(), '-')?, start(words.next(), '+')?))
}

/// The message notes on a change's lines go to its worker in: what it is,
/// then each note under its place and the line it is on.
pub fn notes_message(patch: &str, notes: &[(usize, &str)]) -> String {
    let lines: Vec<&str> = patch.lines().collect();
    let mut out = String::from(
        "Notes on your change, line by line. It stays in your worktree, not applied: \
         change it as they say and send done again.",
    );
    for &(i, text) in notes {
        let place = place(patch, i).unwrap_or_default();
        let line = lines.get(i).copied().map(clean).unwrap_or_default();
        out.push_str(&format!(
            "\n\n{place}\n```diff\n{line}\n```\n{}",
            text.trim()
        ));
    }
    out
}

fn line_style(l: &str) -> Style {
    if l.starts_with("diff --git ") {
        bold(pal().strong)
    } else if l.starts_with("+++ ") || l.starts_with("--- ") || l.starts_with("index ") {
        dim()
    } else if l.starts_with("@@") {
        fg(pal().info)
    } else if l.starts_with('+') {
        fg(pal().ok)
    } else if l.starts_with('-') {
        fg(pal().bad)
    } else {
        Style::new()
    }
}

pub fn draw(f: &mut Frame, area: Rect, app: &App, id: AgentId) {
    let w = &app.world;
    let t = app.lang.text();
    let a = &w.agents[id];
    let Some(c) = w.change_of(id) else {
        let block = Block::bordered()
            .title(fill(t.diff_title, &[&a.name, "", "", "", ""]))
            .border_style(fg(vendor_color(a.vendor)));
        let inner = block.inner(area);
        f.render_widget(block, area);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                fill(t.diff_gone, &[&a.name]),
                dim(),
            ))),
            inner,
        );
        return;
    };
    let (added, removed) = c.counts();
    let block = Block::bordered()
        .title(fill(
            t.diff_title,
            &[
                &a.name,
                &c.files.len().to_string(),
                &added.to_string(),
                &removed.to_string(),
                &c.title,
            ],
        ))
        .border_style(fg(vendor_color(a.vendor)));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows = u16::try_from(c.runs.len().max(1)).unwrap_or(u16::MAX);
    let [runs, inner] =
        Layout::vertical([Constraint::Length(rows), Constraint::Min(0)]).areas(inner);
    f.render_widget(Paragraph::new(run_lines(app.lang, &c.runs)), runs);
    app.diff_page.set(inner.height);
    let height = usize::from(inner.height);
    let lines = patch_rows(&c.patch, app.diff_scroll, &app.notes_of(id), height);
    f.render_widget(Paragraph::new(lines), inner);
}

/// The rows of `patch` from line `top` on, `height` of them: each line
/// cleaned and coloured, the top one (the line at hand) marked, and a
/// note under the line it is on, every line of it after `✎`.
fn patch_rows(patch: &str, top: usize, notes: &[&DiffNote], height: usize) -> Vec<Line<'static>> {
    let mut rows = Vec::new();
    // Each line takes a row at least: `height` lines fill the rows.
    for (i, l) in patch.lines().enumerate().skip(top).take(height) {
        let mut row = Line::from(Span::styled(clean(l), line_style(l)));
        if i == top {
            row = row.style(changed());
        }
        rows.push(row);
        for n in notes.iter().filter(|n| n.line == i) {
            for (k, text) in n.text.lines().enumerate() {
                let lead = if k == 0 { "  ✎ " } else { "    " };
                rows.push(Line::from(vec![
                    Span::styled(lead, bold(pal().accent)),
                    Span::styled(clean(text), fg(pal().accent)),
                ]));
            }
        }
    }
    rows.truncate(height);
    rows
}

/// The runs on a change's tree, one a line, coloured by how they ended;
/// one line saying what it means when there is none.
fn run_lines(lang: Lang, runs: &[Ran]) -> Vec<Line<'static>> {
    if runs.is_empty() {
        let none = lang.text().ran_none_long.to_string();
        return vec![Line::from(Span::styled(none, fg(pal().warn)))];
    }
    runs.iter()
        .map(|r| {
            let colour = if r.ok() { pal().ok } else { pal().bad };
            Line::from(Span::styled(words::ran(lang, r), fg(colour)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH: &str = "\
diff --git a/a.rs b/a.rs
index 1..2 100644
--- a/a.rs
+++ b/a.rs
@@ -1,2 +1,2 @@
-old
--- z
+new
@@ -9,1 +9,2 @@
 ctx
--- y
diff --git a/b.bin b/b.bin
Binary files differ
diff --git a/c.rs b/c.rs
--- a/c.rs
+++ b/c.rs
@@ -1 +1 @@
-c
+C
";

    fn fenced(lines: &[&str]) -> String {
        format!("```diff\n{}\n```", lines.join("\n"))
    }

    #[test]
    fn the_quote_is_the_hunk_at_the_top_under_its_file_s_names() {
        let first = fenced(&[
            "--- a/a.rs",
            "+++ b/a.rs",
            "@@ -1,2 +1,2 @@",
            "-old",
            "--- z",
            "+new",
        ]);
        // On the file's header: its first hunk; in a hunk: that one.
        for top in [0, 3, 4, 7] {
            assert_eq!(quote(PATCH, top).as_deref(), Some(first.as_str()), "{top}");
        }
        // The second hunk keeps the file's names, not a line of the
        // first hunk that looks like one.
        let second = fenced(&[
            "--- a/a.rs",
            "+++ b/a.rs",
            "@@ -9,1 +9,2 @@",
            " ctx",
            "--- y",
        ]);
        for top in [8, 10] {
            assert_eq!(quote(PATCH, top).as_deref(), Some(second.as_str()), "{top}");
        }
        // A binary file has no hunk: the next file's; past the end, the
        // last one.
        let third = fenced(&["--- a/c.rs", "+++ b/c.rs", "@@ -1 +1 @@", "-c", "+C"]);
        for top in [11, 12, 13, 16, 18, 99] {
            assert_eq!(quote(PATCH, top).as_deref(), Some(third.as_str()), "{top}");
        }
        assert_eq!(quote("", 0), None);
        assert_eq!(quote("diff --git a/b b/b\nBinary files differ\n", 0), None);
        assert_eq!(quote("diff --git a/b b/b\nBinary files differ\n", 1), None);
    }

    #[test]
    fn a_line_s_place_is_its_new_line_or_its_old_one_when_removed() {
        let at = |i| place(PATCH, i);
        for header in [0, 1, 2, 3, 4, 8, 11, 12, 13, 14, 15, 16, 19] {
            assert_eq!(at(header), None, "{header}");
        }
        assert_eq!(at(5).as_deref(), Some("a.rs:−1"));
        assert_eq!(
            at(6).as_deref(),
            Some("a.rs:−2"),
            "a removed `-- z` is a line"
        );
        assert_eq!(at(7).as_deref(), Some("a.rs:1"));
        assert_eq!(at(9).as_deref(), Some("a.rs:9"));
        assert_eq!(at(10).as_deref(), Some("a.rs:−10"));
        assert_eq!(at(17).as_deref(), Some("c.rs:−1"));
        assert_eq!(at(18).as_deref(), Some("c.rs:1"));
        // Each kind of line moves its own side on.
        let p = "diff --git a/p b/q\n--- a/p\n+++ b/q\n@@ -3,4 +10,5 @@\n a\n-b\n+c\n+d\n e\n\\ No newline at end of file\n-f\n";
        let places: Vec<Option<String>> = (4..11).map(|i| place(p, i)).collect();
        assert_eq!(
            places,
            [
                Some("q:10".into()),
                Some("q:−4".into()),
                Some("q:11".into()),
                Some("q:12".into()),
                Some("q:13".into()),
                None,
                Some("q:−6".into()),
            ]
        );
        // A header it can not read: no place.
        for bad in [
            "@@ x @@",
            "@@ -1 x @@",
            "@@ -a +1 @@",
            "@@ 1 +1 @@",
            "@@ -1 1 @@",
        ] {
            let p = format!("diff --git a/p b/p\n{bad}\n+x\n");
            assert_eq!(place(&p, 2), None, "{bad}");
        }
        assert_eq!(starts("@@ -7,2 +9 @@"), Some((7, 9)));
    }

    #[test]
    fn notes_go_in_one_message_each_under_its_place_and_line() {
        let m = notes_message(PATCH, &[(7, " make it newer "), (17, "why?\nreally")]);
        assert_eq!(
            m,
            "Notes on your change, line by line. It stays in your worktree, not applied: change it as they say and send done again.\n\n\
             a.rs:1\n```diff\n+new\n```\nmake it newer\n\n\
             c.rs:−1\n```diff\n-c\n```\nwhy?\nreally"
        );
    }

    #[test]
    fn the_rows_mark_the_top_line_and_show_a_note_under_its_line() {
        let note = |line, text: &str| DiffNote {
            agent: 0,
            patch: 0,
            line,
            text: text.into(),
        };
        let (a, b) = (note(6, "one"), note(5, "two\nlines"));
        let rows = patch_rows(PATCH, 5, &[&a, &b], 8);
        let text: Vec<String> = rows.iter().map(|r| r.to_string()).collect();
        assert_eq!(
            text,
            [
                "-old",
                "  ✎ two",
                "    lines",
                "--- z",
                "  ✎ one",
                "+new",
                "@@ -9,1 +9,2 @@",
                " ctx"
            ]
        );
        assert_eq!(rows[0].style, changed(), "the line at hand");
        assert!(rows[1..].iter().all(|r| r.style == Style::new()));
        assert_eq!(rows[1].spans[0].style, bold(pal().accent));
        // Cut at the height, a note's rows too.
        assert_eq!(patch_rows(PATCH, 5, &[&a, &b], 2).len(), 2);
        assert_eq!(patch_rows(PATCH, 17, &[], 8).len(), 2, "the patch ends");
    }

    #[test]
    fn a_long_hunk_is_cut_and_counted_and_every_line_cleaned() {
        let hunk = |n: usize| {
            let mut p = String::from("@@ -1 +1 @@\n");
            for i in 1..n {
                p.push_str(&format!("+{i}\n"));
            }
            p
        };
        // Exactly the most: whole.
        let q = quote(&hunk(QUOTE_LINES), 0).unwrap();
        assert_eq!(q.lines().count(), QUOTE_LINES + 2);
        assert!(q.ends_with("+39\n```"), "{q}");
        // One more: cut, and the rest counted.
        let q = quote(&hunk(QUOTE_LINES + 6), 0).unwrap();
        let lines: Vec<&str> = q.lines().collect();
        assert_eq!(lines.len(), QUOTE_LINES + 3);
        assert_eq!(lines[QUOTE_LINES], "+39");
        assert_eq!(lines[QUOTE_LINES + 1], "… +6");
        // The worker's text: no escapes, no secrets.
        let q = quote("@@ -1 +1 @@\n+\u{1b}[2Jgone\n+TOKEN=abc123\n", 0).unwrap();
        assert_eq!(q, fenced(&["@@ -1 +1 @@", "+ [2Jgone", "[***]"]));
    }

    #[test]
    fn the_runs_on_a_change_head_its_diff_coloured_by_how_they_ended() {
        let ran = |code, timed_out| Ran {
            line: "cargo test".into(),
            code,
            timed_out,
        };
        let shown = |lang, runs: &[Ran]| -> Vec<(String, Style)> {
            run_lines(lang, runs)
                .iter()
                .map(|l| (l.spans[0].content.to_string(), l.spans[0].style))
                .collect()
        };
        let runs = [
            ran(Some(0), false),
            ran(Some(101), false),
            ran(None, true),
            ran(None, false),
        ];
        let (ok, bad) = (fg(pal().ok), fg(pal().bad));
        assert_eq!(
            shown(Lang::En, &runs),
            [
                ("✓ cargo test".to_string(), ok),
                ("✗ cargo test: exit 101".to_string(), bad),
                ("✗ cargo test: killed at the deadline".to_string(), bad),
                ("✗ cargo test: killed".to_string(), bad),
            ]
        );
        assert_eq!(
            shown(Lang::Tr, &runs[2..]),
            [
                ("✗ cargo test: süre dolunca durduruldu".to_string(), bad),
                ("✗ cargo test: durduruldu".to_string(), bad),
            ]
        );
        assert_eq!(
            shown(Lang::En, &[]),
            [(
                "no imece run on exactly this change: whether it builds and passes its \
                 tests is not known"
                    .to_string(),
                fg(pal().warn)
            )]
        );
    }

    #[test]
    fn file_headers_are_where_the_files_start() {
        let patch =
            "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\ndiff --git a/y b/y\n";
        assert_eq!(file_starts(patch), [0, 6]);
        assert_eq!(file_starts(""), Vec::<usize>::new());
    }

    #[test]
    fn added_and_removed_lines_have_their_own_colours_and_headers_stand_out() {
        assert_eq!(line_style("+new"), fg(pal().ok));
        assert_eq!(line_style("-old"), fg(pal().bad));
        assert_eq!(line_style("@@ -1 +1 @@"), fg(pal().info));
        assert_eq!(line_style("diff --git a/x b/x"), bold(pal().strong));
        assert_eq!(line_style("+++ b/x"), dim());
        assert_eq!(line_style("--- a/x"), dim());
        assert_eq!(line_style("index 1..2 100644"), dim());
        assert_eq!(line_style(" same"), Style::new());
    }
}
