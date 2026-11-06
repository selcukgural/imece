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
use crate::paths;
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
        for (i, l) in patch.lines().enumerate() {
            if l.starts_with("diff --git ") {
                let n = file.map_or(0, |n| n + 1);
                file = Some(n);
                shown.lines.push(i);
                if folds.folded(n) {
                    shown.hidden.push((i, 0));
                }
            } else if file.is_some_and(|n| folds.folded(n)) {
                if let Some(last) = shown.hidden.last_mut() {
                    last.1 += 1;
                }
            } else {
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
        let block = Block::bordered()
            .title(fill(t.diff_title, &[&a.name, "", "", "", ""]))
            .border_style(fg(pal().agent));
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
    let copy = app.diff_copy_of(id);
    // Choosing lines to copy: the title counts them and says the keys.
    let title = match copy {
        Some(copy) => fill(t.diff_copy_title, &[&copy.len().to_string()]),
        None => fill(
            t.diff_title,
            &[
                &a.name,
                &c.files.len().to_string(),
                &added.to_string(),
                &removed.to_string(),
                &c.title,
            ],
        ),
    };
    let block = Block::bordered().title(title).border_style(fg(pal().agent));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let mut ran = check_lines(app.lang, &c.check);
    ran.extend(run_lines(app.lang, &c.runs));
    if let Some(clashes) = w.clashes_of(id) {
        ran.insert(0, clash_line(app.lang, clashes, &a.name));
    }
    if let Some(proposal) = w.proposal_of(id) {
        ran.splice(0..0, proposal_lines(app.lang, proposal));
    }
    let review = review_lines(
        app.lang,
        w.review_of(id),
        app.reviewing(id),
        app.review_whole,
        usize::from(inner.width),
        usize::from(inner.height / 2),
    );
    let mut files = file_lines(app.lang, &c.files);
    if let Some(card) = a.card
        && let Some(outside) = outside_line(app.lang, &c.files, &w.cards[card])
    {
        files.insert(0, outside);
    }
    if let Some(split) = kinds_line(app.lang, &c.files) {
        files.insert(0, split);
    }
    if let Some(cost) = spend_line(app.lang, c.spend) {
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
        let mut row = Line::from(Span::styled(clean(l), line_style(l)));
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

/// The files of the change its worker's `card` does not own (record
/// 114), under the kinds line: `2 outside #12: src/b.rs, Cargo.toml`, in
/// the warning colour; nothing when the card owns no paths or every file
/// is in them. The card may be done already: it is still the one the
/// change was made for.
fn outside_line(lang: Lang, files: &[ChangedFile], card: &Card) -> Option<Line<'static>> {
    let outside: Vec<&str> = files
        .iter()
        .filter(|f| paths::strays(&card.paths, &f.path))
        .map(|f| f.path.as_str())
        .collect();
    if outside.is_empty() {
        return None;
    }
    let t = lang.text();
    Some(Line::from(Span::styled(
        format!(
            " {}",
            fill(
                t.diff_outside,
                &[
                    &outside.len().to_string(),
                    &card_ref(card.id),
                    &clean(&outside.join(", ")),
                ],
            )
        ),
        fg(pal().warn),
    )))
}

/// Which proposal in Lonca a change is (record 120): its number, why it
/// waits, how many older ones of the worker's it holds; then what the
/// patch below does not show, loud.
pub(crate) fn proposal_lines(lang: Lang, p: &StoreProposal) -> Vec<Line<'static>> {
    let t = lang.text();
    let mut said = fill(t.diff_proposal, &[&p.seq.to_string(), &p.why.join(", ")]);
    if !p.older.is_empty() {
        said.push_str(&fill(t.diff_proposal_older, &[&p.older.len().to_string()]));
    }
    let verifies = match p.verifies.join(", ") {
        names if names.is_empty() => t.diff_verifies_none.to_string(),
        names => fill(t.diff_verifies, &[&names]),
    };
    let mut lines = vec![
        Line::from(Span::styled(said, dim())),
        Line::from(Span::styled(verifies, dim())),
    ];
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
fn spend_line(lang: Lang, spend: Option<Spend>) -> Option<Line<'static>> {
    let s = spend?;
    Some(Line::from(Span::styled(
        format!(
            " {}",
            fill(lang.text().diff_spend, &[&words::spend(lang, &s)])
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
) -> Vec<Line<'static>> {
    let t = lang.text();
    let Some(r) = review else {
        if !under_way {
            return Vec::new();
        }
        let said = fill(t.review_under_way, &[CLAUDE]);
        return vec![Line::from(Span::styled(said, dim()))];
    };
    let head = fill(t.review_head, &[CLAUDE, &r.model, &dollars(r.cost)]);
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
        Check::None => Vec::new(),
        Check::Ran(d) if d.ok => vec![Line::from(Span::styled(
            fill(t.check_ok, &[&d.end]),
            fg(pal().ok),
        ))],
        Check::Ran(d) => {
            let mut out = vec![Line::from(Span::styled(
                fill(t.check_bad, &[&d.end]),
                fg(pal().bad),
            ))];
            out.extend(
                (d.lines.iter())
                    .map(|l| Line::from(Span::styled(format!("  {}", clean(l)), dim()))),
            );
            out
        }
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
        let none = review_lines(Lang::En, None, false, false, 40, 30);
        assert!(none.is_empty(), "nothing asked: nothing");
        let asked = review_lines(Lang::En, None, true, false, 40, 30);
        assert_eq!(texts(&asked), ["✎ claude reads this change…"]);
        let lines = |n: usize| {
            (1..=n)
                .map(|i| format!("l{i}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let six = a_review(&lines(REVIEW_ROWS));
        let got = texts(&review_lines(Lang::En, Some(&six), false, false, 40, 30));
        assert_eq!(got[0], "✎ review · claude · default · $0.00");
        assert_eq!(got.len(), 1 + REVIEW_ROWS, "all of it, nothing counted");
        assert_eq!(got[REVIEW_ROWS], "l6");
        let nine = a_review(&lines(REVIEW_ROWS + 3));
        let got = texts(&review_lines(Lang::En, Some(&nine), true, false, 40, 30));
        assert_eq!(got.len(), 2 + REVIEW_ROWS, "{got:?}");
        assert_eq!(got.last().unwrap(), "… +3 lines · R shows all");
        // Whole: as many as the room leaves (its head and the count line
        // aside), never fewer than folded.
        let got = texts(&review_lines(Lang::En, Some(&nine), false, true, 40, 30));
        assert_eq!(got.len(), 1 + REVIEW_ROWS + 3, "all of it");
        let got = texts(&review_lines(Lang::En, Some(&nine), false, true, 40, 10));
        assert_eq!(got.len(), 1 + 8 + 1, "{got:?}");
        assert_eq!(got.last().unwrap(), "… +1 lines · R shows all");
        let got = texts(&review_lines(Lang::En, Some(&nine), false, true, 40, 4));
        assert_eq!(got.len(), 2 + REVIEW_ROWS, "folded at least");
        // Long lines wrap at the width and count as rows; text is cleaned.
        let long = a_review("abcdefghij klmnopqrst\u{1b}[2J");
        let got = texts(&review_lines(Lang::En, Some(&long), false, false, 11, 30));
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
    const TWO: &str = "note\ndiff --git a/x b/x\n--- a/x\n+++ b/x\n+one\n\
                       diff --git a/y b/y\n--- a/y\n+++ b/y\n+two\n+three\n";

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
            [
                "diff --git a/x b/x  ▸ 3 lines",
                "diff --git a/y b/y  ▸ 4 lines"
            ],
            "from the line at hand on"
        );
        let one = "diff --git a/x b/x\n+one\ndiff --git a/y b/y\n+two\n";
        let all = Shown::of(one, &Folds::every(true));
        let rows = patch_rows(Lang::Tr, one, &all, 0, &[], &[], None, 8, None);
        assert_eq!(rows[0].to_string(), "diff --git a/x b/x  ▸ 1 satır");
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
        let copy = Copying { anchor: 4, at: 2 };
        let rows = open_rows(PATCH, 0, &[], &[], Some(copy), 8, None);
        assert_eq!(rows[0].style, Style::new(), "not the line at hand");
        assert_eq!(rows[1].style, Style::new());
        assert_eq!(
            rows[2].style,
            changed().add_modifier(Modifier::BOLD),
            "the cursor"
        );
        assert_eq!(rows[3].style, changed());
        assert_eq!(rows[4].style, changed());
        assert_eq!(rows[5].style, Style::new());
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

    #[test]
    fn what_the_change_cost_stands_dim_above_its_files_once_known() {
        assert!(spend_line(Lang::En, None).is_none());
        let s = Spend {
            cost: 8_400,
            ticks: 144,
            rounds: 1,
        };
        let line = spend_line(Lang::En, Some(s)).unwrap();
        assert_eq!(
            texts(std::slice::from_ref(&line)),
            ["cost · $0.84 · 12 min"]
        );
        assert_eq!(line.spans[0].style, dim());
        let line = spend_line(Lang::Tr, Some(s)).unwrap();
        assert_eq!(texts(&[line]), ["maliyet · $0.84 · 12 dk"]);
    }

    #[test]
    fn the_check_s_lines_say_how_it_stands_in_its_colour() {
        let lines = |check: &Check| check_lines(Lang::En, check);
        let ran = |ok: bool, lines: &[&str]| {
            Check::Ran(Checked {
                ok,
                end: if ok {
                    "exit 0, 41.2 s"
                } else {
                    "exit 1, 3.0 s"
                }
                .into(),
                lines: lines.iter().map(|l| l.to_string()).collect(),
            })
        };
        assert!(lines(&Check::None).is_empty());
        let ok = lines(&ran(true, &[]));
        assert_eq!(texts(&ok), ["✓ check · exit 0, 41.2 s"]);
        assert_eq!(ok[0].spans[0].style, fg(pal().ok));
        let bad = lines(&ran(false, &["a", "TOKEN=abc b"]));
        assert_eq!(texts(&bad), ["✗ check · exit 1, 3.0 s", "a", "[***] b"]);
        assert_eq!(bad[0].spans[0].style, fg(pal().bad));
        assert_eq!(bad[1].spans[0].content, "  a", "the output lines stand in");
        assert_eq!(bad[1].spans[0].style, dim());
        let running = lines(&Check::Running);
        assert_eq!(texts(&running), ["◌ the check runs on this change…"]);
        assert_eq!(running[0].spans[0].style, dim());
        let asked = lines(&Check::Asked);
        assert_eq!(
            texts(&asked),
            ["▶ the repository's check waits for your yes in WAITING FOR YOU"]
        );
        assert_eq!(asked[0].spans[0].style, fg(pal().warn));
        let failed = lines(&Check::Failed("TOKEN=abc gone".into()));
        assert_eq!(texts(&failed), ["check did not run: [***] gone"]);
        assert_eq!(failed[0].spans[0].style, fg(pal().warn));
        assert_eq!(
            texts(&check_lines(Lang::Tr, &ran(true, &[]))),
            ["✓ kontrol · exit 0, 41.2 s"]
        );
    }
    #[test]
    fn the_files_a_card_does_not_own_are_counted_and_named_under_the_kinds() {
        let file = |path: &str| ChangedFile {
            path: path.into(),
            added: 1,
            removed: 0,
            binary: false,
        };
        let files = [file("src/a.rs"), file("src/b.rs"), file("Cargo.toml")];
        let mut card = Card {
            id: 11,
            project: 0,
            title: "x".into(),
            owner: Some(1),
            column: Column::Done,
            story: Vec::new(),
            symbols: Vec::new(),
            paths: Vec::new(),
            started: 0,
            finished: 0,
        };
        assert!(
            outside_line(Lang::En, &files, &card).is_none(),
            "no paths, no bounds"
        );
        card.paths = vec![Owned::new("src/a.rs"), Owned::shared("Cargo.toml")];
        let line = outside_line(Lang::En, &files, &card).unwrap();
        assert_eq!(line.to_string(), " 1 outside #12: src/b.rs");
        assert_eq!(line.spans[0].style, fg(pal().warn));
        card.paths = vec![Owned::new("docs")];
        assert_eq!(
            outside_line(Lang::Tr, &files, &card).unwrap().to_string(),
            " 3 kartın dışında (#12): src/a.rs, src/b.rs, Cargo.toml"
        );
        card.paths = vec![Owned::new("src"), Owned::new("Cargo.toml")];
        assert!(
            outside_line(Lang::En, &files, &card).is_none(),
            "all inside"
        );
        assert!(outside_line(Lang::En, &[], &card).is_none());
        // A file name is cleaned on its way to the screen.
        card.paths = vec![Owned::new("docs")];
        let odd = [file("src/\u{1b}[31mx.rs")];
        assert_eq!(
            outside_line(Lang::En, &odd, &card).unwrap().to_string(),
            " 1 outside #12: src/ [31mx.rs"
        );
    }

    fn a_proposal() -> StoreProposal {
        StoreProposal {
            seq: 5,
            change: "c5".into(),
            older: Vec::new(),
            why: vec!["unobserved_session".into(), "approvals=1".into()],
            redacted: Vec::new(),
            cut: false,
            verifies: vec![],
        }
    }

    fn said(lines: &[Line<'static>]) -> Vec<String> {
        (lines.iter()).map(|l| l.to_string()).collect()
    }

    #[test]
    fn a_proposal_says_what_runs_after_the_persons_yes() {
        let p = StoreProposal {
            verifies: vec!["build".into(), "test".into()],
            ..a_proposal()
        };
        let lines = proposal_lines(Lang::En, &p);
        assert_eq!(said(&lines)[1], " after your yes Lonca runs: build, test");
        let lines = proposal_lines(Lang::Tr, &p);
        assert!(
            said(&lines)[1].contains("build, test"),
            "{:?}",
            said(&lines)
        );
    }

    #[test]
    fn a_proposal_says_its_number_and_why_it_waits() {
        let lines = proposal_lines(Lang::En, &a_proposal());
        assert_eq!(
            said(&lines),
            [
                " proposal 5 · waits: unobserved_session, approvals=1",
                " nothing runs after your yes: no build or test in lonca.toml"
            ]
        );
        assert_eq!(lines[0].spans[0].style, dim());
    }

    #[test]
    fn a_proposal_counts_the_older_ones_it_holds() {
        let p = StoreProposal {
            older: vec![2, 3],
            ..a_proposal()
        };
        let line = proposal_lines(Lang::En, &p);
        assert!(
            said(&line)[0].ends_with(" · holds 2 older"),
            "{:?}",
            said(&line)
        );
    }

    #[test]
    fn what_a_patch_does_not_show_is_loud() {
        let p = StoreProposal {
            redacted: vec!["a.env".into(), "b.env".into()],
            cut: true,
            verifies: vec![],
            ..a_proposal()
        };
        let lines = proposal_lines(Lang::En, &p);
        assert_eq!(said(&lines)[2], " left out, redacted: a.env, b.env");
        assert_eq!(said(&lines)[3], Lang::En.text().diff_cut);
        assert_eq!(lines[2].spans[0].style, fg(pal().warn));
        assert_eq!(lines[3].spans[0].style, fg(pal().warn));
    }
}
