//! A worker's change, whole: the patch as git wrote it, file by file,
//! for the person to read before they apply it to their checkout, under
//! the runs that ran on exactly it.

use imece_agent::worktree::CLASH_TRIES;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::*;
use crate::app::{App, Copying, DiffNote, Folds};
use crate::i18n::Text;
use crate::ui::words::card_ref;

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

/// A line git writes above a file's hunks that says nothing the file's
/// name and counts do not.
fn git_says(l: &str) -> bool {
    ["index ", "--- ", "+++ "].iter().any(|p| l.starts_with(p))
}

/// The file a `diff --git` line is about, as it is now.
pub(crate) fn head_path(l: &str) -> &str {
    l.rsplit_once(" b/").map_or(l, |(_, path)| path)
}

/// The lines of a change the view shows, its files folded as `folds`
/// says (by their number in the patch): a folded file shows only its
/// header, which says how many lines it hides.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shown {
    /// The patch's line numbers shown, in order.
    pub lines: Vec<usize>,
    /// A folded file's header line, and how many lines it hides.
    pub hidden: Vec<(usize, usize)>,
}

impl Shown {
    /// The lines of `patch` shown with `folds`; the lines before the
    /// first header, if any, always.
    pub fn of(patch: &str, folds: &Folds) -> Shown {
        let mut shown = Shown::default();
        let mut file: Option<usize> = None;
        // Above a file's first hunk: `--- ` there is git's, not a line
        // of the file that starts with `-- `.
        let mut head = false;
        for (i, l) in patch.lines().enumerate() {
            if l.starts_with("diff --git ") {
                let n = file.map_or(0, |n| n + 1);
                file = Some(n);
                head = true;
                shown.lines.push(i);
                if folds.folded(n) {
                    shown.hidden.push((i, 0));
                }
            } else if file.is_some_and(|n| folds.folded(n)) {
                if let Some(last) = shown.hidden.last_mut() {
                    last.1 += 1;
                }
            } else if !(head && git_says(l)) {
                head = head && !l.starts_with("@@");
                shown.lines.push(i);
            }
        }
        shown
    }

    /// The line shown `by` lines on from `at` (back when `on` is false),
    /// at the first or last shown when it goes past; `at` itself need
    /// not be shown (the shown line before it counts).
    pub fn step(&self, at: usize, by: usize, on: bool) -> usize {
        let Some(&last) = self.lines.last() else {
            return 0;
        };
        // The place of the last shown line at or before `at`.
        let here = self.lines.partition_point(|&i| i <= at).saturating_sub(1);
        let to = if on {
            here + by
        } else {
            here.saturating_sub(by)
        };
        self.lines.get(to).copied().unwrap_or(last)
    }

    /// How many lines a folded header at line `i` hides.
    fn hides(&self, i: usize) -> Option<usize> {
        self.hidden.iter().find(|&&(h, _)| h == i).map(|&(_, n)| n)
    }
}

/// The number of the file line `i` of `patch` is in, counting from 0;
/// none before the first header.
pub fn file_of(patch: &str, i: usize) -> Option<usize> {
    file_starts(patch)
        .iter()
        .filter(|&&s| s <= i)
        .count()
        .checked_sub(1)
}

/// The hunk the diff view has at `top` (its first row), by its `@@`
/// line: the one `top` is in, else, on a file's header, the first one
/// after it; past the end, the last line's. None in a patch without one.
pub fn hunk_at(patch: &str, top: usize) -> Option<usize> {
    let lines: Vec<&str> = patch.lines().collect();
    let file = |i: &usize| lines[*i].starts_with("diff --git ");
    let hunk = |i: &usize| lines[*i].starts_with("@@");
    let top = top.min(lines.len().checked_sub(1)?);
    let before = (0..=top).rev().find(|i| hunk(i) || file(i));
    match before.filter(hunk) {
        Some(at) => Some(at),
        None => (top..lines.len()).find(hunk),
    }
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
    let start = hunk_at(patch, top)?;
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

/// The message a change that clashes with the checkout goes back to its
/// worker in: what to do, then each part that does not fit with git's
/// words on it, cleaned. `solo`: nobody sends a solo's `done`, its turn's
/// end shows what it holds.
pub fn clash_message(c: &Clashes, solo: bool) -> String {
    let again = if solo {
        "end your turn"
    } else {
        "send done again"
    };
    let mut out = format!(
        "Your change does not apply to the checkout, whose HEAD is {}.\n\nNothing of it reached \
         the checkout, and it stays in your worktree. Bring your worktree up to that commit with \
         your change on top, still not committed (imece reads your change against your HEAD), \
         resolve what is listed here and {again}.",
        clean(&c.head)
    );
    if !c.staged.is_empty() {
        out.push_str(&format!(
            "\n\nThe checkout has changes staged and not committed, which that commit does not \
             show, in {}. Read them with `git -C {} diff --cached` and fit yours to them.",
            clean(&c.staged.join(", ")),
            clean(&c.checkout)
        ));
    }
    for clash in &c.hunks {
        let file = clean(&clash.file);
        let why = clean(&clash.why);
        out.push_str(&match &clash.hunk {
            Some(hunk) => format!("\n\n{file}\n```diff\n{}\n```\n{why}", clean(hunk)),
            None => format!("\n\n{file}\n{why}"),
        });
    }
    if c.cut {
        out.push_str(&format!(
            "\n\nimece stopped looking after {CLASH_TRIES} hunks: there may be more."
        ));
    }
    out
}

/// The line above a change that clashes with the checkout: where, and
/// that `b` sends it back to `name`.
/// From this wide a change of several files lists them beside its patch.
const TREE_WIDE: u16 = 120;
/// How wide that list is.
const TREE_WIDTH: u16 = 36;
/// The rows of what the worker said of its change, above everything.
const WHY_ROWS: usize = 4;

/// What the worker said of its change, wrapped at `width`, its first
/// [`WHY_ROWS`] rows, the last of them cut when there is more.
fn why_lines(why: &str, width: usize) -> Vec<Line<'static>> {
    let mut rows = overlay::wrap_words(&clean(why), width.saturating_sub(2));
    let more = rows.len() > WHY_ROWS;
    rows.truncate(WHY_ROWS);
    if more && let Some(last) = rows.last_mut() {
        last.push('…');
    }
    (rows.into_iter())
        .map(|row| Line::from(format!(" {row}")))
        .collect()
}

/// Which of the changes waiting worker `id`'s is, when there are several.
pub(crate) fn place_among(lang: Lang, w: &World, id: AgentId) -> Option<String> {
    let changes: Vec<AgentId> = (w.shown_inbox().into_iter())
        .filter(|&i| w.inbox[i].kind == InboxKind::Change)
        .map(|i| w.inbox[i].agent)
        .collect();
    let at = changes.iter().position(|&a| a == id)?;
    let of = changes.len();
    (of > 1).then(|| {
        let at = (at + 1).to_string();
        fill(lang.text().diff_place, &[&at, &of.to_string()])
    })
}

/// The file `top`, a line of `patch`, is in, by its path.
fn file_at(patch: &str, top: usize) -> Option<&str> {
    let lines: Vec<&str> = patch.lines().collect();
    let starts = file_starts(patch);
    let at = starts.partition_point(|&i| i <= top).checked_sub(1)?;
    Some(head_path(lines[starts[at]]))
}

/// A path in `room` cells: its end, where its name is.
fn path_end(path: &str, room: usize) -> String {
    let chars: Vec<char> = clean(path).chars().collect();
    match chars.len().checked_sub(room.saturating_sub(1)) {
        Some(cut) if chars.len() > room => {
            let end: String = chars[cut..].iter().collect();
            format!("…{end}")
        }
        _ => chars.into_iter().collect(),
    }
}

/// A change's files beside its patch, the one at hand marked, `height`
/// rows of them with that one among them.
fn tree_lines(
    files: &[ChangedFile],
    at: Option<&str>,
    width: usize,
    height: usize,
) -> Vec<Line<'static>> {
    let here = files.iter().position(|f| Some(f.path.as_str()) == at);
    let skip = here.map_or(0, |n| (n + 1).saturating_sub(height));
    (files.iter().enumerate().skip(skip).take(height))
        .map(|(n, f)| {
            let on = here == Some(n);
            let counts = format!(" +{} −{}", f.added, f.removed);
            let room = width.saturating_sub(counts.chars().count() + 2);
            let name = path_end(&f.path, room);
            let style = if on { bold(pal().strong) } else { Style::new() };
            Line::from(vec![
                Span::styled(if on { "▸ " } else { "  " }, bold(pal().strong)),
                Span::styled(name, style),
                Span::styled(counts, dim()),
            ])
        })
        .collect()
}

fn clash_line(lang: Lang, c: &Clashes, name: &str) -> Line<'static> {
    let mut files: Vec<&str> = c.hunks.iter().map(|h| h.file.as_str()).collect();
    files.dedup();
    let said = fill(lang.text().clash_line, &[&c.head, &files.join(", "), name]);
    Line::from(Span::styled(format!(" {said}"), fg(pal().bad)))
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
        let block = Block::bordered().border_style(fg(pal().agent));
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
    let copy = app.diff_copy_of(id);
    // The head line above says whose change and of what (record 125);
    // choosing lines to copy, the title counts them and says the keys.
    let mut block = Block::bordered().border_style(fg(pal().agent));
    if let Some(copy) = copy {
        block = block.title(fill(t.diff_copy_title, &[&copy.len().to_string()]));
    }
    let inner = block.inner(area);
    f.render_widget(block, area);
    // The verdict first, then the worker's words, Lonca's, a clash, what
    // the check printed: the gate in one line, the reasons under it.
    let width = usize::from(inner.width);
    let mut ran = match w.verdict(id) {
        Some(v) => verdict_lines(app.lang, &v, width),
        None => Vec::new(),
    };
    if let Some(why) = w.done_why(id) {
        ran.extend(why_lines(why, width));
    }
    if let Some(proposal) = w.proposal_of(id) {
        ran.extend(proposal_lines(app.lang, proposal));
    }
    if let Some(clashes) = w.clashes_of(id) {
        ran.push(clash_line(app.lang, clashes, &a.name));
    }
    ran.extend(check_lines(app.lang, &c.check));
    let review = review_lines(
        app.lang,
        w.review_of(id),
        app.reviewing(id),
        app.review_whole,
        usize::from(inner.width),
        usize::from(inner.height / 2),
        w.estimated,
    );
    let tree = inner.width >= TREE_WIDE && c.files.len() > 1;
    let mut files = if tree {
        Vec::new()
    } else {
        file_lines(app.lang, &c.files)
    };
    if let Some(split) = kinds_line(app.lang, &c.files) {
        files.insert(0, split);
    }
    if let Some(cost) = spend_line(app.lang, w, c.spend) {
        files.insert(0, cost);
    }
    let rows = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
    let [runs, said, list, inner] = Layout::vertical([
        Constraint::Length(rows(ran.len())),
        Constraint::Length(rows(review.len())),
        Constraint::Length(rows(files.len())),
        Constraint::Min(0),
    ])
    .areas(inner);
    f.render_widget(Paragraph::new(ran), runs);
    f.render_widget(Paragraph::new(review), said);
    f.render_widget(Paragraph::new(files), list);
    let inner = if tree {
        let [side, rule, patch] = Layout::horizontal([
            Constraint::Length(TREE_WIDTH),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(inner);
        let height = usize::from(side.height);
        let at = file_at(&c.patch, app.diff_scroll);
        let names = tree_lines(&c.files, at, usize::from(side.width), height);
        f.render_widget(Paragraph::new(names), side);
        let bar = vec![Line::from(Span::styled("│", dim())); usize::from(rule.height)];
        f.render_widget(Paragraph::new(bar), rule);
        patch
    } else {
        inner
    };
    app.diff_page.set(inner.height);
    let height = usize::from(inner.height);
    let wrap = app.diff_wrap.then_some(usize::from(inner.width));
    let shown = Shown::of(&c.patch, &app.diff_folds_of(id));
    let lines = patch_rows(
        app.lang,
        &c.patch,
        &shown,
        app.diff_scroll,
        &app.notes_of(id),
        &app.chosen_of(id),
        copy,
        height,
        wrap,
    );
    f.render_widget(Paragraph::new(lines), inner);
}

/// The rows of `patch` from line `top` on, `height` of them, of the
/// lines `shown`: each line cleaned and coloured, the top one (the line
/// at hand) marked, a folded file's header with `▸ N lines` after it, a
/// chosen hunk's `@@` line with `✓` after it, and a note under the line
/// it is on, every line of it after `✎`. While lines are being chosen to
/// copy, those are marked instead of the top one, the cursor's in bold.
#[allow(clippy::too_many_arguments)]
fn patch_rows(
    lang: Lang,
    patch: &str,
    shown: &Shown,
    top: usize,
    notes: &[&DiffNote],
    chosen: &[usize],
    copy: Option<Copying>,
    height: usize,
    wrap: Option<usize>,
) -> Vec<Line<'static>> {
    let t = lang.text();
    let mut rows = Vec::new();
    let lines: Vec<&str> = patch.lines().collect();
    let from = shown.lines.partition_point(|&i| i < top);
    // Each line takes a row at least: `height` lines fill the rows.
    for &i in shown.lines.iter().skip(from).take(height) {
        let l = lines[i];
        let text = if l.starts_with("diff --git ") {
            head_path(l)
        } else {
            l
        };
        let mut row = Line::from(Span::styled(clean(text), line_style(l)));
        if let Some(n) = shown.hides(i) {
            let said = match n {
                1 => t.folded_line.to_string(),
                n => fill(t.folded_lines, &[&n.to_string()]),
            };
            row.spans.push(Span::styled(format!("  {said}"), dim()));
        }
        if chosen.contains(&i) {
            row.spans.push(Span::styled("  ✓", bold(pal().ok)));
        }
        let marked = match copy {
            Some(c) => c.holds(i),
            None => i == top,
        };
        if marked {
            row = row.style(changed());
        }
        if copy.is_some_and(|c| c.at == i) {
            row = row.style(changed().add_modifier(Modifier::BOLD));
        }
        // Wrapped, a long line goes on in the rows under it; else the
        // screen cuts it.
        match wrap {
            Some(width) => {
                let style = row.style;
                rows.extend(super::wrap(row, width).into_iter().map(|r| r.style(style)));
            }
            None => rows.push(row),
        }
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

/// What copying lines `from..=to` of `patch` takes, each cleaned: the
/// lines as written (`+`, `-` or a space before each), or with
/// `new_side` the code as it reads after the change: only the hunks'
/// added and kept lines, without their mark (a removed line, a file's
/// head, an `@@` line are left out). `None` when the new side has none
/// of them.
pub(crate) fn copy_patch(patch: &str, from: usize, to: usize, new_side: bool) -> Option<String> {
    let mut out = Vec::new();
    let mut in_hunk = false;
    for (i, l) in patch.lines().enumerate().take(to + 1) {
        let kept = if !new_side {
            Some(l)
        } else if l.starts_with("diff ") {
            in_hunk = false;
            None
        } else if l.starts_with("@@") {
            in_hunk = true;
            None
        } else if !in_hunk {
            None
        } else if l.is_empty() {
            Some(l)
        } else {
            l.strip_prefix('+').or_else(|| l.strip_prefix(' '))
        };
        if i >= from
            && let Some(l) = kept
        {
            out.push(clean(l));
        }
    }
    (!out.is_empty()).then(|| out.join("\n"))
}

/// What a changed file is, for the split above a change's files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Source,
    Tests,
    /// Lock files, minified or generated code: written by a tool, read
    /// by nobody line by line.
    Generated,
}

/// What `path` is, by its name alone.
fn kind(path: &str) -> Kind {
    let lower = path.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    const LOCKS: [&str; 8] = [
        "cargo.lock",
        "package-lock.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "go.sum",
        "poetry.lock",
        "gemfile.lock",
        "composer.lock",
    ];
    let generated = LOCKS.contains(&name)
        || [
            ".min.js",
            ".min.css",
            ".pb.go",
            "_pb2.py",
            ".g.dart",
            ".designer.cs",
        ]
        .iter()
        .any(|e| name.ends_with(e))
        || ["generated/", "dist/", "vendor/"]
            .iter()
            .any(|d| lower.starts_with(d) || lower.contains(&format!("/{d}")));
    let tests = ["tests/", "test/", "__tests__/", "spec/"]
        .iter()
        .any(|d| lower.starts_with(d) || lower.contains(&format!("/{d}")))
        || name.starts_with("test_")
        || [
            "_test.go",
            "_test.rs",
            "_test.py",
            ".test.ts",
            ".test.js",
            ".test.tsx",
            ".spec.ts",
            ".spec.js",
            "tests.cs",
            "test.java",
        ]
        .iter()
        .any(|e| name.ends_with(e));
    match (generated, tests) {
        (true, _) => Kind::Generated,
        (false, true) => Kind::Tests,
        (false, false) => Kind::Source,
    }
}

/// A change's lines by what its files are (`source +20 −1 (2) · tests
/// +18 −0 (1)`), when it has files of more than one kind; binary files
/// count as files, not lines.
fn kinds_line(lang: Lang, files: &[ChangedFile]) -> Option<Line<'static>> {
    let t = lang.text();
    let mut sums: Vec<(Kind, u32, u32, usize)> = Vec::new();
    for f in files {
        let k = kind(&f.path);
        match sums.iter_mut().find(|s| s.0 == k) {
            Some(s) => (s.1, s.2, s.3) = (s.1 + f.added, s.2 + f.removed, s.3 + 1),
            None => sums.push((k, f.added, f.removed, 1)),
        }
    }
    if sums.len() < 2 {
        return None;
    }
    sums.sort_by_key(|s| s.0);
    let words: Vec<String> = (sums.iter())
        .map(|&(k, a, r, n)| {
            let name = match k {
                Kind::Source => t.kind_source,
                Kind::Tests => t.kind_tests,
                Kind::Generated => t.kind_generated,
            };
            format!("{name} +{a} −{r} ({n})")
        })
        .collect();
    Some(Line::from(Span::styled(
        format!(" {}", words.join(" · ")),
        bold(pal().strong),
    )))
}

/// How many names the verdict says of files outside the card or shared
/// with another change; the rest are counted.
const VERDICT_NAMES: usize = 3;

/// `names`, [`VERDICT_NAMES`] at most and the rest counted, cleaned.
pub(crate) fn named(names: &[String]) -> String {
    let mut said = (names.iter().take(VERDICT_NAMES))
        .map(|n| clean(n))
        .collect::<Vec<_>>()
        .join(", ");
    if names.len() > VERDICT_NAMES {
        said.push_str(&format!(" +{}", names.len() - VERDICT_NAMES));
    }
    said
}

/// The verdict on a change (record 125) as spans, parts joined with
/// ` · `: its head (`✓ ready to approve` / `! look first`) when `head`,
/// what proved it (`✓ check`, `✗ cargo test: exit 101`, or `nothing
/// ran`), its files and where they stand to the card's paths, what it
/// overlaps, a clash, a patch not whole, what Lonca runs after the yes,
/// the card's checks the yes asks for and those the policy lacks.
/// Each part in its own colour: good, bad, a warning, or dim for a fact.
/// With `counts` the files part carries the lines added and removed
/// (a row's; the change view's title has them).
pub(crate) fn verdict_spans(
    lang: Lang,
    v: &Verdict,
    head: bool,
    counts: Option<(u32, u32)>,
) -> Vec<Span<'static>> {
    let t = lang.text();
    let counts = match counts {
        Some((added, removed)) => format!(" +{added} −{removed}"),
        None => String::new(),
    };
    let mut parts: Vec<Span<'static>> = Vec::new();
    if head {
        parts.push(if v.ready {
            Span::styled(t.verdict_ready.to_string(), bold(pal().ok))
        } else {
            Span::styled(t.verdict_look.to_string(), bold(pal().warn))
        });
    }
    if v.proof.is_empty() {
        parts.push(Span::styled(t.row_not_run.to_string(), fg(pal().warn)));
    }
    for p in &v.proof {
        let colour = if p.ok() { pal().ok } else { pal().bad };
        let said = match p {
            Proof::Check { ok: true, .. } => format!("✓ {}", t.row_check_word),
            Proof::Check { end, .. } => fill(t.check_bad, &[&clean(end)]),
            Proof::Run(r) => words::ran(lang, r),
        };
        parts.push(Span::styled(said, fg(colour)));
    }
    let files = match v.files {
        1 => format!("{}{counts}", t.row_file),
        n => format!("{}{counts}", fill(t.row_files, &[&n.to_string()])),
    };
    match v.card.map(card_ref) {
        Some(card) if v.outside.is_empty() => {
            let n = v.files.to_string();
            let said = match v.files {
                1 => fill(t.verdict_inside[0], &[&counts, &card]),
                2 => fill(t.verdict_inside[1], &[&counts, &card]),
                _ => fill(t.verdict_inside[2], &[&n, &counts, &card]),
            };
            parts.push(Span::styled(said, dim()));
        }
        Some(card) => {
            parts.push(Span::styled(files, dim()));
            let n = v.outside.len().to_string();
            let said = fill(t.diff_outside, &[&n, &card, &named(&v.outside)]);
            parts.push(Span::styled(said, fg(pal().bad)));
        }
        None => parts.push(Span::styled(files, dim())),
    }
    for o in &v.overlaps {
        let files = named(&o.files);
        let said = match &o.with {
            Other::Change { card: Some(c), .. } => {
                fill(t.verdict_overlaps, &[&card_ref(*c), &files])
            }
            Other::Change { card: None, .. } => fill(t.verdict_overlaps, &["?", &files]),
            Other::Proposal { seq, name } => fill(
                t.verdict_overlaps_proposal,
                &[&seq.to_string(), &clean(name), &files],
            ),
        };
        parts.push(Span::styled(said, fg(pal().bad)));
    }
    if v.overlaps.is_empty() && v.others > 0 {
        parts.push(Span::styled(t.verdict_no_overlap.to_string(), dim()));
    }
    if v.clashes {
        parts.push(Span::styled(t.verdict_clash.to_string(), fg(pal().bad)));
    }
    if v.partial {
        parts.push(Span::styled(t.verdict_partial.to_string(), fg(pal().warn)));
    }
    if v.stale {
        parts.push(Span::styled(t.verdict_stale.to_string(), dim()));
    }
    match &v.verifies {
        Some(Runs::These(names)) if names.is_empty() && v.asks.is_empty() => {
            parts.push(Span::styled(
                t.verdict_verifies_none.to_string(),
                fg(pal().warn),
            ));
        }
        Some(Runs::These(names)) => {
            // The card's checks run too: the yes asks for them (record 127).
            let all: Vec<&str> = (names.iter().chain(&v.asks)).map(String::as_str).collect();
            let said = fill(t.verdict_verifies, &[&clean(&all.join(", "))]);
            parts.push(Span::styled(said, dim()));
        }
        Some(Runs::Unsaid) => {
            parts.push(Span::styled(
                t.verdict_verifies_unsaid.to_string(),
                fg(pal().warn),
            ));
        }
        None => {}
    }
    if !v.asks.is_empty() {
        let said = fill(t.verdict_asks, &[&clean(&crate::checks::listed(&v.asks))]);
        parts.push(Span::styled(said, dim()));
    }
    if !v.unknown.is_empty() {
        let said = fill(
            t.verdict_unknown,
            &[&clean(&crate::checks::listed(&v.unknown))],
        );
        parts.push(Span::styled(said, fg(pal().bad)));
    }
    let mut spans = Vec::with_capacity(parts.len() * 2);
    for (n, part) in parts.into_iter().enumerate() {
        if n > 0 {
            spans.push(Span::styled(" · ", dim()));
        }
        spans.push(part);
    }
    spans
}

/// The verdict line above a change's patch, wrapped to `width`.
pub(crate) fn verdict_lines(lang: Lang, v: &Verdict, width: usize) -> Vec<Line<'static>> {
    super::wrap(Line::from(verdict_spans(lang, v, true, None)), width)
}

/// Lonca's word for why a change waits for a human, as a person says it;
/// a word imece does not know, as Lonca said it.
fn why_it_waits(t: &Text, why: &str) -> String {
    let (word, path) = why.split_once(':').unwrap_or((why, ""));
    let [
        unwatched,
        tainted,
        critical,
        outside,
        written,
        unhooked,
        unhooked_critical,
        replaces,
    ] = t.diff_whys;
    match word {
        "unobserved_session" => unwatched.to_string(),
        "tainted_session" => tainted.to_string(),
        "unhooked_session" => unhooked.to_string(),
        "unhooked_critical_path" => unhooked_critical.to_string(),
        "critical_path" | "unobserved_critical_path" | "tainted_critical_path" => {
            fill(critical, &[path])
        }
        "scope_path" => fill(outside, &[path]),
        "unobserved_write" => fill(written, &[path]),
        // The change it names is Lonca's id, which says nothing here.
        "replaces" => replaces.to_string(),
        _ => clean(why),
    }
    .trim_end_matches([':', ' '])
    .to_string()
}

/// Which proposal in Lonca a change is (record 120): its number and why
/// it waits; then what the patch below does not show, loud.
pub(crate) fn proposal_lines(lang: Lang, p: &StoreProposal) -> Vec<Line<'static>> {
    let t = lang.text();
    let why: Vec<String> = p.why.iter().map(|w| why_it_waits(t, w)).collect();
    let said = fill(t.diff_proposal, &[&why.join(", "), &p.seq.to_string()]);
    let mut lines = vec![Line::from(Span::styled(said, dim()))];
    let loud = |text: String| Line::from(Span::styled(text, fg(pal().warn)));
    if !p.redacted.is_empty() {
        lines.push(loud(fill(t.diff_redacted, &[&p.redacted.join(", ")])));
    }
    if p.cut {
        lines.push(loud(t.diff_cut.to_string()));
    }
    lines
}

/// What the change cost (record 113), above its kinds and files: `cost ·
/// $0.84 · 12 min`, dim; nothing until its worker's turn ended.
fn spend_line(lang: Lang, w: &World, spend: Option<Spend>) -> Option<Line<'static>> {
    let s = spend?;
    Some(Line::from(Span::styled(
        format!(
            " {}",
            fill(lang.text().diff_spend, &[&words::spend(lang, w, &s)])
        ),
        dim(),
    )))
}

/// The files a change shows above its patch, at most this many; the rest
/// are counted.
const FILES_SHOWN: usize = 6;

/// The change's files, one a line with its lines added and removed (or
/// that it is binary), [`FILES_SHOWN`] at most and the rest counted;
/// nothing for a change of one file, which its title says already.
fn file_lines(lang: Lang, files: &[ChangedFile]) -> Vec<Line<'static>> {
    let t = lang.text();
    if files.len() < 2 {
        return Vec::new();
    }
    let mut out: Vec<Line> = (files.iter().take(FILES_SHOWN))
        .map(|f| {
            let counts = match f.binary {
                true => Span::styled(format!(" {}", t.binary), dim()),
                false => Span::styled(format!(" +{} −{}", f.added, f.removed), dim()),
            };
            Line::from(vec![Span::raw(format!(" {}", clean(&f.path))), counts])
        })
        .collect();
    if files.len() > FILES_SHOWN {
        let more = (files.len() - FILES_SHOWN).to_string();
        out.push(Line::from(Span::styled(
            fill(t.more_files, &[&more]),
            dim(),
        )));
    }
    out
}

/// The runs on a change's tree, one a line, coloured by how they ended;
/// one line saying what it means when there is none.
/// The rows a folded review takes under its head (`R` shows it whole).
pub(crate) const REVIEW_ROWS: usize = 6;

/// What a fresh session said of the change (`:review`, record 106), above
/// its file list: a head (who, on what model, for how much), then its
/// lines cleaned and wrapped at `width`: the first [`REVIEW_ROWS`], or
/// `whole` as many as `room` leaves, the rest counted. While one is on
/// its way, a line says who reads the change; else nothing.
pub(crate) fn review_lines(
    lang: Lang,
    review: Option<&Review>,
    under_way: bool,
    whole: bool,
    width: usize,
    room: usize,
    estimated: bool,
) -> Vec<Line<'static>> {
    let t = lang.text();
    let Some(r) = review else {
        if !under_way {
            return Vec::new();
        }
        let said = fill(t.review_under_way, &[CLAUDE]);
        return vec![Line::from(Span::styled(said, dim()))];
    };
    let head = fill(
        t.review_head,
        &[
            CLAUDE,
            &r.model,
            &crate::model::money(r.cost, false, estimated),
        ],
    );
    let mut out = vec![Line::from(Span::styled(head, bold(pal().accent)))];
    let body: Vec<Line<'static>> = (r.text.lines())
        .flat_map(|l| super::wrap(Line::from(clean(l)), width.max(1)))
        .collect();
    let shown = if whole {
        room.saturating_sub(2).max(REVIEW_ROWS)
    } else {
        REVIEW_ROWS
    };
    let left = body.len().saturating_sub(shown);
    out.extend(body.into_iter().take(shown));
    if left > 0 {
        let more = fill(t.review_more, &[&left.to_string()]);
        out.push(Line::from(Span::styled(more, dim())));
    }
    out
}

/// Where the repository's check (record 112) stands on the change, above
/// the runs: how it ended (a failed one with the last lines of its
/// output, dim), that it runs, that it waits for the person's yes, or why
/// it could not run; nothing when there is none.
pub(crate) fn check_lines(lang: Lang, check: &Check) -> Vec<Line<'static>> {
    let t = lang.text();
    match check {
        // How it ended is the verdict's; a failure's last lines stand
        // under it.
        Check::None | Check::Ran(Checked { ok: true, .. }) => Vec::new(),
        Check::Ran(d) => (d.lines.iter())
            .map(|l| Line::from(Span::styled(format!("  {}", clean(l)), dim())))
            .collect(),
        Check::Running => vec![Line::from(Span::styled(t.check_running.to_string(), dim()))],
        Check::Asked => vec![Line::from(Span::styled(
            t.check_asked.to_string(),
            fg(pal().warn),
        ))],
        Check::Failed(why) => vec![Line::from(Span::styled(
            fill(t.check_failed, &[&clean(why)]),
            fg(pal().warn),
        ))],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(lines: &[Line]) -> Vec<String> {
        lines.iter().map(super::super::line_text).collect()
    }

    fn a_review(text: &str) -> Review {
        Review {
            patch: "p".into(),
            model: "default".into(),
            text: text.into(),
            cost: 0,
        }
    }

    fn clashes() -> Clashes {
        let clash = |file: &str, hunk: Option<&str>, why: &str| Clash {
            file: file.into(),
            hunk: hunk.map(str::to_string),
            why: why.into(),
        };
        Clashes {
            checkout: "/home/me/repo".into(),
            head: "abc1234".into(),
            hunks: vec![
                clash("a.rs", Some("@@ -1 +1 @@ fn a"), "patch failed: a.rs:1"),
                clash("a.rs", Some("@@ -9 +9 @@"), "patch failed: a.rs:9"),
                clash("b.png", None, "b.png: patch does not apply"),
            ],
            staged: Vec::new(),
            cut: false,
        }
    }

    #[test]
    fn git_s_lines_above_a_hunk_go_and_a_line_that_only_looks_like_them_stays() {
        let patch = "diff --git a/q.sql b/q.sql\nindex 1..2\n--- a/q.sql\n+++ b/q.sql\n\
                     @@ -1,2 +1,2 @@\n--- a comment\n+++ a sum\n index stays\n";
        let shown = Shown::of(patch, &Folds::default());
        assert_eq!(shown.lines, [0, 4, 5, 6, 7]);
        // A second file's head goes too.
        let two = format!("{patch}diff --git a/r b/r\nindex 3..4\n--- a/r\n+++ b/r\n@@ -1 +1 @@\n");
        let shown = Shown::of(&two, &Folds::default());
        assert_eq!(shown.lines, [0, 4, 5, 6, 7, 8, 12]);
    }

    #[test]
    fn a_file_s_head_is_its_path() {
        assert_eq!(head_path("diff --git a/src/x.rs b/src/x.rs"), "src/x.rs");
        assert_eq!(head_path("no such thing"), "no such thing");
        let patch = "diff --git a/x b/x\n@@ -1 +1 @@\n-a\n+b\n";
        let rows = patch_rows(
            Lang::En,
            patch,
            &Shown::of(patch, &Folds::default()),
            0,
            &[],
            &[],
            None,
            10,
            None,
        );
        assert_eq!(rows[0].spans[0].content, "x");
    }

    #[test]
    fn what_the_worker_said_takes_four_rows_and_says_when_there_is_more() {
        let texts = |lines: Vec<Line>| {
            lines
                .iter()
                .map(|l| l.spans[0].content.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(texts(why_lines("one two", 20)), [" one two"]);
        let four = "aaaa bbbb cccc dddd";
        assert_eq!(texts(why_lines(four, 6)).len(), 4);
        assert!(!texts(why_lines(four, 6))[3].ends_with('…'));
        let five = "aaaa bbbb cccc dddd eeee";
        let rows = texts(why_lines(five, 6));
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[3], " dddd…");
    }

    #[test]
    fn a_change_says_its_place_only_among_several() {
        let (mut w, _) = crate::sim::world(1, 3, 0, 1, Lang::En);
        let change = |agent| InboxItem {
            kind: InboxKind::Change,
            agent,
            detail: String::new(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        };
        w.inbox.clear();
        w.inbox.push(change(1));
        assert_eq!(place_among(Lang::En, &w, 1), None);
        assert_eq!(place_among(Lang::En, &w, 2), None, "it has none waiting");
        w.inbox.push(change(2));
        w.inbox.push(InboxItem {
            kind: InboxKind::Reply,
            ..change(3)
        });
        assert_eq!(
            place_among(Lang::En, &w, 2).as_deref(),
            Some(" change 2 of 2 ")
        );
    }

    #[test]
    fn the_list_marks_the_file_the_top_line_is_in_and_keeps_it_in_view() {
        let patch = "@@ head\ndiff --git a/a b/a\n+x\ndiff --git a/b b/b\n+y\n";
        assert_eq!(file_at(patch, 0), None, "above the first file");
        assert_eq!(file_at(patch, 1), Some("a"));
        assert_eq!(file_at(patch, 2), Some("a"));
        assert_eq!(file_at(patch, 3), Some("b"));
        let file = |path: &str| ChangedFile {
            path: path.into(),
            added: 1,
            removed: 2,
            binary: false,
        };
        let files = [file("a"), file("b"), file("c")];
        let text = |l: &Line| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        };
        let rows = tree_lines(&files, Some("b"), 20, 3);
        assert_eq!(text(&rows[0]), "  a +1 −2");
        assert_eq!(text(&rows[1]), "▸ b +1 −2");
        // Two rows, the third file at hand: it shows, the first went.
        let rows = tree_lines(&files, Some("c"), 20, 2);
        assert_eq!(text(&rows[0]), "  b +1 −2");
        assert_eq!(text(&rows[1]), "▸ c +1 −2");
        // Exactly as many rows as it needs: nothing goes.
        let rows = tree_lines(&files, Some("b"), 20, 2);
        assert_eq!(text(&rows[0]), "  a +1 −2");
    }

    #[test]
    fn a_long_path_keeps_its_end() {
        assert_eq!(path_end("abcd", 4), "abcd");
        assert_eq!(path_end("abcde", 4), "…cde");
        assert_eq!(path_end("ab", 4), "ab");
    }

    #[test]
    fn lonca_s_words_for_why_a_change_waits_are_said_as_a_person_says_them() {
        let t = Lang::En.text();
        for (word, said) in [
            ("unobserved_session", "nobody watched this session's tools"),
            ("tainted_session", "this session read something untrusted"),
            (
                "critical_path:Cargo.toml",
                "it changes a critical path: Cargo.toml",
            ),
            ("unobserved_critical_path", "it changes a critical path"),
            ("tainted_critical_path", "it changes a critical path"),
            ("scope_path:a.rs", "a file outside its scope: a.rs"),
            ("unobserved_write:b.rs", "a file written unseen: b.rs"),
            (
                "unhooked_session",
                "this session's hooks did not run: Lonca saw a tool it was not told of",
            ),
            (
                "unhooked_critical_path",
                "it changes a critical path from a session whose hooks did not run",
            ),
            (
                "replaces:wxmztzlwytrl",
                "it takes the place of its earlier proposal",
            ),
            ("something_new", "something_new"),
        ] {
            assert_eq!(why_it_waits(t, word), said);
        }
        // Every word has a Turkish one of its own, none said twice.
        let tr = Lang::Tr.text();
        let words: std::collections::HashSet<&str> = tr.diff_whys.iter().copied().collect();
        assert_eq!(words.len(), tr.diff_whys.len());
        assert_eq!(
            why_it_waits(tr, "unhooked_session"),
            "bu oturumun kancaları çalışmadı: Lonca haber verilmeyen bir araç gördü"
        );
    }

    #[test]
    fn the_message_says_what_to_do_and_what_does_not_fit() {
        let got = clash_message(&clashes(), false);
        let (head, rest) = got.split_once("\n\n").unwrap();
        assert_eq!(
            head,
            "Your change does not apply to the checkout, whose HEAD is abc1234."
        );
        assert!(
            rest.starts_with(
                "Nothing of it reached the checkout, and it stays in your worktree. Bring your \
                 worktree up to that commit with your change on top, still not committed (imece \
                 reads your change against your HEAD), resolve what is listed here and send done \
                 again.\n\n"
            ),
            "{rest}"
        );
        assert!(
            got.ends_with(
                "\n\na.rs\n```diff\n@@ -1 +1 @@ fn a\n```\npatch failed: a.rs:1\
                 \n\na.rs\n```diff\n@@ -9 +9 @@\n```\npatch failed: a.rs:9\
                 \n\nb.png\nb.png: patch does not apply"
            ),
            "{got}"
        );
        assert!(!got.contains("staged") && !got.contains("stopped looking"));
        // A solo is not told to send done.
        let solo = clash_message(&clashes(), true);
        assert!(solo.contains("listed here and end your turn."), "{solo}");
        assert!(!solo.contains("send done"), "{solo}");
    }

    #[test]
    fn the_message_says_what_is_staged_and_where_it_stopped_looking() {
        let got = clash_message(
            &Clashes {
                staged: vec!["a.rs".into(), "c.rs".into()],
                cut: true,
                ..clashes()
            },
            false,
        );
        assert!(
            got.contains(
                "\n\nThe checkout has changes staged and not committed, which that commit does \
                 not show, in a.rs, c.rs. Read them with `git -C /home/me/repo diff --cached` and \
                 fit yours to them.\n\n"
            ),
            "{got}"
        );
        assert!(
            got.ends_with("\n\nimece stopped looking after 200 hunks: there may be more."),
            "{got}"
        );
    }

    #[test]
    fn what_git_or_an_agent_wrote_reaches_the_message_and_the_line_cleaned() {
        let bad = "x\u{1b}[31my";
        let c = Clashes {
            checkout: bad.into(),
            head: bad.into(),
            hunks: vec![Clash {
                file: bad.into(),
                hunk: Some(bad.into()),
                why: bad.into(),
            }],
            staged: vec![bad.into()],
            cut: false,
        };
        let got = clash_message(&c, false);
        assert!(!got.contains('\u{1b}'), "{got:?}");
        assert_eq!(got.matches(&clean(bad)).count(), 6, "{got:?}");
        let line = super::super::line_text(&clash_line(Lang::En, &c, bad));
        assert!(!line.contains('\u{1b}'), "{line:?}");
    }

    #[test]
    fn the_line_above_a_clash_names_each_file_once() {
        let line = super::super::line_text(&clash_line(Lang::En, &clashes(), "claude-2"));
        assert_eq!(
            line,
            "✗ does not fit your checkout at abc1234: a.rs, b.png · b sends it back to claude-2"
        );
    }

    #[test]
    fn a_review_shows_its_head_and_first_rows_and_counts_the_rest() {
        let none = review_lines(Lang::En, None, false, false, 40, 30, false);
        assert!(none.is_empty(), "nothing asked: nothing");
        let asked = review_lines(Lang::En, None, true, false, 40, 30, false);
        assert_eq!(texts(&asked), ["✎ claude reads this change…"]);
        let lines = |n: usize| {
            (1..=n)
                .map(|i| format!("l{i}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let six = a_review(&lines(REVIEW_ROWS));
        let got = texts(&review_lines(
            Lang::En,
            Some(&six),
            false,
            false,
            40,
            30,
            false,
        ));
        assert_eq!(got[0], "✎ review · claude · default · $0.00");
        assert_eq!(got.len(), 1 + REVIEW_ROWS, "all of it, nothing counted");
        assert_eq!(got[REVIEW_ROWS], "l6");
        let nine = a_review(&lines(REVIEW_ROWS + 3));
        let got = texts(&review_lines(
            Lang::En,
            Some(&nine),
            true,
            false,
            40,
            30,
            false,
        ));
        assert_eq!(got.len(), 2 + REVIEW_ROWS, "{got:?}");
        assert_eq!(got.last().unwrap(), "… +3 lines · R shows all");
        // Whole: as many as the room leaves (its head and the count line
        // aside), never fewer than folded.
        let got = texts(&review_lines(
            Lang::En,
            Some(&nine),
            false,
            true,
            40,
            30,
            false,
        ));
        assert_eq!(got.len(), 1 + REVIEW_ROWS + 3, "all of it");
        let got = texts(&review_lines(
            Lang::En,
            Some(&nine),
            false,
            true,
            40,
            10,
            false,
        ));
        assert_eq!(got.len(), 1 + 8 + 1, "{got:?}");
        assert_eq!(got.last().unwrap(), "… +1 lines · R shows all");
        let got = texts(&review_lines(
            Lang::En,
            Some(&nine),
            false,
            true,
            40,
            4,
            false,
        ));
        assert_eq!(got.len(), 2 + REVIEW_ROWS, "folded at least");
        // Long lines wrap at the width and count as rows; text is cleaned.
        let long = a_review("abcdefghij klmnopqrst\u{1b}[2J");
        let got = texts(&review_lines(
            Lang::En,
            Some(&long),
            false,
            false,
            11,
            30,
            false,
        ));
        assert_eq!(&got[1..], ["abcdefghij", "klmnopqrst", "[2J"], "{got:?}");
    }

    /// `patch_rows` of a change whose files are all open, in English.
    fn open_rows(
        patch: &str,
        top: usize,
        notes: &[&DiffNote],
        chosen: &[usize],
        copy: Option<Copying>,
        height: usize,
        wrap: Option<usize>,
    ) -> Vec<Line<'static>> {
        let shown = Shown::of(patch, &Folds::default());
        patch_rows(
            Lang::En,
            patch,
            &shown,
            top,
            notes,
            chosen,
            copy,
            height,
            wrap,
        )
    }

    /// A line before the first header, a file of three more lines, one
    /// of four.
    const TWO: &str = "note\ndiff --git a/x b/x\nnew file mode 1\n@@ -0 +1 @@\n+one\n\
                       diff --git a/y b/y\nnew file mode 1\n@@ -0 +1 @@\n+two\n+three\n";

    #[test]
    fn a_folded_file_shows_its_header_only() {
        let open = Shown::of(TWO, &Folds::default());
        assert_eq!(open.lines, (0..10).collect::<Vec<_>>());
        assert!(open.hidden.is_empty());
        let mut first = Folds::default();
        first.set(0, true);
        let shown = Shown::of(TWO, &first);
        assert_eq!(shown.lines, [0, 1, 5, 6, 7, 8, 9]);
        assert_eq!(shown.hidden, [(1, 3)]);
        let all = Shown::of(TWO, &Folds::every(true));
        assert_eq!(all.lines, [0, 1, 5]);
        assert_eq!(all.hidden, [(1, 3), (5, 4)]);
        let mut second_open = Folds::every(true);
        second_open.set(1, false);
        assert_eq!(Shown::of(TWO, &second_open).hidden, [(1, 3)]);
        assert_eq!(Shown::of("", &Folds::every(true)), Shown::default());
        assert_eq!(file_of(TWO, 0), None, "before the first header");
        assert_eq!(file_of(TWO, 1), Some(0));
        assert_eq!(file_of(TWO, 4), Some(0));
        assert_eq!(file_of(TWO, 5), Some(1));
        assert_eq!(file_of(TWO, 9), Some(1));
    }

    #[test]
    fn the_scroll_steps_over_the_lines_shown() {
        let mut first = Folds::default();
        first.set(0, true);
        let s = Shown::of(TWO, &first);
        assert_eq!(s.step(1, 1, true), 5, "over the folded file");
        assert_eq!(s.step(5, 1, false), 1);
        assert_eq!(s.step(5, 2, true), 7);
        assert_eq!(s.step(7, 2, false), 5);
        assert_eq!(s.step(3, 0, true), 1, "a hidden line: its header");
        assert_eq!(s.step(3, 1, true), 5);
        assert_eq!(s.step(5, 99, true), 9, "stops at the last");
        assert_eq!(s.step(5, 99, false), 0, "and the first");
        assert_eq!(s.step(99, 0, true), 9);
        assert_eq!(Shown::default().step(4, 1, true), 0);
    }

    #[test]
    fn a_folded_header_says_how_many_lines_it_hides() {
        let all = Shown::of(TWO, &Folds::every(true));
        let rows = patch_rows(Lang::En, TWO, &all, 1, &[], &[], None, 8, None);
        let text: Vec<String> = rows.iter().map(|r| r.to_string()).collect();
        assert_eq!(
            text,
            ["x  ▸ 3 lines", "y  ▸ 4 lines"],
            "from the line at hand on"
        );
        let one = "diff --git a/x b/x\n+one\ndiff --git a/y b/y\n+two\n";
        let all = Shown::of(one, &Folds::every(true));
        let rows = patch_rows(Lang::Tr, one, &all, 0, &[], &[], None, 8, None);
        assert_eq!(rows[0].to_string(), "x  ▸ 1 satır");
        assert_eq!(rows.len(), 2);
    }

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
    fn lines_chosen_to_copy_are_marked_instead_of_the_top_one() {
        let copy = Copying { anchor: 6, at: 4 };
        let rows = open_rows(PATCH, 0, &[], &[], Some(copy), 8, None);
        assert_eq!(rows[0].style, Style::new(), "not the line at hand");
        assert_eq!(
            rows[1].style,
            changed().add_modifier(Modifier::BOLD),
            "the cursor"
        );
        assert_eq!(rows[2].style, changed());
        assert_eq!(rows[3].style, changed());
        assert_eq!(rows[4].style, Style::new());
    }

    #[test]
    fn copying_a_patch_takes_its_lines_as_written_or_the_new_side() {
        let all = PATCH.lines().count() - 1;
        assert_eq!(
            copy_patch(PATCH, 7, 9, false).as_deref(),
            Some("+new\n@@ -9,1 +9,2 @@\n ctx")
        );
        assert_eq!(copy_patch(PATCH, 7, 9, true).as_deref(), Some("new\nctx"));
        assert_eq!(
            copy_patch(PATCH, 8, 9, true).as_deref(),
            Some("ctx"),
            "from on"
        );
        assert_eq!(
            copy_patch(PATCH, 7, 7, true).as_deref(),
            Some("new"),
            "to, no further"
        );
        // Heads, removed lines and what is between files are left out:
        // `+++ b/c.rs` is a head, not an added line.
        assert_eq!(
            copy_patch(PATCH, 0, all, true).as_deref(),
            Some("new\nctx\nC")
        );
        assert_eq!(
            copy_patch(PATCH, 0, 3, false).as_deref(),
            Some("diff --git a/a.rs b/a.rs\nindex 1..2 100644\n--- a/a.rs\n+++ b/a.rs")
        );
        assert_eq!(copy_patch(PATCH, 5, 6, true), None, "removed lines only");
        assert_eq!(copy_patch(PATCH, 0, 3, true), None, "heads only");
        // A blank line in a hunk stays; `\ No newline` does not.
        let p = "@@ -1,3 +1,3 @@\n a\n\n b\n\\ No newline at end of file\n";
        assert_eq!(copy_patch(p, 0, 4, true).as_deref(), Some("a\n\nb"));
        // Cleaned: no escape reaches the clipboard.
        let p = "@@ -1 +1 @@\n+a\x1b]52;c;eA==\x07b\n";
        let copied = copy_patch(p, 0, 1, true).unwrap();
        assert!(
            !copied.contains('\x1b') && !copied.contains('\x07'),
            "{copied:?}"
        );
        let copied = copy_patch(p, 1, 1, false).unwrap();
        assert!(!copied.contains('\x1b'), "{copied:?}");
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
        let rows = open_rows(PATCH, 5, &[&a, &b], &[], None, 8, None);
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
        assert_eq!(open_rows(PATCH, 5, &[&a, &b], &[], None, 2, None).len(), 2);
        assert_eq!(
            open_rows(PATCH, 17, &[], &[], None, 8, None).len(),
            2,
            "the patch ends"
        );
        // A chosen hunk's `@@` line says so; the others do not.
        let rows = open_rows(PATCH, 4, &[], &[8], None, 6, None);
        assert_eq!(rows[4].to_string(), "@@ -9,1 +9,2 @@  ✓");
        assert_eq!(rows[4].spans[1].style, bold(pal().ok));
        assert_eq!(rows[0].to_string(), "@@ -1,2 +1,2 @@");
        // The hunk at hand, for `x`: as `>` finds it.
        assert_eq!(hunk_at(PATCH, 0), Some(4));
        assert_eq!(hunk_at(PATCH, 9), Some(8));
        assert_eq!(hunk_at(PATCH, 11), Some(16));
        assert_eq!(hunk_at("", 0), None);
    }

    #[test]
    fn the_files_head_the_patch_six_at_most_and_the_rest_counted() {
        let file = |n: usize, binary| ChangedFile {
            path: format!("src/f{n}.rs"),
            added: n as u32,
            removed: 1,
            binary,
        };
        let text = |files: &[ChangedFile]| -> Vec<String> {
            file_lines(Lang::En, files)
                .iter()
                .map(|l| l.to_string())
                .collect()
        };
        assert!(
            text(&[file(1, false)]).is_empty(),
            "one file: the title says it"
        );
        assert_eq!(
            text(&[file(1, false), file(2, true)]),
            [" src/f1.rs +1 −1", " src/f2.rs binary"]
        );
        let six: Vec<ChangedFile> = (1..=6).map(|n| file(n, false)).collect();
        assert_eq!(text(&six).len(), 6, "all six, none counted");
        let eight: Vec<ChangedFile> = (1..=8).map(|n| file(n, false)).collect();
        let lines = text(&eight);
        assert_eq!(lines.len(), 7);
        assert_eq!(lines[6], " … 2 more files");
        assert_eq!(
            file_lines(Lang::Tr, &eight)[6].to_string(),
            " … 2 dosya daha"
        );
        let mut odd = file(1, false);
        odd.path = "a\u{1b}[2Jb".into();
        assert_eq!(text(&[odd, file(2, false)])[0], " a [2Jb +1 −1", "cleaned");
    }

    #[test]
    fn a_file_is_source_tests_or_generated_by_its_name() {
        for (path, want) in [
            ("src/parser.rs", Kind::Source),
            ("README.md", Kind::Source),
            ("contest/main.go", Kind::Source),
            ("tests/cli.rs", Kind::Tests),
            ("crates/a/tests/cli.rs", Kind::Tests),
            ("test/x.c", Kind::Tests),
            ("web/__tests__/a.js", Kind::Tests),
            ("spec/models/user_spec.rb", Kind::Tests),
            ("test_parser.py", Kind::Tests),
            ("pkg/parser_test.go", Kind::Tests),
            ("src/lib_test.rs", Kind::Tests),
            ("tools/x_test.py", Kind::Tests),
            ("web/App.test.tsx", Kind::Tests),
            ("web/a.test.ts", Kind::Tests),
            ("web/a.test.js", Kind::Tests),
            ("web/a.spec.ts", Kind::Tests),
            ("web/a.spec.js", Kind::Tests),
            ("Api.Tests/ParserTests.cs", Kind::Tests),
            ("src/ParserTest.java", Kind::Tests),
            ("Cargo.lock", Kind::Generated),
            ("web/package-lock.json", Kind::Generated),
            ("yarn.lock", Kind::Generated),
            ("pnpm-lock.yaml", Kind::Generated),
            ("go.sum", Kind::Generated),
            ("poetry.lock", Kind::Generated),
            ("Gemfile.lock", Kind::Generated),
            ("composer.lock", Kind::Generated),
            ("web/app.min.js", Kind::Generated),
            ("web/app.min.css", Kind::Generated),
            ("api/user.pb.go", Kind::Generated),
            ("api/user_pb2.py", Kind::Generated),
            ("lib/a.g.dart", Kind::Generated),
            ("Form1.Designer.cs", Kind::Generated),
            ("generated/schema.ts", Kind::Generated),
            ("src/generated/schema.ts", Kind::Generated),
            ("dist/app.js", Kind::Generated),
            ("web/dist/app.js", Kind::Generated),
            ("vendor/lib.go", Kind::Generated),
            ("tests/generated/fixture.rs", Kind::Generated),
        ] {
            assert_eq!(kind(path), want, "{path}");
        }
    }

    #[test]
    fn the_split_says_each_kind_s_lines_when_there_are_several() {
        let file = |path: &str, added, removed| ChangedFile {
            path: path.into(),
            added,
            removed,
            binary: false,
        };
        let one_kind = [file("src/a.rs", 1, 0), file("src/b.rs", 2, 0)];
        assert!(kinds_line(Lang::En, &one_kind).is_none());
        let files = [
            file("Cargo.lock", 7, 5),
            file("src/a.rs", 10, 1),
            file("tests/a.rs", 18, 0),
            file("src/b.rs", 3, 2),
        ];
        let line = kinds_line(Lang::En, &files).unwrap();
        assert_eq!(
            line.to_string(),
            " source +13 −3 (2) · tests +18 −0 (1) · generated +7 −5 (1)"
        );
        assert_eq!(line.spans[0].style, bold(pal().strong));
        assert_eq!(
            kinds_line(Lang::Tr, &files[..2]).unwrap().to_string(),
            " kaynak +10 −1 (1) · üretilmiş +7 −5 (1)"
        );
    }

    #[test]
    fn a_wrapped_patch_goes_on_under_a_long_line() {
        let patch = "diff --git a/x b/x\n@@ -1 +1 @@\n+one two three four five six\n";
        let cut = open_rows(patch, 2, &[], &[], None, 9, None);
        assert_eq!(cut.len(), 1);
        let wrapped = open_rows(patch, 2, &[], &[], None, 9, Some(10));
        let text: Vec<String> = wrapped
            .iter()
            .map(|l| l.to_string().trim_end().to_string())
            .collect();
        assert_eq!(text, ["+one two", "three four", "five six"]);
        assert!(
            wrapped.iter().all(|r| r.style == changed()),
            "the line at hand, all of it"
        );
        assert_eq!(
            open_rows(patch, 2, &[], &[], None, 2, Some(10)).len(),
            2,
            "cut at the height"
        );
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

    fn a_verdict() -> Verdict {
        Verdict {
            ready: true,
            proof: vec![Proof::Run(Ran {
                line: "cargo test".into(),
                code: Some(0),
                timed_out: false,
            })],
            files: 2,
            card: Some(5),
            outside: Vec::new(),
            others: 0,
            overlaps: Vec::new(),
            clashes: false,
            partial: false,
            stale: false,
            verifies: None,
            asks: Vec::new(),
            unknown: Vec::new(),
        }
    }

    fn worded(lang: Lang, v: &Verdict, head: bool, counts: Option<(u32, u32)>) -> String {
        (verdict_spans(lang, v, head, counts).iter())
            .map(|s| s.content.as_ref())
            .collect()
    }

    #[test]
    fn the_verdict_heads_with_ready_or_look_first_and_says_what_proved_it() {
        let v = a_verdict();
        assert_eq!(
            worded(Lang::En, &v, true, None),
            "✓ ready to approve · ✓ cargo test · 2 files, both inside #6"
        );
        assert_eq!(
            worded(Lang::Tr, &v, true, None),
            "✓ onaya hazır · ✓ cargo test · 2 dosya, ikisi de #6 içinde"
        );
        let spans = verdict_spans(Lang::En, &v, true, None);
        assert_eq!(spans[0].style, bold(pal().ok));
        assert_eq!(spans[1].content, " · ");
        assert_eq!(spans[1].style, dim());
        assert_eq!(spans[2].style, fg(pal().ok), "a run that passed");
        assert_eq!(spans[4].style, dim(), "the files are a fact");
        let look = Verdict {
            ready: false,
            proof: vec![
                Proof::Check {
                    ok: false,
                    end: "exit 1, 3.0 s".into(),
                },
                Proof::Run(Ran {
                    line: "cargo test".into(),
                    code: Some(101),
                    timed_out: false,
                }),
                Proof::Run(Ran {
                    line: "cargo build".into(),
                    code: None,
                    timed_out: true,
                }),
            ],
            ..a_verdict()
        };
        assert_eq!(
            worded(Lang::En, &look, true, None),
            "! look first · ✗ check · exit 1, 3.0 s · ✗ cargo test: exit 101 · \
             ✗ cargo build: killed at the deadline · 2 files, both inside #6"
        );
        let spans = verdict_spans(Lang::En, &look, true, None);
        assert_eq!(spans[0].style, bold(pal().warn));
        assert_eq!(spans[2].style, fg(pal().bad));
        assert_eq!(spans[4].style, fg(pal().bad));
        assert_eq!(
            worded(Lang::Tr, &look, true, None),
            "! önce bak · ✗ kontrol · exit 1, 3.0 s · ✗ cargo test: exit 101 · \
             ✗ cargo build: süre dolunca durduruldu · 2 dosya, ikisi de #6 içinde"
        );
        let check = Verdict {
            proof: vec![Proof::Check {
                ok: true,
                end: "exit 0, 41.2 s".into(),
            }],
            ..a_verdict()
        };
        assert_eq!(
            worded(Lang::En, &check, true, None),
            "✓ ready to approve · ✓ check · 2 files, both inside #6"
        );
        assert_eq!(
            worded(Lang::Tr, &check, false, None),
            "✓ kontrol · 2 dosya, ikisi de #6 içinde",
            "a row has no head"
        );
        let none = Verdict {
            ready: false,
            proof: Vec::new(),
            ..a_verdict()
        };
        assert_eq!(
            worded(Lang::En, &none, true, None),
            "! look first · nothing ran · 2 files, both inside #6"
        );
        assert_eq!(
            verdict_spans(Lang::En, &none, true, None)[2].style,
            fg(pal().warn)
        );
        assert_eq!(
            worded(Lang::Tr, &none, true, None),
            "! önce bak · hiçbir şey koşmadı · 2 dosya, ikisi de #6 içinde"
        );
    }

    #[test]
    fn the_verdict_counts_the_files_against_the_card_s_paths() {
        let with = |files, card, outside: &[&str]| Verdict {
            files,
            card,
            outside: outside.iter().map(|f| f.to_string()).collect(),
            ..a_verdict()
        };
        let said = |v: &Verdict| worded(Lang::En, v, false, None);
        assert_eq!(
            said(&with(1, Some(5), &[])),
            "✓ cargo test · 1 file, inside #6"
        );
        assert_eq!(
            said(&with(3, Some(5), &[])),
            "✓ cargo test · 3 files, all inside #6"
        );
        assert_eq!(said(&with(1, None, &[])), "✓ cargo test · 1 file");
        assert_eq!(said(&with(3, None, &[])), "✓ cargo test · 3 files");
        assert_eq!(
            said(&with(3, Some(5), &["src/b.rs"])),
            "✓ cargo test · 3 files · 1 outside #6: src/b.rs"
        );
        assert_eq!(
            said(&with(3, Some(5), &["a", "b", "c"])),
            "✓ cargo test · 3 files · 3 outside #6: a, b, c",
            "three names are said whole"
        );
        let v = with(5, Some(5), &["a", "b", "c", "d"]);
        assert_eq!(
            said(&v),
            "✓ cargo test · 5 files · 4 outside #6: a, b, c +1",
            "three names and the rest counted"
        );
        assert_eq!(
            verdict_spans(Lang::En, &v, false, None)[4].style,
            fg(pal().bad)
        );
        assert_eq!(
            worded(Lang::Tr, &with(3, Some(5), &["src/b.rs"]), false, None),
            "✓ cargo test · 3 dosya · 1 kartın dışında (#6): src/b.rs"
        );
        assert_eq!(
            worded(Lang::Tr, &with(1, None, &[]), false, None),
            "✓ cargo test · 1 dosya"
        );
        // A row carries the lines with the files, inside or not.
        assert_eq!(
            worded(Lang::En, &with(2, Some(5), &[]), false, Some((4, 2))),
            "✓ cargo test · 2 files +4 −2, both inside #6"
        );
        assert_eq!(
            worded(Lang::En, &with(1, Some(5), &[]), false, Some((4, 2))),
            "✓ cargo test · 1 file +4 −2, inside #6"
        );
        assert_eq!(
            worded(Lang::En, &with(3, Some(5), &[]), false, Some((4, 2))),
            "✓ cargo test · 3 files +4 −2, all inside #6"
        );
        assert_eq!(
            worded(Lang::En, &with(3, Some(5), &["x"]), false, Some((4, 2))),
            "✓ cargo test · 3 files +4 −2 · 1 outside #6: x"
        );
        assert_eq!(
            worded(Lang::En, &with(1, None, &[]), false, Some((4, 2))),
            "✓ cargo test · 1 file +4 −2"
        );
        // A name is cleaned on its way to the screen.
        assert_eq!(
            said(&with(1, Some(5), &["src/\u{1b}[31mx.rs"])),
            "✓ cargo test · 1 file · 1 outside #6: src/ [31mx.rs"
        );
    }

    #[test]
    fn the_verdict_says_what_it_overlaps_what_does_not_fit_and_what_lonca_runs() {
        let overlap = |card, files: &[&str]| Overlap {
            with: Other::Change { agent: 2, card },
            files: files.iter().map(|f| f.to_string()).collect(),
        };
        let said = |v: &Verdict| worded(Lang::En, v, true, None);
        let alone = Verdict {
            others: 2,
            ..a_verdict()
        };
        assert_eq!(
            said(&alone),
            "✓ ready to approve · ✓ cargo test · 2 files, both inside #6 · \
             no overlap with other changes"
        );
        assert_eq!(
            said(&a_verdict()),
            "✓ ready to approve · ✓ cargo test · 2 files, both inside #6",
            "with no other change waiting, nothing to say"
        );
        let v = Verdict {
            ready: false,
            others: 2,
            overlaps: vec![
                overlap(Some(50), &["Money.cs"]),
                overlap(None, &["a", "b", "c", "d", "e"]),
            ],
            ..a_verdict()
        };
        assert_eq!(
            said(&v),
            "! look first · ✓ cargo test · 2 files, both inside #6 · \
             overlaps #51 on Money.cs · overlaps ? on a, b, c +2"
        );
        let spans = verdict_spans(Lang::En, &v, true, None);
        assert_eq!(spans[6].style, fg(pal().bad));
        assert_eq!(
            worded(Lang::Tr, &v, false, None),
            "✓ cargo test · 2 dosya, ikisi de #6 içinde · #51 ile Money.cs üzerinde çakışıyor · \
             ? ile a, b, c +2 üzerinde çakışıyor"
        );
        // A proposal waiting in Lonca that no change here shows, by its
        // number and who made it (cleaned); a base behind trunk is a fact.
        let v = Verdict {
            ready: false,
            others: 1,
            overlaps: vec![Overlap {
                with: Other::Proposal {
                    seq: 12,
                    name: "ayse\u{7}".into(),
                },
                files: vec!["Money.cs".into()],
            }],
            stale: true,
            ..a_verdict()
        };
        assert_eq!(
            said(&v),
            format!(
                "! look first · ✓ cargo test · 2 files, both inside #6 · \
                 overlaps proposal 12 ({}) on Money.cs · \
                 based before trunk, merged after your yes",
                clean("ayse\u{7}")
            )
        );
        let spans = verdict_spans(Lang::En, &v, true, None);
        assert_eq!(spans[6].style, fg(pal().bad));
        assert_eq!(spans[8].style, dim());
        assert_eq!(
            worded(Lang::Tr, &v, false, None),
            format!(
                "✓ cargo test · 2 dosya, ikisi de #6 içinde · \
                 12 numaralı öneri ({}) ile Money.cs üzerinde çakışıyor · \
                 temeli gövdenin gerisinde, evetinden sonra birleştirilir",
                clean("ayse\u{7}")
            )
        );
        let v = Verdict {
            ready: false,
            clashes: true,
            partial: true,
            ..a_verdict()
        };
        assert_eq!(
            said(&v),
            "! look first · ✓ cargo test · 2 files, both inside #6 · \
             does not fit your checkout · the patch is not whole"
        );
        let spans = verdict_spans(Lang::En, &v, true, None);
        assert_eq!(spans[6].style, fg(pal().bad));
        assert_eq!(spans[8].style, fg(pal().warn));
        assert_eq!(
            worded(Lang::Tr, &v, true, None),
            "! önce bak · ✓ cargo test · 2 dosya, ikisi de #6 içinde · \
             çalışma kopyana uymuyor · yama tam değil"
        );
        let runs = Verdict {
            verifies: Some(Runs::These(vec!["build".into(), "api".into()])),
            ..a_verdict()
        };
        assert_eq!(
            said(&runs),
            "✓ ready to approve · ✓ cargo test · 2 files, both inside #6 · \
             after your yes Lonca runs: build, api"
        );
        assert_eq!(verdict_spans(Lang::En, &runs, true, None)[6].style, dim());
        let nothing = Verdict {
            verifies: Some(Runs::These(Vec::new())),
            ..a_verdict()
        };
        assert_eq!(
            said(&nothing),
            "✓ ready to approve · ✓ cargo test · 2 files, both inside #6 · \
             nothing runs after your yes"
        );
        assert_eq!(
            verdict_spans(Lang::En, &nothing, true, None)[6].style,
            fg(pal().warn)
        );
        assert_eq!(
            worded(Lang::Tr, &runs, true, None),
            "✓ onaya hazır · ✓ cargo test · 2 dosya, ikisi de #6 içinde · \
             evetinden sonra Lonca şunları çalıştırır: build, api"
        );
        assert!(
            worded(Lang::Tr, &nothing, true, None).ends_with("evetinden sonra hiçbir şey çalışmaz")
        );
        let unsaid = Verdict {
            verifies: Some(Runs::Unsaid),
            ..a_verdict()
        };
        assert_eq!(
            said(&unsaid),
            "✓ ready to approve · ✓ cargo test · 2 files, both inside #6 · \
             Lonca did not say what runs after your yes"
        );
        assert_eq!(
            verdict_spans(Lang::En, &unsaid, true, None)[6].style,
            fg(pal().warn)
        );
        assert!(
            worded(Lang::Tr, &unsaid, true, None)
                .ends_with("Lonca evetinden sonra neyin çalışacağını söylemedi")
        );
        // The card's checks (record 127): what the yes asks for runs too.
        let asks = Verdict {
            verifies: Some(Runs::These(vec!["build".into()])),
            asks: vec!["api".into(), "e2e".into()],
            ..a_verdict()
        };
        assert_eq!(
            said(&asks),
            "✓ ready to approve · ✓ cargo test · 2 files, both inside #6 · \
             after your yes Lonca runs: build, api, e2e · the card also asks for api, e2e"
        );
        let spans = verdict_spans(Lang::En, &asks, true, None);
        assert_eq!((spans.len(), spans[8].style), (9, dim()));
        assert_eq!(
            worded(Lang::Tr, &asks, true, None),
            "✓ onaya hazır · ✓ cargo test · 2 dosya, ikisi de #6 içinde · \
             evetinden sonra Lonca şunları çalıştırır: build, api, e2e · \
             kart ayrıca şunları da istiyor: api, e2e"
        );
        let only = Verdict {
            verifies: Some(Runs::These(Vec::new())),
            asks: vec!["api".into()],
            ..a_verdict()
        };
        assert!(
            said(&only).ends_with("after your yes Lonca runs: api · the card also asks for api"),
            "{}",
            said(&only)
        );
        let unsaid = Verdict {
            verifies: Some(Runs::Unsaid),
            asks: vec!["api".into()],
            ..a_verdict()
        };
        assert!(
            said(&unsaid).ends_with(
                "Lonca did not say what runs after your yes · the card also asks for api"
            )
        );
        // A check the policy dropped, last and loud (#390).
        let dropped = Verdict {
            ready: false,
            verifies: Some(Runs::These(vec!["build".into()])),
            asks: vec!["api".into()],
            unknown: vec!["e2e".into(), "smoke".into()],
            ..a_verdict()
        };
        assert_eq!(
            said(&dropped),
            "! look first · ✓ cargo test · 2 files, both inside #6 · \
             after your yes Lonca runs: build, api · the card also asks for api · \
             Lonca's policy no longer has e2e, smoke: :checks changes what the card asks for"
        );
        let spans = verdict_spans(Lang::En, &dropped, true, None);
        assert_eq!((spans.len(), spans[10].style), (11, fg(pal().bad)));
        assert!(worded(Lang::Tr, &dropped, true, None).ends_with(
            "Lonca'nın politikasında artık e2e, smoke yok: :checks kartın istediklerini \
                 değiştirir"
        ));
    }

    #[test]
    fn the_verdict_line_wraps_at_the_width_and_keeps_every_part() {
        let v = Verdict {
            others: 1,
            verifies: Some(Runs::These(vec!["build".into(), "api".into()])),
            ..a_verdict()
        };
        let one = verdict_lines(Lang::En, &v, 200);
        assert_eq!(one.len(), 1);
        let rows = verdict_lines(Lang::En, &v, 40);
        assert!(rows.len() >= 3, "{}", texts(&rows).join("|"));
        for r in &rows {
            assert!(r.to_string().chars().count() <= 40, "{r}");
        }
        let joined = texts(&rows)
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(joined, texts(&one)[0]);
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

    #[test]
    fn what_the_change_cost_stands_dim_above_its_files_once_known() {
        let mut w = crate::live::testing::team(1);
        assert!(spend_line(Lang::En, &w, None).is_none());
        let s = Spend {
            cost: 8_400,
            ticks: 144,
            rounds: 1,
            unpriced: false,
        };
        let line = spend_line(Lang::En, &w, Some(s)).unwrap();
        assert_eq!(
            texts(std::slice::from_ref(&line)),
            ["cost · $0.84 · 12 min"]
        );
        assert_eq!(line.spans[0].style, dim());
        let line = spend_line(Lang::Tr, &w, Some(s)).unwrap();
        assert_eq!(texts(&[line]), ["maliyet · $0.84 · 12 dk"]);
        // A plan's figure is API-equivalent; a part never priced makes it
        // what it came to at least (record 132).
        w.estimated = true;
        let s = Spend {
            unpriced: true,
            ..s
        };
        let line = spend_line(Lang::En, &w, Some(s)).unwrap();
        assert_eq!(texts(&[line]), ["cost · ≈$0.84+? · 12 min"]);
    }

    #[test]
    fn the_check_s_lines_are_a_failure_s_output_or_why_it_has_not_run() {
        let lines = |check: &Check| check_lines(Lang::En, check);
        let ran = |ok: bool, lines: &[&str]| {
            Check::Ran(Checked {
                ok,
                end: "exit 1, 3.0 s".into(),
                lines: lines.iter().map(|l| l.to_string()).collect(),
            })
        };
        assert!(lines(&Check::None).is_empty());
        assert!(
            lines(&ran(true, &["fine"])).is_empty(),
            "a check that passed is the verdict's"
        );
        let bad = lines(&ran(false, &["a", "TOKEN=abc b"]));
        assert_eq!(
            texts(&bad),
            ["a", "[***] b"],
            "how it ended is the verdict's"
        );
        assert_eq!(bad[0].spans[0].content, "  a", "the output lines stand in");
        assert_eq!(bad[0].spans[0].style, dim());
        assert!(lines(&ran(false, &[])).is_empty());
        let running = lines(&Check::Running);
        assert_eq!(texts(&running), ["● the check runs on this change…"]);
        assert_eq!(running[0].spans[0].style, dim());
        let asked = lines(&Check::Asked);
        assert_eq!(
            texts(&asked),
            ["▶ the repository's check asks to run: it is in TO DECIDE"]
        );
        assert_eq!(asked[0].spans[0].style, fg(pal().warn));
        let failed = lines(&Check::Failed("TOKEN=abc gone".into()));
        assert_eq!(texts(&failed), ["check did not run: [***] gone"]);
        assert_eq!(failed[0].spans[0].style, fg(pal().warn));
        assert_eq!(
            texts(&check_lines(Lang::Tr, &Check::Running)),
            ["● kontrol bu değişiklikte çalışıyor…"]
        );
    }

    fn a_proposal() -> StoreProposal {
        StoreProposal {
            seq: 5,
            change: "c5".into(),
            why: vec!["unobserved_session".into(), "approvals=1".into()],
            redacted: Vec::new(),
            cut: false,
            verifies: Runs::default(),
            on_trunk: true,
        }
    }

    fn said(lines: &[Line<'static>]) -> Vec<String> {
        (lines.iter()).map(|l| l.to_string()).collect()
    }

    #[test]
    fn what_runs_after_the_persons_yes_is_the_verdict_s_not_the_proposal_line_s() {
        let p = StoreProposal {
            verifies: Runs::These(vec!["build".into(), "tests".into()]),
            ..a_proposal()
        };
        assert_eq!(proposal_lines(Lang::En, &p).len(), 1);
    }

    #[test]
    fn a_proposal_says_its_number_and_why_it_waits() {
        let lines = proposal_lines(Lang::En, &a_proposal());
        assert_eq!(
            said(&lines),
            [" needs your yes: nobody watched this session's tools, approvals=1 · proposal 5"]
        );
        assert_eq!(lines[0].spans[0].style, dim());
    }

    #[test]
    fn what_a_patch_does_not_show_is_loud() {
        let p = StoreProposal {
            redacted: vec!["a.env".into(), "b.env".into()],
            cut: true,
            verifies: Runs::default(),
            ..a_proposal()
        };
        let lines = proposal_lines(Lang::En, &p);
        assert_eq!(said(&lines)[1], " left out, redacted: a.env, b.env");
        assert_eq!(said(&lines)[2], Lang::En.text().diff_cut);
        assert_eq!(lines[1].spans[0].style, fg(pal().warn));
        assert_eq!(lines[2].spans[0].style, fg(pal().warn));
    }
}
