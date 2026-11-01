//! Drawing. One rule above all: the screen is calm unless something needs
//! the person. Motion and colour are signals, never decoration, and a status
//! never relies on colour alone (every one has its own glyph or word).

mod board;
mod card;
mod code;
mod diff;
pub(crate) mod export;
mod focus;
mod overlay;
pub mod pick;
pub(crate) mod salon;
pub(crate) mod words;

use std::num::NonZeroU16;

use ratatui::Frame;
use ratatui::buffer::{Buffer, CellDiffOption};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::{App, FLASH, Noting, Rung, View, Zoom};
use crate::i18n::{Lang, clean, fill};
use crate::model::*;
pub(crate) use crate::theme::pal;
pub(crate) use diff::{QUOTE_LINES, copy_patch, file_starts, hunk_at, notes_message, place, quote};
pub(crate) use overlay::{chosen_text, fold, in_view, talk_notes_message, thread_lines, word_step};
pub(crate) use salon::{card_rows, journal_rows, reply_jump, thread_of, turn_starts};

pub fn draw(f: &mut Frame, app: &App) {
    paint(f, f.area());
    // The completion menu takes a row above the footer while it is shown.
    let menu_rows = u16::from(app.typing && app.menu.is_some());
    let [head, body, menu, foot] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(menu_rows),
        Constraint::Length(input_rows(app, f.area().height)),
    ])
    .areas(f.area());
    header(f, head, app);
    match app.zoom {
        Zoom::Focus(id) => focus::draw(f, body, app, id),
        Zoom::Trace(id) => focus::trace(f, body, app, id),
        Zoom::Diff(id) => diff::draw(f, body, app, id),
        Zoom::None => match app.view {
            View::Salon => salon::draw(f, body, app),
            View::Board => board::draw(f, body, app),
            View::Code => code::draw(f, body, app),
        },
    }
    if let Some(m) = &app.menu
        && app.typing
    {
        completions(f, menu, m);
    }
    footer(f, foot, app);
    if let Some(th) = &app.thread {
        overlay::thread(f, app, th);
    }
    if let Some(p) = &app.solo_pick {
        overlay::solo_pick(f, app, p);
    } else if app.help {
        overlay::help(f, app);
    } else if let Some(d) = &app.digest {
        overlay::digest(f, app, d);
    } else if let Some(l) = &app.left_off {
        overlay::left_off(f, app, l);
    } else if app.leaving {
        overlay::leaving(f, app);
    }
}

/// The theme's own background and text over `area`, when it has them;
/// the terminal's stay otherwise.
pub(crate) fn paint(f: &mut Frame, area: Rect) {
    let p = pal();
    if let Some(style) = backdrop(p.bg, p.fg) {
        f.render_widget(Block::new().style(style), area);
    }
}

/// The style to lay under the screen for a theme with a background or
/// text colour of its own; none when it leaves both to the terminal.
fn backdrop(bg: Color, fg: Color) -> Option<Style> {
    (bg != Color::Reset || fg != Color::Reset).then(|| Style::new().bg(bg).fg(fg))
}

/// Empties `area` for a window over the screen, in the theme's colours.
pub(crate) fn clear(f: &mut Frame, area: Rect) {
    f.render_widget(Clear, area);
    paint(f, area);
}

/// Dims everything drawn so far but `keep`, so a window laid over the
/// screen reads as the thing in front and what shows around it (the
/// salon's tree, the journal's clipped line ends) as the background.
pub(crate) fn shade(f: &mut Frame, keep: Rect) {
    let area = f.area();
    let buf = f.buffer_mut();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if !keep.contains(ratatui::layout::Position { x, y }) {
                buf[(x, y)].modifier.insert(Modifier::DIM);
            }
        }
    }
}

pub(crate) fn dim() -> Style {
    let p = pal();
    if p.mono {
        Style::new().add_modifier(Modifier::DIM)
    } else {
        Style::new().fg(p.dim)
    }
}

/// A panel's border when it is not the focused one.
pub(crate) fn border() -> Style {
    let p = pal();
    if p.mono {
        dim()
    } else {
        Style::new().fg(p.border)
    }
}

/// Bold text on a coloured badge: the brand, or what waits for the person.
fn badge(bg: Color) -> Style {
    let p = pal();
    if p.mono {
        // Two calls, not `REVERSED | BOLD`: an operator there has an
        // equivalent mutant (`^` on disjoint flags) no test can tell apart.
        Style::new()
            .add_modifier(Modifier::REVERSED)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new()
            .fg(p.on_badge)
            .bg(bg)
            .add_modifier(Modifier::BOLD)
    }
}

pub(crate) fn brand() -> Style {
    badge(pal().accent)
}

pub(crate) fn attention() -> Style {
    badge(pal().warn)
}

/// A key in a hint line.
pub(crate) fn key_style() -> Style {
    let p = pal();
    if p.mono {
        Style::new().add_modifier(Modifier::REVERSED)
    } else {
        Style::new().fg(p.on_key).bg(p.key)
    }
}

/// The row that just changed.
pub(crate) fn changed() -> Style {
    let p = pal();
    if p.mono {
        Style::new().add_modifier(Modifier::BOLD)
    } else {
        Style::new().bg(p.selection)
    }
}

pub(crate) fn fg(c: Color) -> Style {
    Style::new().fg(c)
}

pub(crate) fn bold(c: Color) -> Style {
    Style::new().fg(c).add_modifier(Modifier::BOLD)
}

/// 256-colour indexes, so terminals without true colour show them too.
pub(crate) fn vendor_color(v: Vendor) -> Color {
    pal().vendors[v as usize]
}

/// An agent's own colour in conversations, fixed by its id so a speaker
/// keeps it for the whole run; none of them a status colour.
pub(crate) fn speaker_color(id: AgentId) -> Color {
    let s = &pal().speakers;
    s[id % s.len()]
}

fn party_span(lang: Lang, w: &World, p: Party) -> Span<'static> {
    match p {
        Party::Agent(id) => Span::styled(clean(&w.agents[id].name), bold(speaker_color(id))),
        Party::Person => Span::styled(lang.text().you, bold(pal().strong)),
    }
}

/// A conversation line: `from → to [kind]: text`, each speaker in its own
/// colour.
pub(crate) fn said_spans(lang: Lang, w: &World, s: &words::Said) -> Vec<Span<'static>> {
    let mut spans = vec![
        party_span(lang, w, s.from),
        Span::styled(" → ", dim()),
        party_span(lang, w, s.to),
    ];
    if let Some(k) = s.kind {
        spans.push(Span::styled(
            format!(" [{}]", lang.text().kinds[k as usize]),
            dim(),
        ));
    }
    if s.re.is_some() {
        spans.push(Span::styled(" ↩", dim()));
    }
    spans.push(Span::raw(format!(": {}", plain(s.text))));
    spans
}

pub(crate) fn glyph(a: &Agent) -> (&'static str, Color) {
    if a.role == Role::Leader {
        return ("◆", pal().plan);
    }
    match a.status {
        Status::Idle => ("○", pal().dim),
        Status::Working => ("●", pal().ok),
        Status::Asking { .. } => ("◐", pal().warn),
        Status::Blocked => ("✕", pal().bad),
        Status::WaitingHuman => ("!", pal().warn),
        Status::Researching { .. } => ("◇", pal().research),
        Status::Stopped => ("■", pal().stopped),
        Status::Asleep => ("◦", pal().dim),
    }
}

/// The glyph of agent `a`'s row on the state ladder: a leader always `◆`;
/// stuck as its status says (`✕` blocked, `■` stopped); `!` when something
/// of it waits for the person, whatever it does meanwhile; `✓` done, not
/// seen; `◌` while its session starts; else its status's own glyph (`●`
/// `◐` `◇` working, `○` `◦` idle). Each a shape of its own.
pub(crate) fn rung_glyph(a: &Agent, rung: Rung) -> (&'static str, Color) {
    if a.role == Role::Leader {
        return ("◆", pal().plan);
    }
    match rung {
        Rung::Waiting => ("!", pal().warn),
        Rung::Done => ("✓", pal().ok),
        Rung::Starting => ("◌", pal().ok),
        Rung::Stuck | Rung::Working | Rung::Idle => glyph(a),
    }
}

pub(crate) fn taint_span(a: &Agent) -> Span<'static> {
    match a.taint {
        Taint::Clean => Span::raw(" "),
        Taint::Soft => Span::styled("~", fg(pal().warn)),
        Taint::Full => Span::styled("≈", fg(pal().bad)),
    }
}

pub(crate) fn entry_icon(k: EntryKind) -> (&'static str, Style) {
    match k {
        EntryKind::Assign => ("→", dim()),
        EntryKind::Plan => ("◆", fg(pal().plan)),
        EntryKind::Talk => ("⇄", fg(pal().info)),
        EntryKind::Assumption => ("⚑", fg(pal().warn)),
        EntryKind::Blocked => ("✕", fg(pal().bad)),
        EntryKind::Research => ("◇", fg(pal().research)),
        EntryKind::Sensitive => ("!", bold(pal().warn)),
        EntryKind::Merged => ("✓", fg(pal().ok)),
        EntryKind::Failed => ("✕", fg(pal().stopped)),
        EntryKind::Person => ("›", bold(pal().strong)),
    }
}

/// A row that says which day the lines under it are of: drawn where the
/// journal or a conversation passes from one day to another, and above
/// its first line when that is not of today. The date needs no words.
pub(crate) fn date_row(day: u64) -> Line<'static> {
    Line::from(Span::styled(format!("─ {}", date(day)), dim()))
}

/// Under a reply in a conversation: what it answers (that message's
/// first line), or that the journal no longer has it. None for a message
/// that replies to nothing.
pub(crate) fn re_row(lang: Lang, w: &World, s: &words::Said) -> Option<Line<'static>> {
    let re = s.re?;
    let t = lang.text();
    let text = match w.message(re) {
        Some((_, _, text)) => {
            let first = text.lines().next().unwrap_or_default();
            fill(t.re_line, &[&re.to_string(), first])
        }
        None => fill(t.re_gone, &[&re.to_string()]),
    };
    Some(Line::from(Span::styled(format!("  {text}"), dim())))
}

/// One journal line: time, icon, the event in words.
pub(crate) fn entry_line(app: &App, e: &Entry) -> Line<'static> {
    let w = &app.world;
    let (icon, style) = entry_icon(e.event.kind());
    let mut spans = vec![
        Span::styled(format!("{} ", clock(e.tick)), dim()),
        Span::styled(format!("{icon} "), style),
    ];
    if w.projects.len() > 1 {
        let p = w.agents[e.event.agent()].project;
        spans.push(Span::styled(format!("{} ", w.projects[p].name), dim()));
    }
    match words::said(&e.event) {
        Some(said) => spans.extend(said_spans(app.lang, w, &said)),
        None => spans.push(Span::raw(words::event(app.lang, w, &e.event))),
    }
    Line::from(spans)
}

/// What shows on a line the person marked (`m`).
pub(crate) const MARK: &str = "✱";

/// Journal line `line` (as [`entry_line`] draws it) with the mark in the
/// space after its clock, so marked and unmarked lines stay aligned.
pub(crate) fn bookmarked(mut line: Line<'static>) -> Line<'static> {
    if let Some(clock) = line.spans.first_mut()
        && let Some(bare) = clock.content.strip_suffix(' ')
    {
        clock.content = bare.to_string().into();
        line.spans.insert(1, Span::styled(MARK, bold(pal().accent)));
    }
    line
}

/// An open inbox item `i` as the salon lists it: its kind's glyph, when it
/// came (its date too when that is not today), its project when there are
/// several, and what it is, worded.
pub(crate) fn inbox_line(app: &App, i: usize) -> Line<'static> {
    let w = &app.world;
    let item = &w.inbox[i];
    let (icon, style) = match item.kind {
        InboxKind::Sensitive => ("!", bold(pal().warn)),
        InboxKind::Research => ("?", bold(pal().research)),
        InboxKind::Reply => ("»", bold(pal().info)),
        InboxKind::Change => ("Δ", bold(pal().strong)),
        InboxKind::Question => ("?", bold(pal().warn)),
        InboxKind::Approval => ("⚑", bold(pal().warn)),
    };
    let mut spans = vec![
        Span::styled(format!("{icon} "), style),
        Span::styled(format!("{} ", when(item.tick, day(w.tick))), dim()),
    ];
    // Up front: a long row is cut at the panel's edge.
    if app.deferred.contains(&i) {
        spans.push(Span::styled(app.lang.text().later, dim()));
    }
    if w.projects.len() > 1 {
        let p = w.agents[item.agent].project;
        spans.push(Span::styled(format!("{} ", w.projects[p].name), dim()));
    }
    spans.push(Span::raw(words::inbox(app.lang, w, item)));
    if item.kind == InboxKind::Change
        && let Some(c) = w.change_of(item.agent)
    {
        let colour = if c.runs.is_empty() {
            pal().warn
        } else if c.runs.iter().all(Ran::ok) {
            pal().ok
        } else {
            pal().bad
        };
        spans.push(Span::styled(words::evidence(app.lang, &c.runs), fg(colour)));
    }
    Line::from(spans)
}

/// Journal lines `entries` as text to paste elsewhere: who said what, when
/// (day and time: the text leaves the screen, where "today" means
/// nothing), whole. Cleaned like everything drawn, so what is copied is
/// what the screen shows (no control characters, secrets hidden).
pub(crate) fn copy_text(lang: Lang, w: &World, entries: &[usize]) -> String {
    let party = |p: Party| match p {
        Party::Agent(id) => clean(&w.agents[id].name),
        Party::Person => lang.text().you.to_string(),
    };
    let one = |i: usize| {
        let e = &w.journal[i];
        match words::said(&e.event) {
            Some(s) if entries.len() == 1 => body_lines(s.text).join("\n"),
            Some(s) => {
                let kind = s
                    .kind
                    .map(|k| format!(" [{}]", lang.text().kinds[k as usize]))
                    .unwrap_or_default();
                format!(
                    "{} {} → {}{kind}\n{}",
                    stamp(e.tick),
                    party(s.from),
                    party(s.to),
                    body_lines(s.text).join("\n")
                )
            }
            None => format!("{} {}", stamp(e.tick), words::event(lang, w, &e.event)),
        }
    };
    entries
        .iter()
        .map(|&i| one(i))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// An agent's text as cleaned lines, trailing blank lines dropped.
pub(crate) fn body_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text
        .lines()
        .map(|l| clean(l).trim_end().to_string())
        .collect();
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines
}

/// `text` with its inline Markdown: `code`, **bold**, *italic* and
/// [links](url) (the text underlined, the url dim after it). A marker
/// without its closing twin is plain text, as is `*` next to a space (so
/// `a * b` and snake_case names stay as written). Code is never styled
/// further.
/// One piece of a line's inline Markdown: text as written, or a mark
/// with what it covers.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    Text(String),
    Code(String),
    Bold(String),
    Italic(String),
    Link { label: String, url: String },
}

/// `text` split into its inline marks. The rules of [`inline`]; a mark
/// without its closing twin is text.
pub(crate) fn marks(text: &str) -> Vec<Mark> {
    let mut out: Vec<Mark> = Vec::new();
    let mut plain = String::new();
    let mut rest = text;
    // Bounded by the text's length whatever a mutant does to the steps:
    // every round consumes at least one byte.
    for _ in 0..=text.len() {
        let Some(c) = rest.chars().next() else {
            break;
        };
        // What the marker at `rest`'s start means: its mark and the bytes
        // it covers; none when it is just a character.
        let marked: Option<(Mark, usize)> = match c {
            '`' => rest[1..]
                .find('`')
                .map(|n| (Mark::Code(rest[1..1 + n].to_string()), 2 + n)),
            '*' if rest.starts_with("**") => rest[2..]
                .find("**")
                .filter(|&n| n > 0)
                .map(|n| (Mark::Bold(rest[2..2 + n].to_string()), 4 + n)),
            '*' => {
                let inner = &rest[1..];
                // `n` is never 0: a `**` start takes the arm above.
                let closes = |n: usize| {
                    !inner[..n].starts_with(' ')
                        && !inner[..n].ends_with(' ')
                        && !inner[n..].starts_with("**")
                };
                inner
                    .find('*')
                    .filter(|&n| closes(n))
                    .map(|n| (Mark::Italic(inner[..n].to_string()), 2 + n))
            }
            '[' => rest.find("](").and_then(|t| {
                let url_len = rest[t + 2..].find(')')?;
                let (label, url) = (&rest[1..t], &rest[t + 2..t + 2 + url_len]);
                if label.is_empty() || url.is_empty() {
                    return None;
                }
                let link = Mark::Link {
                    label: label.to_string(),
                    url: url.to_string(),
                };
                Some((link, t + 3 + url_len))
            }),
            _ => None,
        };
        match marked {
            Some((mark, n)) => {
                if !plain.is_empty() {
                    out.push(Mark::Text(std::mem::take(&mut plain)));
                }
                out.push(mark);
                rest = &rest[n..];
            }
            None => {
                plain.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    if !plain.is_empty() {
        out.push(Mark::Text(plain));
    }
    out
}

pub(crate) fn inline(text: &str, base: Style) -> Vec<Span<'static>> {
    let code = fg(pal().code);
    let mut spans: Vec<Span<'static>> = Vec::new();
    for mark in marks(text) {
        match mark {
            Mark::Text(t) => spans.push(Span::styled(t, base)),
            Mark::Code(t) => spans.push(Span::styled(t, code)),
            Mark::Bold(t) => spans.push(Span::styled(t, base.add_modifier(Modifier::BOLD))),
            Mark::Italic(t) => {
                spans.push(Span::styled(t, base.add_modifier(Modifier::ITALIC)));
            }
            Mark::Link { label, url } => {
                spans.push(Span::styled(label, base.add_modifier(Modifier::UNDERLINED)));
                spans.push(Span::styled(format!(" ({url})"), dim()));
            }
        }
    }
    spans
}

/// A Markdown heading's text: `line` starts with one to six `#` and a
/// space or nothing after them. `#323` names a card, it is no heading.
fn heading(line: &str) -> Option<&str> {
    let hashes = line.len() - line.trim_start_matches('#').len();
    let rest = &line[hashes..];
    ((1..=6).contains(&hashes) && (rest.is_empty() || rest.starts_with([' ', '\t'])))
        .then(|| rest.trim_start())
}

/// An agent's text as one row of words: cleaned, its lines joined with a
/// space (blank ones dropped), heading and list markers off the front of
/// each, inline marks replaced by what they cover (a link by its label).
/// For the places that show a message in one line: the journal pane, an
/// inbox item.
pub(crate) fn plain(text: &str) -> String {
    let words = |line: &str| {
        marks(line)
            .into_iter()
            .map(|m| match m {
                Mark::Text(t) | Mark::Code(t) | Mark::Bold(t) | Mark::Italic(t) => t,
                Mark::Link { label, .. } => label,
            })
            .collect::<String>()
    };
    body_lines(text)
        .iter()
        .map(|l| {
            let l = l.trim_start();
            let l = heading(l).unwrap_or(l);
            words(list_marker(l).map_or(l, |(_, item)| item))
        })
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The cells of a table row: `| a | b |` or `a | b`.
fn cells(line: &str) -> Vec<&str> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').map(str::trim).collect()
}

/// A table's header-body separator: `|---|:--:|`.
fn is_separator(line: &str) -> bool {
    let t = line.trim();
    t.contains('|') && t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

/// How many of `lines` from `at` on are one table: a header row, the
/// separator, then rows with a `|` in them; 0 when no table starts there.
fn table_len(lines: &[String], at: usize) -> usize {
    if !lines[at].contains('|') || !lines.get(at + 1).is_some_and(|l| is_separator(l)) {
        return 0;
    }
    2 + lines[at + 2..]
        .iter()
        .take_while(|l| l.contains('|') && !l.trim().is_empty())
        .count()
}

/// A table as aligned columns: the header bold, a rule under it, cells
/// padded to their column's width.
fn table(lines: &[String], indent: &str) -> Vec<Line<'static>> {
    let rows: Vec<Vec<Vec<Span<'static>>>> = lines
        .iter()
        .enumerate()
        .filter(|&(n, _)| n != 1)
        .map(|(n, l)| {
            let base = if n == 0 {
                bold(pal().strong)
            } else {
                Style::new()
            };
            cells(l).into_iter().map(|c| inline(c, base)).collect()
        })
        .collect();
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let cell_width = |cell: &[Span]| cell.iter().map(Span::width).sum::<usize>();
    let widths: Vec<usize> = (0..columns)
        .map(|c| {
            rows.iter()
                .filter_map(|r| r.get(c))
                .map(|cell| cell_width(cell))
                .max()
                .unwrap_or(0)
        })
        .collect();
    let mut out = Vec::new();
    for (n, row) in rows.into_iter().enumerate() {
        let mut spans = vec![Span::raw(indent.to_string())];
        for (c, width) in widths.iter().enumerate() {
            if c > 0 {
                spans.push(Span::styled(" │ ", dim()));
            }
            let cell = row.get(c).cloned().unwrap_or_default();
            let pad = width - cell_width(&cell);
            spans.extend(cell);
            if c + 1 < columns {
                spans.push(Span::raw(" ".repeat(pad)));
            }
        }
        out.push(Line::from(spans));
        if n == 0 {
            let rule = widths
                .iter()
                .map(|&w| "─".repeat(w))
                .collect::<Vec<_>>()
                .join("─┼─");
            out.push(Line::from(vec![
                Span::raw(indent.to_string()),
                Span::styled(rule, dim()),
            ]));
        }
    }
    out
}

/// A list marker at the start of `rest`: `- `, `* `, `+ ` (a bullet) or
/// `1. ` (kept), with what follows it.
fn list_marker(rest: &str) -> Option<(String, &str)> {
    if let Some(item) = rest
        .strip_prefix("- ")
        .or_else(|| rest.strip_prefix("* "))
        .or_else(|| rest.strip_prefix("+ "))
    {
        return Some(("• ".to_string(), item));
    }
    let digits = rest.chars().take_while(char::is_ascii_digit).count();
    if digits > 0
        && let Some(item) = rest[digits..].strip_prefix(". ")
    {
        return Some((rest[..digits + 2].to_string(), item));
    }
    None
}

/// An agent's text drawn lightly as the Markdown it usually is: headings,
/// bullet and numbered lists, quotes, rules, tables, fenced blocks and
/// the inline marks of [`inline`]. Every line is cleaned first.
#[cfg(test)]
pub(crate) fn markdown(text: &str, indent: &str) -> Vec<Line<'static>> {
    markdown_src(text, indent)
        .into_iter()
        .map(|(l, _)| l)
        .collect()
}

/// [`markdown`], each drawn line with the source line it was drawn from
/// (cleaned), which is what copying it takes: a fence is never drawn, so
/// a code block copies as its code; a table's rule copies as the
/// separator line it stands for.
pub(crate) fn markdown_src(text: &str, indent: &str) -> Vec<(Line<'static>, String)> {
    let lines = body_lines(text);
    let code = fg(pal().code);
    let mut fenced = false;
    let mut skip = 0;
    let mut out = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        if skip > 0 {
            skip -= 1;
            continue;
        }
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        let mut spans = vec![Span::raw(indent.to_string())];
        if fenced {
            spans.push(Span::styled("│ ", dim()));
            spans.push(Span::styled(line.clone(), code));
            out.push((Line::from(spans), line.clone()));
            continue;
        }
        let n = table_len(&lines, at);
        if n > 0 {
            let source = lines[at..at + n].iter().cloned();
            out.extend(table(&lines[at..at + n], indent).into_iter().zip(source));
            skip = n - 1;
            continue;
        }
        let lead = line.len() - line.trim_start().len();
        let rest = &line[lead..];
        if let Some(h) = heading(rest) {
            spans.push(Span::styled(h.trim().to_string(), bold(pal().strong)));
        } else if matches!(rest, "---" | "***" | "___") {
            spans.push(Span::styled("─".repeat(24), dim()));
        } else {
            let (mut lead, mut rest) = (line[..lead].to_string(), rest);
            if let Some(quoted) = rest.strip_prefix("> ").or_else(|| rest.strip_prefix('>')) {
                spans.push(Span::raw(std::mem::take(&mut lead)));
                spans.push(Span::styled("▎ ", dim()));
                rest = quoted;
            }
            match list_marker(rest) {
                Some((marker, item)) => {
                    spans.push(Span::raw(format!("{lead}{marker}")));
                    rest = item;
                }
                None => spans.push(Span::raw(lead)),
            }
            spans.extend(inline(rest, Style::new()));
        }
        out.push((Line::from(spans), line.clone()));
    }
    out
}

/// Key hints as the bottom bar shows them: each key on a badge, its
/// meaning after it.
pub(crate) fn hint_spans(hints: &[(&str, &str)]) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (k, label) in hints {
        spans.push(Span::styled(format!(" {k} "), key_style()));
        spans.push(Span::raw(format!(" {label}  ")));
    }
    spans
}

/// A span that only leads a line: indent, a list marker (`• `, `3. `), a
/// code rule, a quote bar.
pub(crate) fn is_lead(s: &Span) -> bool {
    let t = s.content.trim_start_matches(' ');
    matches!(t, "" | "• " | "│ " | "▎ ")
        || t.strip_suffix(". ")
            .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

/// `word` (no wider than a row would hold) cut into pieces at most `room`
/// cells wide; `room` is at least 1.
fn pieces(word: &str, room: usize) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut used = 0;
    for ch in word.chars() {
        let w = ch.width().unwrap_or(0);
        if used > 0 && used + w > room {
            out.push(String::new());
            used = 0;
        }
        out.last_mut().expect("one piece at least").push(ch);
        used += w;
    }
    out
}

/// `line` cut into rows at most `width` cells wide, at spaces where it
/// can (a word wider than a row is cut). Rows after the first start with
/// the line's lead (its indent, a code rule, the width of a bullet), so a
/// wrapped list item or code line still reads as one. Widths are cell
/// widths, so wide characters count double.
pub(crate) fn wrap(line: Line<'static>, width: usize) -> Vec<Line<'static>> {
    let leads = line.spans.iter().take_while(|s| is_lead(s)).count();
    let (lead, rest) = line.spans.split_at(leads);
    let hang: Vec<Span<'static>> = lead
        .iter()
        .map(|s| {
            // Indent, a rule and a bar go on; a list marker does not.
            if matches!(s.content.trim_start_matches(' '), "" | "│ " | "▎ ") {
                s.clone()
            } else {
                Span::raw(" ".repeat(s.content.width()))
            }
        })
        .collect();
    let lead_width: usize = lead.iter().map(|s| s.content.width()).sum();
    let room = width.saturating_sub(lead_width).max(1);
    let mut rows = Vec::new();
    let mut row: Vec<Span<'static>> = lead.to_vec();
    let mut used = 0;
    for span in rest {
        for chunk in span.content.split_inclusive(' ') {
            let word = chunk.trim_end();
            // A word that fits is one piece; the space goes with the last.
            let mut parts = pieces(word, room);
            parts
                .last_mut()
                .expect("one piece at least")
                .push_str(&chunk[word.len()..]);
            for part in parts {
                if used > 0 && used + part.trim_end().width() > room {
                    rows.push(Line::from(std::mem::replace(&mut row, hang.clone())));
                    used = 0;
                }
                used += part.width();
                row.push(Span::styled(part, span.style));
            }
        }
    }
    rows.push(Line::from(row));
    rows
}

/// How many characters of the words a note is on the command line shows
/// while the note is written.
pub(crate) const NOTE_QUOTE: usize = 24;

/// `text`'s words on one line, cut to `n` characters with `…` where cut.
pub(crate) fn brief(text: &str, n: usize) -> String {
    let words = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if words.chars().count() <= n {
        return words;
    }
    let mut out: String = words.chars().take(n.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// `s` cleaned, then cut or padded to exactly `n` characters.
pub(crate) fn fit(s: &str, n: usize) -> String {
    let s = clean(s);
    if s.chars().count() <= n {
        format!("{s:<n$}")
    } else {
        let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Makes the cells of `area` one link to `url` the terminal opens (OSC 8;
/// cells with the same `id` light up together), after ratatui has laid
/// the row out. Each cell opens the link, writes its own symbol and
/// closes it again, and keeps its width (`ForcedWidth(1)`): the width
/// math is untouched, and a cell redrawn alone never leaves a link open
/// over what is written after it. A URL with a control character (which
/// could end the sequence) links nothing; `model::local_url` lets none
/// through anyway.
pub(crate) fn hyperlink(buf: &mut Buffer, area: Rect, url: &str, id: AgentId) {
    if url.chars().any(char::is_control) {
        return;
    }
    let one = NonZeroU16::MIN;
    for (x, y) in area.intersection(buf.area).positions().map(|p| (p.x, p.y)) {
        let cell = &mut buf[(x, y)];
        let symbol = format!(
            "\x1b]8;id=imece-{id};{url}\x1b\\{}\x1b]8;;\x1b\\",
            cell.symbol()
        );
        cell.set_symbol(&symbol)
            .set_diff_option(CellDiffOption::ForcedWidth(one));
    }
}

/// `lines` wrapped to `area`, scrolled so the newest lines are in view.
pub(crate) fn tail(f: &mut Frame, area: Rect, lines: Vec<Line<'static>>) {
    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    let rows = p.line_count(area.width);
    let skip = rows.saturating_sub(usize::from(area.height));
    let skip = u16::try_from(skip).unwrap_or(u16::MAX);
    f.render_widget(p.scroll((skip, 0)), area);
}

pub(crate) fn bar(progress: u8) -> String {
    let full = ((usize::from(progress) * 5 + 50) / 100).min(5);
    format!("{}{}", "▓".repeat(full), "░".repeat(5 - full))
}

/// The most characters a row's status takes, the text it carries
/// included (a question, a reason, the newest exchange).
pub(crate) const STATUS_WIDTH: usize = 40;

/// A quote that would be shorter than this (its quotes included) says
/// nothing and is left out.
pub(crate) const QUOTE_MIN: usize = 8;

/// `text` as a row quotes it, `"…"`, cleaned and cut to `room`
/// characters with its quotes; none for an empty text or a room under
/// `QUOTE_MIN`.
pub(crate) fn quoted(text: &str, room: usize) -> Option<String> {
    let text = clean(text.trim());
    if room < QUOTE_MIN || text.is_empty() {
        return None;
    }
    Some(format!("\"{}\"", fit(&text, room - 2).trim_end()))
}

/// Characters `spans` take on the row.
pub(crate) fn chars(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|s| s.content.chars().count()).sum()
}

/// `text` quoted after `spans`, in `style`, in what is left of
/// `STATUS_WIDTH`; nothing when too little is left.
fn add_quote(spans: &mut Vec<Span<'static>>, text: &str, style: Style) {
    let room = STATUS_WIDTH.saturating_sub(chars(spans) + 1);
    if let Some(q) = quoted(text, room) {
        spans.push(Span::styled(format!(" {q}"), style));
    }
}

/// What an agent is doing, in a few words, with the text that belongs to
/// it: the question that waits for the person, the answer they have not
/// read, what a tool call waits to do, what a blocked worker waits for,
/// why a stopped session stopped.
pub(crate) fn status_spans(lang: Lang, w: &World, a: &Agent) -> Vec<Span<'static>> {
    let t = lang.text();
    match a.status {
        Status::Idle if let Some(it) = w.open_item(a.id, InboxKind::Reply) => {
            let mut spans = vec![Span::styled(t.answered, fg(pal().warn))];
            add_quote(&mut spans, &it.detail, fg(pal().warn));
            spans
        }
        Status::Idle if let Some(it) = w.open_item(a.id, InboxKind::Question) => {
            let mut spans = vec![
                Span::styled(bar(a.progress), dim()),
                Span::styled(format!(" ? → {}", t.you), fg(pal().warn)),
            ];
            add_quote(&mut spans, &it.detail, fg(pal().warn));
            spans
        }
        Status::Idle
            if a.role == Role::Leader
                && w.agents.iter().any(|o| {
                    o.project == a.project
                        && o.role == Role::Worker
                        && !o.gone
                        && o.status != Status::Idle
                }) =>
        {
            vec![Span::styled(t.coordinating, dim())]
        }
        Status::Idle => vec![Span::styled(t.idle, dim())],
        Status::Working if let Some(it) = w.open_item(a.id, InboxKind::Approval) => {
            let mut spans = vec![Span::styled(format!("⚑ {}", t.waiting_yes), fg(pal().warn))];
            add_quote(&mut spans, &words::one_line(&it.detail), fg(pal().warn));
            spans
        }
        Status::Working if a.starting => vec![Span::styled(t.starting, fg(pal().ok))],
        Status::Working => match a.activity.as_deref() {
            Some("") => vec![Span::styled(t.thinking, fg(pal().ok))],
            Some(doing) => vec![Span::styled(
                fit(doing, 40).trim_end().to_string(),
                fg(pal().ok),
            )],
            None => vec![
                Span::styled(bar(a.progress), fg(pal().ok)),
                Span::raw(format!(" {}", Phase::of(a.progress).label())),
            ],
        },
        Status::Asking { to, .. } => vec![
            Span::styled(bar(a.progress), dim()),
            Span::styled(
                format!(" ? → {}", clean(&w.agents[to].name)),
                fg(pal().warn),
            ),
        ],
        Status::Blocked => {
            let wait = match a.researcher {
                Some(r) => format!(" ← {}", clean(&w.agents[r].name)),
                None => t.research_queue.into(),
            };
            let mut spans = vec![Span::styled(t.blocked, bold(pal().bad))];
            // What it waits for, in the room the wait leaves.
            let room = STATUS_WIDTH.saturating_sub(chars(&spans) + 1 + wait.chars().count());
            if let Some(q) = quoted(&a.topic, room) {
                spans.push(Span::styled(format!(" {q}"), fg(pal().bad)));
            }
            spans.push(Span::styled(wait, dim()));
            spans
        }
        Status::WaitingHuman => vec![Span::styled(t.waiting_human, bold(pal().warn))],
        Status::Researching { .. } => vec![Span::styled(
            fill(t.researching, &[&a.topic]),
            fg(pal().research),
        )],
        Status::Stopped if !a.topic.is_empty() => vec![Span::styled(
            fit(&fill(t.stopped_why, &[&a.topic]), STATUS_WIDTH)
                .trim_end()
                .to_string(),
            bold(pal().stopped),
        )],
        Status::Stopped => vec![Span::styled(t.stopped, bold(pal().stopped))],
        Status::Asleep => vec![Span::styled(t.asleep, dim())],
    }
}

/// The fleet at a glance: how many agents are mid-turn, done and not
/// seen, stuck (blocked, or their session stopped); counts of none are
/// left out, and with none at all it says all are idle.
pub(crate) fn fleet(app: &App) -> Vec<Span<'static>> {
    let w = &app.world;
    let t = app.lang.text();
    let here = || w.agents.iter().filter(|a| !a.gone);
    let counts = [
        (t.fleet_working, w.mid_turn().len(), pal().ok),
        (
            t.fleet_done,
            here().filter(|a| app.done_unseen(a.id)).count(),
            pal().ok,
        ),
        (
            t.fleet_stuck,
            here()
                .filter(|a| matches!(a.status, Status::Blocked | Status::Stopped))
                .count(),
            pal().bad,
        ),
    ];
    let mut spans = Vec::new();
    for (words, n, colour) in counts.into_iter().filter(|c| c.1 > 0) {
        if !spans.is_empty() {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(fill(words, &[&n.to_string()]), fg(colour)));
    }
    if spans.is_empty() {
        spans.push(Span::styled(t.fleet_idle, dim()));
    }
    spans
}

/// Percent of a vendor's usage window used from which the header says
/// it the way it says what waits for the person.
const LIMIT_LOUD: u8 = 90;

/// A vendor's tightest usage window as the header shows it.
fn limit_span(lang: Lang, l: &Limit, now: u64) -> Span<'static> {
    let said = words::limit(lang, l, now);
    match l.used >= LIMIT_LOUD {
        true => Span::styled(said, attention()),
        false => Span::raw(said),
    }
}

fn header(f: &mut Frame, area: Rect, app: &App) {
    let w = &app.world;
    let t = app.lang.text();
    let sep = || Span::styled(" │ ", dim());
    let open = w.open_inbox().len();
    let mut spans = vec![Span::styled(" imece ", brand()), Span::raw(" ")];
    // Live work starts without cards and without researchers: no "0/0".
    if !w.cards.is_empty() {
        spans.push(Span::raw(fill(
            t.cards,
            &[&w.cards_done().to_string(), &w.cards.len().to_string()],
        )));
        spans.push(sep());
    }
    spans.extend(fleet(app));
    spans.push(sep());
    let mut rate = fill(t.per_hour, &[&dollars(w.rate_per_hour())]);
    if let Some(minutes) = w.pot_lasts() {
        rate += ", ";
        rate += &fill(t.pot_lasts, &[&words::ago(app.lang, minutes * 60)]);
    }
    let spent = fill(t.budget, &[&dollars(w.spent()), &dollars(w.budget)]);
    if w.pot_low() {
        spans.push(Span::styled(format!("{spent} ({rate})"), attention()));
    } else {
        spans.push(Span::raw(spent));
        spans.push(Span::styled(format!(" ({rate})"), dim()));
    }
    // Each vendor's window nearest to full, loud when little is left.
    for l in w.tightest() {
        spans.push(sep());
        spans.push(limit_span(app.lang, &l, w.now()));
    }
    if w.max_researchers > 0 {
        spans.push(sep());
        spans.push(Span::raw(fill(
            t.researchers,
            &[
                &w.active_researchers().to_string(),
                &w.max_researchers.to_string(),
            ],
        )));
    }
    if open > 0 {
        spans.push(sep());
        spans.push(Span::styled(
            fill(t.waiting_for_you, &[&open.to_string()]),
            attention(),
        ));
    }
    spans.push(Span::styled(format!("  {}", clock(w.tick)), dim()));
    if app.paused {
        spans.push(Span::styled(
            t.paused,
            Style::new().add_modifier(Modifier::REVERSED),
        ));
    } else if app.speed > 1 {
        spans.push(Span::styled(format!(" ×{}", app.speed), dim()));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// The matches for the word being typed, the highlighted one standing
/// out, in a window that always holds it when they do not all fit.
fn completions(f: &mut Frame, area: Rect, m: &crate::complete::Menu) {
    let width = area.width as usize;
    // An item with its note (a command's value, `$`), a space either side.
    let cells: Vec<String> = (m.items.iter().enumerate())
        .map(|(i, item)| match m.notes.get(i).filter(|n| !n.is_empty()) {
            Some(note) => format!(" {} {} ", clean(item), clean(note)),
            None => format!(" {} ", clean(item)),
        })
        .collect();
    let cell = |i: usize| cells[i].chars().count();
    // Start the window at the highlighted item and pull it back while
    // earlier items still fit.
    let mut first = m.chosen;
    let mut used = cell(m.chosen);
    while first > 0 && used + cell(first - 1) <= width {
        first -= 1;
        used += cell(first);
    }
    let mut spans = Vec::new();
    let mut taken = 0;
    for (i, text) in cells.iter().enumerate().skip(first) {
        if taken + cell(i) > width {
            break;
        }
        taken += cell(i);
        let style = if i == m.chosen {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            dim()
        };
        spans.push(Span::styled(text.clone(), style));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// The line being written with its cursor: the character under it in
/// reverse, or a block at the end. A line `clean` would change (a pasted
/// token) is drawn cleaned whole with the block at the end, so moving
/// into it does not split the token past hiding.
fn cursor_spans(input: &str, at: usize) -> Vec<Span<'static>> {
    let whole = clean(input);
    let under = input[at..].chars().next();
    match under {
        Some(c) if whole == input => vec![
            Span::raw(input[..at].to_string()),
            Span::styled(c.to_string(), Style::new().add_modifier(Modifier::REVERSED)),
            Span::raw(input[at + c.len_utf8()..].to_string()),
        ],
        _ => vec![Span::raw(whole), Span::styled("█", dim())],
    }
}

/// The most rows the line being written takes.
const INPUT_ROWS: u16 = 8;

/// How many rows the footer takes: one, or while writing one per line of
/// the text, at most [`INPUT_ROWS`] and a third of the screen.
fn input_rows(app: &App, height: u16) -> u16 {
    if !app.typing {
        return 1;
    }
    let lines = app.input.split('\n').count();
    let most = (height / 3).clamp(1, INPUT_ROWS);
    u16::try_from(lines).unwrap_or(u16::MAX).min(most)
}

/// The text being written, a row per line of it, under who it goes to:
/// the cursor's line always shown (the rows above it go out of view
/// first), the keys to send or cancel after the last row shown, with
/// where the cursor is (`3/12`) when not all lines fit.
fn writing(app: &App, rows: usize) -> Vec<Line<'static>> {
    let t = app.lang.text();
    let to = Span::styled(
        if let Some(Noting::Line(id, line)) = app.noting {
            let patch = app.world.change_of(id).map(|c| c.patch.as_str());
            let place = patch.and_then(|p| place(p, line)).unwrap_or_default();
            fill(t.typing_note, &[&place])
        } else if let Some(Noting::Words(note)) = &app.noting {
            fill(t.typing_words_note, &[&brief(&note.quote, NOTE_QUOTE)])
        } else if app.searching {
            t.typing_history.to_string()
        } else if app.typing_shell() {
            t.typing_shell.to_string()
        } else if app.typing_card() {
            t.typing_card.to_string()
        } else if app.typing_jump() {
            t.typing_jump.to_string()
        } else if app.typing_command() {
            t.typing_command.to_string()
        } else {
            fill(t.typing_to, &[&app.typing_target()])
        },
        bold(pal().strong),
    );
    let indent = Span::raw(" ".repeat(to.width()));
    let at = app.cursor();
    let here = app.input[..at].matches('\n').count();
    let total = app.input.split('\n').count();
    let first = (here + 1).saturating_sub(rows);
    let mut start = 0;
    let mut out = Vec::new();
    for (n, line) in app.input.split('\n').enumerate() {
        let from = start;
        start += line.len() + 1;
        if n < first || out.len() == rows {
            continue;
        }
        let lead = if out.is_empty() {
            to.clone()
        } else {
            indent.clone()
        };
        let text = if n == here {
            cursor_spans(line, at - from)
        } else {
            vec![Span::raw(clean(line))]
        };
        out.push(Line::from([vec![lead], text].concat()));
    }
    let mut hint = Vec::new();
    if total > out.len() {
        hint.push(Span::styled(format!("   {}/{total}", here + 1), dim()));
    }
    let newline = if app.shift_enter {
        t.newline_shift
    } else {
        t.newline_plain
    };
    hint.push(Span::styled(
        match (&app.menu, &app.noting) {
            (Some(_), _) => t.pick_cancel.to_string(),
            (None, Some(_)) => fill(t.keep_cancel, &[newline]),
            (None, None) => fill(t.send_cancel, &[newline]),
        },
        dim(),
    ));
    if let Some(last) = out.last_mut() {
        last.spans.extend(hint);
    }
    out
}

fn footer(f: &mut Frame, area: Rect, app: &App) {
    let t = app.lang.text();
    let key = key_style;
    if app.typing {
        let lines = writing(app, usize::from(area.height));
        return f.render_widget(Paragraph::new(lines), area);
    }
    let line = if let Some((msg, at)) = &app.flash
        && at.elapsed() < FLASH
    {
        Line::from(Span::styled(format!(" {msg}"), fg(pal().warn)))
    } else {
        let hints: Vec<(&str, &str)> = match app.zoom {
            Zoom::None if app.card.is_some() => vec![
                ("tab", t.panel),
                ("enter", t.focus),
                ("n p", t.next_card),
                ("#", t.other_card),
                ("esc", t.back),
                ("?", t.help),
            ],
            Zoom::None => vec![
                ("/", t.write_lead),
                ("@", t.to_agent),
                ("#", t.find_card),
                ("tab", t.panel),
                ("← →", t.fold),
                ("enter", t.focus),
                ("?", t.help),
            ],
            Zoom::Focus(_) => vec![
                ("enter", t.trace),
                ("esc", t.back),
                ("@", t.write_this),
                ("?", t.help),
            ],
            Zoom::Trace(_) => vec![("esc", t.back), ("@", t.write_this), ("?", t.help)],
            // Choosing lines to copy: only copy mode's keys work.
            Zoom::Diff(id) if app.diff_copy_of(id).is_some() => {
                let mut keys = t.copy_hints.to_vec();
                keys.push(("Y", t.new_side));
                keys.push(("?", t.help));
                keys
            }
            Zoom::Diff(_) => vec![
                ("j k  w", t.scroll_wrap),
                ("[ ]", t.file),
                ("a x", t.apply_part),
                ("r", t.reject),
                (">", t.quote),
                ("c n", t.notes),
                ("v", t.copy_lines),
                ("esc", t.back),
                ("?", t.help),
            ],
        };
        let mut spans = vec![Span::raw(" ")];
        spans.extend(hint_spans(&hints));
        if app.zoom == Zoom::None && !app.live() {
            spans.push(Span::styled(" space ", key()));
            spans.push(Span::raw(format!(" {}  ", t.pause)));
        }
        for (i, view) in [View::Salon, View::Board, View::Code]
            .into_iter()
            .enumerate()
        {
            let style = if app.view == view {
                bold(pal().strong)
            } else {
                dim()
            };
            spans.push(Span::styled(format!(" {} {}", i + 1, t.views[i]), style));
        }
        if let Some(c) = app.pending {
            // The first key of a chord, as vim shows it.
            spans.push(Span::styled(format!("   {c}…"), bold(pal().warn)));
        }
        Line::from(spans)
    };
    f.render_widget(Paragraph::new(line), area);
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::app::{App, Need, Pane, Sel};
    use crate::i18n::Lang;

    fn render(app: &App, width: u16, height: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        let buf = t.backend().buffer();
        buf.content
            .chunks(usize::from(width))
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn app_after(projects: usize, workers: usize, ticks: usize, lang: Lang) -> App {
        let mut app = App::new(projects, workers, 4, 7, lang);
        for _ in 0..ticks {
            app.step();
        }
        app
    }

    #[test]
    fn the_salon_shows_projects_the_inbox_and_the_journal() {
        let app = app_after(2, 12, 300, Lang::En);
        let s = render(&app, 140, 45);
        for want in [
            "SALON",
            "▾ shop-api",
            "▾ mobile-app",
            "lead-shop-api",
            "WAITING FOR YOU",
            "JOURNAL",
            "QUEUE",
        ] {
            assert!(s.contains(want), "{want} missing:\n{s}");
        }
    }

    /// A demo team after a while, card #1's screen opened by typing it.
    fn on_card_one(lang: Lang) -> App {
        let mut app = app_after(1, 4, 140, lang);
        for code in [KeyCode::Char('#'), KeyCode::Char('1'), KeyCode::Enter] {
            app.on_key(KeyEvent::from(code));
        }
        assert_eq!(app.card, Some(0));
        app
    }

    #[test]
    fn a_card_s_screen_wears_the_focus_colour_while_it_has_the_keys() {
        let mut app = on_card_one(Lang::En);
        let corner = |app: &App| buffer(app, 140, 36)[(0, 1)].style().fg;
        assert_eq!(app.pane, Pane::Journal);
        assert_ne!(corner(&app), Some(pal().accent));
        app.pane = Pane::Salon;
        assert_eq!(corner(&app), Some(pal().accent));
    }

    #[test]
    fn a_card_s_screen_shows_where_it_stands_who_had_a_part_and_its_lines() {
        let app = on_card_one(Lang::En);
        let s = render(&app, 140, 36);
        let card = &app.world.cards[0];
        assert!(!card.symbols.is_empty() && card.column == Column::Done);
        let owner = &app.world.agents[card.owner.unwrap()].name;
        for want in [
            &format!(" #1 {} ", card.title),
            "shop-api · DONE · ",
            " min",
            "lease  ",
            "WHO",
            "posted    ◆ ▎lead ",
            "owner     ",
            "talked    ",
            "STORY",
            &format!("◎ intent     {owner}: {}", card.title),
            "WAITING FOR YOU · #1 (1)",
            &format!("Δ 14:06 {owner} finished: {}", card.title),
            "JOURNAL · #1 (",
            "n p  next / previous card",
            "#  another card",
        ] {
            assert!(s.contains(want), "{want} missing:\n{s}");
        }
        assert!(
            !s.contains("SALON"),
            "the card takes the salon's place:\n{s}"
        );
        // Its change waits for the person: `!`, whatever it does meanwhile.
        assert!(s.contains(&format!("owner     ! ▎{owner}")), "{s}");
        // Who had a part are the lines to move among.
        let shown = app.shown.borrow().clone();
        let people: Vec<Sel> = app
            .world
            .card_people(0)
            .iter()
            .map(|&(id, _)| Sel::Agent(id))
            .collect();
        assert_eq!(shown, people);
    }

    #[test]
    fn a_card_s_row_keeps_its_cost_in_the_pane_and_quotes_only_what_is_left() {
        let mut app = on_card_one(Lang::En);
        let owner = app.world.cards[0].owner.unwrap();
        let name = app.world.agents[owner].name.clone();
        let a = &mut app.world.agents[owner];
        a.status = Status::Idle;
        a.activity = None;
        a.cost = 12_345;
        a.changed_at = app.world.tick;
        a.stand = Stand::default();
        a.local = None;
        a.plan.clear();
        a.context = None;
        a.talk = Some(Talk {
            with: Party::Person,
            mine: true,
            text: "a".repeat(60),
            until: 0,
        });
        app.world.inbox.clear();
        app.unseen.clear();
        let s = render(&app, 140, 36);
        let row = s
            .lines()
            .find(|l| l.contains(&format!("▎{name}")))
            .unwrap_or_else(|| panic!("{s}"));
        let pane = row.split("││").next().unwrap();
        assert!(
            pane.contains("→ you \"aaaa") && pane.contains("…\"") && pane.contains("$1.23"),
            "the quote is cut to the row inside the card's borders, after the part column: {pane}"
        );
    }

    /// A fresh directory of this test's own under the system's temp one.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("imece-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn export_writes_a_conversation_as_markdown_with_the_day_on_every_time() {
        let mut app = talking();
        app.world.note(Event::YouSaid {
            to: 0,
            text: "my TOKEN=abc123 \x1b[31m".into(),
        });
        let lead = app.world.agents[0].name.clone();
        let text = export::thread(Lang::En, &app.world, &[0, 1, 5]);
        assert!(
            text.starts_with(&format!("# Conversation: {lead} ⇄ you\n\n"))
                || text.starts_with(&format!("# Conversation: you ⇄ {lead}\n\n")),
            "{text}"
        );
        let first = format!("### {} you → {lead}\n\ncheck the readme\n\n", stamp(0));
        assert!(text.contains(&first), "{text}");
        assert!(text.contains(&format!("### {} {lead} → you\n\n# Readme\n", stamp(0))));
        assert!(text.contains("```\ncargo run\n```\n"), "{text}");
        assert!(!text.contains("abc123") && !text.contains('\x1b'), "{text}");
        // A kind is named; a line that is no message is a list item.
        app.world.note(Event::Sent {
            from: 0,
            to: 1,
            kind: MsgKind::Ask,
            text: "which?".into(),
            id: 0,
            re: None,
        });
        app.world.note(Event::Budget {
            leader: 0,
            from: 10_000,
            to: 20_000,
        });
        let worker = app.world.agents[1].name.clone();
        let text = export::thread(Lang::En, &app.world, &[6]);
        assert!(
            text.contains(&format!("{lead} → {worker} [ask]\n\nwhich?")),
            "{text}"
        );
        let text = export::thread(Lang::En, &app.world, &[7]);
        assert_eq!(
            text,
            format!(
                "# Conversation: \n\n- {} you set the budget to $2.00 (was $1.00)\n",
                stamp(0)
            )
        );
        // List items follow each other; a message stands apart.
        let text = export::thread(Lang::En, &app.world, &[7, 7, 6, 7]);
        let budget = format!("- {} you set the budget to $2.00 (was $1.00)\n", stamp(0));
        let ask = format!("### {} {lead} → {worker} [ask]\n\nwhich?\n", stamp(0));
        assert_eq!(
            text,
            format!("# Conversation: {lead} ⇄ {worker}\n\n{budget}{budget}\n{ask}\n{budget}")
        );
        let tr = export::thread(Lang::Tr, &app.world, &[0]);
        assert!(
            tr.starts_with("# Konuşma: ") && tr.contains(" sen → "),
            "{tr}"
        );
    }

    #[test]
    fn export_writes_a_cards_screen_as_markdown() {
        let app = on_card_one(Lang::En);
        let w = &app.world;
        let card = &w.cards[0];
        let owner = &w.agents[card.owner.unwrap()].name;
        let text = export::card(&app, 0);
        let minutes = card::minutes(w, card);
        for want in [
            format!("# #1 {}\n\n", card.title),
            format!("shop-api · DONE · {minutes} min\n\n"),
            "lease  ".to_string(),
            "## WHO\n\n- posted: ".to_string(),
            format!("- owner: {owner}\n"),
            "- talked: ".to_string(),
            "\n## STORY\n\n- ".to_string(),
            format!("{owner}: {}", card.title),
            "\n## Waiting for you\n\n- ".to_string(),
            format!("{owner} finished: {}", card.title),
            format!("- 14:00 ◎ intent {owner}: {}\n", card.title),
            "\n## Journal\n\n".to_string(),
        ] {
            assert!(text.contains(&want), "{want:?} missing:\n{text}");
        }
        // Every journal line of the card, oldest first, none folded.
        let lines = salon::card_lines(w, 0);
        assert!(!lines.is_empty() && lines.len() < w.journal.len());
        assert!(lines.windows(2).all(|p| p[0] < p[1]), "{lines:?}");
        let folded: Vec<usize> = salon::card_rows(w, 0).iter().map(|r| r.0).collect();
        assert!(folded.iter().all(|i| lines.contains(i)));
        let journal = &text[text.find("## Journal").unwrap()..];
        let at: Vec<usize> = (lines.iter())
            .map(|&i| journal.find(&stamp(w.journal[i].tick)).unwrap())
            .collect();
        assert!(
            at.windows(2).all(|p| p[0] <= p[1]),
            "oldest first:\n{journal}"
        );
        // No story, nothing waiting, no lines: said or left out.
        let mut app = on_card_one(Lang::Tr);
        app.world.cards[0].story.clear();
        app.world.cards[0].symbols.clear();
        app.world.inbox.iter_mut().for_each(|i| i.open = false);
        app.world.journal.clear();
        let text = export::card(&app, 0);
        assert!(text.contains("## KİM\n"), "{text}");
        assert!(text.contains("## HİKÂYE\n\nHenüz hikâye yok.\n"), "{text}");
        for gone in ["## Seni bekleyenler", "## Defter", "lease"] {
            assert!(!text.contains(gone), "{gone}:\n{text}");
        }
    }

    #[test]
    fn an_exports_file_is_named_after_what_and_when_and_never_written_over() {
        let digits: String = stamp(42).chars().filter(char::is_ascii_digit).collect();
        assert_eq!(digits.len(), 12);
        assert_eq!(
            export::file_name("card-12", 42),
            format!("imece-card-12-{}-{}.md", &digits[..8], &digits[8..])
        );
        let dir = scratch("export-write");
        // No target: in the directory given.
        let path = export::write(None, &dir, "a.md", "one").unwrap();
        assert_eq!(path, dir.join("a.md"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "one");
        let e = export::write(None, &dir, "a.md", "two").unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "one", "kept");
        // A directory: named in it; a file: that file.
        let sub = dir.join("sub");
        std::fs::create_dir(&sub).unwrap();
        let path = export::write(Some(&sub), &dir, "b.md", "three").unwrap();
        assert_eq!(path, sub.join("b.md"));
        let named = dir.join("mine.md");
        let path = export::write(Some(&named), &dir, "c.md", "four").unwrap();
        assert_eq!(path, named);
        assert_eq!(std::fs::read_to_string(&named).unwrap(), "four");
        assert!(!dir.join("c.md").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn colon_export_writes_what_is_open_and_says_where() {
        let dir = scratch("export-command");
        let typed = |app: &mut App, line: &str| {
            for c in line.chars() {
                key(app, KeyCode::Char(c));
            }
        };
        let mut app = talking();
        typed(&mut app, ":export");
        assert_eq!(app.menu, None, "whole already");
        key(&mut app, KeyCode::Enter);
        let s = render(&app, 160, 40);
        assert!(
            s.contains("nothing to export: open a conversation (v)"),
            "{s}"
        );
        // The conversation open: the menu says so, the file has it.
        app.open_thread(4);
        typed(&mut app, ":exp");
        assert_eq!(app.menu.as_ref().unwrap().notes, ["(the conversation)"]);
        let file = dir.join("talk.md");
        typed(&mut app, &format!("ort {}", file.display()));
        key(&mut app, KeyCode::Enter);
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.starts_with("# Conversation: "), "{text}");
        let s = render(&app, 200, 40);
        assert!(s.contains(&format!("written to {}", file.display())), "{s}");
        // Again: not written over, and said.
        typed(&mut app, &format!(":export {}", file.display()));
        key(&mut app, KeyCode::Enter);
        let s = render(&app, 200, 40);
        assert!(
            s.contains(&format!("could not write {}: ", file.display())),
            "{s}"
        );
        // A card's screen, into a directory.
        let mut app = on_card_one(Lang::En);
        typed(&mut app, ":exp");
        assert_eq!(app.menu.as_ref().unwrap().notes, ["(#1)"]);
        typed(&mut app, &format!("ort {}", dir.display()));
        key(&mut app, KeyCode::Enter);
        let name = export::file_name("card-1", app.world.tick);
        let text = std::fs::read_to_string(dir.join(&name)).unwrap();
        assert!(text.starts_with("# #1 "), "{text}");
        let mut app = app_after(1, 1, 0, Lang::En);
        typed(&mut app, ":exp");
        assert_eq!(app.menu.as_ref().unwrap().notes, [""], "nothing open");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_card_s_screen_speaks_turkish_and_says_when_there_is_no_story() {
        let mut app = on_card_one(Lang::Tr);
        app.world.cards[0].story.clear();
        let s = render(&app, 140, 36);
        for want in [
            "KİM",
            "HİKÂYE",
            "Henüz hikâye yok.",
            "SENİ BEKLEYEN · #1 (1)",
            "DEFTER · #1 (",
            "sahibi    ",
            "sonraki / önceki iş",
        ] {
            assert!(s.contains(want), "{want} missing:\n{s}");
        }
    }

    #[test]
    fn the_salon_offers_cards_and_says_what_is_typed_after_a_hash() {
        let mut app = app_after(1, 4, 140, Lang::En);
        assert!(render(&app, 140, 36).contains("#  card"));
        app.on_key(KeyEvent::from(KeyCode::Char('#')));
        let s = render(&app, 140, 36);
        assert!(s.contains(" › card: #"), "{s}");
        assert!(s.contains("#10 "), "the menu offers the cards:\n{s}");
    }

    #[test]
    fn a_card_s_number_is_no_markdown_heading() {
        assert_eq!(heading("# Head"), Some("Head"));
        assert_eq!(heading("###### six"), Some("six"));
        assert_eq!(heading("##\t tab"), Some("tab"));
        assert_eq!(heading("#"), Some(""));
        assert_eq!(heading("####### seven"), None);
        assert_eq!(heading("#323 Shipping"), None);
        assert_eq!(heading("x # y"), None);
        assert_eq!(
            plain("#5 Money rounding\n## Done"),
            "#5 Money rounding Done"
        );
        let md = |text: &str| -> Vec<String> {
            markdown(text, "")
                .iter()
                .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect())
                .collect()
        };
        assert_eq!(md("#5 Money\n# Head "), ["#5 Money", "Head"]);
        assert_eq!(markdown("# Head", "")[0].spans[1].style, bold(pal().strong));
    }

    #[test]
    fn the_screen_speaks_turkish_when_asked() {
        let app = app_after(1, 12, 300, Lang::Tr);
        let s = render(&app, 140, 45);
        for want in ["SENİ BEKLEYEN", "DEFTER", "KUYRUK", "lidere yaz"] {
            assert!(s.contains(want), "{want} missing:\n{s}");
        }
        assert!(!s.contains("WAITING FOR YOU"));
    }

    #[test]
    fn a_folded_project_shows_only_its_header() {
        let mut app = app_after(2, 12, 300, Lang::En);
        let shown = |app: &App, p: ProjectId| {
            salon::rows(app, 100)
            .iter()
            .filter(
                |r| matches!(r, salon::Row::Agent { id, .. } if app.world.agents[*id].project == p),
            )
            .count()
        };
        assert!(shown(&app, 1) > 1);
        app.collapsed[1] = true;
        assert_eq!(shown(&app, 1), 0);
        assert!(
            shown(&app, 0) > 1,
            "folding one project must not fold the other"
        );
        assert!(render(&app, 140, 45).contains("▸ mobile-app"));
    }

    #[test]
    fn a_folded_project_still_shows_what_waits_for_the_person() {
        let mut app = app_after(2, 24, 0, Lang::En);
        for _ in 0..5000 {
            app.step();
            if app
                .world
                .open_inbox()
                .iter()
                .any(|&i| app.world.agents[app.world.inbox[i].agent].project == 1)
            {
                break;
            }
        }
        app.collapsed[1] = true;
        let s = render(&app, 160, 45);
        let header = s
            .lines()
            .find(|l| l.contains("▸ mobile-app"))
            .expect("no folded header");
        assert!(header.contains("waiting"), "{header}");
    }

    /// Two helpers of the first worker of `app`'s first project; the worker
    /// and the helpers' ids.
    fn with_helpers(app: &mut App) -> (AgentId, [AgentId; 2]) {
        let worker = app
            .world
            .agents
            .iter()
            .find(|a| a.project == 0 && a.role == Role::Worker)
            .unwrap()
            .id;
        let name = app.world.agents[worker].name.clone();
        let mut ids = [0; 2];
        for (n, id) in ids.iter_mut().enumerate() {
            *id = app.world.agents.len();
            let mut h = Agent::new(
                *id,
                0,
                format!("{name}.{}", n + 1),
                Vendor::Claude,
                Role::Helper,
                Status::Working,
                app.world.tick,
            );
            h.parent = Some(worker);
            h.activity = Some("Edit src/x.rs".into());
            app.world.agents.push(h);
        }
        (worker, ids)
    }

    fn salon_rows(app: &App) -> Vec<(AgentId, u8)> {
        salon::rows(app, 100)
            .iter()
            .filter_map(|r| match r {
                salon::Row::Agent { id, depth } => Some((*id, *depth)),
                _ => None,
            })
            .collect()
    }

    /// One project whose agents are all calm and working for a long time:
    /// the leader, the workers in `w`, the researchers in `r`.
    fn calm_team() -> (App, Vec<AgentId>, Vec<AgentId>) {
        let mut app = app_after(1, 4, 0, Lang::En);
        let id = app.world.agents.len();
        app.world.agents.push(Agent::new(
            id,
            0,
            "res-1".into(),
            Vendor::Claude,
            Role::Researcher,
            Status::Working,
            0,
        ));
        app.world.tick = 100;
        app.selected = Sel::Project(0);
        for a in &mut app.world.agents {
            a.status = Status::Working;
            a.talk = None;
            a.changed_at = 0;
            a.gone = false;
        }
        let of = |app: &App, role: Role| -> Vec<AgentId> {
            app.world
                .agents
                .iter()
                .filter(|a| a.role == role)
                .map(|a| a.id)
                .collect()
        };
        let w = of(&app, Role::Worker);
        let r = of(&app, Role::Researcher);
        assert!(w.len() == 4 && !r.is_empty());
        (app, w, r)
    }

    #[test]
    fn a_worker_s_researcher_counts_as_a_line_and_its_talk_does_not_when_the_salon_fills() {
        let (mut app, w, r) = calm_team();
        app.world.agents[r[0]].status = Status::Researching {
            for_agent: w[0],
            until: 0,
        };
        app.world.agents[w[1]].talk = Some(Talk {
            with: Party::Person,
            mine: false,
            text: "on it".into(),
            until: 0,
        });
        let rows = |app: &App, capacity: usize| salon::rows(app, capacity);
        // Project, leader, w0 and its researcher, w1 (its talk on its own
        // row), w2, w3.
        let all = rows(&app, 7);
        assert_eq!(all.len(), 7, "{all:?}");
        assert!(!all.iter().any(|r| matches!(r, salon::Row::Folded { .. })));
        // One line short: the calm workers at the end fold into one line.
        let tight = rows(&app, 6);
        assert_eq!(tight.len(), 6, "{tight:?}");
        assert!(
            matches!(tight.last(), Some(salon::Row::Folded { n: 2, .. })),
            "{tight:?}"
        );
        assert!(
            tight.contains(&salon::Row::Agent { id: r[0], depth: 3 }),
            "{tight:?}"
        );
    }

    #[test]
    fn the_project_header_counts_the_workers_and_helpers_that_are_not_idle() {
        let (mut app, w, _) = calm_team();
        with_helpers(&mut app);
        app.world.agents[w[1]].status = Status::Idle;
        app.world.agents[w[2]].status = Status::Blocked;
        // w0, w2, w3 and the two helpers; not the idle worker, the leader
        // or the researchers.
        let s = render(&app, 140, 45);
        let header = s.lines().find(|l| l.contains("▾ ")).expect("no header");
        assert!(header.contains(" 5 working"), "{header}");
    }

    #[test]
    fn a_worker_s_helpers_sit_under_it_and_fold_with_the_project_keys() {
        let mut app = app_after(1, 4, 50, Lang::En);
        let (worker, [h1, h2]) = with_helpers(&mut app);
        let rows = salon_rows(&app);
        let at = rows.iter().position(|&r| r == (worker, 2)).unwrap();
        assert_eq!(&rows[at..at + 3], [(worker, 2), (h1, 3), (h2, 3)]);
        let s = render(&app, 140, 45);
        let name = app.world.agents[worker].name.clone();
        let line = |s: &str, name: &str| {
            s.lines()
                .find(|l| l.contains(&format!("▎{name} ")))
                .map(str::to_string)
                .unwrap_or_default()
        };
        assert!(line(&s, &name).contains("▾ 2 helpers"), "{s}");
        assert!(
            !line(&s, &name).contains(&format!("{name}.1")),
            "open: the helpers are under it, not on its row: {s}"
        );
        assert!(line(&s, &format!("{name}.1")).contains("├ "), "{s}");
        assert!(line(&s, &format!("{name}.2")).contains("└ "), "{s}");
        assert!(
            line(&s, &format!("{name}.2")).contains("Edit src/x.rs"),
            "{s}"
        );

        // h on a helper folds its worker and lands on it; h again folds the project.
        app.selected = Sel::Agent(h2);
        key(&mut app, KeyCode::Char('h'));
        assert!(app.folded_workers.contains(&worker));
        assert_eq!(app.selected, Sel::Agent(worker));
        assert_eq!(app.collapsed, [false]);
        let rows = salon_rows(&app);
        assert!(rows.contains(&(worker, 2)) && !rows.iter().any(|r| r.1 == 3));
        // A folded group shows its loudest member: both helpers work, the
        // first in order (a wide screen: the row ends with the member).
        let s = render(&app, 200, 45);
        assert!(
            line(&s, &name).contains(&format!("▸ 2 helpers  ● {name}.1")),
            "{s}"
        );
        assert!(!s.contains(&format!("{name}.2")), "{s}");
        app.world.agents[h2].status = Status::Blocked;
        let s = render(&app, 200, 45);
        assert!(
            line(&s, &name).contains(&format!("▸ 2 helpers  ✕ {name}.2")),
            "{s}"
        );
        app.world.agents[h2].status = Status::Working;
        key(&mut app, KeyCode::Char('h'));
        assert_eq!(app.collapsed, [true]);
        assert_eq!(app.selected, Sel::Project(0));
        // l opens the project, then the helpers; a worker without helpers
        // leaves the keys to the project.
        key(&mut app, KeyCode::Char('l'));
        assert_eq!(app.collapsed, [false]);
        app.selected = Sel::Agent(worker);
        key(&mut app, KeyCode::Char('l'));
        assert!(app.folded_workers.is_empty());
        assert_eq!(salon_rows(&app).iter().filter(|r| r.1 == 3).count(), 2);
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('a'));
        assert!(
            app.folded_workers.contains(&worker),
            "za toggles the worker"
        );
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('a'));
        assert!(app.folded_workers.is_empty());
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('M'));
        assert_eq!(app.collapsed, [true]);
        assert_eq!(
            app.folded_workers.iter().copied().collect::<Vec<_>>(),
            [worker]
        );
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('R'));
        assert!(app.collapsed == [false] && app.folded_workers.is_empty());
        // Gone helpers are not shown and not counted.
        app.world.agents[h1].gone = true;
        app.world.agents[h2].gone = true;
        assert!(!salon_rows(&app).iter().any(|r| r.1 == 3));
        assert!(!render(&app, 140, 45).contains("helpers"));
        key(&mut app, KeyCode::Char('h'));
        assert_eq!(app.collapsed, [true], "no helpers: h folds the project");
    }

    #[test]
    fn the_focus_of_a_helper_names_its_worker_and_a_helper_that_left_is_journaled() {
        let mut app = app_after(1, 4, 50, Lang::En);
        let (worker, [h1, _]) = with_helpers(&mut app);
        app.world.note(Event::Left { agent: h1 });
        let name = app.world.agents[worker].name.clone();
        let s = render(&app, 140, 45);
        assert!(s.contains(&format!("{name}.1 finished and left")), "{s}");
        app.zoom = Zoom::Focus(h1);
        let s = render(&app, 140, 45);
        assert!(s.contains(&format!("helper of {name}")), "{s}");
    }

    /// An open answer of `agent` in WAITING FOR YOU.
    fn waiting(app: &mut App, agent: AgentId) {
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Reply,
            agent,
            detail: "an answer".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        });
    }

    fn workers_shown(app: &App) -> Vec<AgentId> {
        salon_rows(app)
            .iter()
            .filter(|r| r.1 == 2)
            .map(|r| r.0)
            .collect()
    }

    #[test]
    fn a_pinned_project_comes_first_wears_its_mark_and_folds_last() {
        let mut app = app_after(2, 8, 0, Lang::En);
        app.world.inbox.clear();
        app.world.tick = 100;
        for a in &mut app.world.agents {
            a.status = Status::Working;
            a.changed_at = 0;
            a.talk = None;
        }
        app.selected = Sel::Project(0);
        let first = |app: &App| salon::rows(app, 100)[0];
        // Which project the folded line is under.
        let folded_under = |app: &App, capacity: usize| {
            let mut under = None;
            for r in salon::rows(app, capacity) {
                match r {
                    salon::Row::Project(p) => under = Some(p),
                    salon::Row::Folded { .. } => return under,
                    _ => {}
                }
            }
            None
        };
        let all = salon::rows(&app, 100).len();
        assert_eq!(first(&app), salon::Row::Project(0));
        assert_eq!(folded_under(&app, all - 1), Some(1), "the bottom one");
        app.world.projects[1].pinned = true;
        assert_eq!(first(&app), salon::Row::Project(1));
        assert_eq!(folded_under(&app, all - 1), Some(0), "the pinned one last");
        let name = app.world.projects[1].name.clone();
        let s = render(&app, 140, 40);
        assert!(s.contains(&format!("▾ ▴ {name}")), "{s}");
        assert!(!s.contains(&format!("▴ {}", app.world.projects[0].name)));
        app.help = true;
        assert!(
            render(&app, 140, 40).contains("▴ pinned"),
            "the legend says it"
        );
    }

    #[test]
    fn the_salon_order_puts_the_pinned_open_projects_first() {
        let mut w = app_after(3, 3, 0, Lang::En).world;
        assert_eq!(w.salon_order(), [0, 1, 2]);
        w.projects[2].pinned = true;
        assert_eq!(w.salon_order(), [2, 0, 1]);
        w.projects[0].pinned = true;
        assert_eq!(w.salon_order(), [0, 2, 1], "each group as opened");
        w.projects[2].closed = true;
        assert_eq!(w.salon_order(), [0, 1], "a closed one is not shown");
    }

    #[test]
    fn colon_pin_pins_and_unpins_the_project_at_hand_or_named() {
        let mut app = app_after(2, 2, 0, Lang::En);
        let (one, two) = (
            app.world.projects[0].name.clone(),
            app.world.projects[1].name.clone(),
        );
        app.selected = Sel::Project(0);
        let typed = |app: &mut App, line: &str| {
            for c in line.chars() {
                key(app, KeyCode::Char(c));
            }
        };
        typed(&mut app, ":pi");
        assert_eq!(app.menu.as_ref().unwrap().notes, [format!("({one})")]);
        typed(&mut app, "n");
        key(&mut app, KeyCode::Enter);
        assert!(app.world.projects[0].pinned);
        assert!(
            render(&app, 140, 40).contains(&format!("{one} is pinned to the top of the salon"))
        );
        typed(&mut app, ":pi");
        assert_eq!(app.menu.as_ref().unwrap().notes, [format!("(▴ {one})")]);
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Esc);
        typed(&mut app, ":pin");
        key(&mut app, KeyCode::Enter);
        assert!(!app.world.projects[0].pinned);
        assert!(render(&app, 140, 40).contains(&format!("{one} is not pinned any more")));
        typed(&mut app, &format!(":pin {two}"));
        key(&mut app, KeyCode::Enter);
        assert!(app.world.projects[1].pinned && !app.world.projects[0].pinned);
        typed(&mut app, ":pin nowhere");
        key(&mut app, KeyCode::Enter);
        assert!(render(&app, 140, 40).contains("no open project named nowhere"));
        assert!(app.world.projects[1].pinned, "nothing else changed");
    }

    #[test]
    fn workers_that_need_the_person_more_come_first_and_fold_last() {
        let (mut app, w, _) = calm_team();
        app.world.inbox.clear();
        app.world.agents[w[1]].status = Status::Idle;
        app.unseen.insert(w[1]);
        app.world.agents[w[2]].status = Status::Blocked;
        waiting(&mut app, w[3]);
        // Stuck first, then waiting for the person, then done, not seen.
        assert_eq!(workers_shown(&app), [w[2], w[3], w[1], w[0]]);
        // Equals keep their order.
        waiting(&mut app, w[0]);
        assert_eq!(workers_shown(&app), [w[2], w[0], w[3], w[1]]);

        // Folding starts at the bottom: done and not seen folds after the
        // rest. Project, lead and four workers in five lines: two fold,
        // since the folded line takes one.
        let (mut app, w, _) = calm_team();
        app.world.inbox.clear();
        app.world.agents[w[3]].status = Status::Idle;
        app.unseen.insert(w[3]);
        let rows = salon::rows(&app, 5);
        assert!(
            matches!(rows.last(), Some(salon::Row::Folded { n: 2, .. })),
            "{rows:?}"
        );
        assert!(
            rows.contains(&salon::Row::Agent { id: w[3], depth: 2 }),
            "{rows:?}"
        );
        assert!(
            !rows.contains(&salon::Row::Agent { id: w[2], depth: 2 }),
            "{rows:?}"
        );
        // The lead's talk is on its own row, no line more: six lines fit
        // six, not five.
        let lead = app.world.projects[0].leader;
        app.world.agents[lead].talk = Some(Talk {
            with: Party::Person,
            mine: true,
            text: "planning".into(),
            until: app.world.tick + 1,
        });
        let folds = |app: &App, capacity| {
            salon::rows(app, capacity)
                .iter()
                .any(|r| matches!(r, salon::Row::Folded { .. }))
        };
        assert!(!folds(&app, 6));
        assert!(folds(&app, 5));
    }

    #[test]
    fn only_a_calm_worker_folds_not_one_talking_just_changed_or_selected() {
        // Project, lead and four workers; in five lines two fold, the last
        // two.
        let folded = |app: &App, capacity, id| {
            !salon::rows(app, capacity).contains(&salon::Row::Agent { id, depth: 2 })
        };
        let (mut app, w, _) = calm_team();
        app.world.inbox.clear();
        assert!(!folded(&app, 6, w[3]), "it fits");
        assert!(folded(&app, 5, w[3]) && folded(&app, 5, w[2]));
        // Talked lately (its talk still news): no line more, but it stays;
        // a talk past its news time is calm again.
        let tick = app.world.tick;
        let talk = |until| {
            Some(Talk {
                with: Party::Person,
                mine: false,
                text: "on it".into(),
                until,
            })
        };
        app.world.agents[w[3]].talk = talk(tick + 1);
        assert!(!folded(&app, 5, w[3]) && folded(&app, 5, w[2]));
        app.world.agents[w[3]].talk = talk(tick);
        assert!(folded(&app, 5, w[3]));
        app.world.agents[w[3]].talk = None;
        // Changed six steps ago is news still; seven is calm.
        app.world.agents[w[3]].changed_at = app.world.tick - 6;
        assert!(!folded(&app, 5, w[3]));
        app.world.agents[w[3]].changed_at = app.world.tick - 7;
        assert!(folded(&app, 5, w[3]));
        // The selection never folds away.
        app.selected = Sel::Agent(w[3]);
        assert!(!folded(&app, 5, w[3]) && folded(&app, 5, w[1]));
    }

    #[test]
    fn the_salon_wears_the_focus_colour_while_it_has_the_keys() {
        let mut app = app_after(1, 4, 50, Lang::En);
        let corner = |app: &App| buffer(app, 140, 36)[(0, 1)].style().fg;
        assert_eq!(app.pane, Pane::Salon);
        assert_eq!(corner(&app), Some(pal().accent));
        app.pane = Pane::Inbox;
        assert_ne!(corner(&app), Some(pal().accent));
    }

    #[test]
    fn needs_only_shows_what_needs_you_with_the_worker_it_hangs_under() {
        let (mut app, w, r) = calm_team();
        app.world.inbox.clear();
        let (worker, [h1, h2]) = with_helpers(&mut app);
        assert_eq!(worker, w[0]);
        let lead = app.world.projects[0].leader;
        app.world.agents[lead].talk = Some(Talk {
            with: Party::Person,
            mine: true,
            text: "planning".into(),
            until: 0,
        });
        app.needs_only = true;
        let t = Lang::En.text();
        let all = |app: &App| salon::rows(app, 100);
        assert!(all(&app).is_empty());
        let s = render(&app, 140, 45);
        assert!(
            s.contains(t.salon_needs.trim()) && s.contains(t.salon_none),
            "{s}"
        );

        // A helper: its worker comes with it, the other helper does not.
        app.world.agents[h2].status = Status::Blocked;
        assert_eq!(salon_rows(&app), [(w[0], 2), (h2, 3)]);
        assert_eq!(all(&app)[0], salon::Row::Project(0));
        // A researcher that needs nothing stays out under a worker that
        // is shown; one with an item comes with its worker.
        app.world.agents[w[2]].status = Status::Blocked;
        app.world.agents[r[0]].status = Status::Researching {
            for_agent: w[2],
            until: 0,
        };
        assert_eq!(salon_rows(&app), [(w[2], 2), (w[0], 2), (h2, 3)]);
        app.world.agents[r[0]].status = Status::Researching {
            for_agent: w[1],
            until: 0,
        };
        waiting(&mut app, r[0]);
        assert_eq!(
            salon_rows(&app),
            [(w[2], 2), (w[0], 2), (h2, 3), (w[1], 2), (r[0], 3)]
        );
        assert!(!salon_rows(&app).contains(&(h1, 3)));
        // The lead when it needs the person; its talk is on its row.
        assert!(!all(&app).contains(&salon::Row::Agent { id: lead, depth: 1 }));
        app.world.agents[lead].status = Status::Blocked;
        assert_eq!(
            all(&app)[..2],
            [
                salon::Row::Project(0),
                salon::Row::Agent { id: lead, depth: 1 },
            ]
        );
        // What needs the person is never folded away, nor the worker a
        // helper hangs under.
        assert_eq!(salon::rows(&app, 1), all(&app));
        let s = render(&app, 140, 45);
        assert!(!s.contains(t.salon_none), "{s}");

        // Everyone again.
        app.needs_only = false;
        assert!(salon_rows(&app).contains(&(h1, 3)));
        let s = render(&app, 140, 45);
        assert!(
            !s.contains(t.salon_needs.trim()) && !s.contains(t.salon_none),
            "{s}"
        );
    }

    #[test]
    fn the_demo_s_salon_shows_only_what_needs_you_after_f() {
        for lang in [Lang::En, Lang::Tr] {
            let mut app = app_after(2, 12, 300, lang);
            key(&mut app, KeyCode::Char('f'));
            assert!(app.needs_only);
            let needs = app.needs();
            let rows = salon_rows(&app);
            assert!(!rows.is_empty(), "the demo has something waiting by now");
            for (id, _) in rows {
                let a = &app.world.agents[id];
                assert!(
                    needs[id] > Need::Nothing || a.role == Role::Worker,
                    "{} needs nothing",
                    a.name
                );
            }
            let s = render(&app, 140, 45);
            assert!(s.contains(lang.text().salon_needs.trim()), "{s}");
        }
    }

    #[test]
    fn many_agents_fold_the_calm_ones() {
        let app = app_after(1, 80, 200, Lang::En);
        let s = render(&app, 120, 30);
        assert!(s.contains("agents folded, loudest"), "no folded line:\n{s}");
    }

    #[test]
    fn the_last_branch_of_each_project_closes_its_tree() {
        let mut app = app_after(2, 6, 0, Lang::En);
        // Per project, the tree mark of each worker (or folded) line.
        let marks = |app: &App, height: u16| -> Vec<Vec<&'static str>> {
            let s = render(app, 140, height);
            let mut out: Vec<Vec<&'static str>> = Vec::new();
            for l in s.lines() {
                let l = l.strip_prefix('│').unwrap_or(l);
                if l.starts_with("▾ ") || l.starts_with("▸ ") {
                    out.push(Vec::new());
                } else if let Some(b) = out.last_mut() {
                    if l.starts_with("  ├ ") {
                        b.push("├");
                    } else if l.starts_with("  └ ") {
                        b.push("└");
                    }
                }
            }
            out
        };
        let all = marks(&app, 45);
        assert_eq!(all.len(), 2, "{all:?}");
        assert!(all.iter().any(|b| b.len() >= 2), "{all:?}");
        for b in &all {
            assert_eq!(b.last(), Some(&"└"), "{all:?}");
            assert!(b[..b.len() - 1].iter().all(|m| *m == "├"), "{all:?}");
        }
        // A folded line is the last branch too.
        for a in &mut app.world.agents {
            a.changed_at = 0;
            a.talk = None;
        }
        app.world.tick = 100;
        let s = render(&app, 140, 12);
        assert!(s.contains("  └ … "), "{s}");
        let tight = marks(&app, 12);
        for b in &tight {
            assert_eq!(b.last(), Some(&"└"), "{tight:?}");
            assert_eq!(b.iter().filter(|m| **m == "└").count(), 1, "{tight:?}");
        }
    }

    #[test]
    fn the_folded_line_names_the_loudest_of_the_folded() {
        // Project, lead and four workers in five lines: the last two in
        // order fold, all idle.
        let (mut app, w, _) = calm_team();
        app.world.inbox.clear();
        for &id in &w {
            app.world.agents[id].status = Status::Idle;
        }
        let folded = |app: &App| match salon::rows(app, 5).last() {
            Some(&salon::Row::Folded { n, loud }) => (n, loud),
            other => panic!("{other:?}"),
        };
        assert_eq!(folded(&app), (2, w[2]), "equals: the first in order");
        app.world.agents[w[3]].status = Status::Working;
        assert_eq!(folded(&app), (2, w[3]), "working over idle");
        app.world.agents[w[2]].status = Status::Working;
        app.world.agents[w[2]].starting = true;
        assert_eq!(folded(&app), (2, w[3]), "working over starting");
        let name = app.world.agents[w[3]].name.clone();
        // Five rows of salon: the frame, the queue line and the header
        // take the rest.
        let s = render(&app, 140, 10);
        let line = s
            .lines()
            .find(|l| l.contains("folded"))
            .unwrap_or_else(|| panic!("{s}"));
        assert!(
            line.contains(&format!("… 2 agents folded, loudest  ● {name}")),
            "{line}"
        );
        app.lang = Lang::Tr;
        let s = render(&app, 140, 10);
        assert!(
            s.contains(&format!("… 2 agent katlı, en yükseği  ● {name}")),
            "{s}"
        );
    }

    #[test]
    fn a_folded_project_s_header_ends_with_its_loudest_member_and_counts_the_stopped() {
        let (mut app, w, _) = calm_team();
        app.world.inbox.clear();
        let text =
            |line: &Line| -> String { line.spans.iter().map(|s| s.content.as_ref()).collect() };
        let header = |app: &App| text(&salon::project_line(app, 0, &app.rungs()));
        let name = |app: &App, id: AgentId| app.world.agents[id].name.clone();
        let lead = app.world.projects[0].leader;
        // Open: the counts, no member.
        let open = header(&app);
        assert!(
            open.contains("▾ ") && !open.contains(&name(&app, w[0])),
            "{open}"
        );
        assert!(!open.contains("✕") && !open.contains("■"), "{open}");
        // Folded: the loudest member last; the lead counts as a member.
        app.collapsed[0] = true;
        let h = header(&app);
        assert!(h.ends_with(&format!("  ◆ {}", name(&app, lead))), "{h}");
        app.world.agents[lead].status = Status::Idle;
        let h = header(&app);
        assert!(h.ends_with(&format!("  ● {}", name(&app, w[0]))), "{h}");
        app.world.agents[w[3]].status = Status::Stopped;
        app.world.agents[w[1]].status = Status::Blocked;
        let h = header(&app);
        assert!(h.contains("  ✕1  ■1"), "{h}");
        assert!(h.ends_with(&format!("  ✕ {}", name(&app, w[1]))), "{h}");
        app.world.agents[w[1]].status = Status::Working;
        let h = header(&app);
        assert!(!h.contains("✕1") && h.contains("  ■1"), "{h}");
        assert!(h.ends_with(&format!("  ■ {}", name(&app, w[3]))), "{h}");
        waiting(&mut app, w[2]);
        let h = header(&app);
        assert!(h.contains("! 1 waiting"), "{h}");
        assert!(
            h.ends_with(&format!("  ■ {}", name(&app, w[3]))),
            "stuck over waiting: {h}"
        );
        app.world.agents[w[3]].status = Status::Working;
        let h = header(&app);
        assert!(h.ends_with(&format!("  ! {}", name(&app, w[2]))), "{h}");
    }

    #[test]
    fn a_row_whose_item_waits_shows_the_waiting_mark_whatever_it_does() {
        let (mut app, w, _) = calm_team();
        app.world.inbox.clear();
        let row = |app: &App, id: AgentId| {
            salon::agent_line(app, &app.world.agents[id], "", &app.rungs(), 200)
        };
        app.world.agents[w[0]].status = Status::Idle;
        assert_eq!(row(&app, w[0]).spans[1].content, "○ ");
        waiting(&mut app, w[0]);
        let line = row(&app, w[0]);
        assert_eq!(
            (line.spans[1].content.as_ref(), line.spans[1].style),
            ("! ", fg(pal().warn))
        );
        assert_eq!(row(&app, w[1]).spans[1].content, "● ");
        waiting(&mut app, w[1]);
        assert_eq!(row(&app, w[1]).spans[1].content, "! ", "working too");
        // Stuck shows as stuck even with an item waiting.
        app.world.agents[w[1]].status = Status::Stopped;
        assert_eq!(row(&app, w[1]).spans[1].content, "■ ");
        // The lead is always the lead.
        let lead = app.world.projects[0].leader;
        waiting(&mut app, lead);
        assert_eq!(row(&app, lead).spans[1].content, "◆ ");
    }

    #[test]
    fn a_starting_session_says_so_on_its_row_until_it_answers() {
        let (mut app, w, _) = calm_team();
        app.world.inbox.clear();
        let text =
            |line: &Line| -> String { line.spans.iter().map(|s| s.content.as_ref()).collect() };
        let row =
            |app: &App| salon::agent_line(app, &app.world.agents[w[0]], "", &app.rungs(), 200);
        app.world.agents[w[0]].activity = Some(String::new());
        assert!(text(&row(&app)).contains("thinking…"));
        app.world.agents[w[0]].starting = true;
        let line = row(&app);
        assert_eq!(
            (line.spans[1].content.as_ref(), line.spans[1].style),
            ("◌ ", fg(pal().ok))
        );
        assert!(text(&line).contains(" starting…"), "{}", text(&line));
        assert!(!text(&line).contains("thinking…"));
        app.lang = Lang::Tr;
        assert!(text(&row(&app)).contains(" başlıyor…"));
        app.world.agents[w[0]].starting = false;
        assert!(text(&row(&app)).contains("düşünüyor…"));
    }

    #[test]
    fn a_bare_colon_solo_opens_the_pick_screen_which_asks_vendor_model_and_effort() {
        let mut app = app_after(1, 1, 0, Lang::En);
        app.home = None;
        let t = Lang::En.text();
        for c in ":solo".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        let s = render(&app, 140, 40);
        assert!(s.contains(t.solo_pick_title.trim()), "{s}");
        assert!(
            s.contains("Which coding agent?") && s.contains(" 1 claude"),
            "{s}"
        );
        assert!(s.contains("esc back"), "{s}");
        // A vendor's row wears its colour; only the chosen row is reversed.
        let buf = buffer(&app, 140, 40);
        let row = |y: u16| -> String { (0..140).map(|x| buf[(x, y)].symbol()).collect() };
        let cell = |needle: &str| {
            let y = (0..40).find(|&y| row(y).contains(needle)).expect(needle);
            let line = row(y);
            // Cells, not bytes: the frame's box characters are wide in bytes.
            let at = line.find(needle).unwrap();
            let x = line[..at].chars().count() + needle.chars().count() - 1;
            buf[(u16::try_from(x).unwrap(), y)].clone()
        };
        let claude = cell(" 1 claude");
        assert_eq!(claude.fg, vendor_color(Vendor::Claude));
        assert!(
            claude.modifier.contains(Modifier::REVERSED),
            "the chosen row"
        );
        let codex = cell(" 2 codex");
        assert_eq!(codex.fg, vendor_color(Vendor::Codex));
        assert!(!codex.modifier.contains(Modifier::REVERSED), "not chosen");
        // The keys go to the screen, not the salon: j moves its row.
        key(&mut app, KeyCode::Char('j'));
        assert_eq!(app.solo_pick.as_ref().unwrap().sel(), 1);
        key(&mut app, KeyCode::Enter);
        let s = render(&app, 140, 40);
        assert!(
            s.contains("Which model?") && s.contains("default (the vendor's own)"),
            "{s}"
        );
        assert!(s.contains(" codex"), "what was picked so far: {s}");
        key(&mut app, KeyCode::Enter);
        let s = render(&app, 140, 40);
        assert!(
            s.contains("How deep should it think?") && s.contains(" 3 medium"),
            "{s}"
        );
        key(&mut app, KeyCode::Char('4'));
        key(&mut app, KeyCode::Enter);
        assert!(app.solo_pick.is_none());
        let head = app.world.projects[1].leader;
        assert_eq!(app.world.agents[head].vendor, Vendor::Codex);
        assert_eq!(app.model_of(head).as_deref(), Some("@high"));
        // Turkish words too.
        app.lang = Lang::Tr;
        for c in ":solo".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        let s = render(&app, 140, 40);
        assert!(s.contains("Hangi kodlama ajanı?"), "{s}");
        key(&mut app, KeyCode::Esc);
        assert!(app.solo_pick.is_none());
    }

    #[test]
    fn a_solo_s_helper_counts_as_a_line_when_the_salon_decides_what_to_fold() {
        let mut app = app_after(1, 3, 0, Lang::En);
        app.home = None;
        // Calm workers: idle, quiet, unchanged for a while, not selected.
        app.world.tick = 20;
        for c in ":solo claude notes".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        let head = app.world.projects[1].leader;
        let helper = app.world.agents.len();
        let mut h = Agent::new(
            helper,
            1,
            "solo-notes.1".into(),
            Vendor::Claude,
            Role::Helper,
            Status::Working,
            20,
        );
        h.parent = Some(head);
        app.world.agents.push(h);
        app.selected = Sel::Project(1);
        // Two headers, the leader, three workers, the solo and its helper.
        let all = salon::rows(&app, 100);
        assert_eq!(all.len(), 8, "{all:?}");
        assert!(!all.iter().any(|r| matches!(r, salon::Row::Folded { .. })));
        assert_eq!(salon::rows(&app, 8).len(), 8, "it fits exactly");
        // One line short: a calm worker folds, because the helper's line
        // was counted.
        let tight = salon::rows(&app, 7);
        assert!(
            tight.iter().any(|r| matches!(r, salon::Row::Folded { .. })),
            "{tight:?}"
        );
    }

    #[test]
    fn agent_text_reaches_the_screen_clean() {
        let mut app = app_after(1, 4, 50, Lang::En);
        let id = app
            .world
            .agents
            .iter()
            .find(|a| a.role == Role::Worker)
            .unwrap()
            .id;
        app.world.said(id, "\u{1b}[2Jhello sk-secret123");
        let s = render(&app, 140, 40);
        assert!(!s.contains('\u{1b}'));
        assert!(!s.contains("sk-secret123"), "{s}");
        assert!(s.contains("hello"));
    }

    #[test]
    fn a_conversation_shows_its_speakers_in_their_colours_and_folds() {
        let mut app = app_after(1, 4, 50, Lang::En);
        let lead = app.world.projects[0].leader;
        let worker = app
            .world
            .agents
            .iter()
            .find(|a| a.role == Role::Worker)
            .unwrap()
            .id;
        for (from, to, text) in [
            (lead, worker, "take the parser"),
            (worker, lead, "on it"),
            (lead, worker, "newest-line"),
        ] {
            app.world.note(Event::Sent {
                from,
                to,
                kind: MsgKind::Inform,
                text: text.into(),
                id: 0,
                re: None,
            });
        }
        let mut t = Terminal::new(TestBackend::new(140, 40)).unwrap();
        t.draw(|f| draw(f, &app)).unwrap();
        let buf = t.backend().buffer();
        let row = (0..buf.area.height)
            .find(|&y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .contains("newest-line")
            })
            .expect("the newest line is in the journal");
        let text: String = (0..buf.area.width)
            .map(|x| buf[(x, row)].symbol())
            .collect();
        assert!(
            text.contains("+2"),
            "the older two lines fold under it: {text}"
        );
        let full = render(&app, 140, 40);
        assert!(!full.contains("on it"), "folded lines stay folded:\n{full}");
        let name = &app.world.agents[lead].name;
        let at = text.find(name.as_str()).unwrap();
        let x = u16::try_from(text[..at].chars().count()).unwrap();
        assert_eq!(buf[(x, row)].fg, speaker_color(lead), "{text}");
    }

    #[test]
    fn a_salon_row_carries_its_speaker_colour_before_the_name() {
        let app = app_after(1, 4, 50, Lang::En);
        let mut t = Terminal::new(TestBackend::new(140, 40)).unwrap();
        t.draw(|f| draw(f, &app)).unwrap();
        let buf = t.backend().buffer();
        for a in app.world.agents.iter().filter(|a| !a.gone) {
            let Some((x, y)) = (0..buf.area.height).find_map(|y| {
                let text: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
                let at = text.find(&format!("{}{}", salon::SPEAKER_MARK, a.name))?;
                Some((u16::try_from(text[..at].chars().count()).unwrap(), y))
            }) else {
                continue; // folded away
            };
            assert_eq!(buf[(x, y)].fg, speaker_color(a.id), "{}", a.name);
            assert_eq!(buf[(x + 1, y)].fg, vendor_color(a.vendor), "{}", a.name);
        }
        let render = render(&app, 140, 40);
        assert!(
            render.contains(&format!(
                "{}{}",
                salon::SPEAKER_MARK,
                app.world.agents[0].name
            )),
            "{render}"
        );
    }

    #[test]
    fn a_long_answer_wraps_and_its_end_stays_in_view() {
        let mut app = app_after(1, 4, 50, Lang::En);
        let id = app
            .world
            .agents
            .iter()
            .find(|a| a.card.is_some())
            .unwrap()
            .id;
        let card = app.world.agents[id].card;
        let long = format!("{} THE-END", "word ".repeat(400));
        for _ in 0..30 {
            app.world.story(card, StoryEvent::Decision("filler".into()));
        }
        app.world.story(card, StoryEvent::Replied(long.clone()));
        app.zoom = Zoom::Focus(id);
        assert!(render(&app, 100, 30).contains("THE-END"));
        app.world.agents[id].trace(0, long);
        app.zoom = Zoom::Trace(id);
        assert!(render(&app, 100, 30).contains("THE-END"));
    }

    #[test]
    fn every_view_draws_at_every_size_in_every_theme() {
        for (theme, lang) in crate::theme::builtins().zip([Lang::En, Lang::Tr].into_iter().cycle())
        {
            assert!(crate::theme::set(theme));
            let mut app = app_after(3, 30, 400, lang);
            let views = [
                (View::Salon, Zoom::None),
                (View::Board, Zoom::None),
                (View::Code, Zoom::None),
                (View::Salon, Zoom::Focus(5)),
                (View::Salon, Zoom::Trace(5)),
                (View::Salon, Zoom::Focus(0)),
                (View::Salon, Zoom::Diff(5)),
            ];
            for (view, zoom) in views {
                app.view = view;
                app.zoom = zoom;
                app.pane = Pane::Inbox;
                app.selected = Sel::Project(1);
                for (w, h) in [(1, 1), (10, 3), (40, 12), (80, 24), (200, 60)] {
                    render(&app, w, h);
                }
            }
            app.help = true;
            render(&app, 30, 10);
        }
        crate::theme::set("terminal");
    }

    #[test]
    fn a_scheme_paints_the_whole_screen_and_the_terminal_theme_leaves_it() {
        let app = app_after(2, 6, 60, Lang::En);
        assert!(crate::theme::set("dracula"));
        let p = pal();
        let buf = buffer(&app, 80, 24);
        assert!(
            buf.content().iter().all(|c| c.bg == p.bg
                || c.bg == p.selection
                || c.bg == p.key
                || c.bg == p.accent
                || c.bg == p.warn),
            "every cell has the scheme's background or a badge's"
        );
        assert!(
            buf.content().iter().all(|c| c.fg != Color::Reset),
            "and its text colour"
        );
        let mut app = app;
        app.help = true;
        let buf = buffer(&app, 80, 24);
        assert!(
            buf.content().iter().all(|c| c.bg != Color::Reset),
            "a window over the screen keeps it"
        );
        crate::theme::set("terminal");
        let buf = buffer(&app, 80, 24);
        assert!(buf.content().iter().any(|c| c.bg == Color::Reset));
    }

    #[test]
    fn capital_t_cycles_the_theme_and_says_which() {
        let mut app = app_after(1, 4, 50, Lang::En);
        key(&mut app, KeyCode::Char('T'));
        assert_eq!(crate::theme::pal().name, "dark");
        assert!(render(&app, 140, 40).contains("theme: dark"));
        key(&mut app, KeyCode::Char('T'));
        assert_eq!(crate::theme::pal().name, "light");
        key(&mut app, KeyCode::Char('T'));
        assert_eq!(crate::theme::pal().name, "mono");
        key(&mut app, KeyCode::Char('T'));
        assert_eq!(crate::theme::pal().name, "dracula");
        for _ in 0..crate::theme::builtins().count() - 4 {
            key(&mut app, KeyCode::Char('T'));
        }
        assert_eq!(crate::theme::pal().name, "terminal", "around the end");
    }

    #[test]
    fn the_views_name_themselves() {
        let mut app = app_after(2, 12, 300, Lang::En);
        app.view = View::Board;
        assert!(render(&app, 140, 40).contains("IN PROGRESS"));
        app.view = View::Code;
        assert!(render(&app, 140, 40).contains("CODE MAP"));
        app.zoom = Zoom::Focus(3);
        assert!(render(&app, 140, 40).contains("FOCUS"));
    }

    fn key(app: &mut App, code: KeyCode) {
        app.on_key(KeyEvent::from(code));
    }

    #[test]
    fn an_answer_is_in_the_inbox_and_enter_reads_it_whole() {
        let mut app = app_after(1, 4, 50, Lang::En);
        let answer = "Short first line.\nTHE-REST";
        app.world.note(Event::Replied {
            agent: 0,
            text: answer.into(),
        });
        app.world.replied(0, answer);
        let screen = render(&app, 140, 40);
        assert!(
            screen.contains("answered you: Short first line."),
            "{screen}"
        );
        app.pane = Pane::Inbox;
        app.inbox_sel = app.world.open_inbox().len() - 1;
        key(&mut app, KeyCode::Enter);
        assert!(app.thread.is_some(), "the conversation opens");
        assert!(render(&app, 140, 40).contains("THE-REST"));
        assert!(
            app.world
                .open_inbox()
                .iter()
                .all(|&i| app.world.inbox[i].kind != InboxKind::Reply)
        );
    }

    #[test]
    fn a_question_in_the_inbox_opens_the_conversation_and_the_composer_to_its_agent() {
        let mut app = app_after(1, 4, 50, Lang::En);
        let worker = app
            .world
            .agents
            .iter()
            .find(|a| a.role == Role::Worker)
            .unwrap()
            .id;
        let name = app.world.agents[worker].name.clone();
        app.world.agents[worker].status = Status::Idle;
        app.world.note(Event::ToYou {
            from: worker,
            kind: MsgKind::Ask,
            text: "Which API?\nTHE-REST".into(),
        });
        app.world.asked_you(worker, "Which API?\nTHE-REST");
        let screen = render(&app, 140, 40);
        assert!(
            screen.contains(&format!("{name} asks you: Which API?")),
            "{screen}"
        );
        // The worker's line says it waits for the person.
        let words = |app: &App, lang| {
            status_spans(lang, &app.world, &app.world.agents[worker])
                .iter()
                .map(|s| s.content.to_string())
                .collect::<String>()
        };
        assert!(
            words(&app, Lang::En).ends_with(" ? → you \"Which API?\""),
            "{:?}",
            words(&app, Lang::En)
        );
        assert!(
            words(&app, Lang::Tr).ends_with(" ? → sen \"Which API?\""),
            "{:?}",
            words(&app, Lang::Tr)
        );
        let i = *app.world.open_inbox().last().unwrap();
        assert_eq!(inbox_line(&app, i).spans[0].content, "? ");
        app.pane = Pane::Inbox;
        app.inbox_sel = app.world.open_inbox().len() - 1;
        key(&mut app, KeyCode::Enter);
        assert!(app.thread.is_some(), "the conversation opens");
        assert!(render(&app, 140, 40).contains("THE-REST"));
        assert!(app.world.has_question(worker), "reading is not answering");
        app.thread = None;
        for k in ['a', 'r'] {
            key(&mut app, KeyCode::Char(k));
            assert!(
                app.typing && app.input == format!("@{name} "),
                "{k}: {:?}",
                app.input
            );
            app.typing = false;
            app.input.clear();
        }
        assert!(app.world.has_question(worker));
        // Writing back closes it.
        app.world.said(worker, "This one.");
        assert!(!app.world.has_question(worker));
        assert_eq!(words(&app, Lang::En), "idle");
    }

    fn a_change(agent: AgentId) -> Change {
        Change {
            agent,
            title: "Fix the parser".into(),
            files: vec![
                ChangedFile {
                    path: "src/p.rs".into(),
                    added: 2,
                    removed: 1,
                    binary: false,
                },
                ChangedFile {
                    path: "src/q.rs".into(),
                    added: 1,
                    removed: 0,
                    binary: false,
                },
            ],
            patch: "diff --git a/src/p.rs b/src/p.rs\n--- a/src/p.rs\n+++ b/src/p.rs\n\
                    @@ -1,2 +1,3 @@\n context\n-OLD-LINE\n+NEW-LINE\n+ANOTHER\n\
                    diff --git a/src/q.rs b/src/q.rs\n--- a/src/q.rs\n+++ b/src/q.rs\n\
                    @@ -1 +1,2 @@\n q\n+Q-LINE\x07\n"
                .into(),
            runs: Vec::new(),
        }
    }

    #[test]
    fn a_change_s_line_says_what_ran_on_its_tree_in_the_colour_of_how_it_went() {
        let mut app = app_after(1, 4, 50, Lang::En);
        app.world.inbox.clear();
        let ran = |line: &str, code| Ran {
            line: line.into(),
            code: Some(code),
            timed_out: false,
        };
        for (runs, says, colour) in [
            (vec![], " · no run on this tree", pal().warn),
            (
                vec![ran("cargo test", 0), ran("cargo build", 0)],
                " · ✓ ran clean on this tree: cargo test, cargo build",
                pal().ok,
            ),
            (
                vec![
                    ran("cargo test", 0),
                    ran("cargo clippy", 101),
                    ran("make", 2),
                ],
                " · ✗ failed on this tree: cargo clippy, make",
                pal().bad,
            ),
        ] {
            app.world.change_ready(Change {
                runs,
                ..a_change(1)
            });
            let open = app.world.open_inbox();
            let line = inbox_line(&app, *open.last().unwrap());
            let last = line.spans.last().unwrap();
            assert_eq!((last.content.as_ref(), last.style), (says, fg(colour)));
        }
        // Only a change's line: the same worker's answer has none.
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Reply,
            agent: 1,
            detail: "an answer".into(),
            researcher: None,
            open: true,
            tick: 50,
            card: None,
        });
        let line = inbox_line(&app, app.world.inbox.len() - 1);
        assert!(
            line.spans.last().unwrap().content.ends_with("an answer"),
            "{line:?}"
        );
        app.lang = Lang::Tr;
        let open = app.world.open_inbox();
        let line = inbox_line(&app, open[0]);
        assert_eq!(
            line.spans.last().unwrap().content,
            " · ✗ bu ağaçta başarısız: cargo clippy, make"
        );
    }

    /// A demo team at its start, everyone idle and nothing waiting.
    fn idle_team(lang: Lang) -> App {
        let mut app = app_after(1, 4, 0, lang);
        app.world.inbox.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
        }
        app
    }

    #[test]
    fn the_fleet_counts_who_works_who_is_done_unseen_and_who_is_stuck() {
        let text =
            |app: &App| -> String { fleet(app).iter().map(|s| s.content.as_ref()).collect() };
        let mut app = idle_team(Lang::En);
        assert_eq!(text(&app), "all idle");
        assert_eq!(fleet(&app)[0].style, dim());
        app.world.agents[0].status = Status::Working;
        app.world.agents[1].status = Status::Researching {
            for_agent: 0,
            until: 9,
        };
        assert_eq!(text(&app), "● 2 working");
        assert_eq!(fleet(&app)[0].style, fg(pal().ok));
        app.unseen.insert(2);
        assert_eq!(text(&app), "● 2 working  ✓ 1 done");
        assert_eq!(fleet(&app)[2].style, fg(pal().ok));
        app.world.agents[3].status = Status::Blocked;
        app.world.agents[4].status = Status::Stopped;
        assert_eq!(text(&app), "● 2 working  ✓ 1 done  ✕ 2 stuck");
        assert_eq!(fleet(&app)[4].style, fg(pal().bad));
        // What is gone is not counted, done or stuck.
        app.world.agents[4].gone = true;
        app.world.agents[2].gone = true;
        assert_eq!(text(&app), "● 2 working  ✕ 1 stuck");
        app.world.agents[0].status = Status::Idle;
        app.world.agents[1].status = Status::Idle;
        assert_eq!(text(&app), "✕ 1 stuck", "no separator before the first");
        app.lang = Lang::Tr;
        app.world.agents[0].status = Status::Working;
        assert_eq!(text(&app), "● 1 çalışıyor  ✕ 1 takıldı");
        let s = render(&app, 160, 30);
        assert!(
            s.contains("● 1 çalışıyor  ✕ 1 takıldı"),
            "in the header: {s}"
        );
    }

    #[test]
    fn a_row_done_and_not_seen_says_so_and_a_leader_keeps_its_mark() {
        let mut app = idle_team(Lang::En);
        let worker = app
            .world
            .agents
            .iter()
            .find(|a| a.role == Role::Worker)
            .unwrap()
            .id;
        let row = |app: &App, id: AgentId| {
            salon::agent_line(app, &app.world.agents[id], "", &app.rungs(), 200)
        };
        let text =
            |line: &Line| -> String { line.spans.iter().map(|s| s.content.as_ref()).collect() };
        assert_eq!(row(&app, worker).spans[1].content, "○ ");
        app.unseen.insert(worker);
        let line = row(&app, worker);
        assert_eq!(
            (line.spans[1].content.as_ref(), line.spans[1].style),
            ("✓ ", fg(pal().ok))
        );
        assert!(
            text(&line).contains("done, not seen yet"),
            "{}",
            text(&line)
        );
        assert!(!text(&line).contains(" idle"), "instead of idle");
        app.unseen.insert(0);
        let line = row(&app, 0);
        assert_eq!(line.spans[1].content, "◆ ");
        assert!(text(&line).contains("done, not seen yet"));
        app.lang = Lang::Tr;
        assert!(text(&row(&app, worker)).contains("bitti, henüz bakılmadı"));
    }

    #[test]
    fn a_worker_s_row_says_where_its_worktree_stands_and_only_what_there_is() {
        let text =
            |spans: &[Span]| -> String { spans.iter().map(|s| s.content.as_ref()).collect() };
        let stand = |branch: Option<&str>, ahead, behind, added, removed| Stand {
            branch: branch.map(String::from),
            ahead,
            behind,
            added,
            removed,
        };
        assert!(salon::stand_spans(&Stand::default()).is_empty());
        let all = salon::stand_spans(&stand(Some("fix-parser"), 2, 1, 12, 3));
        assert_eq!(text(&all), "  ⎇ fix-parser ↑2 ↓1 +12 −3");
        let styles: Vec<Style> = all.iter().skip(1).step_by(2).map(|s| s.style).collect();
        assert_eq!(
            styles,
            [
                dim(),
                fg(pal().info),
                fg(pal().warn),
                fg(pal().ok),
                fg(pal().bad)
            ]
        );
        // Each part alone, the first after two spaces.
        assert_eq!(text(&salon::stand_spans(&stand(None, 1, 0, 0, 0))), "  ↑1");
        assert_eq!(text(&salon::stand_spans(&stand(None, 0, 4, 0, 0))), "  ↓4");
        assert_eq!(text(&salon::stand_spans(&stand(None, 0, 0, 5, 0))), "  +5");
        assert_eq!(text(&salon::stand_spans(&stand(None, 0, 0, 0, 6))), "  −6");
        assert_eq!(
            text(&salon::stand_spans(&stand(None, 0, 0, 5, 6))),
            "  +5 −6"
        );
        // An agent's branch name is cleaned and cut.
        let long = salon::stand_spans(&stand(Some("a-very-long-branch-name\u{1b}[2J"), 0, 0, 0, 0));
        assert_eq!(text(&long), "  ⎇ a-very-long-bra…");

        // On a worker's row, after its cost; never on a leader's.
        let mut app = idle_team(Lang::En);
        let worker = app
            .world
            .agents
            .iter()
            .find(|a| a.role == Role::Worker)
            .unwrap()
            .id;
        let row = |app: &App, id: AgentId| {
            let line = salon::agent_line(app, &app.world.agents[id], "", &app.rungs(), 200);
            text(&line.spans)
        };
        app.world.agents[worker].stand = stand(None, 0, 0, 7, 2);
        app.world.agents[worker].cost = 150;
        let want = format!("{}  +7 −2", dollars(150));
        assert!(row(&app, worker).ends_with(&want), "{}", row(&app, worker));
        app.world.agents[0].stand = stand(None, 0, 0, 7, 2);
        assert!(!row(&app, 0).contains("+7"), "{}", row(&app, 0));
    }

    const OPEN_1: &str = "\x1b]8;id=imece-3;http://localhost:1/\x1b\\";
    const CLOSE: &str = "\x1b]8;;\x1b\\";

    #[test]
    fn a_link_cell_opens_and_closes_its_own_link_and_keeps_its_width() {
        let url = "http://localhost:1/";
        let mut buf = Buffer::with_lines(["abcd"]);
        hyperlink(&mut buf, Rect::new(1, 0, 2, 1), url, 3);
        let symbols: Vec<&str> = (0..4).map(|x| buf[(x, 0)].symbol()).collect();
        assert_eq!(
            symbols,
            [
                "a",
                &format!("{OPEN_1}b{CLOSE}"),
                &format!("{OPEN_1}c{CLOSE}"),
                "d"
            ]
        );
        let one = CellDiffOption::ForcedWidth(NonZeroU16::MIN);
        assert_eq!(buf[(1, 0)].diff_option, one);
        assert_eq!(buf[(2, 0)].diff_option, one);
        assert_eq!(buf[(0, 0)].diff_option, CellDiffOption::None);
        // Cut to the buffer.
        let mut buf = Buffer::with_lines(["abcd", "efgh"]);
        hyperlink(&mut buf, Rect::new(3, 1, 5, 2), url, 3);
        assert_eq!(buf[(3, 1)].symbol(), format!("{OPEN_1}h{CLOSE}"));
        assert_eq!(buf[(2, 1)].symbol(), "g");
        assert_eq!(buf[(3, 0)].symbol(), "d");
        // A control character, which could end the sequence, links nothing.
        let mut buf = Buffer::with_lines(["abcd"]);
        let before = buf.clone();
        hyperlink(
            &mut buf,
            Rect::new(0, 0, 4, 1),
            "http://localhost:1/\x07",
            3,
        );
        assert_eq!(buf, before);
    }

    #[test]
    fn a_link_is_written_in_one_run_and_a_cell_redrawn_alone_closes_its_own() {
        use ratatui::backend::{Backend, CrosstermBackend};
        let url = "http://localhost:1/";
        let written = |prev: &Buffer, next: &Buffer| {
            let mut out = Vec::new();
            CrosstermBackend::new(&mut out)
                .draw(prev.diff(next).into_iter())
                .unwrap();
            String::from_utf8(out).unwrap()
        };
        let mut next = Buffer::with_lines(["abcd"]);
        hyperlink(&mut next, Rect::new(1, 0, 2, 1), url, 3);
        let all = written(&Buffer::empty(next.area), &next);
        // One cursor move: each cell is one column wide, as ratatui counts.
        assert!(
            all.contains(&format!("a{OPEN_1}b{CLOSE}{OPEN_1}c{CLOSE}d")),
            "{all:?}"
        );
        assert_eq!(all.matches('H').count(), 1, "{all:?}");
        let mut later = Buffer::with_lines(["abxd"]);
        hyperlink(&mut later, Rect::new(1, 0, 2, 1), url, 3);
        assert_eq!(next.diff(&later).len(), 1);
        let one = written(&next, &later);
        assert!(one.contains(&format!("{OPEN_1}x{CLOSE}")), "{one:?}");
        assert_eq!(one.matches(CLOSE).count(), 1);
        // Gone again: the plain cells are written over the linked ones.
        let plain = Buffer::with_lines(["abcd"]);
        assert_eq!(later.diff(&plain).len(), 2);
    }

    #[test]
    fn a_worker_s_or_helper_s_loopback_url_is_shown_as_it_is_and_no_one_else_s() {
        let text =
            |spans: &[Span]| -> String { spans.iter().map(|s| s.content.as_ref()).collect() };
        let agent = |role, url: &str| {
            let mut a = Agent::new(3, 0, "w".into(), Vendor::Claude, role, Status::Idle, 0);
            a.local = Some(url.into());
            a
        };
        let url = "http://localhost:5173/";
        for role in [Role::Worker, Role::Helper, Role::Solo] {
            assert_eq!(salon::linked(&agent(role, url)), Some(url));
            let spans = salon::local_spans(&agent(role, url));
            assert_eq!(text(&spans), "  ↗ localhost:5173/");
            assert_eq!(spans[0].style, dim());
            assert_eq!(
                spans[1].style,
                fg(pal().info).add_modifier(Modifier::UNDERLINED)
            );
        }
        for role in [Role::Leader, Role::Researcher] {
            assert_eq!(salon::linked(&agent(role, url)), None);
            assert!(salon::local_spans(&agent(role, url)).is_empty());
        }
        let mut none = agent(Role::Worker, url);
        none.local = None;
        assert!(salon::local_spans(&none).is_empty());
        // Only `http://` is left out of the text.
        let https = agent(Role::Worker, "https://127.0.0.1:8443/");
        assert_eq!(
            text(&salon::local_spans(&https)),
            "  ↗ https://127.0.0.1:8443/"
        );
        // A path cleaning would hide is not shown: the text is the link.
        let secret = agent(Role::Worker, "http://localhost:1/TOKEN=abc");
        assert_eq!(salon::linked(&secret), None);
        assert!(salon::local_spans(&secret).is_empty());
    }

    #[test]
    fn a_link_covers_only_a_url_its_row_shows_whole() {
        let list = Rect::new(2, 3, 20, 5);
        assert_eq!(
            salon::link_cells(list, 1, 5, 15),
            Some(Rect::new(7, 4, 15, 1))
        );
        assert_eq!(
            salon::link_cells(list, 4, 0, 20),
            Some(Rect::new(2, 7, 20, 1))
        );
        assert_eq!(salon::link_cells(list, 1, 5, 16), None, "cut at the edge");
        assert_eq!(salon::link_cells(list, 5, 0, 1), None, "below the list");
        assert_eq!(salon::link_cells(list, 1, 70_000, 1), None);
    }

    #[test]
    fn the_salon_links_a_worker_s_loopback_url_on_its_row() {
        let mut app = idle_team(Lang::En);
        let worker = app
            .world
            .agents
            .iter()
            .find(|a| a.role == Role::Worker)
            .unwrap()
            .id;
        app.world.agents[worker].local = Some("http://localhost:5173/".into());
        app.world.agents[0].local = Some("http://localhost:9999/".into());
        let open = format!("\x1b]8;id=imece-{worker};http://localhost:5173/\x1b\\");
        let drawn = |width| {
            let mut t = Terminal::new(TestBackend::new(width, 30)).unwrap();
            t.draw(|f| draw(f, &app)).unwrap();
            t.backend().buffer().clone()
        };
        let buf = drawn(200);
        let cells: Vec<ratatui::layout::Position> = buf
            .area
            .positions()
            .filter(|&p| buf[p].symbol().starts_with(&open))
            .collect();
        let shown: String = cells
            .iter()
            .map(|&p| {
                let s = buf[p].symbol().strip_prefix(&open).unwrap();
                s.strip_suffix(CLOSE).unwrap().to_string()
            })
            .collect();
        assert_eq!(shown, "localhost:5173/");
        // Side by side on the worker's row, right after the arrow.
        let (x, y) = (cells[0].x, cells[0].y);
        assert!(
            cells
                .iter()
                .enumerate()
                .all(|(i, p)| p.y == y && p.x == x + i as u16)
        );
        let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
        assert!(row.contains(&app.world.agents[worker].name), "{row:?}");
        let arrow: String = (x - 4..x).map(|x| buf[(x, y)].symbol()).collect();
        assert_eq!(arrow, "  ↗ ");
        let all: String = buf.content.iter().map(|c| c.symbol()).collect();
        assert!(!all.contains("9999"), "never on a leader's row");
        assert_eq!(all.matches("]8;id=").count(), 15, "nothing else is linked");
        // Too narrow to show it whole: the text is cut, nothing linked.
        let narrow = drawn(116);
        let all: String = narrow.content.iter().map(|c| c.symbol()).collect();
        assert!(!all.contains("]8;"), "{all:?}");
        assert!(all.contains("↗ local") && !all.contains("localhost:5173/"));
    }

    #[test]
    fn before_you_go_lists_who_is_mid_turn_and_the_changes_not_reviewed() {
        let quiet = |projects| {
            let mut app = app_after(projects, 4, 0, Lang::En);
            app.world.inbox.clear();
            app.world.change_ready(a_change(1));
            for a in &mut app.world.agents {
                a.status = Status::Idle;
            }
            app.leaving = true;
            app
        };
        let rows = |app: &App| -> Vec<String> {
            render(app, 140, 40).lines().map(str::to_string).collect()
        };
        let at = |rows: &[String], words: &str| rows.iter().position(|r| r.contains(words));
        let mut app = quiet(1);
        app.world.agents[2].status = Status::Working;
        let name = app.world.agents[2].name.clone();
        let s = rows(&app);
        let title = at(&s, "BEFORE YOU GO").expect("the window");
        assert_eq!(
            at(&s, "Mid-turn: quitting cuts these turns short"),
            Some(title + 1)
        );
        assert!(s[title + 2].contains(&name), "{}", s[title + 2]);
        let project = app.world.projects[0].name.clone();
        assert!(!s[title + 2].contains(&project), "one project: no name");
        assert!(
            s[title + 3].trim_matches(['│', ' ']).is_empty(),
            "a blank row between"
        );
        assert_eq!(at(&s, "Changes not reviewed"), Some(title + 4));
        assert!(s[title + 5].contains("Fix the parser"), "{}", s[title + 5]);
        assert!(s[title + 7].contains("q quits · W opens the next waiting"));
        // Only the change.
        app.world.agents[2].status = Status::Idle;
        let s = rows(&app);
        let title = at(&s, "BEFORE YOU GO").unwrap();
        assert_eq!(at(&s, "Changes not reviewed"), Some(title + 1));
        assert_eq!(at(&s, "Mid-turn"), None);
        // Only the turn: the change was reviewed.
        app.world.agents[2].status = Status::Working;
        app.world.inbox.iter_mut().for_each(|i| i.open = false);
        let s = rows(&app);
        assert!(at(&s, "Mid-turn").is_some());
        assert_eq!(at(&s, "Changes not reviewed"), None);
        // Two projects: each agent's project is named.
        let mut app = quiet(2);
        app.world.agents[2].status = Status::Working;
        let project = app.world.projects[app.world.agents[2].project].name.clone();
        let s = rows(&app);
        let title = at(&s, "BEFORE YOU GO").unwrap();
        assert!(s[title + 2].contains(&project), "{}", s[title + 2]);
        app.lang = Lang::Tr;
        let s = rows(&app);
        assert!(
            at(&s, "ÇIKMADAN ÖNCE").is_some() && at(&s, "İncelenmemiş değişiklikler").is_some()
        );
    }

    #[test]
    fn an_item_put_off_says_later_up_front_in_both_languages() {
        let mut app = app_after(1, 4, 0, Lang::En);
        app.world.inbox.clear();
        for n in 0..2 {
            app.world.inbox.push(InboxItem {
                kind: InboxKind::Sensitive,
                agent: 1,
                detail: format!("item {n}"),
                researcher: None,
                open: true,
                tick: 0,
                card: None,
            });
        }
        app.deferred = vec![1];
        let third = |app: &App, i| inbox_line(app, i).spans[2].clone();
        assert!(
            third(&app, 0).content.ends_with("item 0"),
            "{:?}",
            third(&app, 0)
        );
        assert_eq!(
            (third(&app, 1).content.as_ref(), third(&app, 1).style),
            ("later · ", dim()),
            "after the icon and the time"
        );
        app.lang = Lang::Tr;
        assert_eq!(third(&app, 1).content, "sonra · ");
    }

    #[test]
    fn a_change_waits_in_the_inbox_and_enter_shows_its_diff() {
        let mut app = app_after(1, 4, 50, Lang::En);
        app.world.inbox.clear();
        app.world.change_ready(a_change(1));
        let name = app.world.agents[1].name.clone();
        let s = render(&app, 220, 40);
        assert!(
            s.contains("Δ ")
                && s.contains(&format!("{name} finished: Fix the parser (2 files, +3 −1)")),
            "{s}"
        );
        app.pane = Pane::Inbox;
        app.inbox_sel = 0;
        let s = render(&app, 220, 40);
        assert!(
            s.contains("enter see the diff · a apply to the checkout · r reject"),
            "{s}"
        );
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.zoom, Zoom::Diff(1));
        let s = render(&app, 140, 40);
        assert!(
            s.contains(&format!("CHANGE · {name} · 2 files +3 −1 · Fix the parser")),
            "{s}"
        );
        assert!(s.contains("-OLD-LINE") && s.contains("+NEW-LINE"), "{s}");
        // What ran on it heads the diff, its files under that, then the
        // patch.
        let row = |needle: &str| s.lines().position(|l| l.contains(needle)).unwrap();
        let ran = row("no imece run on exactly this change");
        assert_eq!(ran + 1, row("│ src/p.rs +"), "{s}");
        assert_eq!(ran + 3, row("diff --git a/src/p.rs"), "{s}");
        assert!(
            s.contains("+Q-LINE \n") || s.contains("+Q-LINE "),
            "control characters go: {s}"
        );
        assert!(
            s.contains(" j k ") && s.contains(" a x  apply · choose a hunk"),
            "{s}"
        );
        assert!(s.contains(" >  quote to it"), "{s}");
        // The agent at hand is the change's: `@` writes to it.
        key(&mut app, KeyCode::Char('@'));
        assert_eq!(app.input, format!("@{name} "));
        key(&mut app, KeyCode::Esc);
        // Scrolling: lines, pages, ends, files.
        key(&mut app, KeyCode::Char('j'));
        key(&mut app, KeyCode::Char('j'));
        assert_eq!(app.diff_scroll, 2);
        key(&mut app, KeyCode::Char('k'));
        assert_eq!(app.diff_scroll, 1);
        key(&mut app, KeyCode::Char(']'));
        assert_eq!(app.diff_scroll, 8, "the next file's header");
        key(&mut app, KeyCode::Char(']'));
        assert_eq!(app.diff_scroll, 8, "no file after the last");
        key(&mut app, KeyCode::Char('['));
        assert_eq!(app.diff_scroll, 0);
        key(&mut app, KeyCode::Char('['));
        assert_eq!(app.diff_scroll, 0);
        key(&mut app, KeyCode::Char('G'));
        assert_eq!(app.diff_scroll, 13, "the last of 14 lines");
        key(&mut app, KeyCode::Char('g'));
        key(&mut app, KeyCode::Char('g'));
        assert_eq!(app.diff_scroll, 0);
        key(&mut app, KeyCode::PageDown);
        assert!(
            app.diff_scroll > 0 && app.diff_scroll <= 13,
            "{}",
            app.diff_scroll
        );
        key(&mut app, KeyCode::PageUp);
        assert_eq!(app.diff_scroll, 0);
        ctrl(&mut app, 'd');
        let half = app.diff_scroll;
        assert!(half > 0, "{half}");
        ctrl(&mut app, 'u');
        assert_eq!(app.diff_scroll, 0);
        ctrl(&mut app, 'f');
        assert!(app.diff_scroll >= half, "{}", app.diff_scroll);
        ctrl(&mut app, 'b');
        assert_eq!(app.diff_scroll, 0);
        key(&mut app, KeyCode::End);
        assert_eq!(app.diff_scroll, 13);
        key(&mut app, KeyCode::Home);
        assert_eq!(app.diff_scroll, 0);
        key(&mut app, KeyCode::Char(' '));
        assert!(app.diff_scroll > 0);
        // Esc: back to the inbox.
        app.pane = Pane::Salon;
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.zoom, Zoom::None);
        assert_eq!(app.pane, Pane::Inbox);
        // The demo applies it as live does: said at once, done on the
        // next step; the item stays until then.
        key(&mut app, KeyCode::Enter);
        key(&mut app, KeyCode::Char('a'));
        assert_eq!(app.zoom, Zoom::None);
        assert!(render(&app, 140, 40).contains(&format!("applying {name}'s change")));
        assert_eq!(app.world.open_inbox().len(), 1);
        // A change that went away leaves an empty view, not a crash.
        app.zoom = Zoom::Diff(1);
        app.world.change_rejected(1);
        let s = render(&app, 140, 40);
        assert!(
            s.contains(&format!("nothing of {name}'s waits any more")),
            "{s}"
        );
        key(&mut app, KeyCode::Char('j'));
        key(&mut app, KeyCode::Char(']'));
        key(&mut app, KeyCode::Char('G'));
        assert_eq!(app.diff_scroll, 0);
    }

    #[test]
    fn a_live_change_is_applied_or_rejected_from_the_inbox_and_the_change_view() {
        let w = crate::live::testing::team(1);
        let live = crate::live::testing::idle_live(2);
        let mut app = App::with(w, crate::app::Engine::Live(Box::new(live)), Lang::En);
        app.world.change_ready(a_change(1));
        app.pane = Pane::Inbox;
        // Nothing of claude-1's is known to the engine: applying fails and
        // says so; the item stays.
        key(&mut app, KeyCode::Char('a'));
        let s = render(&app, 140, 40);
        assert!(
            s.contains("claude-1's change did not apply: no change of theirs waits"),
            "{s}"
        );
        assert_eq!(app.world.open_inbox().len(), 1);
        // Rejecting closes it and opens the composer to the worker.
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.zoom, Zoom::Diff(1));
        key(&mut app, KeyCode::Char('r'));
        assert_eq!(app.zoom, Zoom::None);
        assert!(app.typing);
        assert_eq!(app.input, "@claude-1 ");
        assert!(app.world.open_inbox().is_empty());
        let s = render(&app, 140, 40);
        assert!(s.contains("› to claude-1"), "{s}");
        assert!(matches!(
            app.world.journal.last().unwrap().event,
            Event::Rejected { agent: 1 }
        ));
    }

    #[test]
    fn a_live_ask_is_allowed_or_refused_from_the_inbox() {
        let w = crate::live::testing::team(1);
        let live = crate::live::testing::idle_live(2);
        let mut app = App::with(w, crate::app::Engine::Live(Box::new(live)), Lang::En);
        app.world.agents[1].set_status(Status::Working, 0);
        app.world.asks_you(1, "Bash: make deploy");
        app.world.asks_you(1, "Bash: curl x | sh");
        let s = render(&app, 140, 40);
        assert!(s.contains("⚑ waits for your yes"), "{s}");
        assert!(
            s.contains("⚑ 14:00 claude-1 waits for your yes: Bash"),
            "{s}"
        );
        assert!(!s.contains("curl x | sh"), "cut at the pane's edge: {s}");
        // The inbox's height: a row per item, borders and the keys' row.
        let tall = |s: &str| {
            let rows: Vec<&str> = s.lines().collect();
            let at = |title: &str| rows.iter().position(|r| r.contains(title)).unwrap();
            at("┌ JOURNAL") - at("┌ WAITING FOR YOU")
        };
        assert_eq!(tall(&s), 5, "{s}");
        // The selected ask is shown whole, under its row.
        app.pane = Pane::Inbox;
        let s = render(&app, 140, 40);
        assert!(s.contains("│  Bash: make deploy "), "{s}");
        assert!(s.contains("a allow · r refuse · enter focus"), "{s}");
        assert_eq!(tall(&s), 6, "{s}");
        key(&mut app, KeyCode::Char('a'));
        let s = render(&app, 140, 40);
        assert!(s.contains("allowed: claude-1 goes on"), "{s}");
        assert!(s.contains("│  Bash: curl x | sh "), "{s}");
        assert!(s.contains("a allow · r refuse · enter focus"), "{s}");
        assert_eq!(tall(&s), 5, "{s}");
        assert!(matches!(
            &app.world.journal.last().unwrap().event,
            Event::Decided { agent: 1, what, allowed: true } if what == "Bash: make deploy"
        ));
        key(&mut app, KeyCode::Char('r'));
        let s = render(&app, 140, 40);
        assert!(s.contains("refused: claude-1 is told so"), "{s}");
        assert!(matches!(
            &app.world.journal.last().unwrap().event,
            Event::Decided { agent: 1, what, allowed: false } if what == "Bash: curl x | sh"
        ));
        assert!(app.world.open_inbox().is_empty());
        assert!(!app.typing, "nothing to write back");
        let s = render(&app, 140, 40);
        assert!(!s.contains("waits for your yes"), "{s}");
    }

    #[test]
    fn the_demo_decides_an_ask_as_live_does() {
        let mut app = app_after(1, 4, 0, Lang::En);
        let name = app.world.agents[1].name.clone();
        for (k, allowed) in [('a', true), ('r', false)] {
            app.world.asks_you(1, "Bash: make");
            let item = app.world.inbox.len() - 1;
            app.pane = Pane::Inbox;
            app.inbox_sel = app
                .world
                .open_inbox()
                .iter()
                .position(|&i| i == item)
                .unwrap();
            key(&mut app, KeyCode::Char(k));
            assert!(!app.world.inbox[item].open, "{k}");
            assert!(matches!(
                &app.world.journal.last().unwrap().event,
                Event::Decided { agent: 1, what, allowed: a } if what == "Bash: make" && *a == allowed
            ));
            let how = if allowed { "allowed" } else { "refused" };
            let s = render(&app, 140, 40);
            assert!(s.contains(&format!("{how}: {name}")), "{s}");
        }
    }

    /// A world whose journal is only a conversation of the person and the
    /// leader, with another pair talking in between.
    fn talking() -> App {
        let mut app = app_after(1, 4, 0, Lang::En);
        app.world.journal.clear();
        let you = |text: &str| Event::YouSaid {
            to: 0,
            text: text.into(),
        };
        let lead = |text: &str| Event::Replied {
            agent: 0,
            text: text.into(),
        };
        app.world.note(you("check the readme"));
        app.world.note(lead(
            "# Readme\nIt is one line: `# demo`.\n- no usage\n```\ncargo run\n```",
        ));
        app.world.note(Event::Talk {
            from: 1,
            to: 2,
            text: "between workers".into(),
        });
        app.world.note(you("assign it"));
        app.world
            .note(lead(&format!("{} THE-END", "long ".repeat(300))));
        app
    }

    #[test]
    fn tab_reaches_the_journal_and_enter_opens_the_whole_conversation() {
        let mut app = talking();
        key(&mut app, KeyCode::Tab);
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.pane, Pane::Journal);
        // The newest line is selected; its conversation folds the older one.
        assert_eq!(app.journal_sel, Some(4));
        assert!(render(&app, 140, 40).contains("enter open the conversation"));
        key(&mut app, KeyCode::Enter);
        let th = app.thread.as_ref().unwrap();
        // Every line of the pair, not only the neighbouring ones; not the
        // workers' talk.
        assert_eq!(th.entries, [0, 1, 3, 4]);
        assert_eq!(th.sel, 3);
        let screen = render(&app, 120, 30);
        assert!(screen.contains("CONVERSATION"), "{screen}");
        assert!(screen.contains("THE-END"), "the selected line is in view");
        key(&mut app, KeyCode::Home);
        let screen = render(&app, 120, 30);
        assert!(screen.contains("check the readme"), "{screen}");
        assert!(!screen.contains("# Readme"), "headings drop their marks");
        assert!(screen.contains("• no usage") && screen.contains("│ cargo run"));
        key(&mut app, KeyCode::Esc);
        assert!(app.thread.is_none());
        assert_eq!(app.pane, Pane::Journal, "esc only closes the window");
    }

    #[test]
    fn c_copies_a_line_and_capital_c_the_conversation_as_shown() {
        let mut app = talking();
        app.world.note(Event::Replied {
            agent: 0,
            text: "token sk-abc123 \u{1b}[31mred".into(),
        });
        app.pane = Pane::Journal;
        app.journal_sel = Some(5);
        key(&mut app, KeyCode::Char('c'));
        let one = app.copy.take().unwrap();
        assert!(
            !one.contains("sk-abc123") && !one.contains('\u{1b}'),
            "{one}"
        );
        key(&mut app, KeyCode::Enter);
        key(&mut app, KeyCode::Char('C'));
        let all = app.copy.take().unwrap();
        assert!(
            all.contains("1970-01-01 14:00 you → lead\ncheck the readme"),
            "dated: the text leaves the screen\n{all}"
        );
        assert!(all.contains("lead → you\n# Readme"), "copied as written");
        assert!(!all.contains("between workers"));
    }

    /// `talking()` with its first two messages on the simulation's first
    /// day and the rest on the second (the sim's clock starts at 14:00, so
    /// tick 7200 is the next midnight); the world is in the second day.
    fn talking_over_two_days() -> App {
        let mut app = talking();
        for (i, e) in app.world.journal.iter_mut().enumerate() {
            e.tick = if i < 3 { 0 } else { 7200 };
        }
        app.world.tick = 7300;
        app
    }

    #[test]
    fn the_journal_says_where_an_earlier_day_begins() {
        let app = talking_over_two_days();
        let screen = render(&app, 140, 40);
        let lines: Vec<&str> = screen.lines().collect();
        let at = |needle: &str| {
            lines
                .iter()
                .position(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("{needle} not on\n{screen}"))
        };
        let day_row = at("─ 1970-01-01");
        assert_eq!(
            screen.matches("─ 1970-0").count(),
            1,
            "one day row: today's lines need none\n{screen}"
        );
        // Newest first: today's conversation, then yesterday under its date.
        assert!(at("00:00 ") < day_row, "{screen}");
        assert_eq!(at("14:00 ⇄"), day_row + 1, "the workers' talk\n{screen}");
        assert!(lines[day_row + 2].contains("14:00 "), "{screen}");
        // Nothing of today: the newest line stands under its own date.
        let mut app = app;
        for e in &mut app.world.journal {
            e.tick = 0;
        }
        let screen = render(&app, 140, 40);
        let lines: Vec<&str> = screen.lines().collect();
        let day_row = lines
            .iter()
            .position(|l| l.contains("─ 1970-01-01"))
            .unwrap();
        assert!(lines[day_row - 1].contains("JOURNAL"), "{screen}");
        assert!(lines[day_row + 1].contains("14:00 "), "{screen}");
        // All of today: no date at all.
        app.world.tick = 10;
        assert!(!render(&app, 140, 40).contains("─ 1970"));
    }

    #[test]
    fn the_journal_highlights_the_selected_line_and_not_the_first() {
        let mut app = talking();
        app.pane = Pane::Journal;
        // Rows newest first: the conversation (4 over 3), the workers'
        // talk (2), the older conversation (1 over 0).
        app.journal_sel = Some(2);
        let screen = render(&app, 140, 40);
        let lines: Vec<&str> = screen.lines().collect();
        let keys = lines
            .iter()
            .position(|l| l.contains("enter open the conversation"))
            .unwrap_or_else(|| panic!("{screen}"));
        assert!(
            lines[keys - 1].contains("⇄") && lines[keys - 1].contains("codex-1"),
            "the keys stand under the talk, the selected line\n{screen}"
        );
    }

    #[test]
    fn the_journal_scrolled_to_an_old_line_knows_the_day_above_its_first_row() {
        // Forty events that never fold (the talk is between two pairs in
        // turn), two a day, newest first: rows 2k and 2k+1 share a day. The
        // first row shown gets a day row exactly when it is the first of
        // its pair, whatever the window's height makes the first row.
        let mut app = app_after(1, 4, 0, Lang::En);
        app.world.journal.clear();
        for i in 0..40u64 {
            app.world.journal.push(Entry {
                tick: (i / 2) * 17_280,
                event: Event::Talk {
                    from: 1 + (i % 2) as usize,
                    to: 3,
                    text: format!("m{i}"),
                },
            });
        }
        app.world.tick = 19 * 17_280 + 100;
        app.pane = Pane::Journal;
        app.journal_sel = Some(0);
        let mut parities = std::collections::BTreeSet::new();
        for height in [30u16, 31, 32, 33, 34, 35] {
            let screen = render(&app, 140, height);
            let lines: Vec<&str> = screen.lines().collect();
            let head = lines
                .iter()
                .position(|l| l.contains("JOURNAL"))
                .unwrap_or_else(|| panic!("{screen}"));
            let dated = lines[head + 1].contains("─ 1970-");
            let first = lines[head + 1 + usize::from(dated)];
            let i: usize = first
                .split("m")
                .nth(1)
                .and_then(|t| t.split(|c: char| !c.is_ascii_digit()).next())
                .and_then(|n| n.parse().ok())
                .unwrap_or_else(|| panic!("no entry on {first:?}\n{screen}"));
            let row = 39 - i;
            assert_eq!(
                dated,
                row.is_multiple_of(2),
                "row {row} first at height {height}\n{screen}"
            );
            assert!(
                lines.iter().any(|l| l.contains(": m0 ")),
                "the selected, oldest line is in view\n{screen}"
            );
            parities.insert(row % 2);
        }
        assert_eq!(parities.len(), 2, "both cases were seen");
    }

    #[test]
    fn the_conversation_dates_the_messages_of_other_days() {
        let mut app = talking_over_two_days();
        app.open_thread(4);
        let th = app.thread.as_ref().unwrap();
        let (rows, ranges) = overlay::thread_rows(app.lang, &app.world, th, 60);
        let lines = text(&rows);
        assert_eq!(
            lines[0], "  ─ 1970-01-01",
            "the first message is not of today"
        );
        assert_eq!(ranges[0].0, 1, "the date is no part of the message");
        assert!(lines[1].starts_with("  14:00 you → lead"), "{}", lines[1]);
        let (second_day, _) = ranges[2];
        assert_eq!(lines[second_day - 1], "  ─ 1970-01-02");
        assert!(
            lines[second_day].starts_with("  00:00 you → lead"),
            "{}",
            lines[second_day]
        );
        assert_eq!(
            lines.iter().filter(|l| l.starts_with("  ─ ")).count(),
            2,
            "two days, two rows"
        );
        let screen = render(&app, 120, 30);
        assert!(screen.contains("─ 1970-01-02"), "{screen}");
    }

    #[test]
    fn a_waiting_item_of_another_day_carries_its_date() {
        let mut app = app_after(1, 4, 50, Lang::En);
        app.world.replied(0, "Short.");
        app.world.inbox.last_mut().unwrap().tick = 0;
        let screen = render(&app, 140, 40);
        assert!(
            screen.contains("» 14:00 ") && !screen.contains("1970"),
            "today: the clock alone\n{screen}"
        );
        app.world.tick = 7300;
        let screen = render(&app, 140, 40);
        assert!(screen.contains("» 1970-01-01 14:00 "), "{screen}");
    }

    /// The row above the footer, where the completions are.
    fn menu_row(app: &App, width: u16) -> String {
        let s = render(app, width, 40);
        let mut rows = s.lines().rev();
        rows.next();
        rows.next().unwrap_or_default().to_string()
    }

    #[test]
    fn an_at_offers_the_agents_and_tab_steps_through_them_with_the_text_following() {
        let mut app = app_after(1, 3, 0, Lang::En);
        let names: Vec<String> = app.world.agents.iter().map(|a| a.name.clone()).collect();
        key(&mut app, KeyCode::Char('@'));
        let m = app.menu.clone().expect("@ alone offers everyone");
        assert_eq!(m.items, names);
        let row = menu_row(&app, 140);
        for n in &names {
            assert!(row.contains(n.as_str()), "{row}");
        }
        let s = render(&app, 140, 40);
        assert!(s.contains("tab next · enter take · esc hide"), "{s}");
        // tab puts the first one in, and again steps on; ⇧tab steps back.
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.input, format!("@{} ", names[0]));
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.input, format!("@{} ", names[1]));
        key(&mut app, KeyCode::BackTab);
        assert_eq!(app.input, format!("@{} ", names[0]));
        key(&mut app, KeyCode::Down);
        assert_eq!(app.input, format!("@{} ", names[1]));
        key(&mut app, KeyCode::Up);
        assert_eq!(app.input, format!("@{} ", names[0]));
        ctrl(&mut app, 'n');
        assert_eq!(app.input, format!("@{} ", names[1]));
        ctrl(&mut app, 'p');
        assert_eq!(app.input, format!("@{} ", names[0]));
        ctrl(&mut app, 'x');
        assert_eq!(
            app.input,
            format!("@{} ", names[0]),
            "other ctrl keys do nothing"
        );
        // Typing on continues from the inserted name, menu gone.
        for c in "hi".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        assert_eq!(app.menu, None);
        assert!(render(&app, 140, 40).contains("enter send · \\ enter new line · esc cancel"));
        key(&mut app, KeyCode::Enter);
        assert!(!app.typing);
        assert!(
            app.world.journal.iter().any(
                |e| matches!(&e.event, Event::YouSaid { to, text } if *to == 0 && text == "hi")
            )
        );
    }

    #[test]
    fn a_prefix_narrows_the_offers_and_enter_takes_one_before_it_sends() {
        let mut app = app_after(1, 3, 0, Lang::En);
        let names: Vec<String> = app.world.agents.iter().map(|a| a.name.clone()).collect();
        let (head, _) = names[2].split_at(2);
        key(&mut app, KeyCode::Char('@'));
        for c in head.chars() {
            key(&mut app, KeyCode::Char(c));
        }
        let m = app.menu.clone().unwrap();
        assert!(m.items.iter().all(|n| n.starts_with(head)), "{:?}", m.items);
        assert!(m.items.contains(&names[2]));
        // enter takes the highlighted name instead of sending.
        key(&mut app, KeyCode::Enter);
        assert!(app.typing);
        assert_eq!(app.input, format!("@{} ", m.items[0]));
        assert!(
            !app.world
                .journal
                .iter()
                .any(|e| matches!(&e.event, Event::YouSaid { .. }))
        );
        // Backspace brings the menu back for the shorter word.
        key(&mut app, KeyCode::Backspace);
        key(&mut app, KeyCode::Backspace);
        assert!(app.menu.is_some(), "{}", app.input);
        // esc hides the menu first, then cancels the writing.
        key(&mut app, KeyCode::Esc);
        assert!(app.typing);
        assert_eq!(app.menu, None);
        key(&mut app, KeyCode::Esc);
        assert!(!app.typing);
        assert_eq!(app.input, "");
        // A line nothing completes has no menu and tab does nothing to it.
        key(&mut app, KeyCode::Char('i'));
        for c in "hello".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.input, "hello");
        assert_eq!(app.menu, None);
        key(&mut app, KeyCode::Esc);
        // An agent offers the command line its session listed; the
        // demo's listed none.
        key(&mut app, KeyCode::Char('/'));
        key(&mut app, KeyCode::Char('/'));
        assert_eq!(app.menu, None);
        key(&mut app, KeyCode::Esc);
        app.world.agents[0].commands = vec!["/clear".into(), "/compact".into()];
        key(&mut app, KeyCode::Char('/'));
        key(&mut app, KeyCode::Char('/'));
        key(&mut app, KeyCode::Char('c'));
        assert_eq!(app.menu.as_ref().unwrap().items, ["/clear", "/compact"]);
        key(&mut app, KeyCode::Tab);
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.input, "/compact ");
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Char('!'));
        assert_eq!(app.menu, None);
    }

    #[test]
    fn the_completion_row_draws_each_note_after_its_item() {
        let mut m = menu_of(&["budget", "open", "help"], 1);
        m.notes = vec!["(spent $1.00 of $5.00)".into(), "$".into(), String::new()];
        let (row, out) = completion_row(&m, 60);
        assert_eq!(
            row.trim_end(),
            " budget (spent $1.00 of $5.00)  open $  help",
            "an empty note adds nothing"
        );
        let open = row.find(" open").unwrap();
        assert!(
            out[open..open + 8].iter().all(|&o| o),
            "the note stands out too"
        );
        assert!(!out[open + 8], "{row}");
        // The window counts the notes: the highlighted item stays in view.
        let (row, _) = completion_row(&m, 14);
        assert!(row.starts_with(" open $  help"), "{row}");
    }

    #[test]
    fn live_the_colon_menu_leaves_out_the_demos_clock_and_its_names_say_so() {
        let w = crate::live::testing::team(1);
        let live = crate::live::testing::idle_live(2);
        let mut app = App::with(w, crate::app::Engine::Live(Box::new(live)), Lang::En);
        key(&mut app, KeyCode::Char(':'));
        let m = app.menu.clone().unwrap();
        let want: Vec<&str> = (crate::complete::COMMANDS.into_iter())
            .filter(|c| !crate::complete::DEMO_ONLY.contains(c))
            .collect();
        assert_eq!(m.items, want);
        assert_eq!(m.notes.len(), m.items.len());
        key(&mut app, KeyCode::Char('p'));
        assert_eq!(
            app.menu.as_ref().unwrap().items,
            ["pending", "pin"],
            "not pause"
        );
        key(&mut app, KeyCode::Backspace);
        for c in "fa".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        assert_eq!(app.menu, None, "nothing left to offer");
        for c in "ster".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.speed, 1);
        let s = render(&app, 140, 40);
        assert!(s.contains(":faster is for the demo"), "{s}");
        for c in ":pause".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        assert!(!app.paused);
    }

    #[test]
    fn a_colon_offers_the_commands_and_lead_its_vendors_to_the_end() {
        let mut app = app_after(1, 2, 0, Lang::En);
        key(&mut app, KeyCode::Char(':'));
        // The demo's own commands too.
        assert_eq!(app.menu.as_ref().unwrap().items, crate::complete::COMMANDS);
        let row = menu_row(&app, 140);
        assert!(row.contains("budget") && row.contains("board"), "{row}");
        key(&mut app, KeyCode::Char('l'));
        key(&mut app, KeyCode::Char('e'));
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.input, ":lead ");
        key(&mut app, KeyCode::Char('c'));
        assert_eq!(app.menu.as_ref().unwrap().items, ["claude", "codex"]);
        key(&mut app, KeyCode::Tab);
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.input, ":lead codex");
        key(&mut app, KeyCode::Enter);
        assert!(!app.typing);
        assert_eq!(app.primary.as_deref(), Some("codex"));
        // Turkish hint too.
        app.lang = Lang::Tr;
        key(&mut app, KeyCode::Char(':'));
        assert!(render(&app, 140, 40).contains("tab sonraki · enter al · esc gizle"));
    }

    #[test]
    fn the_menu_row_keeps_the_highlighted_item_in_view_when_they_do_not_all_fit() {
        let mut app = app_after(2, 8, 0, Lang::En);
        key(&mut app, KeyCode::Char('@'));
        let items = app.menu.as_ref().unwrap().items.clone();
        assert!(items.len() >= 10, "{}", items.len());
        let last = items.last().unwrap().clone();
        let row = menu_row(&app, 40);
        assert!(row.contains(&items[0]), "{row}");
        assert!(!row.contains(&last), "{row}");
        key(&mut app, KeyCode::BackTab);
        assert_eq!(app.input, format!("@{last} "));
        let row = menu_row(&app, 40);
        assert!(row.contains(&last), "{row}");
        assert!(!row.contains(&items[0]), "{row}");
        // A wide screen shows them all.
        let row = menu_row(&app, 400);
        for n in &items {
            assert!(row.contains(n.as_str()), "{row}");
        }
    }

    #[test]
    fn backspace_takes_a_letter_back_and_the_offers_widen_again() {
        let mut app = app_after(1, 3, 0, Lang::En);
        let everyone = app.world.agents.len();
        key(&mut app, KeyCode::Char('@'));
        key(&mut app, KeyCode::Char('z'));
        assert_eq!(app.input, "@z");
        assert_eq!(app.menu, None, "nobody is called z…");
        key(&mut app, KeyCode::Backspace);
        assert_eq!(app.input, "@");
        assert_eq!(app.menu.as_ref().map(|m| m.items.len()), Some(everyone));
    }

    #[test]
    fn writing_without_offers_takes_no_row_from_the_body() {
        // A short screen, so the body is full and a row less would show.
        let mut app = app_after(1, 3, 30, Lang::En);
        let before = render(&app, 140, 12);
        key(&mut app, KeyCode::Char('/'));
        for c in "hello".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        assert!(app.typing);
        assert_eq!(app.menu, None);
        let after = render(&app, 140, 12);
        let body = |s: &str| s.lines().take(11).map(str::to_string).collect::<Vec<_>>();
        assert_eq!(body(&before), body(&after), "only the footer changed");
        assert_ne!(before, after);
    }

    fn alt(app: &mut App, code: KeyCode) {
        app.on_key(KeyEvent::new(
            code,
            ratatui::crossterm::event::KeyModifiers::ALT,
        ));
    }

    fn write(app: &mut App, text: &str) {
        for c in text.chars() {
            key(app, KeyCode::Char(c));
        }
    }

    #[test]
    fn readline_keys_edit_the_line_at_the_cursor() {
        let mut app = app_after(1, 3, 0, Lang::En);
        key(&mut app, KeyCode::Char('/'));
        write(&mut app, "fix parser");
        ctrl(&mut app, 'a');
        assert_eq!((app.cursor(), app.after), (0, 10));
        write(&mut app, "please ");
        assert_eq!(app.input, "please fix parser");
        alt(&mut app, KeyCode::Char('f'));
        assert_eq!(app.cursor(), 10);
        ctrl(&mut app, 'k');
        assert_eq!(
            (app.input.as_str(), app.cut.as_str()),
            ("please fix", " parser")
        );
        alt(&mut app, KeyCode::Char('b'));
        ctrl(&mut app, 'w');
        assert_eq!((app.input.as_str(), app.cut.as_str()), ("fix", "please "));
        ctrl(&mut app, 'e');
        ctrl(&mut app, 'y');
        assert_eq!(app.input, "fixplease ");
        ctrl(&mut app, 'u');
        assert_eq!((app.input.as_str(), app.cut.as_str()), ("", "fixplease "));
        assert!(app.typing, "ctrl keys edit, they do not leave");
    }

    #[test]
    fn the_menu_is_offered_only_with_the_cursor_at_the_end() {
        let mut app = app_after(1, 3, 0, Lang::En);
        key(&mut app, KeyCode::Char('@'));
        write(&mut app, "cl");
        assert!(app.menu.is_some());
        key(&mut app, KeyCode::Left);
        assert_eq!(app.menu, None, "moved off the end");
        key(&mut app, KeyCode::Tab);
        assert_eq!(
            (app.input.as_str(), app.menu.is_none()),
            ("@cl", true),
            "tab waits for the end"
        );
        key(&mut app, KeyCode::End);
        assert!(app.menu.is_some(), "back at the end");
        // A cut that leaves the cursor where it is but at the end now.
        key(&mut app, KeyCode::Left);
        write(&mut app, "x");
        assert_eq!((app.input.as_str(), app.menu.is_none()), ("@cxl", true));
        key(&mut app, KeyCode::Left);
        ctrl(&mut app, 'k');
        assert_eq!(app.input, "@c");
        assert!(
            app.menu.is_some(),
            "the line changed and ends at the cursor"
        );
        // A key that does nothing leaves the menu as it was.
        key(&mut app, KeyCode::Tab);
        let menu = app.menu.clone();
        assert!(menu.as_ref().unwrap().inserted);
        key(&mut app, KeyCode::Right);
        ctrl(&mut app, 'x');
        assert_eq!(app.menu, menu);
    }

    #[test]
    fn tab_at_the_end_with_the_menu_hidden_offers_it_again() {
        let mut app = app_after(1, 3, 0, Lang::En);
        key(&mut app, KeyCode::Char('@'));
        write(&mut app, "cl");
        key(&mut app, KeyCode::Esc);
        assert!(app.typing && app.menu.is_none(), "esc hid the menu");
        key(&mut app, KeyCode::Tab);
        let m = app.menu.as_ref().unwrap();
        assert!(m.inserted);
        assert_eq!(app.input, format!("@{} ", m.items[0]));
    }

    #[test]
    fn the_cursor_starts_at_the_end_after_esc_and_after_sending() {
        let mut app = app_after(1, 3, 0, Lang::En);
        key(&mut app, KeyCode::Char('/'));
        write(&mut app, "ab");
        key(&mut app, KeyCode::Left);
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Char(':'));
        write(&mut app, "b");
        assert_eq!(app.input, ":b");
        key(&mut app, KeyCode::Left);
        key(&mut app, KeyCode::Enter);
        assert!(!app.typing);
        key(&mut app, KeyCode::Char('!'));
        write(&mut app, "ls");
        assert_eq!(app.input, "!ls");
    }

    #[test]
    fn a_stale_cursor_lands_on_the_next_character_edge() {
        let mut app = app_after(1, 3, 0, Lang::En);
        app.input = "aç".into();
        for (after, at) in [(0, 3), (1, 3), (2, 1), (3, 0), (9, 0)] {
            app.after = after;
            assert_eq!(app.cursor(), at, "{after}");
        }
    }

    fn with(app: &mut App, code: KeyCode, mods: KeyModifiers) {
        app.on_key(KeyEvent::new(code, mods));
    }

    /// What the person last said, to whom.
    fn last_said(app: &App) -> Option<(usize, String)> {
        app.world.journal.iter().rev().find_map(|e| match &e.event {
            Event::YouSaid { to, text } => Some((*to, text.clone())),
            _ => None,
        })
    }

    #[test]
    fn shift_enter_alt_enter_and_a_backslash_enter_write_a_new_line_and_enter_sends_it_all() {
        let mut app = app_after(1, 3, 0, Lang::En);
        let lead = app.world.projects[0].leader;
        key(&mut app, KeyCode::Char('/'));
        write(&mut app, "fix");
        with(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
        write(&mut app, "the");
        with(&mut app, KeyCode::Enter, KeyModifiers::ALT);
        write(&mut app, "parser\\");
        key(&mut app, KeyCode::Enter);
        assert!(app.typing, "a backslash enter does not send");
        write(&mut app, "now");
        assert_eq!(app.input, "fix\nthe\nparser\nnow");
        key(&mut app, KeyCode::Enter);
        assert!(!app.typing);
        assert_eq!(
            last_said(&app),
            Some((lead, "fix\nthe\nparser\nnow".to_string()))
        );
        // To one agent: the name ends at the new line too.
        let name = app.world.agents[1].name.clone();
        key(&mut app, KeyCode::Char('@'));
        write(&mut app, &name);
        with(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
        write(&mut app, "hi");
        assert_eq!(app.typing_target(), name);
        key(&mut app, KeyCode::Enter);
        assert_eq!(last_said(&app), Some((1, "hi".to_string())));
    }

    #[test]
    fn a_new_line_hides_the_menu_and_up_and_down_go_between_lines_without_one() {
        let mut app = app_after(1, 3, 0, Lang::En);
        key(&mut app, KeyCode::Char('@'));
        assert!(app.menu.is_some());
        with(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
        assert_eq!((app.input.as_str(), app.menu.is_none()), ("@\n", true));
        assert!(app.typing, "the menu did not take the enter");
        // Alt-enter as well, the menu open again.
        key(&mut app, KeyCode::Backspace);
        assert!(app.menu.is_some());
        with(&mut app, KeyCode::Enter, KeyModifiers::ALT);
        assert_eq!((app.input.as_str(), app.menu.is_none()), ("@\n", true));
        write(&mut app, "abc");
        key(&mut app, KeyCode::Up);
        assert_eq!((app.cursor(), app.after), (1, 4), "the first line's end");
        key(&mut app, KeyCode::Down);
        assert_eq!(app.cursor(), 3, "column 1, under it");
        key(&mut app, KeyCode::Down);
        assert_eq!(app.cursor(), 3, "the last line: it stays");
        assert!(app.typing && app.menu.is_none());
    }

    #[test]
    fn a_paste_goes_into_the_line_whole_and_only_while_writing() {
        let mut app = app_after(1, 3, 0, Lang::En);
        app.paste("ls");
        assert!(!app.typing && app.input.is_empty(), "not writing: left out");
        let said = Lang::En.text().paste_not_writing;
        assert!(render(&app, 140, 40).contains(said));
        key(&mut app, KeyCode::Char('/'));
        write(&mut app, "ab");
        key(&mut app, KeyCode::Left);
        app.paste("one\r\ntwo\rthree\x1b[1m\tç\x07");
        assert_eq!(app.input, "aone\ntwo\nthree[1m\tçb");
        assert_eq!(
            app.after, 1,
            "the cursor after what was pasted, before the b"
        );
        assert!(app.typing, "a paste never sends");
        // An overlay over the line takes nothing either.
        let line = app.input.clone();
        for open in 0..4 {
            match open {
                0 => app.help = true,
                1 => app.digest = Some(Default::default()),
                2 => app.left_off = Some(Default::default()),
                _ => app.leaving = true,
            }
            app.flash = None;
            app.paste("x");
            assert_eq!(app.input, line, "overlay {open}");
            assert!(app.flash.is_some(), "overlay {open}");
            (app.help, app.digest, app.left_off, app.leaving) = (false, None, None, false);
        }
        app.paste("x");
        assert_ne!(app.input, line, "and with none open it goes in");
    }

    #[test]
    fn the_footer_grows_with_the_lines_and_keeps_the_cursor_s_in_view() {
        let mut app = app_after(1, 3, 0, Lang::En);
        let lead = app.world.agents[app.world.projects[0].leader].name.clone();
        let to = format!(" › to {lead}: ");
        key(&mut app, KeyCode::Char('/'));
        write(&mut app, "one");
        with(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
        write(&mut app, "two");
        let s = render(&app, 140, 40);
        let rows: Vec<&str> = s.lines().collect();
        let (first, last) = (rows[38], rows[39]);
        assert!(first.starts_with(&format!("{to}one")), "{first}");
        let indent = " ".repeat(to.chars().count());
        assert!(last.starts_with(&format!("{indent}two")), "{last}");
        assert!(last.contains("enter send · \\ enter new line · esc cancel"));
        assert!(!first.contains("enter send"), "the hint ends the last row");
        assert!(!s.contains("/2"), "all lines fit: no count");
        app.shift_enter = true;
        assert!(render(&app, 140, 40).contains("enter send · ⇧enter new line · esc cancel"));

        // Twelve lines: eight rows on a tall screen, four on a short one.
        for n in 3..=12 {
            with(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
            write(&mut app, &format!("line{n}"));
        }
        let s = render(&app, 140, 40);
        let rows: Vec<&str> = s.lines().collect();
        assert!(rows[32].starts_with(&format!("{to}line5")), "{}", rows[32]);
        assert!(
            rows[39].starts_with(&format!("{indent}line12")),
            "{}",
            rows[39]
        );
        assert!(rows[39].contains("12/12"), "{}", rows[39]);
        assert!(!s.contains("line4"), "out of view above");
        let short = render(&app, 140, 12);
        let rows: Vec<&str> = short.lines().collect();
        assert!(rows[8].starts_with(&format!("{to}line9")), "{}", rows[8]);
        assert!(!short.contains("line8"));
        // The cursor goes to the top: the rows follow it.
        for _ in 0..11 {
            key(&mut app, KeyCode::Up);
        }
        let s = render(&app, 140, 40);
        let rows: Vec<&str> = s.lines().collect();
        assert!(rows[32].starts_with(&format!("{to}one")), "{}", rows[32]);
        assert!(
            rows[39].starts_with(&format!("{indent}line8")),
            "{}",
            rows[39]
        );
        assert!(rows[39].contains("1/12"), "{}", rows[39]);
        assert!(!s.contains("line9"));
        // Not writing: one row again.
        key(&mut app, KeyCode::Esc);
        let s = render(&app, 140, 40);
        assert!(!s.lines().nth(38).unwrap().contains("line"));
    }

    #[test]
    fn the_cursor_is_drawn_on_its_own_line_at_its_own_character() {
        let mut app = app_after(1, 3, 0, Lang::En);
        key(&mut app, KeyCode::Char('/'));
        app.input = "one\ntwo\nthree".into();
        app.after = 4; // under the h of three
        let buf = buffer(&app, 140, 40);
        let reversed: Vec<(u16, u16, String)> = (37..40)
            .flat_map(|y| (0..140).map(move |x| (x, y)))
            .filter(|&(x, y)| buf[(x, y)].modifier.contains(Modifier::REVERSED))
            .map(|(x, y)| (x, y, buf[(x, y)].symbol().to_string()))
            .collect();
        assert_eq!(reversed.len(), 1, "{reversed:?}");
        assert_eq!((reversed[0].1, reversed[0].2.as_str()), (39, "h"));
        app.after = 8; // under the w of two
        let buf = buffer(&app, 140, 40);
        let under: Vec<(u16, String)> = (37..40)
            .flat_map(|y| (0..140).map(move |x| (x, y)))
            .filter(|&(x, y)| buf[(x, y)].modifier.contains(Modifier::REVERSED))
            .map(|(x, y)| (y, buf[(x, y)].symbol().to_string()))
            .collect();
        assert_eq!(under, vec![(38, "w".to_string())]);
    }

    #[test]
    fn the_character_under_the_cursor_is_drawn_in_reverse() {
        let texts = |spans: &[Span]| {
            spans
                .iter()
                .map(|s| s.content.to_string())
                .collect::<Vec<_>>()
        };
        let reversed = |s: &Span| s.style.add_modifier.contains(Modifier::REVERSED);
        let spans = cursor_spans("açb", 1);
        assert_eq!(texts(&spans), ["a", "ç", "b"]);
        assert_eq!(
            spans.iter().map(reversed).collect::<Vec<_>>(),
            [false, true, false]
        );
        let spans = cursor_spans("ab", 2);
        assert_eq!(texts(&spans), ["ab", "█"]);
        assert!(!reversed(&spans[1]));
        // A line `clean` changes is drawn cleaned, with the block at the
        // end, wherever the cursor is.
        assert_eq!(texts(&cursor_spans("x sk-abc", 3)), ["x [***]", "█"]);
        // On screen: the footer has the line and the reversed letter.
        let mut app = app_after(1, 3, 0, Lang::En);
        key(&mut app, KeyCode::Char('/'));
        write(&mut app, "hello");
        key(&mut app, KeyCode::Home);
        assert!(render(&app, 140, 40).contains("hello"));
        assert!(!render(&app, 140, 40).contains("hello█"));
    }

    /// `completions` alone: a row of `width` cells with `m` drawn in it,
    /// and whether the cell at `x` stands out.
    fn completion_row(m: &crate::complete::Menu, width: u16) -> (String, Vec<bool>) {
        let mut t = Terminal::new(TestBackend::new(width, 1)).unwrap();
        t.draw(|f| completions(f, f.area(), m)).unwrap();
        let buf = t.backend().buffer();
        let row = (0..width)
            .map(|x| buf[(x, 0)].symbol().to_string())
            .collect::<String>();
        let out = (0..width)
            .map(|x| buf[(x, 0)].modifier.contains(Modifier::REVERSED))
            .collect();
        (row, out)
    }

    fn menu_of(items: &[&str], chosen: usize) -> crate::complete::Menu {
        crate::complete::Menu {
            start: 1,
            items: items.iter().map(|s| s.to_string()).collect(),
            chosen,
            tail: " ",
            inserted: false,
            notes: Vec::new(),
        }
    }

    #[test]
    fn the_completion_row_fits_what_it_can_around_the_highlighted_item() {
        // Each item takes its letters and a space either side: 4 5 6 4 3.
        let items = ["aa", "bbb", "cccc", "dd", "e"];
        let (row, out) = completion_row(&menu_of(&items, 0), 22);
        assert_eq!(row, " aa  bbb  cccc  dd  e ", "an exact fit shows all");
        assert!(out[1] && out[2], "the highlighted one stands out");
        assert!(out[..4].iter().all(|&o| o));
        assert!(out[4..].iter().all(|&o| !o), "{out:?}");
        // One cell short: the last item is left out, nothing is cut.
        let (row, _) = completion_row(&menu_of(&items, 0), 21);
        assert_eq!(row, " aa  bbb  cccc  dd   ");
        // The last one highlighted: the window starts where the earlier
        // ones still fit before it.
        let (row, out) = completion_row(&menu_of(&items, 4), 21);
        assert_eq!(row, " bbb  cccc  dd  e    ");
        assert!(out[15] && out[16] && out[17] && !out[14] && !out[18]);
        // Pulling the window back looks at the item before, not the one
        // counted already: a big one before a small highlighted one.
        let (row, out) = completion_row(&menu_of(&["xxxxxxxx", "a"], 1), 12);
        assert_eq!(row, " a          ");
        assert!(out[0] && out[1] && out[2] && !out[3]);
    }

    #[test]
    fn a_colon_command_sets_the_budget_and_the_journal_says_so() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let was = app.world.budget;
        app.world.spending.warned = true;
        key(&mut app, KeyCode::Char(':'));
        assert!(app.typing);
        assert_eq!(app.input, ":");
        assert!(render(&app, 140, 40).contains("› command:"));
        for c in "budget 80".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.world.budget, 80 * 10_000);
        assert!(
            !app.world.spending.warned,
            "a new pot is worth a new warning"
        );
        app.world.spending.warned = true;
        let s = render(&app, 140, 40);
        assert!(s.contains("of $80.00"), "{s}");
        assert!(s.contains("the budget is $80.00 now"), "{s}");
        let said = format!("you set the budget to $80.00 (was {})", dollars(was));
        assert!(s.contains(&said), "{s}");
        assert!(
            !app.world
                .journal
                .iter()
                .any(|e| matches!(&e.event, Event::YouSaid { .. })),
            "a command is not a message to the lead"
        );

        // Not dollars: nothing changes, the screen says why.
        for c in ":b lots".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.world.budget, 80 * 10_000);
        assert!(app.world.spending.warned, "the pot did not change");
        assert!(render(&app, 140, 40).contains("lots is not dollars"));

        // An unknown command.
        for c in ":nope".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        assert!(render(&app, 140, 40).contains("no command nope"));

        // Plain writing is to the lead, not a command.
        key(&mut app, KeyCode::Char('i'));
        let s = render(&app, 140, 40);
        assert!(s.contains("› to lead"), "{s}");
        assert!(!s.contains("› command:"), "{s}");
    }

    fn type_line(app: &mut App, line: &str) {
        key(app, KeyCode::Char('/'));
        for c in line.chars() {
            key(app, KeyCode::Char(c));
        }
        key(app, KeyCode::Enter);
    }

    fn you_said(app: &App) -> Vec<String> {
        app.world
            .journal
            .iter()
            .filter_map(|e| match &e.event {
                Event::YouSaid { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn words_reach_the_agent_and_a_command_its_own_command_line_or_a_refusal() {
        // The demo, as live: a command needs a session, words start one.
        let mut app = app_after(1, 2, 0, Lang::En);
        app.world.journal.clear();
        type_line(&mut app, "/cost");
        assert!(you_said(&app).is_empty(), "a command is not said");
        let s = render(&app, 140, 40);
        assert!(
            s.contains("lead took no command: it has no session yet"),
            "{s}"
        );
        type_line(&mut app, "hello");
        assert_eq!(you_said(&app), ["hello"]);
        assert!(render(&app, 140, 40).contains("sent to lead"));
        type_line(&mut app, "/cost");
        assert_eq!(you_said(&app), ["hello", "/cost"], "taken, as live's");
        assert!(render(&app, 140, 40).contains("sent to lead"));
        type_line(&mut app, "/nope");
        let s = render(&app, 140, 40);
        assert!(
            s.contains("lead took no command: /nope: no such command"),
            "{s}"
        );

        // Live: a command needs a session, words start one.
        let w = crate::live::testing::team(1);
        let live = crate::live::testing::idle_live(2);
        let mut app = App::with(w, crate::app::Engine::Live(Box::new(live)), Lang::En);
        app.world.journal.clear();
        type_line(&mut app, "/cost");
        assert!(you_said(&app).is_empty(), "{:?}", you_said(&app));
        let s = render(&app, 140, 40);
        assert!(
            s.contains("lead took no command: it has no session yet"),
            "{s}"
        );
        type_line(&mut app, "hello");
        assert_eq!(you_said(&app), ["hello"]);
        assert!(render(&app, 140, 40).contains("sent to lead"));
    }

    #[test]
    fn typing_works_with_the_conversation_open() {
        let mut app = talking();
        app.open_thread(1);
        key(&mut app, KeyCode::Char('/'));
        assert!(app.typing);
        key(&mut app, KeyCode::Char('x'));
        assert_eq!(app.input, "x");
        assert!(app.thread.is_some());
    }

    fn ctrl(app: &mut App, c: char) {
        app.on_key(KeyEvent::new(
            KeyCode::Char(c),
            ratatui::crossterm::event::KeyModifiers::CONTROL,
        ));
    }

    #[test]
    fn gg_and_capital_g_jump_in_every_list_and_the_chord_shows() {
        let mut app = app_after(2, 12, 300, Lang::En);
        render(&app, 140, 40);
        key(&mut app, KeyCode::Char('g'));
        assert!(
            render(&app, 140, 40).contains("g…"),
            "the pending key shows"
        );
        key(&mut app, KeyCode::Char('g'));
        assert_eq!(app.selected, Sel::Project(0));
        key(&mut app, KeyCode::Char('G'));
        let last = *app.shown.borrow().last().unwrap();
        assert_eq!(app.selected, last);
        app.pending = None;
        // The journal, newest first: gg is the newest line.
        app.pane = Pane::Journal;
        key(&mut app, KeyCode::Char('G'));
        let rows = journal_rows(&app.world);
        assert_eq!(app.journal_sel, Some(rows.last().unwrap().0));
        key(&mut app, KeyCode::Char('g'));
        key(&mut app, KeyCode::Char('g'));
        assert_eq!(app.journal_sel, Some(rows[0].0));
        // Inside a conversation too.
        let mut app = talking();
        app.open_thread(4);
        key(&mut app, KeyCode::Char('g'));
        key(&mut app, KeyCode::Char('g'));
        assert_eq!(app.thread.as_ref().unwrap().sel, 0);
        key(&mut app, KeyCode::Char('G'));
        assert_eq!(app.thread.as_ref().unwrap().sel, 3);
    }

    #[test]
    fn z_chords_fold_one_project_all_or_none() {
        let mut app = app_after(3, 12, 100, Lang::En);
        render(&app, 140, 40);
        app.selected = Sel::Agent(app.world.projects[1].leader);
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('a'));
        assert_eq!(app.collapsed, [false, true, false]);
        assert_eq!(app.selected, Sel::Project(1), "the selection stays visible");
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('a'));
        assert_eq!(app.collapsed, [false, false, false]);
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('M'));
        assert_eq!(app.collapsed, [true, true, true]);
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('R'));
        assert_eq!(app.collapsed, [false, false, false]);
        // A chord that means nothing is dropped, not kept pending.
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('x'));
        assert_eq!(app.pending, None);
        key(&mut app, KeyCode::Char('j'));
        assert_ne!(app.selected, Sel::Project(0), "j moved as usual");
    }

    #[test]
    fn t_opens_the_trace_and_v_the_conversation_of_the_agent_at_hand() {
        let mut app = talking();
        app.selected = Sel::Agent(0);
        key(&mut app, KeyCode::Char('t'));
        assert_eq!(app.zoom, Zoom::Trace(0));
        key(&mut app, KeyCode::Char('v'));
        let th = app.thread.as_ref().unwrap();
        assert_eq!(th.entries, [0, 1, 3, 4], "you ⇄ lead, from the trace too");
        app.thread = None;
        app.zoom = Zoom::None;
        app.selected = Sel::Agent(2);
        key(&mut app, KeyCode::Char('v'));
        assert!(app.thread.is_none());
        let screen = render(&app, 140, 40);
        assert!(screen.contains("no conversation with"), "{screen}");
        // On a project line, the leader is at hand.
        app.selected = Sel::Project(0);
        key(&mut app, KeyCode::Char('v'));
        assert!(app.thread.is_some());
    }

    #[test]
    fn panels_cycle_both_ways_and_w_goes_to_the_inbox() {
        let mut app = app_after(1, 4, 50, Lang::En);
        key(&mut app, KeyCode::BackTab);
        assert_eq!(app.pane, Pane::Journal);
        assert!(app.journal_sel.is_some());
        key(&mut app, KeyCode::Char('['));
        assert_eq!(app.pane, Pane::Inbox);
        key(&mut app, KeyCode::Char(']'));
        assert_eq!(app.pane, Pane::Journal);
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.pane, Pane::Salon);
        key(&mut app, KeyCode::Char('w'));
        assert_eq!(app.pane, Pane::Inbox);
        key(&mut app, KeyCode::Char('i'));
        assert!(app.typing, "i writes, as in vim");
    }

    #[test]
    fn a_bang_opens_the_shell_line_and_enter_hands_the_command_to_the_main_loop() {
        let mut app = app_after(1, 2, 0, Lang::En);
        key(&mut app, KeyCode::Char('!'));
        assert!(app.typing);
        assert_eq!(app.input, "!");
        assert!(app.typing_shell());
        assert!(!app.typing_command());
        let s = render(&app, 140, 40);
        assert!(s.contains("› shell:"), "{s}");
        assert!(!s.contains("› command:"), "{s}");
        for c in "  cargo test -- --nocapture  ".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.shell.as_deref(), Some("cargo test -- --nocapture"));
        assert!(!app.typing);
        assert_eq!(app.input, "");
        assert!(
            !app.world
                .journal
                .iter()
                .any(|e| matches!(&e.event, Event::YouSaid { .. })),
            "a shell command is not a message to the lead"
        );
        // A bare `!` runs nothing and says so.
        key(&mut app, KeyCode::Char('!'));
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.shell.as_deref(), Some("cargo test -- --nocapture"));
        let s = render(&app, 140, 40);
        assert!(s.contains("the command is empty"), "{s}");
        key(&mut app, KeyCode::Char('!'));
        for c in "   ".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.shell.as_deref(), Some("cargo test -- --nocapture"));
        // The demo runs commands where it was started; the shell line
        // is the same in Turkish.
        assert_eq!(app.cwd(), std::path::PathBuf::from("."));
        app.lang = Lang::Tr;
        key(&mut app, KeyCode::Char('!'));
        assert!(render(&app, 140, 40).contains("› kabuk:"));
        key(&mut app, KeyCode::Esc);
        // Both keys are in the help, whole, in both languages, `O` on the
        // `!` row.
        for (lang, bang, w) in [
            (
                Lang::En,
                "run cmd in your shell; O quotes its output",
                "straight to waiting for you",
            ),
            (
                Lang::Tr,
                "kmt kendi kabuğunda; O çıktısını alıntılar",
                "doğrudan seni bekleyenlere",
            ),
        ] {
            app.lang = lang;
            app.help = true;
            let s = render(&app, 140, 40);
            assert!(s.contains(bang), "{s}");
            let row = s.lines().find(|l| l.contains(w)).unwrap_or_default();
            assert!(row.contains(" w "), "{row}");
        }
    }

    #[test]
    fn a_command_that_ran_is_journaled_under_the_project_and_flashed() {
        use crate::shell::{Caught, Ran};
        use std::time::Duration;
        let back = |ended: Ended, ms: u64| Ran {
            ended,
            took: Duration::from_millis(ms),
        };
        let ran = |ended: Ended, ms: u64| {
            Ok(Caught {
                ran: back(ended, ms),
                kept: Vec::new(),
                lines: 0,
            })
        };
        let mut app = app_after(2, 2, 0, Lang::En);
        app.selected = Sel::Project(1);
        app.ran("cargo test", ran(Ended::Exit(0), 2_400));
        let e = app.world.journal.last().unwrap();
        let leader = app.world.projects[1].leader;
        assert!(
            matches!(&e.event, Event::Shell { leader: l, command, ended: Ended::Exit(0), took: Some(2_400) }
                if *l == leader && command == "cargo test"),
            "{:?}",
            e.event
        );
        assert_eq!(e.event.agent(), leader);
        assert_eq!(e.event.kind(), EntryKind::Person);
        let s = render(&app, 140, 40);
        assert!(s.contains("you ran `cargo test` → exit 0 · 2 s"), "{s}");
        app.ran("sleep 9", ran(Ended::Signal(2), 61_000));
        let s = render(&app, 140, 40);
        assert!(
            s.contains("you ran `sleep 9` → stopped by signal 2 · 1 min 1 s"),
            "{s}"
        );
        app.ran("make", ran(Ended::Exit(2), 40));
        assert!(render(&app, 140, 40).contains("you ran `make` → exit 2 · 40 ms"));
        // A shell that could not start is said, not journaled.
        let n = app.world.journal.len();
        app.ran("x", Err(std::io::Error::other("no shell")));
        assert_eq!(app.world.journal.len(), n);
        let s = render(&app, 140, 40);
        assert!(s.contains("could not run `x`: no shell"), "{s}");
        // Turkish words it the same way, and the line under the output.
        app.lang = Lang::Tr;
        app.ran("make", ran(Ended::Exit(1), 3_000));
        let s = render(&app, 140, 40);
        assert!(
            s.contains("`make` çalıştırdın → çıkış 1 · 3 sn sürdü"),
            "{s}"
        );
        assert_eq!(
            app.shell_back(back(Ended::Exit(3), 5_000)),
            "[imece] çıkış 3 · 5 sn sürdü · herhangi bir tuş: ekrana dön\n"
        );
        assert_eq!(
            app.shell_back(back(Ended::Signal(9), 5_000)),
            "[imece] 9 sinyali durdurdu · 5 sn sürdü · herhangi bir tuş: ekrana dön\n"
        );
        app.lang = Lang::En;
        assert_eq!(
            app.shell_back(back(Ended::Exit(0), 7)),
            "[imece] exit 0 · 7 ms · any key: back to the screen\n"
        );
        assert_eq!(
            app.shell_back(back(Ended::Signal(15), 1_500)),
            "[imece] stopped by signal 15 · 1 s · any key: back to the screen\n"
        );
        // A line saved before the time was kept says none.
        let old = Event::Shell {
            leader,
            command: "ls".into(),
            ended: Ended::Exit(0),
            took: None,
        };
        let line = crate::ui::words::event(Lang::En, &app.world, &old);
        assert_eq!(line, "you ran `ls` → exit 0");
        // What the person typed is cleaned like everything else drawn.
        app.ran("echo \x1b[31mred TOKEN=abc123", ran(Ended::Exit(0), 1));
        let s = render(&app, 140, 40);
        assert!(!s.contains("abc123") && !s.contains('\x1b'), "{s}");
    }

    #[test]
    fn ctrl_d_and_u_page_the_conversation_and_the_lists() {
        let mut app = talking();
        app.open_thread(4);
        render(&app, 100, 20);
        let th = app.thread.as_ref().unwrap();
        th.scroll.set(0);
        ctrl(&mut app, 'd');
        let after = app.thread.as_ref().unwrap().scroll.get();
        assert!(after > 0, "scrolled half a page");
        ctrl(&mut app, 'u');
        assert_eq!(app.thread.as_ref().unwrap().scroll.get(), 0);
        app.thread = None;
        let mut app = app_after(1, 20, 100, Lang::En);
        render(&app, 140, 60);
        app.selected = Sel::Project(0);
        ctrl(&mut app, 'd');
        let pos = app
            .shown
            .borrow()
            .iter()
            .position(|&s| s == app.selected)
            .unwrap();
        assert_eq!(pos, 5);
    }

    #[test]
    fn a_solo_s_row_names_its_vendor_and_choice_and_its_helpers_hang_under_it() {
        let mut app = app_after(1, 1, 0, Lang::En);
        app.home = None;
        let typed = |app: &mut App, line: &str| {
            for c in line.chars() {
                key(app, KeyCode::Char(c));
            }
            key(app, KeyCode::Enter);
        };
        typed(&mut app, ":solo codex:gpt-5.6-terra@xhigh notes");
        let head = app.world.projects[1].leader;
        let text =
            |line: &Line| -> String { line.spans.iter().map(|s| s.content.as_ref()).collect() };
        let line = salon::agent_line(&app, &app.world.agents[head], "", &app.rungs(), 200);
        assert!(
            text(&line).contains("solo-notes") && text(&line).contains("codex gpt-5.6-terra@xhigh"),
            "{}",
            text(&line)
        );
        assert_eq!(
            line.spans[1].content, "○ ",
            "a solo's glyph is its status, not ◆"
        );
        app.world.agents[head].stand.added = 3;
        let line = text(&salon::agent_line(
            &app,
            &app.world.agents[head],
            "",
            &app.rungs(),
            200,
        ));
        assert!(
            line.contains("+3"),
            "a solo's row shows where its worktree stands: {line}"
        );
        // Without a choice: the vendor alone.
        typed(&mut app, ":solo claude other");
        let other = app.world.projects[2].leader;
        let line = text(&salon::agent_line(
            &app,
            &app.world.agents[other],
            "",
            &app.rungs(),
            200,
        ));
        assert!(
            line.contains(" claude ") && !line.contains("claude @"),
            "{line}"
        );
        // Its helpers are rows under it, as a worker's are.
        let helper = app.world.agents.len();
        let mut h = Agent::new(
            helper,
            1,
            "solo-notes.1".into(),
            Vendor::Codex,
            Role::Helper,
            Status::Working,
            0,
        );
        h.parent = Some(head);
        app.world.agents.push(h);
        let rows = salon::rows(&app, 40);
        let at = |id| {
            rows.iter()
                .position(|r| matches!(r, salon::Row::Agent { id: i, .. } if *i == id))
        };
        let (head_at, helper_at) = (at(head).unwrap(), at(helper).unwrap());
        assert_eq!(helper_at, head_at + 1);
        assert!(matches!(
            rows[helper_at],
            salon::Row::Agent { depth: 2, .. }
        ));
        // The project's header counts a working solo as working.
        app.world.agents[head].set_status(Status::Working, 0);
        let header = text(&salon::project_line(&app, 1, &app.rungs()));
        assert!(
            header.contains("2 working") || header.contains("working"),
            "{header}"
        );
    }

    #[test]
    fn the_help_names_the_new_line_key_this_terminal_has_after_the_writing_keys() {
        for lang in [Lang::En, Lang::Tr] {
            let t = lang.text();
            for shift_enter in [true, false] {
                let keys = overlay::help_keys(lang, shift_enter);
                assert_eq!(keys[18].0, "^w ^u ^k ^y", "{lang:?}");
                let want = t.help_newline[usize::from(!shift_enter)];
                assert_eq!(keys[19], want, "{lang:?} {shift_enter}");
                assert_eq!(keys.len(), t.help_keys.len() + 1);
            }
        }
        let mut app = app_after(1, 4, 0, Lang::En);
        app.help = true;
        let plain = render(&app, 140, 40);
        assert!(plain.contains("here ⇧enter is enter"), "{plain}");
        app.shift_enter = true;
        let kitty = render(&app, 140, 40);
        assert!(kitty.contains("a new line; ↑ ↓ between lines"), "{kitty}");
        assert!(!kitty.contains("here ⇧enter is enter"));
    }

    #[test]
    fn the_help_lists_the_vim_keys_in_two_columns_when_wide() {
        let mut app = app_after(1, 4, 50, Lang::En);
        app.help = true;
        let wide = render(&app, 140, 40);
        assert!(
            wide.contains("gg  G") && wide.contains("za  zM  zR"),
            "{wide}"
        );
        // Two columns: a line of the first half carries one of the second.
        let keys = overlay::help_keys(Lang::En, false);
        let row = keys.iter().position(|k| k.0 == "gg  G").unwrap();
        let other = keys[row + keys.len().div_ceil(2)].1;
        let line = wide.lines().find(|l| l.contains("gg  G")).unwrap();
        assert!(line.contains(other), "two columns: {line}");
        let narrow = render(&app, 80, 40);
        let line = narrow.lines().find(|l| l.contains("gg  G")).unwrap();
        assert!(!line.contains(other), "one column: {line}");
    }

    #[test]
    fn a_colon_lead_command_sets_the_primary_vendor_and_ask_forgets_it() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let typed = |app: &mut App, line: &str| {
            for c in line.chars() {
                key(app, KeyCode::Char(c));
            }
            key(app, KeyCode::Enter);
        };
        typed(&mut app, ":lead codex");
        assert_eq!(app.primary.as_deref(), Some("codex"));
        let s = render(&app, 140, 40);
        assert!(s.contains("codex leads from the next start on"), "{s}");
        typed(&mut app, ":lead qwen");
        assert_eq!(app.primary.as_deref(), Some("codex"), "unchanged");
        assert!(render(&app, 140, 40).contains("qwen is not claude, codex, gemini or ask"));
        typed(&mut app, ":lead ask");
        assert_eq!(app.primary, None);
        assert!(render(&app, 140, 40).contains("the next start asks who leads"));
        assert!(
            !app.world
                .journal
                .iter()
                .any(|e| matches!(&e.event, Event::YouSaid { .. })),
            "a command is not a message to the lead"
        );
    }

    fn buffer(app: &App, width: u16, height: u16) -> ratatui::buffer::Buffer {
        let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        t.backend().buffer().clone()
    }

    #[test]
    fn the_backdrop_is_laid_when_the_theme_has_a_background_or_a_text_colour() {
        assert_eq!(backdrop(Color::Reset, Color::Reset), None);
        let (bg, fg) = (Color::Rgb(1, 2, 3), Color::Indexed(7));
        assert_eq!(backdrop(bg, fg), Some(Style::new().bg(bg).fg(fg)));
        assert_eq!(
            backdrop(bg, Color::Reset),
            Some(Style::new().bg(bg).fg(Color::Reset)),
            "a background alone"
        );
        assert_eq!(
            backdrop(Color::Reset, fg),
            Some(Style::new().bg(Color::Reset).fg(fg)),
            "a text colour alone"
        );
    }

    #[test]
    fn fold_lowers_the_case_and_the_turkish_letters() {
        assert_eq!(overlay::fold("ÇçĞğİıÖöŞşÜü"), "ccggiioossuu");
        assert_eq!(overlay::fold("ISIM Abc ⇧enter"), "isim abc ⇧enter");
    }

    #[test]
    fn the_help_filter_keeps_what_has_every_word_in_either_column() {
        let text = |k: &(&str, &str)| format!("{} {}", k.0, k.1);
        let keys = overlay::help_keys(Lang::En, false);
        assert_eq!(overlay::found(&keys, "", text).len(), keys.len(), "all");
        assert_eq!(overlay::found(&keys, "  ", text).len(), keys.len());
        let got = overlay::found(&keys, "FIRST  last", text);
        assert_eq!(got, [&("gg  G", "first / last line")], "every word");
        assert!(overlay::found(&keys, "first zzz", text).is_empty());
        let got = overlay::found(&keys, "zM", text);
        let fold = ("za  zM  zR", "fold: toggle · all (:fold) · none (:unfold)");
        assert_eq!(got, [&fold], "keys");
        // Turkish letters typed or not.
        let tr = overlay::help_keys(Lang::Tr, false);
        for filter in ["butceyi artir", "bütçeyi artır", "BÜTÇEYİ"] {
            let got = overlay::found(&tr, filter, text);
            assert_eq!(got.len(), 1, "{filter}");
            assert_eq!(got[0].0, ":budget USD");
        }
    }

    #[test]
    fn slash_in_the_help_filters_it_until_enter_keeps_or_esc_drops_it() {
        let mut app = app_after(1, 4, 0, Lang::En);
        let typed = |app: &mut App, s: &str| s.chars().for_each(|c| key(app, KeyCode::Char(c)));
        key(&mut app, KeyCode::Char('?'));
        assert!(app.help && !app.help_typing);
        key(&mut app, KeyCode::Char('/'));
        assert!(app.help && app.help_typing);
        typed(&mut app, "f?/qu");
        assert_eq!(app.help_find, "f?/qu", "every key is typed, ? q u too");
        ctrl(&mut app, 'u');
        assert_eq!(app.help_find, "", "^u empties it");
        ctrl(&mut app, 'x');
        alt(&mut app, KeyCode::Char('x'));
        key(&mut app, KeyCode::Up);
        assert_eq!((app.help_find.as_str(), app.help_typing), ("", true));
        typed(&mut app, "folds");
        key(&mut app, KeyCode::Backspace);
        assert_eq!(app.help_find, "fold");
        let s = render(&app, 140, 40);
        assert!(s.contains(" /fold▏") && s.contains("enter keep"), "{s}");
        // Enter keeps it; / goes back to it; any other key closes.
        key(&mut app, KeyCode::Enter);
        assert!(app.help && !app.help_typing);
        assert_eq!(app.help_find, "fold");
        let s = render(&app, 140, 40);
        assert!(s.contains(" /fold ") && !s.contains("▏"), "{s}");
        assert!(s.contains("/ change · any other key: close"), "{s}");
        key(&mut app, KeyCode::Char('/'));
        typed(&mut app, "s");
        assert_eq!((app.help_find.as_str(), app.help_typing), ("folds", true));
        // Esc drops it and the whole help is back.
        key(&mut app, KeyCode::Esc);
        assert!(app.help && !app.help_typing);
        assert_eq!(app.help_find, "");
        let s = render(&app, 140, 40);
        assert!(s.contains("/ filter · any other key: close"), "{s}");
        // Backspace past the start leaves the filter, not the help.
        key(&mut app, KeyCode::Char('/'));
        typed(&mut app, "x");
        key(&mut app, KeyCode::Backspace);
        assert!(app.help_typing, "x went, the filter stays");
        key(&mut app, KeyCode::Backspace);
        assert!(app.help && !app.help_typing);
        // A kept filter closes with the help and is gone when it opens.
        key(&mut app, KeyCode::Char('/'));
        typed(&mut app, "gg");
        key(&mut app, KeyCode::Enter);
        key(&mut app, KeyCode::Char('j'));
        assert!(!app.help, "closed");
        key(&mut app, KeyCode::Char('?'));
        assert!(app.help && app.help_find.is_empty(), "{:?}", app.help_find);
    }

    #[test]
    fn a_filtered_help_shows_what_matches_and_keeps_its_size() {
        let t = Lang::En.text();
        let mut app = app_after(1, 4, 50, Lang::En);
        app.help = true;
        // The rows of the frame's top and bottom corners.
        let frame = |app: &App, width: u16| {
            let buf = buffer(app, width, 40);
            let ys: Vec<u16> = (0..40)
                .filter(|&y| (0..width).any(|x| matches!(buf[(x, y)].symbol(), "┌" | "└")))
                .collect();
            (ys[0], ys[ys.len() - 1])
        };
        let (wide, narrow) = (frame(&app, 140), frame(&app, 80));
        app.help_typing = true;
        for (find, keys, legend, none) in [
            ("fold", &["za  zM  zR", "← →  h l"][..], false, false),
            ("tainted", &[][..], true, false),
            ("zzz", &[][..], false, true),
            ("", &["gg  G", "za  zM  zR"][..], true, false),
        ] {
            app.help_find = find.into();
            let s = render(&app, 140, 40);
            for k in keys {
                assert!(s.contains(k), "{find}: {k} {s}");
            }
            assert_eq!(s.contains("gg  G"), find.is_empty(), "{find}: {s}");
            assert_eq!(s.contains("≈ tainted"), legend, "{find}: {s}");
            assert_eq!(s.contains("◆ lead"), find.is_empty(), "{find}: {s}");
            assert_eq!(s.contains(t.help_none.trim()), none, "{find}: {s}");
            assert_eq!(frame(&app, 140), wide, "{find}: the same size");
            assert_eq!(frame(&app, 80), narrow, "{find}: narrow too");
        }
    }

    #[test]
    fn a_paste_goes_into_the_help_filter_while_it_is_typed() {
        let mut app = app_after(1, 4, 0, Lang::En);
        (app.help, app.help_typing) = (true, true);
        app.help_find = "a".into();
        app.paste("b\nc\td\u{1b}");
        assert_eq!(app.help_find, "ab c d", "one line, no control characters");
        assert!(app.flash.is_none() && app.input.is_empty());
        app.help_typing = false;
        app.paste("x");
        assert_eq!(app.help_find, "ab c d");
        assert!(app.flash.is_some(), "with the filter not typed, as before");
        // Not into a filter left from a help that closed.
        (app.help, app.help_typing) = (false, true);
        app.flash = None;
        app.paste("x");
        assert_eq!(app.help_find, "ab c d");
    }

    #[test]
    fn the_help_window_is_as_tall_as_its_lines_and_their_frame() {
        let mut app = app_after(1, 4, 50, Lang::En);
        app.help = true;
        let t = Lang::En.text();
        // Forty rows: the help must fit a screen that tall, whole.
        let buf = buffer(&app, 140, 40);
        let row = |y: u16| -> String { (0..140).map(|x| buf[(x, y)].symbol()).collect() };
        let top = (0..40).find(|&y| row(y).contains(t.help_title)).unwrap();
        // Two columns of keys, a blank line, the legend, a blank line, close.
        let keys = overlay::help_keys(Lang::En, false).len();
        assert_eq!(keys, t.help_keys.len() + 1, "and the new-line line");
        let lines = u16::try_from(keys.div_ceil(2) + t.help_legend.len() + 3).unwrap();
        let x = (140 - 118) / 2;
        assert_eq!(buf[(x, top)].symbol(), "┌");
        assert!(
            row(top + lines).contains(t.help_close),
            "{}",
            row(top + lines)
        );
        assert_eq!(
            buf[(x, top + lines + 1)].symbol(),
            "└",
            "the frame closes under the last line"
        );
        assert_ne!(buf[(x, top + lines + 2)].symbol(), "│", "and no further");
        assert!(
            (top..top + lines).any(|y| row(y).contains("⎇ branch  ↑ own commits")),
            "the legend says what a worker's row counts"
        );
        assert!(
            (top..top + lines).any(|y| row(y).contains("↗ a local address the worker wrote")),
            "and what its link is"
        );
    }

    #[test]
    fn a_colon_theme_command_switches_the_theme_and_t_is_short_for_it() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let typed = |app: &mut App, line: &str| {
            for c in line.chars() {
                key(app, KeyCode::Char(c));
            }
            key(app, KeyCode::Enter);
        };
        typed(&mut app, ":theme dracula");
        assert_eq!(crate::theme::pal().name, "dracula");
        assert!(render(&app, 140, 40).contains("theme: dracula"));
        typed(&mut app, ":t mono");
        assert_eq!(crate::theme::pal().name, "mono");
        typed(&mut app, ":theme nope");
        assert_eq!(crate::theme::pal().name, "mono", "unchanged");
        let s = render(&app, 140, 40);
        assert!(s.contains("theme nope: not one of"), "{s}");
        crate::theme::set("terminal");
    }

    #[test]
    fn styles_follow_the_theme_and_mono_keeps_the_signal_without_colour() {
        let p = pal();
        assert_eq!(p.name, "terminal");
        assert_eq!(dim(), Style::new().fg(p.dim));
        assert_eq!(border(), Style::new().fg(p.border));
        assert_eq!(
            brand(),
            Style::new()
                .fg(p.on_badge)
                .bg(p.accent)
                .add_modifier(Modifier::BOLD)
        );
        assert_eq!(
            attention(),
            Style::new()
                .fg(p.on_badge)
                .bg(p.warn)
                .add_modifier(Modifier::BOLD)
        );
        assert_eq!(key_style(), Style::new().fg(p.on_key).bg(p.key));
        assert_eq!(changed(), Style::new().bg(p.selection));
        assert_eq!(vendor_color(Vendor::Claude), p.vendors[0]);
        assert_eq!(vendor_color(Vendor::Codex), p.vendors[1]);
        assert_eq!(speaker_color(0), p.speakers[0]);
        assert_eq!(speaker_color(13), speaker_color(1), "colours wrap by id");
        crate::theme::set("mono");
        assert_eq!(dim(), Style::new().add_modifier(Modifier::DIM));
        assert_eq!(
            brand(),
            Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        );
        assert_eq!(attention(), brand());
        assert_eq!(key_style(), Style::new().add_modifier(Modifier::REVERSED));
        assert_eq!(changed(), Style::new().add_modifier(Modifier::BOLD));
        assert_eq!(border(), dim());
        crate::theme::set("terminal");
    }

    #[test]
    fn every_glyph_icon_and_status_has_its_own_mark() {
        let mut a = Agent::new(
            1,
            0,
            "w".into(),
            Vendor::Claude,
            Role::Worker,
            Status::Working,
            0,
        );
        assert_eq!(glyph(&a).0, "●");
        a.role = Role::Leader;
        assert_eq!(glyph(&a), ("◆", pal().plan), "a leader is always a leader");
        a.role = Role::Worker;
        let marks: Vec<&str> = [
            Status::Idle,
            Status::Working,
            Status::Asking { to: 0, until: 1 },
            Status::Blocked,
            Status::WaitingHuman,
            Status::Researching {
                for_agent: 0,
                until: 1,
            },
            Status::Stopped,
            Status::Asleep,
        ]
        .into_iter()
        .map(|s| {
            a.status = s;
            glyph(&a).0
        })
        .collect();
        assert_eq!(marks, ["○", "●", "◐", "✕", "!", "◇", "■", "◦"]);
        // On the ladder: a rung's own glyph for waiting, done and
        // starting, the status's for the rest; a leader's always `◆`.
        assert!(!a.starting, "new: not starting");
        a.status = Status::Working;
        assert_eq!(rung_glyph(&a, Rung::Waiting), ("!", pal().warn));
        assert_eq!(rung_glyph(&a, Rung::Done), ("✓", pal().ok));
        assert_eq!(rung_glyph(&a, Rung::Starting), ("◌", pal().ok));
        assert_eq!(rung_glyph(&a, Rung::Working), ("●", pal().ok));
        a.status = Status::Stopped;
        assert_eq!(rung_glyph(&a, Rung::Stuck), ("■", pal().stopped));
        a.status = Status::Blocked;
        assert_eq!(rung_glyph(&a, Rung::Stuck), ("✕", pal().bad));
        a.status = Status::Asleep;
        assert_eq!(rung_glyph(&a, Rung::Idle), ("◦", pal().dim));
        a.role = Role::Leader;
        for r in [Rung::Waiting, Rung::Done, Rung::Starting, Rung::Stuck] {
            assert_eq!(rung_glyph(&a, r), ("◆", pal().plan), "{r:?}");
        }
        a.role = Role::Worker;
        for lang in [Lang::En, Lang::Tr] {
            let legend = lang.text().help_legend.join(" ");
            for g in ["◆", "◌", "●", "◐", "✕", "!", "◇", "○", "✓", "■"] {
                assert!(legend.contains(&format!(" {g} ")), "{lang:?} {g}");
            }
        }
        a.taint = Taint::Clean;
        assert_eq!(taint_span(&a).content, " ");
        a.taint = Taint::Soft;
        assert_eq!(taint_span(&a).content, "~");
        a.taint = Taint::Full;
        assert_eq!(taint_span(&a).content, "≈");
        for k in [
            EntryKind::Assign,
            EntryKind::Plan,
            EntryKind::Talk,
            EntryKind::Assumption,
            EntryKind::Blocked,
            EntryKind::Research,
            EntryKind::Sensitive,
            EntryKind::Merged,
            EntryKind::Failed,
            EntryKind::Person,
        ] {
            assert!(!entry_icon(k).0.is_empty(), "{k:?}");
        }
        assert_eq!(entry_icon(EntryKind::Merged).0, "✓");
        assert_eq!(entry_icon(EntryKind::Person).0, "›");
        // Every status says something, in words.
        let w = app_after(1, 4, 50, Lang::En).world;
        let mut idle = Agent::new(
            9,
            0,
            "w".into(),
            Vendor::Claude,
            Role::Worker,
            Status::Idle,
            0,
        );
        let words = |a: &Agent| {
            status_spans(Lang::En, &w, a)
                .iter()
                .map(|s| s.content.to_string())
                .collect::<String>()
        };
        assert_eq!(words(&idle), "idle");
        let mut seen = vec!["idle".to_string()];
        for status in [
            Status::Blocked,
            Status::Stopped,
            Status::Asleep,
            Status::WaitingHuman,
            Status::Working,
        ] {
            idle.status = status;
            let w = words(&idle);
            assert!(!w.is_empty() && !seen.contains(&w), "{status:?}: {w:?}");
            seen.push(w);
        }
    }

    #[test]
    fn the_header_says_how_long_the_pot_lasts_and_warns_when_it_runs_low() {
        let mut app = app_after(1, 2, 0, Lang::En);
        for a in &mut app.world.agents {
            a.cost = 0;
        }
        app.world.run_spent = 0;
        app.world.budget = 10_000;
        let head = |app: &App| render(app, 160, 40).lines().next().unwrap().to_string();
        let at = |app: &App, word: &str| {
            let row = head(app);
            let x = row.chars().take(row.find(word).unwrap()).count() as u16;
            let c = buffer(app, 160, 40)[(x, 0)].clone();
            (c.fg, c.modifier)
        };
        let looks = |s: Style| (s.fg.unwrap_or(Color::Reset), s.add_modifier);
        assert_ne!(looks(attention()), looks(Style::default()));
        assert_ne!(looks(dim()), looks(Style::default()));
        assert!(
            head(&app).contains("spent $0.00 of $1.00 ($0.00/h)"),
            "{}",
            head(&app)
        );
        app.world.tick = 1;
        app.world.agents[0].cost = 100;
        app.world.watch_spending();
        let row = head(&app);
        assert!(
            row.contains("spent $0.01 of $1.00 ($0.06/h, ~16 h 30 min left)"),
            "{row}"
        );
        assert_eq!(at(&app, "spent"), looks(Style::default()));
        assert_eq!(at(&app, "$0.06/h"), looks(dim()));
        app.world.budget = 100 + 299;
        let row = head(&app);
        assert!(
            row.contains("spent $0.01 of $0.03 ($0.06/h, ~29 min left)"),
            "{row}"
        );
        assert_eq!(at(&app, "spent"), looks(attention()));
        assert_eq!(at(&app, "$0.06/h"), looks(attention()));
        app.lang = Lang::Tr;
        let row = head(&app);
        assert!(
            row.contains("$0.01 / $0.03 harcandı ($0.06/sa, ~29 dk kaldı)"),
            "{row}"
        );
    }

    #[test]
    fn the_header_shows_what_is_spent_of_the_budget_and_the_footer_hides_pause_where_it_means_nothing()
     {
        let mut app = app_after(1, 2, 0, Lang::En);
        app.world.budget = 10_000;
        for a in &mut app.world.agents {
            a.cost = 0;
        }
        app.world.agents[0].cost = 2_500;
        app.world.agents[1].cost = 2_500;
        let screen = render(&app, 140, 40);
        assert!(screen.contains("imece"));
        assert!(screen.contains("spent $0.50 of $1.00"), "{screen}");
        let footer = screen.lines().last().unwrap().to_string();
        assert!(footer.contains("space"), "{footer}");
        // The current view's name is the bold one.
        let buf = buffer(&app, 140, 40);
        let bold_runs: String = (0..140)
            .map(|x| {
                let c = &buf[(x, 39)];
                if c.style().add_modifier.contains(Modifier::BOLD) {
                    c.symbol().to_string()
                } else {
                    " ".into()
                }
            })
            .collect();
        assert!(bold_runs.contains("salon"), "{bold_runs:?}");
        assert!(!bold_runs.contains("board"), "{bold_runs:?}");
        app.zoom = Zoom::Focus(1);
        let footer = render(&app, 140, 40).lines().last().unwrap().to_string();
        assert!(!footer.contains("space"), "{footer}");
    }

    #[test]
    fn a_project_line_counts_its_blocked_workers_only_when_there_are_some() {
        let mut app = app_after(1, 4, 50, Lang::En);
        for a in &mut app.world.agents {
            if a.role == Role::Worker {
                a.status = Status::Working;
            }
        }
        app.world.inbox.clear();
        let screen = render(&app, 140, 40);
        assert!(!screen.contains("✕0"), "{screen}");
        let worker = app
            .world
            .agents
            .iter()
            .find(|a| a.role == Role::Worker)
            .unwrap()
            .id;
        app.world.agents[worker].status = Status::Blocked;
        let screen = render(&app, 140, 40);
        assert!(screen.contains("✕1"), "{screen}");
    }

    #[test]
    fn a_row_quotes_its_talk_with_the_partner_s_name_and_who_said_it() {
        let mut app = app_after(1, 4, 50, Lang::En);
        for a in &mut app.world.agents {
            a.talk = None;
            a.activity = None;
            a.status = Status::Idle;
        }
        app.world.inbox.clear();
        let (me, other) = (1, 2);
        let partner = app.world.agents[other].name.clone();
        let mine = app.world.agents[me].name.clone();
        for (said, arrow) in [(true, "→"), (false, "←")] {
            app.world.agents[me].talk = Some(Talk {
                with: Party::Agent(other),
                mine: said,
                text: "shall we split it".into(),
                until: 1_000_000,
            });
            let screen = render(&app, 200, 40);
            let row = screen
                .lines()
                .find(|l| l.contains("shall we split it"))
                .expect("the row with the talk");
            // The salon's part of the row, before the side panel.
            let salon = row.split("││").next().unwrap();
            assert!(salon.contains(&mine), "its own row\n{salon}");
            assert!(
                salon.contains(&format!("{arrow} {partner} \"shall we split it\"")),
                "{salon}"
            );
            assert!(!salon.contains("you"), "{salon}");
        }
    }

    #[test]
    fn the_inbox_border_warns_only_while_something_waits_and_it_is_not_focused() {
        let mut app = app_after(1, 4, 50, Lang::En);
        app.world.inbox.clear();
        // The inbox block's top-left corner: the second `┌` of the top row.
        let corner = |app: &App| {
            let buf = buffer(app, 140, 40);
            let x = (0..140)
                .filter(|&x| buf[(x, 1)].symbol() == "┌")
                .nth(1)
                .expect("two panes");
            buf[(x, 1)].style().fg
        };
        assert_eq!(corner(&app), Some(pal().dim), "nothing waits");
        app.world.replied(0, "an answer");
        assert_eq!(corner(&app), Some(pal().warn), "something waits");
        app.pane = Pane::Inbox;
        assert_eq!(
            corner(&app),
            Some(pal().accent),
            "focused: the focus colour"
        );
    }

    #[test]
    fn the_digest_window_shows_what_happened() {
        let mut app = app_after(1, 4, 50, Lang::En);
        app.digest = Some(crate::app::Digest {
            minutes: 7,
            merged: 2,
            blocked: 1,
            assumptions: 0,
            talks: 3,
            waiting: 0,
            highlights: Vec::new(),
        });
        let screen = render(&app, 140, 40);
        assert!(screen.contains("WHILE YOU WERE AWAY"), "{screen}");
        assert!(screen.contains("last 7 min"), "{screen}");
    }

    /// The project's name stands on an inbox line only when there are
    /// several projects to tell apart.
    #[test]
    fn an_inbox_line_names_the_project_only_among_several() {
        for (projects, named) in [(1, false), (2, true)] {
            let mut app = app_after(projects, 4, 0, Lang::En);
            let leader = app.world.projects[0].leader;
            app.world.inbox.push(InboxItem {
                kind: InboxKind::Sensitive,
                agent: leader,
                detail: "bump a dependency".into(),
                researcher: None,
                open: true,
                tick: 0,
                card: None,
            });
            let line = text(&[inbox_line(&app, app.world.inbox.len() - 1)]).remove(0);
            let name = &app.world.projects[0].name;
            assert_eq!(
                line.contains(&format!(" {name} ")),
                named,
                "{projects} projects: {line}"
            );
            assert!(line.starts_with("! 14:00 "), "{line}");
        }
    }

    #[test]
    fn the_where_you_left_off_window_tells_what_was_left() {
        let mut app = talking_over_two_days();
        let leader = app.world.projects[0].leader;
        let worker = app
            .world
            .agents
            .iter()
            .find(|a| a.role == Role::Worker)
            .map(|a| a.id)
            .unwrap();
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Reply,
            agent: leader,
            detail: "the answer's first line".into(),
            researcher: None,
            open: true,
            tick: 7300,
            card: None,
        });
        let item = app.world.inbox.len() - 1;
        // The last lines: one of yesterday, then today's.
        let last: Vec<Entry> = app.world.journal[2..5].to_vec();
        assert_eq!((last[0].tick, last[1].tick), (0, 7200));
        app.left_off = Some(crate::app::LeftOff {
            ago: Some(3 * 3600 + 5 * 60),
            spent: 1_234,
            budget: 500_000,
            projects: vec![("shop-api".into(), 3, 2), ("other".into(), 1, 0)],
            waiting: vec![item],
            waiting_total: 2,
            interrupted: vec![leader, worker],
            last,
        });
        let screen = render(&app, 140, 40);
        for want in [
            "WHERE YOU LEFT OFF · 3 h 5 min ago",
            "$0.12 of $50.00 spent",
            "shop-api: 3 agents, 2 open cards",
            "other: 1 agents, 0 open cards",
            "! 2 wait for you:",
            "» 00:08 ",
            "the answer's first line",
            "… and 1 more",
            &format!(
                "turns cut short when imece closed: {}, {}",
                app.world.agents[leader].name, app.world.agents[worker].name
            ),
            "the last lines before:",
            "─ 1970-01-01",
            "any key: close",
        ] {
            assert!(screen.contains(want), "{want} missing:\n{screen}");
        }
        // Oldest first, like a conversation: each day's lines under its
        // date, the window's own rows (the journal pane has a day row too).
        let lines: Vec<&str> = screen.lines().collect();
        let top = lines
            .iter()
            .position(|l| l.contains("WHERE YOU LEFT OFF"))
            .unwrap();
        let window: Vec<&str> = lines[top..]
            .iter()
            .take_while(|l| !l.contains("any key: close"))
            .copied()
            .collect();
        let at = |needle: &str| {
            window
                .iter()
                .position(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("{needle} not in the window\n{screen}"))
        };
        assert_eq!(at("14:00 ⇄ codex-1"), at("─ 1970-01-01") + 1, "{screen}");
        assert_eq!(at("00:00 › you"), at("─ 1970-01-02") + 1, "{screen}");
        assert_eq!(
            window.iter().filter(|l| l.contains("─ 1970-0")).count(),
            2,
            "two days, two rows; the second day's second line needs none\n{screen}"
        );
        // Every waiting item shown: no "more" line.
        app.left_off.as_mut().unwrap().waiting_total = 1;
        let screen = render(&app, 140, 40);
        assert!(!screen.contains("… and"), "{screen}");
        // Without a time, without waiting items, without cut turns.
        let l = app.left_off.as_mut().unwrap();
        l.ago = None;
        l.waiting.clear();
        l.waiting_total = 0;
        l.interrupted.clear();
        l.last.clear();
        let screen = render(&app, 140, 40);
        assert!(screen.contains("WHERE YOU LEFT OFF "), "{screen}");
        assert!(!screen.contains(" ago "), "{screen}");
        assert!(screen.contains("Nothing needs you right now."), "{screen}");
        for gone in ["wait for you", "cut short", "the last lines"] {
            assert!(!screen.contains(gone), "{gone} still on\n{screen}");
        }
        app.lang = Lang::Tr;
        app.left_off.as_mut().unwrap().ago = Some(60);
        let screen = render(&app, 140, 40);
        assert!(screen.contains("NEREDE KALMIŞTIN · 1 dk önce"), "{screen}");
        // The help wins over it, the key closes one at a time.
        app.help = true;
        let screen = render(&app, 140, 40);
        assert!(!screen.contains("NEREDE KALMIŞTIN"), "{screen}");
    }

    #[test]
    fn keys_do_only_what_they_do_where_they_do_it() {
        let mut app = app_after(2, 8, 100, Lang::En);
        render(&app, 140, 40);
        key(&mut app, KeyCode::Char('?'));
        assert!(app.help);
        key(&mut app, KeyCode::Esc);
        assert!(!app.help, "any key closes the help");
        // Panels cycle only on the salon, outside a focus.
        app.view = View::Board;
        for code in [
            KeyCode::Tab,
            KeyCode::BackTab,
            KeyCode::Char('['),
            KeyCode::Char(']'),
            KeyCode::Char('w'),
        ] {
            key(&mut app, code);
            assert_eq!(app.pane, Pane::Salon, "{code:?} on the board");
        }
        app.view = View::Salon;
        app.zoom = Zoom::Focus(1);
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.pane, Pane::Salon, "tab in a focus");
        app.zoom = Zoom::None;
        // Copying is the journal's; chords that fold are the salon's. A
        // stale journal selection must not make c copy elsewhere.
        app.journal_sel = Some(0);
        key(&mut app, KeyCode::Char('c'));
        assert!(app.copy.is_none(), "c in the salon copies nothing");
        app.pane = Pane::Journal;
        app.zoom = Zoom::Focus(1);
        key(&mut app, KeyCode::Char('c'));
        assert!(app.copy.is_none(), "c in a focus copies nothing");
        app.zoom = Zoom::None;
        app.pane = Pane::Inbox;
        for second in ['a', 'M'] {
            key(&mut app, KeyCode::Char('z'));
            key(&mut app, KeyCode::Char(second));
            assert_eq!(app.collapsed, [false, false], "z{second} outside the salon");
        }
        app.view = View::Board;
        app.collapsed = vec![true, true];
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('R'));
        assert_eq!(app.collapsed, [true, true], "zR on the board");
        app.view = View::Salon;
        app.pane = Pane::Salon;
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('R'));
        assert_eq!(app.collapsed, [false, false], "zR in the salon");
        key(&mut app, KeyCode::Char('q'));
        key(&mut app, KeyCode::Char('q'));
        assert!(app.quit);
    }

    #[test]
    fn one_q_asks_and_only_a_second_q_quits() {
        let mut app = app_after(1, 4, 50, Lang::En);
        render(&app, 140, 40);
        let before = app.selected;
        key(&mut app, KeyCode::Char('q'));
        assert!(!app.quit, "one q does not quit");
        assert_eq!(app.pending, Some('q'));
        let s = render(&app, 140, 40);
        // The demo's agents are at work by now: the question counts them.
        assert!(
            s.contains("quit imece? agents mid-turn: ") && s.contains("· q again quits"),
            "{s}"
        );
        // Any other key stays, and is spent on the answer.
        key(&mut app, KeyCode::Char('j'));
        assert!(!app.quit);
        assert_eq!(app.pending, None);
        assert_eq!(
            app.selected, before,
            "j answered the question, it did not move"
        );
        assert!(app.flash.is_none(), "the question is gone");
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Char('q'));
        key(&mut app, KeyCode::Esc);
        assert!(!app.quit && app.pending.is_none(), "esc stays too");
        key(&mut app, KeyCode::Char('q'));
        key(&mut app, KeyCode::Char('q'));
        assert!(app.quit, "qq quits");
        // In Turkish too, and with a conversation open.
        let mut app = talking();
        app.lang = Lang::Tr;
        app.open_thread(1);
        key(&mut app, KeyCode::Char('q'));
        assert!(render(&app, 140, 40).contains("imece kapatılsın mı?"));
        assert!(app.thread.is_some());
        key(&mut app, KeyCode::Char('q'));
        assert!(app.quit);
    }

    #[test]
    fn ctrl_f_and_b_page_whole_pages_and_the_conversation_takes_half_and_whole() {
        let mut app = app_after(1, 20, 100, Lang::En);
        render(&app, 140, 60);
        app.selected = Sel::Project(0);
        ctrl(&mut app, 'f');
        let pos = |app: &App| {
            app.shown
                .borrow()
                .iter()
                .position(|&s| s == app.selected)
                .unwrap()
        };
        assert_eq!(pos(&app), 10);
        ctrl(&mut app, 'b');
        assert_eq!(pos(&app), 0);
        let mut app = talking();
        app.open_thread(4);
        render(&app, 100, 20);
        let th = app.thread.as_ref().unwrap();
        th.scroll.set(0);
        let page = th.page.get();
        assert!(page > 2, "{page}");
        ctrl(&mut app, 'd');
        assert_eq!(app.thread.as_ref().unwrap().scroll.get(), page / 2);
        ctrl(&mut app, 'u');
        ctrl(&mut app, 'f');
        assert_eq!(app.thread.as_ref().unwrap().scroll.get(), page);
        key(&mut app, KeyCode::PageDown);
        assert_eq!(app.thread.as_ref().unwrap().scroll.get(), 2 * page);
        key(&mut app, KeyCode::Home);
        assert_eq!(app.thread.as_ref().unwrap().sel, 0);
        key(&mut app, KeyCode::End);
        assert_eq!(app.thread.as_ref().unwrap().sel, 3);
        key(&mut app, KeyCode::Char('c'));
        assert!(
            app.copy.take().is_some(),
            "c copies the line in a conversation"
        );
    }

    #[test]
    fn the_side_panel_resizes_within_bounds() {
        let mut app = app_after(1, 4, 50, Lang::En);
        let start = app.side;
        key(&mut app, KeyCode::Char('<'));
        assert_eq!(app.side, start + 4);
        for _ in 0..30 {
            key(&mut app, KeyCode::Char('<'));
        }
        assert_eq!(app.side, crate::prefs::SIDE_MAX);
        render(&app, 140, 40);
        for _ in 0..30 {
            key(&mut app, KeyCode::Char('>'));
        }
        assert_eq!(app.side, crate::prefs::SIDE_MIN);
        render(&app, 140, 40);
        render(&app, 60, 20);
    }

    #[test]
    fn without_a_merge_queue_there_is_no_trunk_or_lease() {
        let mut app = app_after(1, 4, 300, Lang::En);
        app.world.projects[0].trunk = None;
        app.world.projects[0].repo = &[];
        for c in &mut app.world.cards {
            c.symbols.clear();
        }
        app.world.max_researchers = 0;
        let salon = render(&app, 140, 40);
        assert!(!salon.contains("trunk #"), "{salon}");
        assert!(!salon.contains("researchers"), "{salon}");
        app.zoom = Zoom::Focus(1);
        assert!(!render(&app, 140, 40).contains("lease"));
        app.zoom = Zoom::None;
        app.view = View::Code;
        assert!(render(&app, 140, 40).contains("no shared trunk yet"));
        // Before any card exists there is no "0/0 cards" either.
        app.view = View::Salon;
        app.world.cards.clear();
        app.world.journal.clear();
        app.world.inbox.clear();
        app.world.plans.clear();
        for a in &mut app.world.agents {
            a.card = None;
        }
        let screen = render(&app, 140, 40);
        assert!(!screen.contains("cards"), "{screen}");
    }

    fn text(rows: &[Line]) -> Vec<String> {
        rows.iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    /// A quiet team at tick 100: everyone idle since now, nothing said,
    /// nothing waiting, nothing new.
    fn quiet_rows() -> App {
        let mut app = app_after(1, 4, 50, Lang::En);
        app.world.tick = 100;
        app.world.inbox.clear();
        app.unseen.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
            a.activity = None;
            a.talk = None;
            a.topic.clear();
            a.starting = false;
            a.changed_at = 100;
            a.local = None;
            a.plan.clear();
            a.context = None;
            a.cost = 0;
        }
        app
    }

    /// Agent `id`'s row, `width` wide, as one string.
    fn row(app: &App, id: AgentId, width: usize) -> String {
        salon::agent_line(app, &app.world.agents[id], "", &app.rungs(), width)
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect()
    }

    fn told(a: &mut Agent, with: Party, mine: bool, text: &str) {
        a.talk = Some(Talk {
            with,
            mine,
            text: text.into(),
            until: 0,
        });
    }

    #[test]
    fn quoted_cleans_cuts_to_the_room_and_says_nothing_under_the_minimum() {
        assert_eq!(quoted("abc", 8).as_deref(), Some("\"abc\""));
        assert_eq!(quoted("abc", 7), None, "under QUOTE_MIN");
        assert_eq!(
            quoted("abcdefgh", 8).as_deref(),
            Some("\"abcde…\""),
            "cut, its quotes inside the room"
        );
        assert_eq!(
            quoted("abcdef", 8).as_deref(),
            Some("\"abcdef\""),
            "six letters fit in eight"
        );
        assert_eq!(quoted("", 20), None);
        assert_eq!(quoted("   ", 20), None);
        assert_eq!(quoted("  x  ", 20).as_deref(), Some("\"x\""), "trimmed");
        let q = quoted("a\x1b[31mb", 20).unwrap();
        assert!(!q.contains('\x1b'), "{q:?}");
    }

    #[test]
    fn a_status_carries_the_text_that_belongs_to_it() {
        let mut app = quiet_rows();
        let w = 1;
        let words = |app: &App, lang: Lang| -> String {
            status_spans(lang, &app.world, &app.world.agents[w])
                .iter()
                .map(|s| s.content.as_ref())
                .collect()
        };
        // An answer not read yet: its first line.
        app.world.replied(w, "\nUse the enum.\nmore");
        assert_eq!(words(&app, Lang::En), "answered you \"Use the enum.\"");
        app.world.inbox.clear();
        // A tool call waiting for a yes: what it would do, on one line.
        app.world.agents[w].status = Status::Working;
        app.world.asks_you(w, "cargo test\n-q");
        assert_eq!(
            words(&app, Lang::En),
            "⚑ waits for your yes \"cargo test ↵ -q\""
        );
        // The forty columns: the mark and the words take 20, a space and
        // the quotes 3, so 17 letters show whole and 18 are cut.
        app.world.inbox.clear();
        app.world.asks_you(w, &"c".repeat(17));
        assert!(words(&app, Lang::En).ends_with(&format!("\"{}\"", "c".repeat(17))));
        app.world.inbox.clear();
        app.world.asks_you(w, &"c".repeat(18));
        assert!(words(&app, Lang::En).ends_with(&format!("\"{}…\"", "c".repeat(16))));
        app.world.inbox.clear();
        // Blocked: what it waits for, then on whom.
        app.world.agents[w].status = Status::Blocked;
        app.world.agents[w].topic = "JWT clock skew".into();
        // The queue's words leave eight of the forty.
        assert_eq!(
            words(&app, Lang::En),
            "BLOCKED \"JWT c…\" in the researcher queue"
        );
        let r = (app.world.agents.iter())
            .find(|a| a.role == Role::Researcher)
            .unwrap()
            .id;
        app.world.agents[w].researcher = Some(r);
        let name = app.world.agents[r].name.clone();
        assert_eq!(
            words(&app, Lang::En),
            format!("BLOCKED \"JWT clock skew\" ← {name}")
        );
        // The topic gets what the wait leaves of the forty columns, eight
        // at the least: BLOCKED, a space, ` ← ` and 21 letters leave 8.
        app.world.agents[r].name = "r".repeat(21);
        assert_eq!(
            words(&app, Lang::En),
            format!("BLOCKED \"JWT c…\" ← {}", "r".repeat(21))
        );
        app.world.agents[r].name = "r".repeat(22);
        assert_eq!(
            words(&app, Lang::En),
            format!("BLOCKED ← {}", "r".repeat(22)),
            "seven would be left: no quote"
        );
        // Stopped: why, within the forty columns; without a why, what to
        // do about it.
        app.world.agents[w].status = Status::Stopped;
        app.world.agents[w].topic = "exited with code 1".into();
        assert_eq!(words(&app, Lang::En), "■ stopped: exited with code 1");
        assert_eq!(words(&app, Lang::Tr), "■ durdu: exited with code 1");
        app.world.agents[w].topic = "x".repeat(50);
        let said = words(&app, Lang::En);
        assert_eq!(said.chars().count(), 40, "{said}");
        assert!(
            said.starts_with("■ stopped: xxx") && said.ends_with('…'),
            "{said}"
        );
        app.world.agents[w].topic.clear();
        assert_eq!(
            words(&app, Lang::En),
            "■ stopped, write to it to start again"
        );
    }

    #[test]
    fn a_row_quotes_its_newest_exchange_only_when_the_status_says_no_text() {
        let mut app = quiet_rows();
        let (w, other) = (1, 2);
        let lead = app.world.projects[0].leader;
        let other_name = app.world.agents[other].name.clone();
        let lead_name = app.world.agents[lead].name.clone();
        // Idle: what it last said.
        told(
            &mut app.world.agents[w],
            Party::Person,
            true,
            "parser fixed",
        );
        let r = row(&app, w, 200);
        assert!(r.contains("idle → you \"parser fixed\""), "{r}");
        // Told by the lead: the lead's words, marked as told.
        told(
            &mut app.world.agents[w],
            Party::Agent(lead),
            false,
            "fix the parser",
        );
        let r = row(&app, w, 200);
        assert!(
            r.contains(&format!("idle ← {lead_name} \"fix the parser\"")),
            "{r}"
        );
        // Working: while it thinks, the quote; what it does says enough.
        app.world.agents[w].status = Status::Working;
        app.world.agents[w].activity = Some(String::new());
        let r = row(&app, w, 200);
        assert!(
            r.contains(&format!("thinking… ← {lead_name} \"fix the parser\"")),
            "{r}"
        );
        app.world.agents[w].activity = Some("Edit src/p.rs".into());
        let r = row(&app, w, 200);
        assert!(
            r.contains("Edit src/p.rs") && !r.contains("fix the parser"),
            "{r}"
        );
        app.world.agents[w].activity = None;
        let r = row(&app, w, 200);
        assert!(
            r.contains("\"fix the parser\""),
            "the bar says nothing: {r}"
        );
        app.world.agents[w].starting = true;
        let r = row(&app, w, 200);
        assert!(
            r.contains(&format!("starting… ← {lead_name} \"fix the parser\"")),
            "{r}"
        );
        app.world.agents[w].starting = false;
        // Asking: its own question to the one it waits for is quoted
        // alone, the status names them; another's word, or a word to
        // someone else, says whose.
        app.world.agents[w].status = Status::Asking {
            to: other,
            until: 0,
        };
        told(
            &mut app.world.agents[w],
            Party::Agent(other),
            true,
            "which parser?",
        );
        let r = row(&app, w, 200);
        assert!(
            r.contains(&format!("? → {other_name} \"which parser?\"")),
            "{r}"
        );
        assert!(!r.contains(&format!("{other_name} → ")), "{r}");
        told(
            &mut app.world.agents[w],
            Party::Agent(other),
            false,
            "the fast one",
        );
        let r = row(&app, w, 200);
        assert!(
            r.contains(&format!("? → {other_name} ← {other_name} \"the fast one\"")),
            "{r}"
        );
        told(
            &mut app.world.agents[w],
            Party::Agent(lead),
            true,
            "which parser?",
        );
        let r = row(&app, w, 200);
        assert!(
            r.contains(&format!("? → {other_name} → {lead_name} \"which parser?\"")),
            "{r}"
        );
        // A status with a text of its own quotes nothing more.
        told(
            &mut app.world.agents[w],
            Party::Person,
            true,
            "parser fixed",
        );
        app.world.agents[w].topic = "a topic".into();
        for status in [
            Status::Blocked,
            Status::Researching {
                for_agent: other,
                until: 0,
            },
            Status::Stopped,
        ] {
            app.world.agents[w].status = status;
            assert!(!row(&app, w, 200).contains("parser fixed"), "{status:?}");
        }
        app.world.agents[w].topic.clear();
        let r = row(&app, w, 200);
        assert!(
            r.contains("■ stopped, write to it to start again") && !r.contains('"'),
            "stopped without a why: what to do fills the column: {r}"
        );
        app.world.agents[w].status = Status::Idle;
        app.world.replied(w, "done");
        assert!(
            !row(&app, w, 200).contains("parser fixed"),
            "the answer says more"
        );
        app.world.inbox.clear();
        app.world.asked_you(w, "may I?");
        assert!(
            !row(&app, w, 200).contains("parser fixed"),
            "the question says more"
        );
        app.world.inbox.clear();
        app.world.agents[w].status = Status::Working;
        app.world.asks_you(w, "rm -r build");
        assert!(
            !row(&app, w, 200).contains("parser fixed"),
            "the ask says more"
        );
        app.world.inbox.clear();
        app.world.agents[w].status = Status::WaitingHuman;
        let r = row(&app, w, 200);
        assert!(r.contains("waiting for you → you \"parser fixed\""), "{r}");
        app.world.agents[w].status = Status::Asleep;
        let r = row(&app, w, 200);
        assert!(
            r.contains("write to it to wake it") && !r.contains('"'),
            "asleep: the hint fills the column: {r}"
        );
        // Done, not seen: what it said at the end of its turn.
        app.world.agents[w].status = Status::Idle;
        app.unseen.insert(w);
        let r = row(&app, w, 200);
        assert!(
            r.contains("done, not seen yet → you \"parser fixed\""),
            "{r}"
        );
        // The quote keeps to the status column: `idle`, a space and
        // `→ you ` leave 29 of its forty.
        app.unseen.clear();
        told(
            &mut app.world.agents[w],
            Party::Person,
            true,
            &"a".repeat(40),
        );
        let r = row(&app, w, 200);
        assert!(
            r.contains(&format!("idle → you \"{}…\"", "a".repeat(26))),
            "{r}"
        );
        assert!(!r.contains(&"a".repeat(27)), "{r}");
    }

    fn steps(n: usize, done: usize) -> Vec<Step> {
        (1..=n)
            .map(|i| Step {
                text: format!("s{i}"),
                state: if i <= done {
                    StepState::Done
                } else if i == done + 1 {
                    StepState::Doing
                } else {
                    StepState::Todo
                },
            })
            .collect()
    }

    #[test]
    fn a_row_marks_how_far_its_to_do_list_is_and_the_focus_lists_the_steps() {
        let mut app = quiet_rows();
        let w = 1;
        assert!(!row(&app, w, 200).contains('≡'));
        let a = &mut app.world.agents[w];
        a.plan = vec![
            Step {
                text: "Read the parser".into(),
                state: StepState::Done,
            },
            Step {
                text: "Change it".into(),
                state: StepState::Doing,
            },
            Step {
                text: "Test <it>".into(),
                state: StepState::Todo,
            },
        ];
        a.cost = 12_345;
        a.changed_at = 64;
        let r = row(&app, w, 200);
        assert!(r.contains("idle · 3 min  ≡ 1/3  $1.23"), "{r}");
        app.zoom = Zoom::Focus(w);
        let s = render(&app, 140, 40);
        for line in [
            "TO DO 1/3",
            "✓ Read the parser",
            "▸ Change it",
            "○ Test <it>",
        ] {
            assert!(s.contains(line), "{line}\n{s}");
        }
        app.lang = Lang::Tr;
        assert!(render(&app, 140, 40).contains("YAPILACAKLAR 1/3"));
        // No list, no title.
        app.world.agents[w].plan.clear();
        assert!(!render(&app, 140, 40).contains("YAPILACAKLAR"));
    }

    #[test]
    fn a_long_to_do_list_folds_its_done_steps_and_counts_what_it_leaves_off() {
        let t = Lang::En.text();
        let text = |lines: Vec<Line>| -> Vec<String> {
            lines
                .iter()
                .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
                .collect()
        };
        // Eight fit whole.
        assert_eq!(
            text(focus::plan_lines(t, &steps(8, 2))),
            [
                "✓ s1", "✓ s2", "▸ s3", "○ s4", "○ s5", "○ s6", "○ s7", "○ s8"
            ]
        );
        // Twelve, four done: the done fold, the rest to the cap, the count.
        assert_eq!(
            text(focus::plan_lines(t, &steps(12, 4))),
            [
                "✓ 4 done",
                "▸ s5",
                "○ s6",
                "○ s7",
                "○ s8",
                "○ s9",
                "○ s10",
                "… 2 more"
            ]
        );
        // Nine with none done: nothing to fold, seven and the count.
        assert_eq!(
            text(focus::plan_lines(t, &steps(9, 0))),
            [
                "▸ s1",
                "○ s2",
                "○ s3",
                "○ s4",
                "○ s5",
                "○ s6",
                "○ s7",
                "… 2 more"
            ]
        );
        // All done: the fold alone.
        assert_eq!(text(focus::plan_lines(t, &steps(9, 9))), ["✓ 9 done"]);
        let tr = text(focus::plan_lines(Lang::Tr.text(), &steps(12, 4)));
        assert_eq!((tr[0].as_str(), tr[7].as_str()), ("✓ 4 bitti", "… 2 daha"));
        // The legend names the mark in both languages.
        assert!(
            t.help_legend[4].ends_with("  ≡ 3/7 its to-do steps done"),
            "{}",
            t.help_legend[4]
        );
        assert!(
            Lang::Tr.text().help_legend[4].ends_with("  ≡ 3/7 yapılacaklarından biten"),
            "{}",
            Lang::Tr.text().help_legend[4]
        );
    }

    #[test]
    fn the_to_do_mark_gives_way_after_the_quote_and_the_time() {
        let mut app = quiet_rows();
        let (worker, _) = with_helpers(&mut app);
        let a = &mut app.world.agents[worker];
        a.status = Status::Idle;
        a.changed_at = 64;
        a.local = Some("http://localhost:5173/".into());
        a.plan = steps(3, 1);
        told(a, Party::Person, true, "parser fixed");
        // The 113 of the row without the mark, and the mark's 7.
        let whole = row(&app, worker, 200);
        assert_eq!(whole.chars().count(), 120, "{whole}");
        assert!(
            whole.contains(
                "idle → you \"parser fixed\" · 3 min  ≡ 1/3  ▾ 2 helpers  ↗ localhost:5173/"
            ),
            "{whole}"
        );
        assert_eq!(row(&app, worker, 120), whole, "fits exactly");
        // One short: the quote shrinks first.
        let r = row(&app, worker, 119);
        assert!(r.contains("\"parser fix…\" · 3 min  ≡ 1/3  ▾"), "{r}");
        // Then the quote goes, then the time; the mark stays longest.
        let r = row(&app, worker, 100);
        assert!(r.contains("idle · 3 min  ≡ 1/3  ▾ 2 helpers"), "{r}");
        // The time needs its eight after the mark: at 99 exactly, at 98 not.
        let r = row(&app, worker, 99);
        assert!(r.contains("idle · 3 min  ≡ 1/3  ▾ 2 helpers"), "{r}");
        let r = row(&app, worker, 98);
        assert!(
            r.contains("idle  ≡ 1/3  ▾ 2 helpers") && !r.contains("min"),
            "{r}"
        );
        // The mark needs its seven: at 91 exactly, at 90 not.
        let r = row(&app, worker, 91);
        assert!(
            r.contains("idle  ≡ 1/3  ▾ 2 helpers") && !r.contains("min"),
            "{r}"
        );
        let r = row(&app, worker, 90);
        assert!(r.contains("idle  ▾ 2 helpers") && !r.contains('≡'), "{r}");
    }

    #[test]
    fn a_row_warns_of_a_full_context_from_seventy_percent_and_the_focus_has_the_figure() {
        let mut app = quiet_rows();
        let w = 1;
        let fill = |used, size| {
            Some(Fill {
                used,
                size: Some(size),
            })
        };
        app.world.agents[w].context = fill(139_999, 200_000);
        assert!(!row(&app, w, 200).contains('◔'), "69%: not on the row");
        app.world.agents[w].context = fill(140_000, 200_000);
        let r = row(&app, w, 200);
        assert!(r.contains("idle  ◔ 70%"), "{r}");
        let spans = salon::context_spans(&app.world.agents[w]);
        assert_eq!(spans[0].style.fg, dim().fg, "quiet under ninety");
        app.world.agents[w].context = fill(179_999, 200_000);
        assert_eq!(
            salon::context_spans(&app.world.agents[w])[0].style.fg,
            dim().fg
        );
        app.world.agents[w].context = fill(180_000, 200_000);
        let spans = salon::context_spans(&app.world.agents[w]);
        assert_eq!(spans[0].content.as_ref(), "  ◔ 90%");
        assert_eq!(spans[0].style.fg, Some(pal().warn), "loud from ninety");
        // No size: nothing on the row, the tokens on the focus.
        app.world.agents[w].context = Some(Fill {
            used: 45_506,
            size: None,
        });
        assert!(salon::context_spans(&app.world.agents[w]).is_empty());
        app.zoom = Zoom::Focus(w);
        let s = render(&app, 140, 40);
        assert!(s.contains("context:  45k tokens"), "{s}");
        app.world.agents[w].context = fill(45_506, 200_000);
        let s = render(&app, 140, 40);
        assert!(s.contains("context:  22% · 45k of 200k"), "{s}");
        // The focus line is loud from ninety too, by the exact token.
        let colour = |app: &App| {
            let buf = buffer(app, 140, 40);
            let y = (0..40)
                .find(|&y| {
                    (0..140)
                        .map(|x| buf[(x, y)].symbol())
                        .collect::<String>()
                        .contains("context:")
                })
                .unwrap();
            let x = (0..140).find(|&x| buf[(x, y)].symbol() == "c").unwrap();
            buf[(x, y)].style().fg
        };
        app.world.agents[w].context = fill(179_999, 200_000);
        assert_eq!(colour(&app), dim().fg, "89%: quiet");
        app.world.agents[w].context = fill(180_000, 200_000);
        assert_eq!(colour(&app), Some(pal().warn), "90%: loud");
        app.world.agents[w].context = fill(45_506, 200_000);
        app.lang = Lang::Tr;
        let s = render(&app, 140, 40);
        assert!(s.contains("bağlam:   %22 · 45k / 200k"), "{s}");
        app.world.agents[w].context = None;
        assert!(!render(&app, 140, 40).contains("bağlam:"));
        // Token counts in a few characters.
        for (n, s) in [
            (980, "980"),
            (1_000, "1k"),
            (45_506, "45k"),
            (999_999, "999k"),
            (1_000_000, "1M"),
            (1_250_000, "1.2M"),
            (2_000_000, "2M"),
        ] {
            assert_eq!(words::tokens(n), s, "{n}");
        }
        // The legend names the mark in both languages.
        assert!(
            Lang::En.text().help_legend[2]
                .ends_with("  ◔ 72% of its context window used (from 70%)")
        );
        assert!(
            Lang::Tr.text().help_legend[2].ends_with("  ◔ %72 bağlam penceresi dolu (%70'ten)")
        );
    }

    #[test]
    fn the_context_warning_stays_on_the_row_longer_than_the_to_do_mark() {
        let mut app = quiet_rows();
        let (worker, _) = with_helpers(&mut app);
        let a = &mut app.world.agents[worker];
        a.status = Status::Idle;
        a.changed_at = 64;
        a.local = Some("http://localhost:5173/".into());
        a.plan = steps(3, 1);
        a.context = Some(Fill {
            used: 150_000,
            size: Some(200_000),
        });
        told(a, Party::Person, true, "parser fixed");
        // The 120 of the row with the mark, and the warning's 7.
        let whole = row(&app, worker, 200);
        assert_eq!(whole.chars().count(), 127, "{whole}");
        assert!(
            whole.contains("\"parser fixed\" · 3 min  ≡ 1/3  ◔ 75%  ▾ 2 helpers"),
            "{whole}"
        );
        // Head 52, tail 32: the warning needs its seven, the mark seven
        // more, the time eight more.
        let r = row(&app, worker, 106);
        assert!(r.contains("idle · 3 min  ≡ 1/3  ◔ 75%  ▾"), "{r}");
        let r = row(&app, worker, 105);
        assert!(
            r.contains("idle  ≡ 1/3  ◔ 75%  ▾") && !r.contains("min"),
            "{r}"
        );
        let r = row(&app, worker, 98);
        assert!(r.contains("idle  ≡ 1/3  ◔ 75%  ▾"), "{r}");
        let r = row(&app, worker, 97);
        assert!(r.contains("idle  ◔ 75%  ▾") && !r.contains('≡'), "{r}");
        let r = row(&app, worker, 91);
        assert!(r.contains("idle  ◔ 75%  ▾"), "{r}");
        let r = row(&app, worker, 90);
        assert!(r.contains("idle  ▾ 2 helpers") && !r.contains('◔'), "{r}");
    }

    #[test]
    fn the_history_search_says_so_over_the_line_and_the_help_names_its_keys() {
        let mut app = quiet_rows();
        app.world.recall_line("@claude-1 fix the clock");
        key(&mut app, KeyCode::Char('/'));
        let s = render(&app, 160, 40);
        assert!(!s.contains(" › history: "), "{s}");
        app.on_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        let s = render(&app, 160, 40);
        assert!(s.contains(" › history: "), "{s}");
        assert!(s.contains("@claude-1 fix the clock"), "{s}");
        app.lang = Lang::Tr;
        assert!(render(&app, 160, 40).contains(" › geçmiş: "));
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Esc);
        app.help = true;
        let help = render(&app, 140, 40);
        assert!(
            help.contains("tab ↑ ↓ ^r   yazarken: tamamla · gönderilenler · :history"),
            "{help}"
        );
        app.lang = Lang::En;
        let help = render(&app, 140, 40);
        assert!(
            help.contains("tab ↑ ↓ ^r   writing: complete · sent lines · :history"),
            "{help}"
        );
        // Every help row fits its cell: 13 columns of keys, 44 of words.
        for lang in [Lang::En, Lang::Tr] {
            let t = lang.text();
            let (k, what) = t
                .help_keys
                .iter()
                .find(|(k, _)| *k == "tab ↑ ↓ ^r")
                .expect("the row");
            assert!(
                k.chars().count() <= 13 && what.chars().count() <= 44,
                "{lang:?} {k} {what}"
            );
        }
    }

    #[test]
    fn going_to_a_helper_unfolds_its_worker_and_project_and_shows_everyone_again() {
        let mut app = quiet_rows();
        let (worker, [h1, _]) = with_helpers(&mut app);
        let helper = app.world.agents[h1].name.clone();
        app.world.agents[h1].status = Status::Idle;
        app.world.agents[h1].activity = None;
        app.folded_workers.insert(worker);
        app.collapsed[0] = true;
        app.needs_only = true;
        app.zoom = Zoom::Focus(worker);
        key(&mut app, KeyCode::Char('&'));
        let s = render(&app, 160, 40);
        assert!(s.contains(" › go to: &"), "{s}");
        for c in helper.chars() {
            key(&mut app, KeyCode::Char(c));
        }
        let m = app.menu.as_ref().expect("the menu");
        assert!(m.items[0].ends_with(&format!(" {helper}")), "{:?}", m.items);
        assert_eq!(m.notes[0], "○ idle", "the helper with its glyph and status");
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.selected, Sel::Agent(h1));
        assert!(!app.collapsed[0], "the project unfolded");
        assert!(
            !app.folded_workers.contains(&worker),
            "its worker's helpers unfolded"
        );
        assert!(!app.needs_only, "everyone shown, so the row is there");
        assert_eq!(
            (app.view, app.zoom, app.pane),
            (View::Salon, Zoom::None, Pane::Salon)
        );
        let s = render(&app, 160, 40);
        assert!(s.contains(&format!("▎{helper}")), "{s}");
        // In Turkish the line says where it goes too; the help names the key.
        app.lang = Lang::Tr;
        key(&mut app, KeyCode::Char('&'));
        assert!(render(&app, 160, 40).contains(" › git: &"));
        for lang in [Lang::En, Lang::Tr] {
            assert!(
                lang.text().help_keys.iter().any(|(k, _)| *k == "&  :goto"),
                "{lang:?}"
            );
        }
    }

    #[test]
    fn a_row_says_how_long_it_has_been_in_its_state_from_a_minute_on() {
        let mut app = quiet_rows();
        let w = 1;
        // Eleven ticks are 55 seconds; twelve, a minute.
        app.world.agents[w].changed_at = 89;
        let r = row(&app, w, 200);
        assert!(!r.contains(" · "), "{r}");
        app.world.agents[w].changed_at = 88;
        let r = row(&app, w, 200);
        assert!(r.contains("idle · 1 min"), "{r}");
        app.world.agents[w].changed_at = 76;
        let r = row(&app, w, 200);
        assert!(r.contains("idle · 2 min"), "{r}");
        // Something waiting for the person: since it came, not since the
        // status changed.
        app.world.agents[w].changed_at = 100;
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Question,
            agent: w,
            detail: "q?".into(),
            researcher: None,
            open: true,
            tick: 64,
            card: None,
        });
        let r = row(&app, w, 200);
        assert!(r.contains("? → you \"q?\" · 3 min"), "{r}");
        app.lang = Lang::Tr;
        let r = row(&app, w, 200);
        assert!(r.contains("? → sen \"q?\" · 3 dk"), "{r}");
    }

    #[test]
    fn the_time_and_the_quote_give_way_to_the_row_s_fixed_parts() {
        let mut app = quiet_rows();
        let (worker, _) = with_helpers(&mut app);
        let a = &mut app.world.agents[worker];
        a.status = Status::Idle;
        a.changed_at = 64;
        a.local = Some("http://localhost:5173/".into());
        a.plan.clear();
        told(a, Party::Person, true, "parser fixed");
        let tail = "  ▾ 2 helpers  ↗ localhost:5173/";
        // Glyph, mark, name, taint, card and `idle` take 52; the quote 21,
        // the time 8, the tail 32: 113 in all.
        let whole = row(&app, worker, 200);
        assert_eq!(whole.chars().count(), 113, "{whole}");
        assert!(
            whole.contains("idle → you \"parser fixed\" · 3 min") && whole.ends_with(tail),
            "{whole}"
        );
        assert_eq!(row(&app, worker, 113), whole, "fits exactly");
        // One short: the quote shrinks first, to what is left.
        let r = row(&app, worker, 112);
        assert!(r.contains("idle → you \"parser fix…\" · 3 min"), "{r}");
        assert!(r.ends_with(tail) && r.chars().count() <= 112, "{r}");
        // The shortest quote needs 15 free after the time: 107 shows one,
        // 106 keeps the time alone.
        let r = row(&app, worker, 107);
        assert!(r.contains("\"parse…\" · 3 min"), "{r}");
        let r = row(&app, worker, 106);
        assert!(r.contains("idle · 3 min") && !r.contains('"'), "{r}");
        assert!(r.ends_with(tail) && r.chars().count() <= 106, "{r}");
        // Eight columns free take the time, seven do not: the fixed parts
        // alone.
        let r = row(&app, worker, 92);
        assert!(r.contains("idle · 3 min") && r.ends_with(tail), "{r}");
        let r = row(&app, worker, 91);
        assert!(!r.contains(" · ") && !r.contains('"'), "{r}");
        assert!(r.ends_with(tail) && r.chars().count() <= 91, "{r}");
    }

    #[test]
    fn wrap_cuts_at_spaces_and_keeps_the_lead_on_every_row() {
        let bullet = Line::from(vec![
            Span::raw("  "),
            Span::raw("• "),
            Span::raw("alpha beta gamma"),
        ]);
        assert_eq!(
            text(&wrap(bullet, 12)),
            ["  • alpha ", "    beta ", "    gamma"],
            "a bullet's width, not the bullet, leads the rows after it"
        );
        let code = fg(pal().code);
        let rule = Line::from(vec![
            Span::raw("  "),
            Span::styled("│ ", dim()),
            Span::styled("cargo test --workspace", code),
        ]);
        let rows = wrap(rule, 15);
        assert_eq!(text(&rows), ["  │ cargo test ", "  │ --workspace"]);
        assert_eq!(rows[1].spans[1].style, dim(), "the rule keeps its style");
        assert_eq!(rows[1].spans[2].style, code, "the code too");
        let short = Line::from(vec![Span::raw("  "), Span::raw("fits")]);
        assert_eq!(text(&wrap(short, 10)), ["  fits"]);
        let numbered = Line::from(vec![Span::raw("3. "), Span::raw("alpha beta")]);
        assert_eq!(
            text(&wrap(numbered, 8)),
            ["3. alpha ", "   beta"],
            "a number with a dot leads"
        );
        let lettered = Line::from(vec![Span::raw("x. "), Span::raw("alpha beta")]);
        assert_eq!(
            text(&wrap(lettered, 8)),
            ["x. alpha ", "beta"],
            "letters with a dot do not, and a word that just fills the row stays on it"
        );
        let dot = Line::from(vec![Span::raw(". "), Span::raw("alpha beta")]);
        assert_eq!(text(&wrap(dot, 8)), [". alpha ", "beta"], "nor a dot alone");
        let exact = Line::from(Span::raw("abcde fghij"));
        assert_eq!(
            text(&wrap(exact, 5)),
            ["abcde ", "fghij"],
            "a word that just fits stays whole; its space may hang over"
        );
    }

    #[test]
    fn wrap_cuts_a_word_wider_than_a_row_by_cell_width() {
        let long = Line::from(vec![Span::raw("  "), Span::raw("abcdefghij end")]);
        assert_eq!(text(&wrap(long, 6)), ["  abcd", "  efgh", "  ij ", "  end"]);
        let wide = Line::from(Span::raw("日本語 çok"));
        assert_eq!(
            text(&wrap(wide, 4)),
            ["日本", "語 ", "çok"],
            "a wide character takes two cells"
        );
        let narrow = Line::from(vec![Span::raw("    "), Span::raw("ab")]);
        assert_eq!(
            text(&wrap(narrow, 2)),
            ["    a", "    b"],
            "a width under the lead still leaves one cell"
        );
        assert_eq!(text(&wrap(Line::raw(""), 10)), [""]);
        assert_eq!(
            text(&wrap(Line::from(Span::raw("日本")), 1)),
            ["日", "本"],
            "a character wider than the row is a row of its own, no empty row first"
        );
        assert_eq!(pieces("日本", 1), ["日", "本"]);
        assert_eq!(pieces("abc", 1), ["a", "b", "c"]);
        assert_eq!(pieces("abc", 2), ["ab", "c"]);
        assert_eq!(pieces("", 3), [""]);
    }

    #[test]
    fn a_note_being_written_says_where_it_goes() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let worker = (app.world.agents.iter())
            .find(|a| a.role == Role::Worker)
            .unwrap()
            .id;
        app.world.change_ready(Change {
            agent: worker,
            title: "did it".into(),
            files: Vec::new(),
            patch: "diff --git a/x b/x\n@@ -1 +1 @@\n-a\n+b\n".into(),
            runs: Vec::new(),
        });
        app.zoom = Zoom::Diff(worker);
        app.diff_scroll = 3;
        let screen = render(&app, 120, 20);
        assert!(screen.contains(" c n  note · send notes"), "{screen}");
        key(&mut app, KeyCode::Char('c'));
        let screen = render(&app, 120, 20);
        assert!(screen.contains("note on x:1 ▸"), "{screen}");
        assert!(screen.contains("enter keep · "), "{screen}");
        assert!(!screen.contains("enter send"), "{screen}");
        app.lang = Lang::Tr;
        assert!(render(&app, 120, 20).contains("x:1 için not ▸"));
    }

    #[test]
    fn the_header_shows_each_vendor_s_tightest_window_loud_when_nearly_full() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let at = |vendor, minutes, used| Limit {
            vendor,
            minutes,
            used,
            resets_at: None,
        };
        app.world.limits = vec![
            at(Vendor::Claude, 300, 52),
            at(Vendor::Claude, 10_080, 10),
            at(Vendor::Codex, 10_080, 90),
        ];
        let screen = render(&app, 200, 20);
        let top = screen.lines().next().unwrap();
        assert!(
            top.contains(" │ claude 5h: 48% left │ codex 7d: 10% left"),
            "{top}"
        );
        assert!(!top.contains("claude 7d"), "{top}");
        let span = |used| limit_span(Lang::En, &at(Vendor::Codex, 300, used), 0);
        assert_eq!(span(89).style, Style::new());
        assert_eq!(span(90).style, attention());
        assert_eq!(span(90).content, "codex 5h: 10% left");
    }

    #[test]
    fn hint_spans_put_each_key_on_a_badge() {
        let spans = hint_spans(&[("c", "copy"), ("esc", "close")]);
        let text: Vec<&str> = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, [" c ", " copy  ", " esc ", " close  "]);
        assert_eq!(spans[0].style, key_style());
        assert_eq!(spans[1].style, Style::new());
        assert!(hint_spans(&[]).is_empty());
    }

    #[test]
    fn a_marked_line_shows_its_mark_once_in_the_gutter_and_in_the_journal() {
        let mut app = talking();
        app.open_thread(4);
        let entries = app.thread.as_ref().unwrap().entries.clone();
        // The long last message, selected, and the first one, not.
        app.world.marks.extend([entries[0], entries[3]]);
        let th = app.thread.as_ref().unwrap();
        let (rows, ranges) = overlay::thread_rows(app.lang, &app.world, th, 60);
        let gutter = |n: usize| -> String {
            let r: &Line = &rows[n];
            r.spans.iter().take(2).map(|s| s.content.as_ref()).collect()
        };
        for (at, first) in [(0, " ✱"), (3, "▌✱")] {
            let (start, end) = ranges[at];
            assert_eq!(gutter(start), first);
            assert_eq!(rows[start].spans[1].style, bold(pal().accent));
            for n in start + 1..end {
                assert!(!gutter(n).contains(MARK), "only its first row: {n}");
            }
        }
        let (start, _) = ranges[1];
        assert_eq!(gutter(start), "  ", "an unmarked line");
        assert!(rows.iter().all(|r| r.width() <= 60));
        // In the journal the mark takes the space after the clock.
        let e = &app.world.journal[entries[0]];
        let plain = entry_line(&app, e);
        let line = bookmarked(entry_line(&app, e));
        assert_eq!(line.width(), plain.width(), "aligned with the others");
        let clock = plain.spans[0].content.trim_end().to_string();
        assert_eq!(line.spans[0].content, clock);
        assert_eq!(line.spans[1].content, MARK);
        assert_eq!(line.spans[1].style, bold(pal().accent));
        assert_eq!(line.spans[2..], plain.spans[1..]);
    }

    /// A conversation between the lead (0) and agent 1 on the bus: an
    /// ask, a progress, the answer to the ask and an answer to a message
    /// the journal no longer has, numbered 5 on.
    fn replies() -> App {
        let mut app = app_after(1, 4, 0, Lang::En);
        app.world.journal.clear();
        app.world.messages = 4;
        let lines = [
            (0, 1, MsgKind::Ask, "Which parser?\nThe old one?", None),
            (1, 0, MsgKind::Progress, "Half way.", None),
            (1, 0, MsgKind::Answer, "The new one.", Some(5)),
            (1, 0, MsgKind::Answer, "Late.", Some(2)),
        ];
        for (from, to, kind, text, re) in lines {
            let id = app.world.number();
            app.world.note(Event::Sent {
                from,
                to,
                kind,
                text: text.into(),
                id,
                re,
            });
        }
        app.open_thread(0);
        assert_eq!(app.thread.as_ref().unwrap().entries, [0, 1, 2, 3]);
        app
    }

    #[test]
    fn a_conversation_s_copy_lines_are_heads_reply_lines_and_what_was_written() {
        let app = replies();
        let worker = app.world.agents[1].name.clone();
        let th = app.thread.as_ref().unwrap();
        let lines = thread_lines(Lang::En, &app.world, &th.entries);
        let entries: Vec<usize> = lines.iter().map(|l| l.entry).collect();
        assert_eq!(entries, [0, 0, 0, 1, 1, 2, 2, 2, 3, 3, 3]);
        let head = &lines[3].text;
        assert!(
            head.ends_with(&format!("{worker} → lead [progress]")),
            "{head}"
        );
        assert_eq!(head.trim(), head, "no blanks around");
        assert_eq!(lines[6].text, "↩ re 5: Which parser?");
        assert_eq!(lines[7].text, "The new one.");
        assert_eq!(
            overlay::copy_lines(&lines, 2, 4),
            format!("The old one?\n\n{head}\nHalf way.")
        );
        assert_eq!(
            overlay::copy_lines(&lines, 1, 2),
            "Which parser?\nThe old one?"
        );
        assert_eq!(overlay::copy_lines(&lines, 7, 7), "The new one.");
        let tr = thread_lines(Lang::Tr, &app.world, &th.entries);
        assert_eq!(tr[6].text, "↩ 5 yanıtı: Which parser?");
    }

    #[test]
    fn lines_chosen_to_copy_carry_the_mark_and_the_cursor_an_arrow() {
        let mut app = replies();
        let th = app.thread.as_mut().unwrap();
        th.sel = 0;
        th.copy = Some(crate::app::Copying { anchor: 1, at: 3 });
        let th = app.thread.as_ref().unwrap();
        let (rows, ranges) = overlay::thread_rows(Lang::En, &app.world, th, 80);
        let bar = |row: usize| rows[row].spans[0].content.to_string();
        let (first, second) = (ranges[0].0, ranges[1].0);
        assert_eq!(bar(first), " ", "the selected message's head is not chosen");
        assert_eq!(rows[first].style, Style::new());
        assert_eq!([bar(first + 1), bar(first + 2)], ["▌", "▌"]);
        assert_eq!(rows[first + 1].style, changed());
        assert_eq!(bar(second), "▶", "the cursor");
        assert_eq!(rows[second].style, changed());
        assert_eq!(bar(second + 1), " ");
    }

    #[test]
    fn choosing_lines_of_a_conversation_titles_it_and_keeps_the_cursor_in_view() {
        let mut app = app_after(1, 4, 0, Lang::En);
        let long: Vec<String> = (0..40).map(|i| format!("row{i}")).collect();
        app.world.said(1, &long.join("\n"));
        app.open_thread(app.world.journal.len() - 1);
        let s = render(&app, 124, 20);
        assert!(s.contains("choose") && !s.contains("row37"), "{s}");
        let th = app.thread.as_mut().unwrap();
        th.copy = Some(crate::app::Copying { anchor: 38, at: 38 });
        th.follow.set(true);
        let s = render(&app, 124, 20);
        assert!(s.contains("COPY · 1 chosen") && s.contains("row37"), "{s}");
        assert!(s.contains("leave") && s.contains("w b h l  word"), "{s}");
        assert!(s.contains("s  start here") && s.contains("c  note"), "{s}");
        assert!(!s.contains("send notes"), "{s}");
        app.lang = Lang::Tr;
        let s = render(&app, 124, 20);
        assert!(s.contains("KOPYALA · 1 seçili") && s.contains("çık"), "{s}");
    }

    #[test]
    fn choosing_lines_of_a_change_titles_it_with_the_keys() {
        let mut app = app_after(1, 4, 50, Lang::En);
        app.world.inbox.clear();
        app.world.change_ready(a_change(1));
        app.zoom = Zoom::Diff(1);
        let patch = crate::app::patch_key(&app.world.change_of(1).unwrap().patch);
        app.diff_copy = Some((1, patch, crate::app::Copying { anchor: 0, at: 2 }));
        let s = render(&app, 160, 40);
        assert!(
            s.contains("COPY · 3 chosen · y as written · Y the new side"),
            "{s}"
        );
        assert!(!s.contains("CHANGE ·"), "{s}");
        assert!(
            s.contains(" y  copy   v esc  leave   Y  copy the new side   ?  help"),
            "{s}"
        );
        assert!(
            !s.contains("note · send notes"),
            "the change's keys rest: {s}"
        );
        app.diff_copy = None;
        let s = render(&app, 160, 40);
        assert!(
            s.contains("CHANGE ·") && s.contains(" v  copy lines   esc  back"),
            "{s}"
        );
        app.lang = Lang::Tr;
        app.diff_copy = Some((1, patch, crate::app::Copying { anchor: 0, at: 2 }));
        let s = render(&app, 160, 40);
        assert!(
            s.contains("KOPYALA · 3 seçili · y yazıldığı gibi") && s.contains("yeni hali kopyala"),
            "{s}"
        );
    }

    #[test]
    fn markdown_keeps_each_drawn_line_s_source_for_copying() {
        let md = "# Head\n```rust\nlet x = 1;\n```\n| a | b |\n|---|---|\n| 1 | 2 |\n- item **b**";
        let src: Vec<String> = markdown_src(md, "  ").into_iter().map(|(_, s)| s).collect();
        assert_eq!(
            src,
            [
                "# Head",
                "let x = 1;",
                "| a | b |",
                "|---|---|",
                "| 1 | 2 |",
                "- item **b**"
            ]
        );
        assert_eq!(markdown_src(md, "  ").len(), markdown(md, "  ").len());
    }

    #[test]
    fn a_reply_shows_what_it_answers_and_percent_goes_there_and_back() {
        let mut app = replies();
        let worker = app.world.agents[1].name.clone();
        let th = app.thread.as_ref().unwrap();
        let (rows, ranges) = overlay::thread_rows(Lang::En, &app.world, th, 80);
        let lines = text(&rows);
        let head = |n: usize| lines[ranges[n].0].clone();
        assert!(
            head(2).ends_with(&format!("{worker} → lead [answer] ↩")),
            "{}",
            head(2)
        );
        assert!(
            head(0).ends_with(
                "lead → {worker} [ask]"
                    .replace("{worker}", &worker)
                    .as_str()
            )
        );
        assert_eq!(lines[ranges[2].0 + 1], "    ↩ re 5: Which parser?");
        assert_eq!(lines[ranges[2].0 + 2], "    The new one.");
        assert_eq!(
            lines[ranges[3].0 + 1],
            "    ↩ re 2, which the journal no longer has"
        );
        assert_eq!(
            lines[ranges[1].0 + 1],
            "    Half way.",
            "no reply, no ↩ row"
        );
        let th = app.thread.as_ref().unwrap();
        let tr = text(&overlay::thread_rows(Lang::Tr, &app.world, th, 80).0);
        assert_eq!(tr[ranges[2].0 + 1], "    ↩ 5 yanıtı: Which parser?");
        assert_eq!(tr[ranges[3].0 + 1], "    ↩ 2 yanıtı; günlükte artık yok");
        // % from the answer to the ask, and from the ask to its answer.
        let press = |app: &mut App, from: usize| {
            let th = app.thread.as_mut().unwrap();
            th.sel = from;
            th.follow.set(false);
            key(app, KeyCode::Char('%'));
            let th = app.thread.as_ref().unwrap();
            (th.sel, th.follow.get())
        };
        assert_eq!(press(&mut app, 2), (0, true));
        assert_eq!(press(&mut app, 0), (2, true));
        assert_eq!(press(&mut app, 1), (1, false), "neither: it stays");
        assert_eq!(press(&mut app, 3), (3, false), "what it answers is gone");
        assert!(app.thread.is_some(), "% does not close it");
    }

    #[test]
    fn the_conversation_marks_every_row_of_the_selected_message() {
        let mut app = talking();
        app.open_thread(4);
        let th = app.thread.as_ref().unwrap();
        let (rows, ranges) = overlay::thread_rows(app.lang, &app.world, th, 60);
        assert_eq!(ranges.len(), 4);
        let (start, end) = ranges[3];
        assert!(end - start > 10, "the long message wraps into many rows");
        assert_eq!(end, rows.len() - 1, "a blank row closes it");
        let marked = |n: usize| rows[n].spans.first().is_some_and(|s| s.content == "▌");
        assert!((start..end).all(marked), "every row of it carries the mark");
        assert!(
            (start..end).all(|n| rows[n].style == changed()),
            "and the selection background"
        );
        assert!((0..start).all(|n| !marked(n) && rows[n].style == Style::new()));
        assert!(
            rows.iter().all(|r| r.width() <= 60),
            "{:?}",
            rows.iter().map(Line::width).max()
        );
        // The head: when, who to whom; the text under it, indented.
        let lines = text(&rows);
        assert!(
            lines[start].starts_with("▌ 14:00 lead → you"),
            "{}",
            lines[start]
        );
        assert!(
            lines[start + 1].starts_with("▌   long long"),
            "{}",
            lines[start + 1]
        );
        // The person's message, not selected: a plain gutter.
        assert!(lines[0].starts_with("  14:00 you → lead"), "{}", lines[0]);
        assert_eq!(lines[1], "    check the readme");
        // Title and hints on the screen, in both languages.
        let screen = render(&app, 120, 30);
        assert!(
            screen.contains("CONVERSATION · lead ⇄ you · 4/4"),
            "{screen}"
        );
        assert!(
            screen.contains(" c C  copy   v  choose   n  send notes   esc  close"),
            "{screen}"
        );
        assert!(
            screen.contains("message   [ ]  turn   %  reply   h l zM zR  fold   c C"),
            "{screen}"
        );
        app.lang = Lang::Tr;
        let screen = render(&app, 120, 30);
        assert!(screen.contains("KONUŞMA · lead ⇄ sen · 4/4"), "{screen}");
        assert!(
            screen.contains(" c C  kopyala   v  seç   n  notları gönder   esc  kapat"),
            "{screen}"
        );
        assert!(
            screen.contains("mesaj   [ ]  sıra   %  yanıt   h l zM zR  katla   c C"),
            "{screen}"
        );
    }

    /// A row's characters, each with its style (the row's own under it).
    fn styled(row: &Line) -> Vec<(char, Style)> {
        (row.spans.iter())
            .flat_map(|s| {
                s.content
                    .chars()
                    .map(move |c| (c, row.style.patch(s.style)))
            })
            .collect()
    }

    /// The styles of the characters of `word` where it first stands in `row`.
    fn word_styles(row: &Line, word: &str) -> Vec<Style> {
        let chars = styled(row);
        let text: Vec<char> = chars.iter().map(|&(c, _)| c).collect();
        let want: Vec<char> = word.chars().collect();
        let at = (0..text.len())
            .find(|&i| text[i..].starts_with(&want))
            .unwrap_or_else(|| panic!("{word} in {text:?}"));
        chars[at..at + want.len()].iter().map(|&(_, s)| s).collect()
    }

    #[test]
    fn a_drawn_line_s_words_skip_what_only_leads_it() {
        let line = |spans: Vec<Span<'static>>| Line::from(spans);
        let words = |l: &Line| overlay::line_words(l);
        let plain = line(vec![
            Span::raw("  "),
            Span::raw("• "),
            Span::raw("alpha "),
            Span::styled("be", bold(pal().strong)),
            Span::raw("ta  gamma "),
        ]);
        assert_eq!(words(&plain), ["alpha", "beta", "gamma"]);
        let code = line(vec![Span::raw("  "), Span::raw("│ "), Span::raw("x = 1")]);
        assert_eq!(words(&code), ["x", "=", "1"]);
        assert!(words(&line(vec![Span::raw("  ")])).is_empty());
        let head = line(vec![Span::raw("14:00 "), Span::raw("lead")]);
        assert_eq!(words(&head), ["14:00", "lead"], "a clock is a word");
    }

    #[test]
    fn painting_words_chooses_them_and_the_blanks_between_them() {
        let base = Style::new().fg(Color::Red);
        let line = Line::from(vec![
            Span::raw("  "),
            Span::styled("alpha beta ", base),
            Span::raw("gamma delta"),
        ])
        .style(Style::new().fg(Color::Blue));
        let painted = overlay::paint_words(line.clone(), 1, 2, Some(2));
        assert_eq!(painted.style, line.style, "the line's own style stays");
        assert_eq!(painted.spans[0], Span::raw("  "), "the lead untouched");
        let text: String = painted.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "  alpha beta gamma delta");
        let chars: Vec<Style> = (painted.spans.iter())
            .flat_map(|s| s.content.chars().map(move |_| s.style))
            .collect();
        let chosen = base.patch(changed());
        let cursor = Style::new()
            .patch(changed())
            .add_modifier(Modifier::REVERSED);
        // "  alpha beta gamma delta"
        //  01234567890123456789012
        assert_eq!(&chars[2..7], [base; 5], "alpha: before the choice");
        assert_eq!(chars[7], base, "the blank before the first chosen word");
        assert_eq!(&chars[8..12], [chosen; 4], "beta");
        assert_eq!(chars[12], chosen, "the blank between two chosen words");
        assert_eq!(&chars[13..18], [cursor; 5], "gamma, the cursor's");
        assert_eq!(chars[18], Style::new(), "the blank after the last");
        assert_eq!(&chars[19..], [Style::new(); 5], "delta: after it");
        // The last word chosen, the cursor not on it: chosen all the same.
        let painted = overlay::paint_words(line.clone(), 1, 2, None);
        let chars: Vec<Style> = (painted.spans.iter())
            .flat_map(|s| s.content.chars().map(move |_| s.style))
            .collect();
        assert_eq!(&chars[13..18], [changed(); 5], "gamma");
        assert_eq!(chars[18], Style::new());
        // No cursor: nothing reversed; a choice past the end ends there.
        let painted = overlay::paint_words(line.clone(), 3, usize::MAX, None);
        let last = painted.spans.last().unwrap();
        assert_eq!(last.content, "delta");
        assert_eq!(last.style, changed());
        assert!((painted.spans.iter()).all(|s| !s.style.add_modifier.contains(Modifier::REVERSED)));
        // Spans of one style stay one span.
        let painted = overlay::paint_words(line, 5, 5, None);
        assert_eq!(painted.spans.len(), 3, "{:?}", painted.spans);
    }

    #[test]
    fn cutting_a_line_ends_it_with_an_ellipsis_where_it_was_cut() {
        let line = Line::from(vec![Span::raw("ab"), Span::styled("cdef", dim())]);
        let (same, cut) = overlay::cut(line.clone(), 6);
        assert_eq!((same, cut), (line.clone(), false), "it fits exactly");
        let (short, cut) = overlay::cut(line.clone(), 5);
        assert!(cut);
        assert_eq!(text(std::slice::from_ref(&short)), ["abcd…"]);
        assert_eq!(short.spans.len(), 3, "{:?}", short.spans);
        let (short, _) = overlay::cut(line.clone(), 3);
        assert_eq!(text(std::slice::from_ref(&short)), ["ab…"]);
        assert_eq!(short.spans.len(), 2, "no empty span after the cut");
        // A wide character that does not fit is left out whole.
        let (short, _) = overlay::cut(Line::raw("a界bc"), 3);
        assert_eq!(text(&[short]), ["a…"]);
        let (short, _) = overlay::cut(Line::raw("a界bc"), 4);
        assert_eq!(text(&[short]), ["a界…"]);
        // A cut after a blank drops it.
        let (short, _) = overlay::cut(Line::raw("ab cd"), 4);
        assert_eq!(text(&[short]), ["ab…"]);
    }

    #[test]
    fn the_word_cursor_steps_over_line_ends_and_stops_at_the_edges() {
        let counts = [2, 0, 3];
        let on = |at| overlay::word_step(&counts, at, true);
        let back = |at| overlay::word_step(&counts, at, false);
        assert_eq!(on((0, 0)), (0, 1));
        assert_eq!(on((0, 1)), (1, 0), "past the last word, the next line");
        assert_eq!(on((1, 0)), (2, 0), "an empty line is passed too");
        assert_eq!(on((2, 2)), (2, 2), "the last word of the last line");
        assert_eq!(on((0, 9)), (1, 0), "a word past the end is the last");
        assert_eq!(back((2, 2)), (2, 1));
        assert_eq!(back((2, 0)), (1, 0), "before the first, the line above");
        assert_eq!(back((1, 0)), (0, 1), "to its last word");
        assert_eq!(back((0, 0)), (0, 0), "the first word of the first line");
        assert_eq!(back((2, 9)), (2, 1));
        assert_eq!(overlay::word_step(&[], (0, 0), true), (0, 0));
    }

    #[test]
    fn brief_puts_words_on_one_line_and_cuts_them() {
        assert_eq!(brief("a  b\n c", 10), "a b c");
        assert_eq!(brief("abcde", 5), "abcde", "exactly as long");
        assert_eq!(brief("abcdef", 5), "abcd…");
    }

    /// The person and the lead: "assign it" (one line), "a\nb" (two
    /// short ones), "" (none), the lead's long line, the lead's readme.
    fn folding() -> App {
        let mut app = app_after(1, 4, 0, Lang::En);
        app.world.journal.clear();
        let you = |text: &str| Event::YouSaid {
            to: 0,
            text: text.into(),
        };
        app.world.note(you("assign it"));
        app.world.note(you("a\nb"));
        app.world.note(you(""));
        app.world.note(Event::Replied {
            agent: 0,
            text: format!("{} THE-END", "long ".repeat(40)),
        });
        app.world.note(Event::Replied {
            agent: 0,
            text: "# Readme\nIt is one line.\n- no usage".into(),
        });
        app.open_thread(0);
        app
    }

    #[test]
    fn a_folded_message_is_one_row_that_says_more_is_there() {
        let mut app = folding();
        let th = app.thread.as_mut().unwrap();
        th.folds = crate::app::Folds::every(true);
        th.sel = 3;
        let (rows, ranges) = overlay::thread_rows(Lang::En, &app.world, th, 60);
        assert_eq!(ranges, [(0, 1), (2, 3), (4, 5), (6, 7), (8, 9)]);
        let lines = text(&rows);
        assert_eq!(
            lines[0], "  14:00 you → lead: assign it",
            "nothing more to see"
        );
        assert_eq!(lines[2], "  14:00 you → lead: a b ▸ 2 lines", "two lines");
        assert_eq!(lines[4], "  14:00 you → lead: ");
        assert!(lines[6].ends_with(" long long… ▸ 1 line"), "{}", lines[6]);
        assert!(rows[6].width() >= 59, "cut to fit, not shorter");
        assert_eq!(
            lines[8],
            "  14:00 lead → you: Readme It is one line. no usa… ▸ 3 lines"
        );
        assert!(rows.iter().all(|r| r.width() <= 60));
        // The selected one carries the mark and the background.
        assert_eq!(rows[6].spans[0].content, "▌");
        assert_eq!(rows[6].style, changed());
        assert!([0, 2, 4, 8].iter().all(|&n| rows[n].style == Style::new()));
        // One message opened; the rest stay folded.
        th.folds.set(4, false);
        let (_, ranges) = overlay::thread_rows(Lang::En, &app.world, th, 60);
        assert_eq!(ranges[4], (8, 12));
        // While choosing, every message is open.
        th.copy = Some(crate::app::Copying::new(0));
        let (_, ranges) = overlay::thread_rows(Lang::En, &app.world, th, 60);
        assert_eq!(ranges[0], (0, 2));
        // In Turkish.
        th.copy = None;
        let tr = text(&overlay::thread_rows(Lang::Tr, &app.world, th, 60).0);
        assert!(tr[2].ends_with("▸ 2 satır") && tr[6].ends_with("▸ 1 satır"));
    }

    #[test]
    fn a_folded_line_says_more_only_when_it_was_cut() {
        let mut app = folding();
        let th = app.thread.as_mut().unwrap();
        th.folds = crate::app::Folds::every(true);
        th.sel = 1;
        // "  14:00 you → lead: assign it" is 29 wide.
        let row = |w: u16, n: usize| {
            text(&overlay::thread_rows(Lang::En, &app.world, th, w).0)[n].clone()
        };
        assert_eq!(row(29, 0), "  14:00 you → lead: assign it");
        assert_eq!(row(28, 0), "  14:00 you → lead… ▸ 1 line");
        // Nothing to open: cut, but no lines to count.
        assert_eq!(row(12, 4), "  14:00 you…");
        // Its notes' count takes room too: 33 wide with it.
        let notes = [crate::app::TalkNote {
            to: 0,
            entry: 0,
            quote: "assign".into(),
            text: "now".into(),
        }];
        let row = |w: u16| {
            let th = app.thread.as_ref().unwrap();
            text(&overlay::thread_layout(Lang::En, &app.world, &notes, th, w).0)[0].clone()
        };
        assert_eq!(row(33), "  14:00 you → lead: assign it ✎ 1");
        assert_eq!(row(32), "  14:00 you → lead… ▸ 1 line ✎ 1");
    }

    #[test]
    fn notes_show_under_their_message_and_count_on_a_folded_one() {
        let mut app = folding();
        let note = |quote: &str, text: &str| crate::app::TalkNote {
            to: 0,
            entry: 4,
            quote: quote.into(),
            text: text.into(),
        };
        app.talk_notes = vec![
            note("It is\none", "why one?"),
            note("no usage", "\u{1b}[2Jadd it"),
        ];
        let th = app.thread.as_mut().unwrap();
        let (rows, _, _) = overlay::thread_layout(Lang::En, &app.world, &app.talk_notes, th, 60);
        let lines = text(&rows);
        let at = lines.iter().position(|l| l.contains('✎')).unwrap();
        assert_eq!(lines[at], "      ✎ \"It is one\" why one?");
        assert!(lines[at + 1].starts_with("      ✎ \"no usage\" "));
        assert!(lines[at + 1].ends_with("[2Jadd it") && !lines[at + 1].contains('\u{1b}'));
        assert_eq!(lines[at + 2], "", "the message ends after them");
        assert!(lines[at - 1].contains("no usage"), "under the message");
        let (narrow, _, _) = overlay::thread_layout(Lang::En, &app.world, &app.talk_notes, th, 16);
        assert!(text(&narrow).iter().any(|l| l == "      ✎ \"It is…"));
        // Folded: counted at the end of its row.
        th.folds = crate::app::Folds::every(true);
        let (rows, _, _) = overlay::thread_layout(Lang::En, &app.world, &app.talk_notes, th, 90);
        let lines = text(&rows);
        assert!(lines[8].ends_with("no usage ▸ 3 lines ✎ 2"), "{}", lines[8]);
        assert!(!lines[0].contains('✎'));
        // A message folded with nothing more than its notes: counted alone.
        app.talk_notes = vec![crate::app::TalkNote {
            entry: 0,
            ..note("assign", "now")
        }];
        let th = app.thread.as_ref().unwrap();
        let (rows, _, _) = overlay::thread_layout(Lang::En, &app.world, &app.talk_notes, th, 90);
        assert_eq!(text(&rows)[0], "▌ 14:00 you → lead: assign it ✎ 1");
    }

    #[test]
    fn the_conversation_title_counts_its_notes_and_the_line_names_the_words() {
        let mut app = folding();
        let s = render(&app, 120, 30);
        assert!(!s.contains('✎'), "{s}");
        app.talk_notes.push(crate::app::TalkNote {
            to: 0,
            entry: 4,
            quote: "It is one line.".into(),
            text: "why?".into(),
        });
        app.talk_notes.push(crate::app::TalkNote {
            to: 0,
            entry: 99,
            quote: "elsewhere".into(),
            text: "not here".into(),
        });
        let s = render(&app, 120, 30);
        assert!(s.contains("· 1/5 ✎ 1 "), "{s}");
        app.typing = true;
        app.noting = Some(crate::app::Noting::Words(crate::app::TalkNote {
            to: 0,
            entry: 4,
            quote: "x".repeat(30),
            text: String::new(),
        }));
        let s = render(&app, 120, 30);
        let quote = format!("note on \"{}…\" ▸ ", "x".repeat(NOTE_QUOTE - 1));
        assert!(s.contains(&quote), "{s}");
        app.lang = Lang::Tr;
        let s = render(&app, 120, 30);
        assert!(s.contains("…\" için not ▸ "), "{s}");
    }

    #[test]
    fn words_chosen_carry_the_background_and_the_cursor_word_is_reversed() {
        let mut app = folding();
        let th = app.thread.as_mut().unwrap();
        let lines = thread_lines(Lang::En, &app.world, &th.entries);
        let at = lines
            .iter()
            .position(|l| l.text == "It is one line.")
            .unwrap();
        th.copy = Some(crate::app::Copying { anchor: at, at });
        th.words = Some((1, 2));
        let (rows, _, line_ranges) = overlay::thread_layout(Lang::En, &app.world, &[], th, 60);
        let row = &rows[line_ranges[at].0];
        assert_eq!(row.spans[0].content, "▶");
        assert_ne!(row.style, changed(), "part of the line: not the row");
        assert!(word_styles(row, "It").iter().all(|s| s.bg != changed().bg));
        assert!(word_styles(row, "is").iter().all(|s| s.bg == changed().bg));
        let one = word_styles(row, "one");
        assert!(
            one.iter()
                .all(|s| s.add_modifier.contains(Modifier::REVERSED))
        );
        assert!(
            word_styles(row, "line.")
                .iter()
                .all(|s| s.bg != changed().bg)
        );
        // The line chosen whole: the row's background.
        th.words = Some((0, 3));
        let (rows, _, line_ranges) = overlay::thread_layout(Lang::En, &app.world, &[], th, 60);
        assert_eq!(rows[line_ranges[at].0].style, changed());
        // A line between the ends is whole; the ones around it are not chosen.
        th.copy = Some(crate::app::Copying {
            anchor: at - 1,
            at: at + 1,
        });
        th.words = Some((1, 0));
        let (rows, _, line_ranges) = overlay::thread_layout(Lang::En, &app.world, &[], th, 60);
        assert_eq!(rows[line_ranges[at].0].style, changed());
        assert_eq!(rows[line_ranges[at].0].spans[0].content, "▌");
        assert_ne!(rows[line_ranges[at - 1].0].style, changed());
        assert_eq!(rows[line_ranges[at - 2].0].spans[0].content, " ");
    }

    #[test]
    fn the_conversation_scrolls_only_as_far_as_the_selection_needs() {
        let mut app = talking();
        app.open_thread(4);
        render(&app, 100, 20);
        let th = app.thread.as_ref().unwrap();
        let (rows, ranges) = overlay::thread_rows(app.lang, &app.world, th, 96);
        let (start, end) = ranges[3];
        assert_eq!(
            usize::from(th.scroll.get()),
            start,
            "the long last message shows from its start"
        );
        let last_row = rows.len() - 1;
        assert!(end > 17 && last_row < 2 * 17, "{end} {last_row}");
        key(&mut app, KeyCode::Up);
        render(&app, 100, 20);
        assert_eq!(
            usize::from(app.thread.as_ref().unwrap().scroll.get()),
            ranges[2].0,
            "the message above was out of view: scrolled up to it"
        );
        key(&mut app, KeyCode::Up);
        render(&app, 100, 20);
        assert_eq!(
            usize::from(app.thread.as_ref().unwrap().scroll.get()),
            ranges[1].0,
            "the readme (6 rows) was above too"
        );
        key(&mut app, KeyCode::Down);
        render(&app, 100, 20);
        assert_eq!(
            usize::from(app.thread.as_ref().unwrap().scroll.get()),
            ranges[1].0,
            "going back down: still in view, no scroll"
        );
        // Paging stops at the last row; a page is the body less one row of
        // overlap (20 rows: 2 bars, 2 border, 1 hints: 15 of text).
        let th = app.thread.as_ref().unwrap();
        assert_eq!(th.page.get(), 14);
        for _ in 0..9 {
            key(&mut app, KeyCode::PageDown);
        }
        render(&app, 100, 20);
        assert_eq!(
            usize::from(app.thread.as_ref().unwrap().scroll.get()),
            rows.len() - 15,
            "{} rows, 15 shown",
            rows.len()
        );
    }

    fn parts(spans: &[Span]) -> Vec<(String, Style)> {
        spans
            .iter()
            .map(|s| (s.content.to_string(), s.style))
            .collect()
    }

    #[test]
    fn inline_marks_code_bold_italic_and_links_and_leaves_lone_marks_alone() {
        let base = Style::new();
        let code = fg(pal().code);
        let bold_ = base.add_modifier(Modifier::BOLD);
        let italic = base.add_modifier(Modifier::ITALIC);
        let under = base.add_modifier(Modifier::UNDERLINED);
        assert_eq!(
            parts(&inline("a **b** `c **d**` *e* [f](http://x) g", base)),
            [
                ("a ".into(), base),
                ("b".into(), bold_),
                (" ".into(), base),
                ("c **d**".into(), code),
                (" ".into(), base),
                ("e".into(), italic),
                (" ".into(), base),
                ("f".into(), under),
                (" (http://x)".into(), dim()),
                (" g".into(), base),
            ]
        );
        for plain in [
            "**x",
            "x**",
            "****",
            "a * b * c",
            "snake_case_name",
            "`open",
            "[x](",
            "[](http://x)",
            "[x]()",
            "*",
            "* x",
            "*x**",
        ] {
            assert_eq!(
                parts(&inline(plain, base)),
                [(plain.into(), base)],
                "{plain}"
            );
        }
        assert_eq!(
            parts(&inline("çok **önemli**", bold_)),
            [("çok ".into(), bold_), ("önemli".into(), bold_)],
            "a base style carries into the marks"
        );
        assert!(inline("", base).is_empty());
        assert_eq!(
            parts(&inline("[the docs](http://x/y) tail", base)),
            [
                ("the docs".into(), under),
                (" (http://x/y)".into(), dim()),
                (" tail".into(), base),
            ],
            "a label longer than one character"
        );
    }

    #[test]
    fn a_table_becomes_aligned_columns_with_the_header_bold() {
        let md = "before
| Command | Result |
|---|---|
| `cargo test` | 171 passed |
| short |

after | not a table";
        let lines = markdown(md, "  ");
        let shown = text(&lines);
        assert_eq!(
            shown,
            [
                "  before",
                "  Command    │ Result",
                "  ───────────┼───────────",
                "  cargo test │ 171 passed",
                "  short      │ ",
                "  ",
                "  after | not a table",
            ]
        );
        assert_eq!(lines[1].spans[1].style, bold(pal().strong));
        assert_eq!(
            lines[3].spans[1].style,
            fg(pal().code),
            "cells take inline marks"
        );
        assert_eq!(lines[2].spans[1].style, dim());
        let rows: Vec<String> = ["a|b", "-|-", "c|d", "", "e|f"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(table_len(&rows, 0), 3, "a blank line ends the table");
        let rows: Vec<String> = ["a|b", "-|-", "c|d", "prose", "e|f"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(table_len(&rows, 0), 3, "a line without a column too");
        assert_eq!(table_len(&rows, 4), 0, "no separator, no table");
        assert_eq!(table_len(&rows, 2), 0);
        assert!(is_separator("| :---: | --- |"));
        assert!(!is_separator("---"), "a rule has no column");
        assert!(!is_separator("| | |"));
        assert_eq!(cells("| a | b |"), ["a", "b"]);
        assert_eq!(cells("a|b"), ["a", "b"]);
        assert_eq!(cells("|"), [""]);
    }

    #[test]
    fn lists_quotes_and_rules_draw_and_wrap_with_their_marks() {
        let md = "1. one
2. two
> said
>quoted
---
+ plus
  * nested
10. ten";
        assert_eq!(
            text(&markdown(md, "  ")),
            [
                "  1. one",
                "  2. two",
                "  ▎ said",
                "  ▎ quoted",
                &format!("  {}", "─".repeat(24)),
                "  • plus",
                "    • nested",
                "  10. ten",
            ]
        );
        let numbered = markdown("1. alpha beta gamma", "  ").remove(0);
        assert_eq!(
            text(&wrap(numbered, 12)),
            ["  1. alpha ", "     beta ", "     gamma"],
            "the number's width leads the rows after it"
        );
        let quoted = markdown("> alpha beta gamma", "  ").remove(0);
        assert_eq!(
            text(&wrap(quoted, 12)),
            ["  ▎ alpha ", "  ▎ beta ", "  ▎ gamma"],
            "the quote bar goes on"
        );
        assert_eq!(list_marker("- a"), Some(("• ".into(), "a")));
        assert_eq!(list_marker("3. a"), Some(("3. ".into(), "a")));
        assert_eq!(list_marker("3.a"), None);
        assert_eq!(list_marker("-a"), None);
        assert_eq!(list_marker(". a"), None);
    }

    #[test]
    fn a_closed_project_is_not_in_the_salon_rows() {
        use super::salon::{Row, rows};
        let mut app = app_after(2, 12, 300, Lang::En);
        app.world.tick = 300;
        for a in &mut app.world.agents {
            if a.role == Role::Worker {
                a.status = Status::Idle;
                a.talk = None;
                a.changed_at = 0;
            }
        }
        let of = |app: &App, capacity: usize| rows(app, capacity);
        assert!(of(&app, 100).contains(&Row::Project(1)));
        app.world.projects[1].closed = true;
        let open = of(&app, 100);
        assert!(!open.contains(&Row::Project(1)));
        assert!(open.contains(&Row::Project(0)));
        assert!(
            open.iter()
                .all(|r| !matches!(r, Row::Agent { id, .. } if app.world.agents[*id].project == 1)),
            "{open:?}"
        );
        // The closed project takes no lines: with room for the open one
        // alone nothing folds; one line less, something does.
        let n = open.len();
        let tight = of(&app, n);
        assert!(
            !tight.iter().any(|r| matches!(r, Row::Folded { .. })),
            "{tight:?}"
        );
        let tighter = of(&app, n - 1);
        assert!(
            tighter.iter().any(|r| matches!(r, Row::Folded { .. })),
            "{tighter:?}"
        );
    }

    #[test]
    fn the_help_lists_the_project_commands_in_both_languages() {
        let mut app = app_after(1, 2, 0, Lang::En);
        app.help = true;
        for (lang, keys) in [
            (
                Lang::En,
                [
                    ":open :solo",
                    ":close NAME",
                    ":rename NAME",
                    "a solo you drive",
                ],
            ),
            (
                Lang::Tr,
                [":open :solo", ":close AD", ":rename AD", "sürdüğün solo"],
            ),
        ] {
            app.lang = lang;
            let s = render(&app, 140, 40);
            for k in keys {
                assert!(s.contains(k), "{lang:?} {k}: {s}");
            }
        }
    }

    #[test]
    fn colon_open_close_and_rename_manage_projects_from_the_screen() {
        use super::salon::{Row, rows};
        use crate::app::Zoom;
        use crate::live::testing::{checkout, idle_live_in, team};
        let one = checkout("ui-one");
        let two = checkout("ui-two");
        let three = checkout("ui-three");
        let four = checkout("ui-four");
        let three_name = three.file_name().unwrap().to_str().unwrap().to_string();
        let mut app = App::with(
            team(1),
            crate::app::Engine::Live(Box::new(idle_live_in(&one, 2))),
            Lang::En,
        );
        app.home = Some(three.parent().unwrap().to_path_buf());
        let typed = |app: &mut App, line: &str| {
            for c in line.chars() {
                key(app, KeyCode::Char(c));
            }
            key(app, KeyCode::Enter);
        };
        let flashed = |app: &App| app.flash.as_ref().map(|f| f.0.clone()).unwrap_or_default();
        let project_rows = |app: &App| -> Vec<Row> {
            rows(app, 100)
                .into_iter()
                .filter(|r| matches!(r, Row::Project(_)))
                .collect()
        };

        typed(&mut app, ":open");
        assert!(
            flashed(&app).starts_with(":open PATH [NAME] opens"),
            "{}",
            flashed(&app)
        );
        typed(&mut app, &format!(":open {} api", two.display()));
        assert_eq!(app.world.projects.len(), 2);
        assert_eq!(app.collapsed, [false, false]);
        let lead_api = app.world.projects[1].leader;
        assert_eq!(app.selected, Sel::Agent(lead_api));
        assert_eq!(app.zoom, Zoom::None);
        let said = flashed(&app);
        assert!(
            said.starts_with("opened api at ") && said.ends_with("; write to lead-api"),
            "{said}"
        );
        let s = render(&app, 140, 40);
        assert!(s.contains("lead-api"), "{s}");
        assert!(s.contains("you opened api at"), "{s}");
        assert_eq!(project_rows(&app), [Row::Project(0), Row::Project(1)]);

        // The short form, `~`, the name from the directory.
        typed(&mut app, &format!(":o ~/{three_name}"));
        assert_eq!(app.world.projects.len(), 3);
        assert_eq!(app.world.projects[2].name, three_name);
        assert_eq!(
            app.world.agents[app.world.projects[2].leader].name,
            format!("lead-{three_name}")
        );
        assert_eq!(app.collapsed.len(), 3);

        // Refusals, each in its own words; nothing opened.
        typed(&mut app, ":open /nonexistent-dir");
        assert!(
            flashed(&app).starts_with("could not open: /nonexistent-dir"),
            "{}",
            flashed(&app)
        );
        typed(&mut app, &format!(":open {} other", two.display()));
        assert!(
            flashed(&app).ends_with(" is open already, as api"),
            "{}",
            flashed(&app)
        );
        typed(&mut app, &format!(":open {} api", four.display()));
        assert_eq!(flashed(&app), "a project named api is open already");
        typed(&mut app, &format!(":open {} a/b", four.display()));
        assert!(
            flashed(&app).starts_with("\"a/b\" can not name a project"),
            "{}",
            flashed(&app)
        );
        assert_eq!(app.world.projects.len(), 3);

        // Rename the one at hand.
        app.selected = Sel::Project(1);
        typed(&mut app, ":rename shop");
        assert_eq!(app.world.projects[1].name, "shop");
        assert_eq!(app.world.agents[lead_api].name, "lead-shop");
        assert_eq!(flashed(&app), "api is now shop; write to lead-shop");
        assert!(render(&app, 140, 40).contains("you renamed api to shop"));
        typed(&mut app, &format!(":rename {three_name}"));
        assert_eq!(
            flashed(&app),
            format!("a project named {three_name} is open already")
        );
        typed(&mut app, ":rename a b");
        assert!(
            flashed(&app).starts_with("\"a b\" can not name a project"),
            "{}",
            flashed(&app)
        );
        assert_eq!(app.world.projects[1].name, "shop", "refused: unchanged");

        // Close by name: gone from the salon; a zoom on it ends, a
        // selection elsewhere stays, the inbox selection fits what is left.
        let lead3 = app.world.projects[2].leader;
        app.zoom = Zoom::Focus(lead_api);
        app.selected = Sel::Agent(lead3);
        app.inbox_sel = 5;
        typed(&mut app, ":close shop");
        assert!(app.world.projects[1].closed);
        assert_eq!(app.zoom, Zoom::None);
        assert_eq!(app.selected, Sel::Agent(lead3), "not in the closed project");
        assert_eq!(app.inbox_sel, 0);
        assert_eq!(
            flashed(&app),
            "closed shop: its agents left; their worktrees stay on disk"
        );
        assert_eq!(project_rows(&app), [Row::Project(0), Row::Project(2)]);
        assert!(render(&app, 140, 40).contains("you closed shop"));
        typed(&mut app, ":close nope");
        assert_eq!(flashed(&app), "no open project named nope");
        // Opened again in place: same id, the fold undone, its new lead selected.
        app.collapsed[1] = true;
        typed(&mut app, &format!(":open {}", two.display()));
        assert_eq!(app.world.projects.len(), 3);
        assert_eq!(app.collapsed, [false, false, false]);
        assert!(!app.world.projects[1].closed);
        assert_ne!(app.world.projects[1].leader, lead_api);
        assert_eq!(app.selected, Sel::Agent(app.world.projects[1].leader));
        typed(&mut app, ":close");
        assert!(
            app.world.projects[1].closed,
            "the one at hand, by the selection: {} {:?}",
            flashed(&app),
            app.selected
        );
        // A selection (and a zoom) in the closed project moves to the first open one.
        app.selected = Sel::Project(0);
        app.zoom = Zoom::Trace(lead3);
        typed(&mut app, ":close repo");
        assert!(app.world.projects[0].closed);
        assert_eq!(app.selected, Sel::Agent(lead3));
        assert_eq!(app.zoom, Zoom::Trace(lead3), "a zoom elsewhere stays");
        // The one at hand (`:c` alone would take the menu's `close`), the last one.
        typed(&mut app, ":c ");
        assert_eq!(
            flashed(&app),
            format!("{three_name} is the last open project; q q quits")
        );
        assert!(!app.world.projects[2].closed);
        assert_eq!(app.current_project(), 2);

        // Turkish wording of the journal, by the projects' current names.
        app.lang = Lang::Tr;
        let said: Vec<String> = app
            .world
            .journal
            .iter()
            .map(|e| words::event(Lang::Tr, &app.world, &e.event))
            .collect();
        let two_name = two.file_name().unwrap().to_str().unwrap();
        assert!(
            said.iter()
                .any(|s| s.starts_with(&format!("{two_name} projesini açtın ("))),
            "{said:?}"
        );
        assert!(
            said.contains(&"api projesine shop adını verdin".to_string()),
            "{said:?}"
        );
        assert!(
            said.contains(&format!("{two_name} projesini kapattın")),
            "{said:?}"
        );
        assert!(
            said.contains(&"repo projesini kapattın".to_string()),
            "{said:?}"
        );
        for dir in [&one, &two, &three, &four] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn plain_makes_one_row_of_words_out_of_an_agents_markdown() {
        assert_eq!(
            plain("## Head\n\n- **bold** `code` *it* [the docs](http://u)\n2. two\n   \n"),
            "Head bold code it the docs two"
        );
        assert_eq!(
            plain("**x\n\u{1b}[2Jy"),
            "**x [2Jy",
            "a lone mark and a control code"
        );
        assert_eq!(plain("* bullet\n+ plus\n# \n"), "bullet plus");
        assert_eq!(plain(""), "");
        assert_eq!(plain("\n\n"), "");
    }

    #[test]
    fn the_journal_row_shows_a_message_as_plain_words() {
        let app = app_after(1, 2, 10, Lang::En);
        let e = Event::Replied {
            agent: 0,
            text: "**Done**\n\n- `a.rs`\n- b".into(),
        };
        let said = words::said(&e).unwrap();
        let text = said_spans(Lang::En, &app.world, &said)
            .iter()
            .map(|s| s.content.to_string())
            .collect::<String>();
        assert!(text.ends_with(": Done a.rs b"), "{text}");
    }

    #[test]
    fn shade_dims_every_cell_but_the_kept_ones() {
        let mut t = Terminal::new(TestBackend::new(3, 2)).unwrap();
        t.draw(|f| {
            f.render_widget(Paragraph::new("abc\ndef"), f.area());
            shade(
                f,
                Rect {
                    x: 1,
                    y: 0,
                    width: 1,
                    height: 1,
                },
            );
        })
        .unwrap();
        let buf = t.backend().buffer();
        let dimmed = |x: u16, y: u16| buf[(x, y)].modifier.contains(Modifier::DIM);
        assert!(!dimmed(1, 0), "the kept cell");
        for (x, y) in [(0, 0), (2, 0), (0, 1), (1, 1), (2, 1)] {
            assert!(dimmed(x, y), "({x}, {y})");
        }
        assert_eq!(buf[(1, 0)].symbol(), "b", "the text stays");
    }

    #[test]
    fn a_window_over_the_salon_dims_what_shows_around_it() {
        let mut app = app_after(1, 2, 200, Lang::En);
        let before = buffer(&app, 200, 50);
        assert!(
            !before[(2, 3)].modifier.contains(Modifier::DIM),
            "no window, nothing dim"
        );
        app.help = true;
        let buf = buffer(&app, 200, 50);
        // The salon's top-left corner lies outside every window.
        assert!(
            buf[(2, 3)].modifier.contains(Modifier::DIM),
            "around the help"
        );
        let mid = buf[(100, 25)].modifier;
        assert!(
            !mid.contains(Modifier::DIM),
            "inside the help window: {mid:?}"
        );
    }
}
