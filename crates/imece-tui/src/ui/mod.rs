//! Drawing. One rule above all: the screen is calm unless something needs
//! the person. Motion and colour are signals, never decoration, and a status
//! never relies on colour alone (every one has its own glyph or word).

mod board;
pub(crate) mod brand;
mod card;
mod code;
pub(crate) mod detail;
mod diff;
pub(crate) mod export;
mod focus;
mod home;
pub(crate) mod ledger;
pub(crate) mod overlay;
pub(crate) mod salon;
pub(crate) mod story;
pub(crate) mod timeline;
pub(crate) mod words;

use std::num::NonZeroU16;

use ratatui::Frame;
use ratatui::buffer::{Buffer, CellDiffOption};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::{App, Caret, FLASH, Field, Noting, Rung, Undoable, Window, Zoom};
use crate::i18n::{Lang, clean, fill};
use crate::model::*;
use crate::motion::{GlowOn, Lit};
use crate::screen::{Look, Shape};
pub(crate) use crate::theme::pal;
pub(crate) use diff::{
    QUOTE_LINES, Shown, clash_message, copy_patch, file_of, file_starts, hunk_at, notes_message,
    place, quote,
};
pub(crate) use overlay::{chosen_text, fold, in_view, talk_notes_message, thread_lines, word_step};
pub(crate) use salon::{card_rows, journal_rows, reply_jump, thread_of, turn_starts};

/// The narrowest screen imece draws on (record 131): a phone in
/// portrait; under it one line says so and no key acts but `ctrl-c`.
pub(crate) const MIN_COLUMNS: u16 = 40;
/// The shortest: the header, the status line and ten rows between.
pub(crate) const MIN_ROWS: u16 = 12;
/// Under this many columns the screen is narrow (record 131): Home's
/// entries stack, the help's text takes what the key leaves, a head
/// line breaks in two.
pub(crate) const NARROW: u16 = 80;

/// Whether a screen `(columns, rows)` is at the floor or above it.
pub(crate) fn fits_floor((columns, rows): (u16, u16)) -> bool {
    columns >= MIN_COLUMNS && rows >= MIN_ROWS
}

pub fn draw(f: &mut Frame, app: &App) {
    // The salon puts back where its glyphs breathe; anything else drawn
    // over it takes them away again (`patch` is then not used).
    *app.breathing.borrow_mut() = Some(Vec::new());
    app.caret_at.set(None);
    app.mark_before.set(app.mark_drawn.get());
    app.mark_drawn.set(false);
    let size = (f.area().width, f.area().height);
    app.drawn_size.set(Some(size));
    if !fits_floor(size) {
        *app.breathing.borrow_mut() = None;
        return too_small(f, app, size);
    }
    // An archive's view is a screen of its own, its banner in place of
    // its header.
    if let Some(v) = &app.viewing {
        *app.breathing.borrow_mut() = None;
        draw(f, v);
        return view_banner(f, app, v);
    }
    paint(f, f.area());
    // The completion menu takes a row above the footer while it is shown.
    let menu_rows = u16::from(app.typing && app.menu.is_some());
    let [head, body, menu, foot] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(menu_rows),
        Constraint::Length(input_rows(app, f.area().width, f.area().height)),
    ])
    .areas(f.area());
    header(f, head, app);
    // A piece of work on screen is framed (record 125): its head, a
    // question of it, the tabs; the tab's own screen draws under them.
    let body = match app.detail() {
        Some((id, tab)) => detail::frame(f, body, app, id, tab),
        None => body,
    };
    match app.zoom {
        // A card's screen is the log tab of its detail (record 125).
        Zoom::Focus(_) if app.card.is_some() => {
            if let Some(c) = app.card {
                card::draw(f, body, app, c);
            }
        }
        Zoom::Focus(id) => focus::draw(f, body, app, id, true),
        Zoom::Trace(id) => focus::trace(f, body, app, id),
        Zoom::Diff(id) => diff::draw(f, body, app, id),
        Zoom::None => home::draw(f, body, app),
    }
    if let Some(m) = &app.menu
        && app.typing
    {
        completions(f, menu, m, app.lang);
    }
    let line = footer(f, foot, app);
    // Windows drawn over the footer: the cursor is not shown under them.
    let mut over = Vec::new();
    if let Some(th) = &app.thread {
        over.push(overlay::thread(f, app, th, body));
    }
    if let Some(id) = app.peek {
        over.push(overlay::peek(f, app, id));
    }
    if let Some((item, at)) = app.script {
        over.push(overlay::script(f, app, item, at));
    }
    if let Some((item, at)) = app.plan_window {
        over.push(overlay::plan(f, app, item, at));
    }
    if let Some(win) = app.tidy {
        over.push(overlay::tidy(f, app, win));
    }
    if let Some(win) = app.ps {
        over.push(overlay::ps(f, app, win));
    }
    if let Some(win) = app.lonca_win {
        over.push(overlay::lonca(f, app, win));
    }
    if let Some(at) = app.ledger_win {
        over.push(ledger::window(f, app, at));
    }
    if let Some(win) = app.window {
        over.push(overlay::view(f, app, win));
    }
    if let Some(ask) = &app.teardown {
        over.push(overlay::teardown(f, app, ask));
    }
    if let Some(ask) = &app.importing {
        over.push(overlay::import(f, app, ask));
    }
    let line = line.filter(|&at| !over.iter().any(|r| r.contains(at)));
    let mut filter = None;
    let mut window = true;
    if let Some(p) = &app.solo_pick {
        overlay::solo_pick(f, app, p);
    } else if let Some(b) = &app.archives {
        filter = overlay::archives(f, app, b);
    } else if app.help {
        filter = overlay::help(f, app);
    } else if !app.news.is_empty() {
        overlay::news(f, app);
    } else if app.welcome {
        overlay::welcome(f, app);
    } else if let Some(d) = &app.digest {
        overlay::digest(f, app, d);
    } else if let Some(l) = &app.left_off {
        overlay::left_off(f, app, l);
    } else if app.leaving {
        overlay::leaving(f, app);
    } else {
        window = false;
    }
    if window || !over.is_empty() {
        *app.breathing.borrow_mut() = None;
    }
    // The terminal's own cursor, where the person types; hidden otherwise.
    let at = match app.caret() {
        Some((Field::Line, _)) => line,
        Some((Field::Help | Field::Archives, _)) => filter,
        None => None,
    };
    if let Some(at) = at {
        f.set_cursor_position(at);
    }
    app.caret_at.set(at);
}

/// A glyph that breathes, where the last drawing put it: `width` cells
/// from column `x` of row `y`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub x: u16,
    pub y: u16,
    pub width: u16,
}

/// Draws the frame as `last` (the last drawing, as the terminal shows
/// it) with only its breath changed: every glyph that breathes in this
/// frame's tone, the cursor where it was. False, with nothing drawn, when
/// `last` cannot be used (another size, the glyphs not all known).
pub fn patch(f: &mut Frame, app: &App, last: &Buffer) -> bool {
    let breathing = app.breathing.borrow();
    let Some(glyphs) = breathing.as_ref() else {
        return false;
    };
    let buf = f.buffer_mut();
    if buf.area != last.area {
        return false;
    }
    buf.content.clone_from_slice(&last.content);
    let dim = app.breath();
    for g in glyphs {
        for x in g.x..g.x + g.width {
            if let Some(cell) = buf.cell_mut((x, g.y)) {
                cell.modifier.set(Modifier::DIM, dim);
            }
        }
    }
    if let Some(at) = app.caret_at.get() {
        f.set_cursor_position(at);
    }
    true
}

/// Span `at` of `line`, drawn on row `i` of `list`, is a glyph that
/// breathes: where it is goes to [`App::breathing`]. One cut by the
/// list's right edge leaves the drawing unpatched; one below its foot is
/// not on the screen.
pub(super) fn note_glyph(app: &App, line: &Line, at: usize, list: Rect, i: usize) {
    let y = list.y + i as u16;
    if y >= list.bottom() {
        return;
    }
    let before: usize = line.spans[..at].iter().map(Span::width).sum();
    let x = list.x as usize + before;
    let width = line.spans[at].width();
    let mut breathing = app.breathing.borrow_mut();
    match breathing.as_mut() {
        Some(glyphs) if x + width <= usize::from(list.right()) => glyphs.push(Glyph {
            x: x as u16,
            y,
            width: width as u16,
        }),
        _ => *breathing = None,
    }
}

/// Whether agent `a`'s glyph breathes on rung `rung` while motion moves:
/// a working row's, not a leader's.
pub(crate) fn breathes(a: &Agent, rung: Rung) -> bool {
    rung == Rung::Working && a.role != Role::Leader
}

/// The top row over an archive's view `v`: which archive, when it was
/// put away, what waits for the person on the screen behind it, and how
/// to go back.
fn view_banner(f: &mut Frame, app: &App, v: &App) {
    let Some(viewed) = v.viewed() else {
        return;
    };
    let text = fill(
        v.lang.text().view_banner,
        &[
            &v.world.projects[viewed.project].name,
            &viewed.title,
            &date(day(viewed.at)),
            &app.world.open_inbox().len().to_string(),
        ],
    );
    let area = Rect {
        height: 1.min(f.area().height),
        ..f.area()
    };
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(Span::styled(text, brand())), area);
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
/// The command line's lead while it holds a `!` command.
fn shell() -> Style {
    badge(pal().shell)
}

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

/// The glyph of agent `a`'s status, one of the five (record 125): `●`
/// working (asking a teammate and researching are turns under way), `○`
/// idle (asleep too), `!` something of it waits for the person, `■` stuck
/// (blocked, or its session stopped). A leader's is its status's like
/// anyone's: its role is a word on its row, not a shape. The colour
/// tells blocked from stopped; the status words do without colour.
pub(crate) fn glyph(a: &Agent) -> (&'static str, Color) {
    match a.status {
        Status::Idle => ("○", pal().dim),
        Status::Working => ("●", pal().ok),
        Status::Asking { .. } => ("●", pal().warn),
        Status::Blocked => ("■", pal().bad),
        Status::WaitingHuman => ("!", pal().warn),
        Status::Researching { .. } => ("●", pal().research),
        Status::Stopped => ("■", pal().stopped),
        Status::Asleep => ("○", pal().dim),
    }
}

/// The glyph of agent `a`'s row on the state ladder: `■` stuck as its
/// status says; `!` when something of it waits for the person, whatever
/// it does meanwhile; `✓` done, not seen; `●` while its session starts
/// (a start is work under way); else its status's own glyph.
pub(crate) fn rung_glyph(a: &Agent, rung: Rung) -> (&'static str, Color) {
    match rung {
        Rung::Waiting => ("!", pal().warn),
        Rung::Done => ("✓", pal().ok),
        Rung::Starting => ("●", pal().ok),
        Rung::Stuck | Rung::Working | Rung::Idle => glyph(a),
    }
}

/// The style of agent `a`'s glyph in colour `color` on rung `rung`: a
/// working row's glyph (not a leader's) breathes, its second tone dimmed
/// in every other beat while motion moves (record 84); never its shape.
pub(crate) fn glyph_style(app: &App, a: &Agent, rung: Rung, color: Color) -> Style {
    let style = fg(color);
    if breathes(a, rung) && app.breath() {
        style.add_modifier(Modifier::DIM)
    } else {
        style
    }
}

/// How a row lit for a moment looks: reversed for a blink; settling, a
/// background from the theme's own `ok` toward its background in three
/// steps where both are RGB, else reversed, then the changed rows' look.
pub(crate) fn lit_style(lit: Lit) -> Style {
    let step = match lit {
        Lit::Reversed => return Style::new().add_modifier(Modifier::REVERSED),
        Lit::Settle(step) => step,
    };
    let p = pal();
    match (p.ok, p.bg) {
        (Color::Rgb(r, g, b), Color::Rgb(br, bg, bb)) => {
            let percent = SETTLE_SHADES[usize::from(step).min(SETTLE_SHADES.len() - 1)];
            let c = crate::theme::Rgb(br, bg, bb).blend(crate::theme::Rgb(r, g, b), percent);
            Style::new().bg(Color::Rgb(c.0, c.1, c.2))
        }
        _ if step == 0 => Style::new().add_modifier(Modifier::REVERSED),
        _ => changed(),
    }
}

/// How much of the theme's `ok` a settling row's background has, step by
/// step.
const SETTLE_SHADES: [u8; 3] = [35, 20, 8];

pub(crate) fn taint_span(a: &Agent) -> Span<'static> {
    match a.taint {
        Taint::Clean | Taint::Soft => Span::raw(" "),
        Taint::Full => Span::styled("≈", fg(pal().bad)),
    }
}

pub(crate) fn entry_icon(k: EntryKind) -> (&'static str, Style) {
    match k {
        EntryKind::Assign => ("→", dim()),
        EntryKind::Plan => ("→", fg(pal().plan)),
        EntryKind::Talk => ("⇄", fg(pal().info)),
        EntryKind::Assumption => ("⚑", fg(pal().warn)),
        EntryKind::Blocked => ("■", fg(pal().bad)),
        EntryKind::Research => ("◇", fg(pal().research)),
        EntryKind::Sensitive => ("!", bold(pal().warn)),
        EntryKind::Merged => ("✓", fg(pal().ok)),
        EntryKind::Failed => ("■", fg(pal().stopped)),
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
        InboxKind::Fault => ("■", bold(pal().bad)),
        InboxKind::Script | InboxKind::Check => ("▶", bold(pal().warn)),
        InboxKind::Plan => ("▤", bold(pal().warn)),
        InboxKind::Tries | InboxKind::Rounds => ("⊘", bold(pal().warn)),
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
        if let Some((mark, good)) = words::check_mark(app.lang, &c.check) {
            let style = match good {
                Some(true) => fg(pal().ok),
                Some(false) => fg(pal().bad),
                None => dim(),
            };
            spans.push(Span::styled(mark, style));
        }
        let colour = if c.runs.is_empty() {
            pal().warn
        } else if c.runs.iter().all(Ran::ok) {
            pal().ok
        } else {
            pal().bad
        };
        spans.push(Span::styled(words::evidence(app.lang, &c.runs), fg(colour)));
        if let Some(s) = c.spend {
            spans.push(Span::styled(
                format!(" · {}", words::spend(app.lang, &app.world, &s)),
                dim(),
            ));
        }
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
            Mark::Text(t) => spans.extend(linked(&t, base)),
            Mark::Code(t) => spans.push(Span::styled(t, code)),
            Mark::Bold(t) => spans.push(Span::styled(t, base.add_modifier(Modifier::BOLD))),
            Mark::Italic(t) => {
                spans.push(Span::styled(t, base.add_modifier(Modifier::ITALIC)));
            }
            Mark::Link { label, url } => {
                spans.push(Span::styled(label, base.add_modifier(Modifier::UNDERLINED)));
                spans.push(Span::styled(" (", dim()));
                spans.extend(linked(&url, dim()));
                spans.push(Span::styled(")", dim()));
            }
        }
    }
    spans
}

/// How a web address the terminal can open looks.
pub(crate) fn link_style() -> Style {
    fg(pal().info).add_modifier(Modifier::UNDERLINED)
}

/// `text` in `base`, each web address in it ([`web_url_at`], one that
/// starts a word) a span of its own in [`link_style`]: what
/// [`link_rows`] makes a link of. `text` is cleaned already.
pub(crate) fn linked(text: &str, base: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut from = 0;
    for (at, _) in text.match_indices("http") {
        let starts_word = !text[..at]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric());
        let url = web_url_at(&text[at..]).filter(|_| at >= from && starts_word);
        if let Some(url) = url {
            if at > from {
                spans.push(Span::styled(text[from..at].to_string(), base));
            }
            spans.push(Span::styled(url.to_string(), link_style()));
            from = at + url.len();
        }
    }
    if from < text.len() {
        spans.push(Span::styled(text[from..].to_string(), base));
    }
    spans
}

/// `lines` as rows `width` cells wide ([`wrap`]). A web address too wide
/// for a row is cut by the wrapping, so it is drawn in the link's colour
/// without the underline, and [`link_rows`] links none of its pieces.
pub(crate) fn rows_of(lines: Vec<Line<'static>>, width: u16) -> Vec<Line<'static>> {
    let width = usize::from(width);
    let mut rows = Vec::new();
    for mut line in lines {
        let leads = line.spans.iter().take_while(|s| is_lead(s)).count();
        let lead: usize = line.spans[..leads].iter().map(Span::width).sum();
        for s in &mut line.spans[leads..] {
            if s.style == link_style() && s.width() > width.saturating_sub(lead) {
                s.style = fg(pal().info);
            }
        }
        rows.extend(wrap(line, width));
    }
    rows
}

/// Makes each web address `rows` show whole (a span in [`link_style`]
/// that is one address, its cells inside `area`) a link the terminal
/// opens, `rows` drawn from `area`'s top: what is clicked is what is
/// read, cleaned as it was drawn.
pub(crate) fn link_rows(buf: &mut Buffer, area: Rect, rows: &[Line<'static>]) {
    for (y, row) in (0..area.height).zip(rows) {
        let mut x = 0usize;
        for s in &row.spans {
            let width = s.width();
            if s.style == link_style()
                && is_web_url(&s.content)
                && x + width <= usize::from(area.width)
                && let (Ok(x), Ok(w)) = (u16::try_from(x), u16::try_from(width))
            {
                let cells = Rect::new(area.x + x, area.y + y, w, 1);
                hyperlink(buf, cells, &s.content, LINK_ID);
            }
            x += width;
        }
    }
}

/// The link id of web addresses in agent text ([`hyperlink`]'s `id`):
/// cells of one address light up together, those of another do not, as
/// their addresses differ.
const LINK_ID: AgentId = usize::MAX;

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
/// padded to their column's width, each drawn line with what copying it
/// takes. A table wider than `room` (cells, the indent counted) narrows
/// its widest columns to the most that lets a row fit ([`column_widths`])
/// and wraps a cell in its column, so a row may take more lines; a web
/// address too long for its column is no link then. A table that fits
/// copies as its source (the rule as the separator), one that wraps as
/// its lines read.
fn table(lines: &[String], indent: &str, room: usize) -> Vec<(Line<'static>, String)> {
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
    let natural: Vec<usize> = (0..columns)
        .map(|c| {
            rows.iter()
                .filter_map(|r| r.get(c))
                .map(|cell| cell_width(cell))
                .max()
                .unwrap_or(0)
        })
        .collect();
    let gaps = 3 * columns.saturating_sub(1);
    let widths = column_widths(&natural, room.saturating_sub(indent.width() + gaps));
    let fits = widths == natural;
    let mut out = Vec::new();
    let mut source = lines.iter();
    for (n, row) in rows.into_iter().enumerate() {
        // Each cell's lines in its column.
        let cells: Vec<Vec<Line<'static>>> = widths
            .iter()
            .enumerate()
            .map(|(c, &width)| {
                let mut cell = row.get(c).cloned().unwrap_or_default();
                for s in &mut cell {
                    if s.style == link_style() && s.width() > width {
                        s.style = fg(pal().info);
                    }
                }
                cell.insert(0, Span::raw(""));
                wrap(Line::from(cell), width)
            })
            .collect();
        let height = cells.iter().map(Vec::len).max().unwrap_or(1);
        let from = source.next().cloned().unwrap_or_default();
        for k in 0..height {
            let mut spans = vec![Span::raw(indent.to_string())];
            for (c, width) in widths.iter().enumerate() {
                if c > 0 {
                    spans.push(Span::styled(" │ ", dim()));
                }
                // The cell's lines without the empty lead that kept its
                // own marks from being read as a list's.
                let part: Vec<Span> = (cells[c].get(k).into_iter())
                    .flat_map(|l| l.spans.iter().filter(|s| !s.content.is_empty()).cloned())
                    .collect();
                let pad = width.saturating_sub(cell_width(&part));
                spans.extend(part);
                if c + 1 < columns {
                    spans.push(Span::raw(" ".repeat(pad)));
                }
            }
            let line = Line::from(spans);
            let text = match fits {
                true => from.clone(),
                false => line_text(&line),
            };
            out.push((line, text));
        }
        if n == 0 {
            let rule = widths
                .iter()
                .map(|&w| "─".repeat(w))
                .collect::<Vec<_>>()
                .join("─┼─");
            let line = Line::from(vec![
                Span::raw(indent.to_string()),
                Span::styled(rule, dim()),
            ]);
            let text = match fits {
                true => source.next().cloned().unwrap_or_default(),
                false => {
                    source.next();
                    line_text(&line)
                }
            };
            out.push((line, text));
        }
    }
    out
}

/// What a drawn line reads as: its spans' text, the blanks at its ends
/// dropped.
pub(crate) fn line_text(line: &Line) -> String {
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    text.trim().to_string()
}

/// The widths of a table's columns, `natural` the widest cell of each,
/// in `free` cells: as they are when they fit; else no column wider than
/// the most that lets them fit, one cell at least each, so the narrow
/// columns keep their width and the wide ones give way.
fn column_widths(natural: &[usize], free: usize) -> Vec<usize> {
    if natural.iter().sum::<usize>() <= free {
        return natural.to_vec();
    }
    let widest = natural.iter().copied().max().unwrap_or(0);
    let under = |cap: usize| natural.iter().map(|&w| w.min(cap)).sum::<usize>();
    let cap = (1..widest)
        .rev()
        .find(|&cap| under(cap) <= free)
        .unwrap_or(1);
    natural.iter().map(|&w| w.min(cap).max(1)).collect()
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
    markdown_src(text, indent, usize::MAX)
        .into_iter()
        .map(|(l, _)| l)
        .collect()
}

/// [`markdown`], each drawn line with the source line it was drawn from
/// (cleaned), which is what copying it takes: a fence is never drawn, so
/// a code block copies as its code; a table's rule copies as the
/// separator line it stands for. `room` is how wide the lines may be
/// (cells, the indent counted), which only a table heeds: a wider one
/// wraps its cells in their columns ([`table`]); the rest is wrapped
/// where it is drawn.
pub(crate) fn markdown_src(text: &str, indent: &str, room: usize) -> Vec<(Line<'static>, String)> {
    markdown_in(text, indent, Style::new(), room)
}

/// [`markdown_src`] with its plain words and list items in `base` (an
/// agent's colour in the trace); headings, code and quotes as ever.
pub(crate) fn markdown_in(
    text: &str,
    indent: &str,
    base: Style,
    room: usize,
) -> Vec<(Line<'static>, String)> {
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
            out.extend(table(&lines[at..at + n], indent, room));
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
            spans.extend(inline(rest, base));
        }
        out.push((Line::from(spans), line.clone()));
    }
    out
}

/// A screen under the floor: its size and the floor's on the middle row,
/// nothing else.
fn too_small(f: &mut Frame, app: &App, (columns, rows): (u16, u16)) {
    paint(f, f.area());
    let words = fill(
        app.lang.text().too_small,
        &[
            &columns.to_string(),
            &rows.to_string(),
            &MIN_COLUMNS.to_string(),
            &MIN_ROWS.to_string(),
        ],
    );
    let area = f.area();
    let lines: Vec<Line> = overlay::wrap_words(&words, usize::from(area.width).max(1))
        .into_iter()
        .map(|row| Line::from(Span::styled(row, fg(pal().warn))))
        .collect();
    let tall = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .min(area.height);
    let rows = Rect {
        y: area.y + (area.height - tall) / 2,
        height: tall,
        ..area
    };
    f.render_widget(Paragraph::new(lines).centered(), rows);
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

/// How many cells `spans` take.
fn spans_width(spans: &[Span]) -> usize {
    spans.iter().map(Span::width).sum()
}

/// The footer's keys in `room` cells: they go from the end, but for the
/// way back and the help, so a narrow footer loses what is least wanted,
/// never `esc` or `?`.
fn fit_hints<'a>(mut hints: Vec<(&'a str, &'a str)>, room: usize) -> Vec<(&'a str, &'a str)> {
    let width = |h: &[(&str, &str)]| spans_width(&hint_spans(h));
    while width(&hints) > room {
        let Some(last) = hints.iter().rposition(|k| !matches!(k.0, "esc" | "?")) else {
            break;
        };
        hints.remove(last);
    }
    hints
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
    let mut spans = doing_status(lang, w, a);
    // Mail that waits with it: nothing else on the row says so.
    if a.held_mail > 0 {
        let held = fill(lang.text().held_mail_mark, &[&a.held_mail.to_string()]);
        spans.insert(0, Span::styled(held, fg(pal().warn)));
    }
    // The person's yes for its turn holds: said first, so it is never
    // cut off the row.
    if a.turn_yes.is_some() {
        spans.insert(0, Span::styled(lang.text().turn_yes_mark, bold(pal().warn)));
    }
    spans
}

/// [`status_spans`] without the mark of a yes for the turn.
fn doing_status(lang: Lang, w: &World, a: &Agent) -> Vec<Span<'static>> {
    let t = lang.text();
    match a.status {
        Status::Idle if let Some(it) = w.open_item(a.id, InboxKind::Reply) => {
            let mut spans = vec![Span::styled(t.answered, fg(pal().warn))];
            add_quote(&mut spans, &it.detail, fg(pal().warn));
            spans
        }
        Status::Idle if let Some(it) = w.open_item(a.id, InboxKind::Fault) => {
            let mut spans = vec![Span::styled(t.fault_row, bold(pal().bad))];
            add_quote(&mut spans, &it.detail, fg(pal().bad));
            spans
        }
        Status::Idle if let Some(r) = a.retry => {
            let left = r.at.saturating_sub(w.tick) * SECONDS_PER_TICK;
            vec![Span::styled(
                fill(
                    t.retry_row,
                    &[
                        &r.attempt.to_string(),
                        &RETRY_TICKS.len().to_string(),
                        &words::took_words(lang, left * 1000),
                    ],
                ),
                fg(pal().warn),
            )]
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
        Status::Idle if w.open_item(a.id, InboxKind::Change).is_some() => {
            vec![Span::styled(t.story_change, fg(pal().warn))]
        }
        Status::Idle => vec![Span::styled(t.idle, dim())],
        Status::Working if let Some(it) = w.open_item(a.id, InboxKind::Approval) => {
            let mut spans = vec![Span::styled(format!("⚑ {}", t.waiting_yes), fg(pal().warn))];
            add_quote(&mut spans, &words::one_line(&it.detail), fg(pal().warn));
            spans
        }
        Status::Working if a.starting => vec![Span::styled(t.starting, fg(pal().ok))],
        Status::Working if let Some(step) = on_step(w, a) => vec![step_span(step, STEP_WIDTH)],
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

/// Columns of a working row's step, `▸` not counted: what the agent does
/// for it keeps the rest of `STATUS_WIDTH`.
pub(crate) const STEP_WIDTH: usize = 24;

/// A step is never cut to fewer columns than this, `▸` not counted.
pub(crate) const STEP_MIN: usize = 8;

/// `▸ Run the tests`: the step a working agent is on, cleaned and cut to
/// `n` columns.
pub(crate) fn step_span(step: &str, n: usize) -> Span<'static> {
    Span::styled(format!("▸ {}", fit(step, n).trim_end()), fg(pal().ok))
}

/// What the agent does cut to fewer columns than this says nothing and is
/// left off the row.
pub(crate) const DOING_MIN: usize = 8;

/// The step a working row says the agent is on, `▸ Run the tests`: the
/// step of its to-do list in progress, unless a tool call of it waits for
/// the person or its session is still starting, which the row says
/// instead.
pub(crate) fn on_step<'a>(w: &World, a: &'a Agent) -> Option<&'a str> {
    let free = a.status == Status::Working
        && !a.starting
        && w.open_item(a.id, InboxKind::Approval).is_none();
    free.then(|| a.step()).flatten()
}

/// After a step, what the agent does for it now, dim: its tool call, or
/// `thinking…`; what it is after comes first, how it goes about it
/// second. It takes what `STATUS_WIDTH` leaves after the step's `used`
/// characters and never more than `room`; nothing in fewer than
/// `DOING_MIN` columns.
pub(crate) fn doing_spans(lang: Lang, a: &Agent, used: usize, room: usize) -> Vec<Span<'static>> {
    let doing = match a.activity.as_deref() {
        Some("") => lang.text().thinking,
        Some(d) => d,
        None => return Vec::new(),
    };
    let room = STATUS_WIDTH
        .saturating_sub(used + 2)
        .min(room.saturating_sub(2));
    if room < DOING_MIN {
        return Vec::new();
    }
    vec![Span::styled(
        format!("  {}", fit(doing, room).trim_end()),
        dim(),
    )]
}

/// The projects open, the one Home is narrowed to lit: one of the
/// header's four things (record 125).
pub(crate) fn projects_spans(app: &App) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for p in app.world.open_projects() {
        if !spans.is_empty() {
            spans.push(Span::styled(" · ", dim()));
        }
        let lit = app.at_home.scope == Some(p.id);
        let style = if lit {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
        };
        spans.push(Span::styled(clean(&p.name), style));
        // Its solo writes straight in the folder: no Δ (record 73).
        if p.no_git {
            let t = app.lang.text();
            spans.push(Span::styled(format!(" ⌀ {}", t.no_git), fg(pal().warn)));
        }
    }
    spans
}

/// Percent of a usage window used from which the header says
/// it the way it says what waits for the person.
const LIMIT_LOUD: u8 = 90;

/// A Claude Code's tightest usage window as the header shows it.
fn limit_span(lang: Lang, l: &Limit, now: u64) -> Span<'static> {
    let said = words::limit(lang, l, now);
    match l.used >= LIMIT_LOUD {
        true => Span::styled(said, attention()),
        false => Span::raw(said),
    }
}

/// A part of the header: what it says, whether it follows the one before
/// without a `│` (the pot's rate, the clock), and how long it stays when
/// the line is wider than the screen: the lowest `keep` goes first, the
/// last of equals first; `KEEP` parts never go.
pub(crate) struct HeadPart {
    pub spans: Vec<Span<'static>>,
    pub glued: bool,
    pub keep: u8,
}

/// A header part that never goes: what waits for the person, the pause, a
/// pot or a window running out.
pub(crate) const KEEP: u8 = u8::MAX;

fn header(f: &mut Frame, area: Rect, app: &App) {
    let line = fit_header(header_parts(app), usize::from(area.width));
    f.render_widget(Paragraph::new(line), area);
}

/// The header's parts, most wanted first (record 125): what waits for
/// the person, the fleet, the pot, a usage window nearly full, the clock
/// with the pause or the demo's speed glued to it; on a narrow screen the
/// clock goes, then the fleet, never the rest. The pot's rate, how
/// long it lasts while that is not pressing, and a window not yet loud
/// are the `:budget` note's (`App::command_note`).
pub(crate) fn header_parts(app: &App) -> Vec<HeadPart> {
    let w = &app.world;
    let t = app.lang.text();
    let part = |spans: Vec<Span<'static>>, keep: u8| HeadPart {
        spans,
        glued: false,
        keep,
    };
    let glued = |spans: Vec<Span<'static>>, keep: u8| HeadPart {
        spans,
        glued: true,
        keep,
    };
    let mut parts = Vec::new();
    let mut project_spans = Vec::new();
    if let Some(spin) = app.head_spin() {
        project_spans.push(Span::styled(format!("{spin} "), fg(pal().ok)));
    }
    project_spans.extend(projects_spans(app));
    if !project_spans.is_empty() {
        parts.push(part(project_spans, 8));
    }
    let open = w.open_inbox().len();
    if open > 0 {
        parts.push(part(
            vec![Span::styled(
                fill(t.waiting_for_you, &[&open.to_string()]),
                attention(),
            )],
            KEEP,
        ));
    }
    let spent = fill(t.budget, &[&w.spent_words(), &dollars(w.budget)]);
    match w.pot_lasts().filter(|_| w.pot_low()) {
        Some(minutes) => parts.push(part(
            vec![Span::styled(
                format!(
                    "{spent}, {}",
                    fill(t.pot_lasts, &[&words::ago(app.lang, minutes * 60)])
                ),
                attention(),
            )],
            KEEP,
        )),
        // One of the header's four things: kept, as what waits is.
        None => parts.push(part(vec![Span::raw(spent)], KEEP)),
    }
    // A window only when little is left: then it stops the team before
    // the pot does.
    if let Some(l) = w.tightest().filter(|l| l.used >= LIMIT_LOUD) {
        parts.push(part(vec![limit_span(app.lang, &l, w.now())], KEEP));
    }
    parts.push(glued(
        vec![Span::styled(format!("  {}", clock(w.tick)), dim())],
        3,
    ));
    if app.paused {
        parts.push(glued(
            vec![Span::styled(
                t.paused,
                Style::new().add_modifier(Modifier::REVERSED),
            )],
            KEEP,
        ));
    } else if app.speed > 1 {
        parts.push(glued(
            vec![Span::styled(format!(" ×{}", app.speed), dim())],
            3,
        ));
    }
    parts
}

/// The header in `width` columns: ` imece `, then the parts, a `│`
/// before each that is not glued to the one before; while the line is
/// wider, the part with the lowest `keep` goes, the last of equals first,
/// so what matters most is never what the screen's edge cuts off; when
/// what is left is still wider, the brand goes too (record 131: a phone).
pub(crate) fn fit_header(mut parts: Vec<HeadPart>, width: usize) -> Line<'static> {
    const BRAND: usize = 8;
    const SEP: usize = 3;
    let columns = |parts: &[HeadPart]| -> usize {
        let text: usize = parts.iter().flat_map(|p| &p.spans).map(Span::width).sum();
        let seps = parts.iter().skip(1).filter(|p| !p.glued).count();
        text + SEP * seps
    };
    while BRAND + columns(&parts) > width {
        let Some(i) = (0..parts.len())
            .filter(|&i| parts[i].keep < KEEP)
            .min_by_key(|&i| (parts[i].keep, std::cmp::Reverse(i)))
        else {
            break;
        };
        parts.remove(i);
    }
    let mut spans = if BRAND + columns(&parts) > width {
        vec![Span::raw(" ")]
    } else {
        vec![Span::styled(" imece ", brand()), Span::raw(" ")]
    };
    for (i, p) in parts.into_iter().enumerate() {
        if i > 0 && !p.glued {
            spans.push(Span::styled(" │ ", dim()));
        }
        spans.extend(p.spans);
    }
    Line::from(spans)
}

/// The completion menu's row: the items with their notes, the highlighted
/// one reversed, in a window that always holds it. When not all fit,
/// `‹ N` before says how many are out of view before the window and
/// `+N more ›` after how many after it, so the person knows `tab` has
/// more to show.
fn completions(f: &mut Frame, area: Rect, m: &crate::complete::Menu, lang: Lang) {
    let width = area.width as usize;
    // An item with its note (a command's value, `$`), a space either side.
    let cells: Vec<String> = (m.items.iter().enumerate())
        .map(|(i, item)| match m.notes.get(i).filter(|n| !n.is_empty()) {
            Some(note) => format!(" {} {} ", clean(item), clean(note)),
            None => format!(" {} ", clean(item)),
        })
        .collect();
    let cell = |i: usize| cells[i].chars().count();
    let n = cells.len();
    let before = |k: usize| format!(" ‹ {k} ");
    let after = |k: usize| fill(lang.text().menu_more, &[&k.to_string()]);
    let marks = |first: usize, last: usize| {
        let b = if first > 0 {
            before(first).chars().count()
        } else {
            0
        };
        let a = if last < n {
            after(n - last).chars().count()
        } else {
            0
        };
        a + b
    };
    let sum = |from: usize, to: usize| (from..to).map(cell).sum::<usize>();
    // All when they fit; else the window starts at the highlighted item,
    // is pulled back while earlier items still fit, then filled on, the
    // marks for what stays out counted in.
    // Ranges searched, never a loop a changed operator could make endless.
    let fits = |first: usize, last: usize| sum(first, last) + marks(first, last) <= width;
    let (mut first, mut last) = (0, n);
    if sum(0, n) > width {
        let end = m.chosen + 1;
        first = (0..m.chosen)
            .rev()
            .take_while(|&f| fits(f, end))
            .last()
            .unwrap_or(m.chosen);
        last = (end + 1..=n)
            .take_while(|&l| fits(first, l))
            .last()
            .unwrap_or(end);
    }
    let mut spans = Vec::new();
    if first > 0 {
        spans.push(Span::styled(before(first), bold(pal().info)));
    }
    for (i, text) in cells.iter().enumerate().take(last).skip(first) {
        let style = if i == m.chosen {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            dim()
        };
        spans.push(Span::styled(text.clone(), style));
    }
    if last < n {
        spans.push(Span::styled(after(n - last), bold(pal().info)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// The line being written as it is drawn, and the byte of it the cursor
/// stands before. A line `clean` would change (a pasted token) is drawn
/// cleaned whole with the cursor at its end, so moving into it does not
/// split the token past hiding.
fn caret_line(input: &str, at: usize) -> (String, usize) {
    let whole = clean(input);
    let before = if whole == input { at } else { whole.len() };
    (whole, before)
}

/// `text` cut into rows of at most `room` cells, cell by cell (a wide
/// character that does not fit goes on to the next row), with the row and
/// column the cursor stands at when it is before byte `caret`. A cursor
/// at the end of a full row stands at the start of the next, which is
/// there for it; an empty text is one empty row.
fn wrap_cells(text: &str, room: usize, caret: Option<usize>) -> (Vec<String>, (usize, usize)) {
    let room = room.max(1);
    let mut rows = vec![String::new()];
    let (mut used, mut at) = (0, (0, 0));
    for (i, c) in text.char_indices() {
        let w = c.width().unwrap_or(0);
        if used + w > room && used > 0 {
            rows.push(String::new());
            used = 0;
        }
        if caret == Some(i) {
            at = (rows.len() - 1, used);
        }
        rows.last_mut().expect("a row").push(c);
        used += w;
    }
    if caret.is_some_and(|c| c >= text.len()) {
        if used >= room {
            rows.push(String::new());
            used = 0;
        }
        at = (rows.len() - 1, used);
    }
    (rows, at)
}

/// Who the text being written goes to, before its first row; the rows
/// after it start under it.
fn writing_to(app: &App) -> Span<'static> {
    let t = app.lang.text();
    // A command for the person's own shell stands out from a message or
    // a `:` command: its lead on the theme's shell colour.
    if app.noting.is_none() && !app.searching && app.typing_shell() {
        return Span::styled(t.typing_shell, shell());
    }
    Span::styled(
        if let Some(Noting::Line(id, line)) = app.noting {
            let patch = app.world.change_of(id).map(|c| c.patch.as_str());
            let place = patch.and_then(|p| place(p, line)).unwrap_or_default();
            fill(t.typing_note, &[&place])
        } else if let Some(Noting::Words(note)) = &app.noting {
            fill(t.typing_words_note, &[&brief(&note.quote, NOTE_QUOTE)])
        } else if app.searching && app.recalling {
            t.typing_recall.to_string()
        } else if app.searching {
            t.typing_history.to_string()
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
    )
}

/// The text being written in rows `width` cells wide after its lead
/// ([`writing_to`]): each line cut where it reaches the edge
/// ([`wrap_cells`]), cleaned. With the row and column (in the text) of
/// the cursor.
fn written_rows(app: &App, width: usize) -> (Vec<String>, (usize, usize)) {
    let room = width.saturating_sub(writing_to(app).width());
    let at = app.cursor();
    let mut start = 0;
    let mut rows = Vec::new();
    let mut caret = (0, 0);
    for line in app.input.split('\n') {
        let from = start;
        start += line.len() + 1;
        let (text, here) = if (from..start).contains(&at) {
            let (text, before) = caret_line(line, at - from);
            (text, Some(before))
        } else {
            (clean(line), None)
        };
        let (cut, (row, column)) = wrap_cells(&text, room, here);
        if here.is_some() {
            caret = (rows.len() + row, column);
        }
        rows.extend(cut);
    }
    (rows, caret)
}

/// The most rows the line being written takes.
const INPUT_ROWS: u16 = 8;

/// How many rows the footer takes: one, or while writing one per row of
/// the text in a footer `width` wide ([`written_rows`]), at most
/// [`INPUT_ROWS`] and a third of the screen.
fn input_rows(app: &App, width: u16, height: u16) -> u16 {
    if !app.typing {
        return 1;
    }
    let rows = written_rows(app, usize::from(width)).0.len();
    let most = (height / 3).clamp(1, INPUT_ROWS);
    u16::try_from(rows).unwrap_or(u16::MAX).min(most)
}

/// The text being written, `rows` rows of a footer `width` wide under who
/// it goes to: a line too long for a row goes on in the next, the
/// cursor's row always shown (the rows above it go out of view first),
/// where the cursor is (`3/12`) when not all rows fit, and the keys to
/// send or cancel only while nothing is written yet. With the rows, the
/// cursor's row among them and its column.
fn writing(app: &App, rows: usize, width: usize) -> (Vec<Line<'static>>, (usize, usize)) {
    let t = app.lang.text();
    let to = writing_to(app);
    let indent = Span::raw(" ".repeat(to.width()));
    let (all, (row, column)) = written_rows(app, width);
    let first = (row + 1).saturating_sub(rows);
    let mut out: Vec<Line<'static>> = (all.iter().enumerate().skip(first).take(rows.max(1)))
        .map(|(n, text)| {
            let lead = if n == first {
                to.clone()
            } else {
                indent.clone()
            };
            Line::from(vec![lead, Span::raw(text.clone())])
        })
        .collect();
    let caret = (row - first, to.width() + column);
    let mut hint = Vec::new();
    if all.len() > out.len() {
        hint.push(Span::styled(format!("   {}/{}", row + 1, all.len()), dim()));
    }
    let newline = if app.shift_enter {
        t.newline_shift
    } else {
        t.newline_plain
    };
    // The menu's keys while it is open; the keys to send only until
    // something is written.
    let keys = match (&app.menu, &app.noting) {
        (Some(m), _) if m.items.len() > 1 => Some(format!(
            "{}{}",
            t.pick_cancel,
            fill(
                t.menu_at,
                &[&(m.chosen + 1).to_string(), &m.items.len().to_string()]
            )
        )),
        (Some(_), _) => Some(t.pick_cancel.to_string()),
        _ if !app.input.is_empty() => None,
        (None, Some(_)) => Some(fill(t.keep_cancel, &[newline])),
        (None, None) => Some(fill(t.send_cancel, &[newline])),
    };
    hint.extend(keys.map(|k| Span::styled(k, dim())));
    if let Some(what) = command_hint(app) {
        hint.push(Span::styled(format!("   {what}"), fg(pal().info)));
    }
    if let Some(last) = out.last_mut() {
        last.spans.extend(hint);
    }
    (out, caret)
}

/// What the `:` command `tab` just put on the line does, in a line: shown
/// after the menu's keys until the person writes on (the menu is made
/// again then, with nothing put in).
fn command_hint(app: &App) -> Option<&'static str> {
    let m = app.menu.as_ref().filter(|m| m.inserted && m.start == 1)?;
    if !app.input.starts_with(':') {
        return None;
    }
    let name = m.items.get(m.chosen)?;
    let hints = app.lang.text().command_hints;
    hints.iter().find(|(c, _)| c == name).map(|(_, what)| *what)
}

/// The footer: what is being written, else the keys or a word for a
/// moment. While writing, where the cursor goes.
fn footer(f: &mut Frame, area: Rect, app: &App) -> Option<Position> {
    let t = app.lang.text();
    if app.typing {
        let (lines, (row, column)) =
            writing(app, usize::from(area.height), usize::from(area.width));
        f.render_widget(Paragraph::new(lines), area);
        return caret_in(area, row, column);
    }
    let line = if let Some((msg, at)) = &app.flash
        && at.elapsed() < FLASH
    {
        Line::from(Span::styled(format!(" {msg}"), fg(pal().warn)))
    } else if let Some((what, left)) = app.grace_left(std::time::Instant::now()) {
        let left = left.to_string();
        let said = match what {
            Undoable::Close(p) => fill(t.grace_close, &[&app.world.projects[p].name, &left]),
            Undoable::Forget(id) => fill(t.grace_forget, &[&app.world.agents[id].name, &left]),
            Undoable::Archive(p) => {
                let name = &app.world.projects[p].name;
                let cut = (app.world.mid_turn().into_iter())
                    .filter(|&a| app.world.agents[a].project == p)
                    .count();
                match cut {
                    0 => fill(t.grace_archive, &[name, &app.archive_title, &left]),
                    n => fill(
                        t.grace_archive_cut,
                        &[name, &app.archive_title, &left, &n.to_string()],
                    ),
                }
            }
            Undoable::Clear(what) => fill(t.grace_clear[what.index()], &[&left]),
        };
        Line::from(Span::styled(format!(" {said}"), fg(pal().warn)))
    } else if let Some(asking) = app.asking() {
        // Waiting for an answer: the spinner turns while motion moves.
        Line::from(vec![
            Span::styled(format!(" {} ", app.spin().unwrap_or("…")), fg(pal().info)),
            Span::styled(asking.to_string(), dim()),
        ])
    } else if let Some(chord) = which_key(app) {
        chord
    } else {
        // The status line (record 125): where the person is, then at most
        // three keys for the selected thing, the way back and the help.
        let tail = |keys: &mut Vec<(&'static str, &'static str)>, back: bool| {
            if back {
                keys.push(("esc", t.back));
            }
            keys.push(("?", t.help));
        };
        let hints: Vec<(&str, &str)> = match app.zoom {
            _ if app.script.is_some() => t.setup_hints.to_vec(),
            _ if app.plan_window.is_some() => t.plan_hints.to_vec(),
            // A window over the screen: its own keys, no place.
            _ if app.window == Some(Window::Timeline) => {
                let mut keys = vec![
                    ("[ ]", t.timeline_scale),
                    ("enter", t.focus),
                    ("@", t.to_agent),
                ];
                tail(&mut keys, true);
                keys
            }
            _ if app.window.is_some() => vec![("esc", t.back), ("?", t.help)],
            // The conversation has its keys on its own last row.
            _ if app.thread.is_some() => vec![("esc", t.back), ("?", t.help)],
            _ if app.peek.is_some() => {
                let mut keys = vec![("enter", t.focus), ("@", t.write_this)];
                tail(&mut keys, true);
                keys
            }
            // A card's screen: the log tab of its detail (record 125).
            Zoom::None | Zoom::Focus(_) if app.card.is_some() => {
                let mut keys = vec![("enter", t.trace), ("n p", t.next_card)];
                tail(&mut keys, true);
                keys
            }
            Zoom::None => home_hints(app),
            Zoom::Focus(_) => {
                let mut keys =
                    ask_hints(app).unwrap_or_else(|| vec![("enter", t.trace), ("@", t.write_this)]);
                tail(&mut keys, true);
                keys
            }
            Zoom::Trace(_) => {
                let mut keys = ask_hints(app).unwrap_or_else(|| vec![("@", t.write_this)]);
                tail(&mut keys, true);
                keys
            }
            // Choosing lines to copy: only copy mode's keys work.
            Zoom::Diff(id) if app.diff_copy_of(id).is_some() => {
                let mut keys = t.copy_hints.to_vec();
                keys.push(("Y", t.new_side));
                keys.push(("?", t.help));
                keys
            }
            Zoom::Diff(id) => {
                let mut keys = vec![("a", t.apply), ("r", t.reject), ("c", t.note_key)];
                if app.world.clashes_of(id).is_some() {
                    keys.pop();
                    keys.insert(0, ("b", t.send_back));
                }
                tail(&mut keys, true);
                keys
            }
        };
        let mut spans = vec![Span::raw(" ")];
        let (place, hints) = status_parts(place_spans(app), hints, usize::from(area.width));
        if let Some(place) = place {
            spans.push(place);
            spans.push(Span::raw("   "));
        }
        spans.extend(hint_spans(&hints));
        Line::from(spans)
    };
    f.render_widget(Paragraph::new(line), area);
    None
}

/// The keys of Home: what the row at hand comes to first, a row's own
/// keys never out of sight.
pub(crate) fn home_hints(app: &App) -> Vec<(&'static str, &'static str)> {
    let t = app.lang.text();
    let w = &app.world;
    let kind = match app.home_at() {
        Some(crate::app::HomeRow::Item(i)) => Some(w.inbox[i].kind),
        _ => None,
    };
    let mut keys = match kind {
        Some(InboxKind::Change) => vec![("enter", t.home_read), ("a", t.apply), ("r", t.reject)],
        Some(InboxKind::Question | InboxKind::Fault) => vec![
            ("a", t.home_answer),
            ("enter", t.home_open),
            ("s", t.home_later),
        ],
        Some(InboxKind::Approval) => vec![("a", t.allow_once), ("r", t.refuse), ("e", t.this_turn)],
        Some(InboxKind::Reply) => vec![
            ("enter", t.home_open),
            ("r", t.home_dismiss),
            ("s", t.home_later),
        ],
        Some(InboxKind::Tries | InboxKind::Rounds) => vec![
            ("enter", t.home_open),
            ("a", t.home_once_more),
            ("r", t.home_fail),
        ],
        Some(_) => vec![("enter", t.home_open), ("a", t.home_yes), ("r", t.home_no)],
        None => match app.home_at() {
            Some(crate::app::HomeRow::Project(_)) => {
                vec![
                    ("enter", t.home_open),
                    ("h l", t.home_fold),
                    ("s", t.home_hide),
                ]
            }
            _ => vec![
                ("enter", t.home_open),
                ("s", t.home_hide),
                ("@", t.write_this),
            ],
        },
    };
    keys.push(("?", t.help));
    keys
}

/// The keys that decide what the agent of the open focus or raw stream
/// waits for, first in the footer: `a A r` on its ask, `a r` on its
/// change; none when it waits for neither or no focus is open.
fn ask_hints(app: &App) -> Option<Vec<(&'static str, &'static str)>> {
    let t = app.lang.text();
    let item = app.focus_ask(app.zoomed_agent()?)?;
    Some(match app.world.inbox[item].kind {
        InboxKind::Approval => vec![("a", t.allow_once), ("r", t.refuse), ("e", t.this_turn)],
        _ => vec![("a", t.apply), ("r", t.reject)],
    })
}

/// While the first key of a chord waits for its second (`g…`, `z…`,
/// `q…`, or a key the person's file pairs), the footer lists what the
/// second key can be and what each does, as which-key does in vim:
/// imece's own pairs, then the file's, each as what it acts as. Nothing
/// while no chord waits.
fn which_key(app: &App) -> Option<Line<'static>> {
    let t = app.lang.text();
    let first = match (&app.keys_held, app.pending) {
        (Some((k, _)), _) => crate::keys::Key::of(k),
        (None, Some(c)) => crate::keys::Key::parse(&c.to_string())?,
        (None, None) => return None,
    };
    let own: &[(&str, &str)] = match app.pending {
        Some('g') => &t.chord_g,
        Some('z') => &t.chord_z,
        Some('q') => &t.chord_q,
        _ => &[],
    };
    let mut keys: Vec<(String, String)> = own
        .iter()
        .map(|(k, w)| (k.to_string(), w.to_string()))
        .collect();
    // `gd` `gw` act on Home's list alone: said there alone.
    if app.pending == Some('g') && app.home_keys() {
        keys.extend((t.chord_home.iter()).map(|(k, w)| (k.to_string(), w.to_string())));
    }
    for (from, to) in app.keys.given() {
        let from = from.keys();
        if from.len() == 2 && from[0] == first {
            keys.push((from[1].name(), fill(t.chord_as, &[&to.name()])));
        }
    }
    let mut spans = vec![Span::styled(
        format!(" {}…  ", first.name()),
        bold(pal().warn),
    )];
    for (k, what) in keys {
        spans.push(Span::styled(format!(" {k} "), key_style()));
        spans.push(Span::raw(format!(" {what}  ")));
    }
    Some(Line::from(spans))
}

/// Where the person is, dim, before the footer's keys: the view or the
/// screen over it, the project at hand, and the card (else the agent)
/// the open screen or the selected row is about. Nothing in a window
/// that has a title of its own, or under hint letters.
fn place_spans(app: &App) -> Vec<Span<'static>> {
    let t = app.lang.text();
    let w = &app.world;
    let copying = matches!(app.zoom, Zoom::Diff(id) if app.diff_copy_of(id).is_some());
    if app.script.is_some() || app.plan_window.is_some() || app.window.is_some() || copying {
        return Vec::new();
    }
    let place = if app.thread.is_some() {
        t.places[3]
    } else if app.peek.is_some() {
        t.places[5]
    } else {
        match app.zoom {
            Zoom::None if app.card.is_some() => t.places[4],
            Zoom::None => t.home_place,
            Zoom::Focus(_) => t.places[0],
            Zoom::Trace(_) => t.places[1],
            Zoom::Diff(_) => t.places[2],
        }
    };
    let mut words = vec![place.to_string()];
    let p = app.project_at_hand();
    if let Some(project) = w.projects.get(p) {
        words.push(project.name.clone());
    }
    // On Home's list the row at hand; anywhere else (a screen over Home
    // too) the agent at hand.
    // An item's row is about the card it came on, not the one its agent
    // has since taken.
    let (about, card) = if app.zoom == Zoom::None {
        match app.home_at() {
            Some(crate::app::HomeRow::Item(i)) => {
                let item = w.inbox.get(i);
                (item.map(|item| item.agent), item.and_then(|item| item.card))
            }
            Some(crate::app::HomeRow::Agent(id)) => (Some(id), None),
            // A project's heading is about no card: the project is named.
            Some(crate::app::HomeRow::Project(_)) | None => (None, None),
        }
    } else {
        (app.agent_at_hand(), None)
    };
    if let Some(id) = about.filter(|&id| id < w.agents.len()) {
        match card
            .or_else(|| w.card_of(id))
            .and_then(|c| w.cards.iter().find(|card| card.id == c))
        {
            Some(card) => words.push(format!(
                "{} {}",
                words::card_ref(card.id),
                cut_title(&card.title, PLACE_TITLE)
            )),
            None => words.push(w.agents[id].name.clone()),
        }
    }
    vec![Span::styled(clean(&words.join(" · ")), dim())]
}

/// The place's narrowest on the status line: under it the place goes.
const PLACE_MIN: usize = 12;

/// The status line's parts on a line `width` wide (record 131): every
/// key first, the place in what they leave, cut with `…` while it keeps
/// `PLACE_MIN` cells, else gone; only then keys go, from the end, never
/// `esc` or `?`. A blank leads the line and three stand between the two.
fn status_parts<'a>(
    place: Vec<Span<'static>>,
    hints: Vec<(&'a str, &'a str)>,
    width: usize,
) -> (Option<Span<'static>>, Vec<(&'a str, &'a str)>) {
    let room = width.saturating_sub(1);
    let left = room.saturating_sub(spans_width(&hint_spans(&hints)) + 3);
    let place = place.into_iter().next().filter(|p| p.width() > 0);
    match place {
        Some(p) if left >= PLACE_MIN => {
            let words = cut_title(&p.content, left);
            (Some(Span::styled(words, p.style)), hints)
        }
        _ => (None, fit_hints(hints, room)),
    }
}

/// How much of a card's title the status line carries.
const PLACE_TITLE: usize = 30;

/// `title` whole when it fits `room` cells, else its start and `…`.
fn cut_title(title: &str, room: usize) -> String {
    if title.width() <= room {
        return title.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in title.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w > room.saturating_sub(1) {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.trim_end().to_string() + "…"
}

/// The cell `column` columns into row `row` of `area`, where the cursor
/// goes: on the area's last column when the text runs past its edge (the
/// line is cut there), none on a row the area does not have.
pub(crate) fn caret_in(area: Rect, row: usize, column: usize) -> Option<Position> {
    let row = u16::try_from(row).ok().filter(|&r| r < area.height)?;
    let room = usize::from(area.width.checked_sub(1)?);
    let column = u16::try_from(column.min(room)).ok()?;
    Some(Position::new(area.x + column, area.y + row))
}

/// How the terminal's cursor looks while the person types (`App::caret`):
/// a blinking bar for a message or a `!` command, a steady bar in the
/// theme's accent for a `:` command, a blinking underline for words that
/// find something. `None` while it is hidden.
pub fn look(app: &App) -> Option<Look> {
    let (_, caret) = app.caret()?;
    Some(match caret {
        Caret::Write => Look {
            shape: Shape::BlinkingBar,
            colour: None,
        },
        Caret::Command => Look {
            shape: Shape::SteadyBar,
            colour: rgb(pal().accent),
        },
        Caret::Find => Look {
            shape: Shape::BlinkingUnderline,
            colour: None,
        },
    })
}

/// A colour the terminal can be told as `#rrggbb`: only one given as
/// such. A palette colour is the terminal's own to choose, so the cursor
/// keeps the terminal's colour then.
fn rgb(c: Color) -> Option<[u8; 3]> {
    match c {
        Color::Rgb(r, g, b) => Some([r, g, b]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::app::{App, HomeRow, Pane, Sel};
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
    fn the_ps_window_sums_each_session_and_asks_before_one_ends() {
        use crate::procs::{Proc, Procs};
        let mut app = app_after(1, 2, 0, Lang::En);
        app.command("ps");
        let p = |pid, ppid, cpu, kib, name: &str| Proc {
            pid,
            ppid,
            cpu,
            rss: kib,
            name: name.into(),
        };
        app.world.procs = Procs::default();
        let s = render(&app, 120, 30);
        assert!(s.contains("PROCESSES · 0"), "{s}");
        assert!(s.contains("no agent has a session running"), "{s}");
        app.world.procs.looking = true;
        assert!(render(&app, 120, 30).contains("looking…"));
        app.world.procs = Procs {
            agents: vec![(
                1,
                vec![
                    p(500, 1, 123, 300 * 1024, "claude"),
                    p(501, 500, 0, 9 * 1024, "imece"),
                    p(502, 500, 950, 400 * 1024, "cargo\u{1b}[2J"),
                ],
            )],
            looking: true,
            failed: None,
            left: Vec::new(),
        };
        app.ps = Some(crate::app::PsWin { at: 2, sure: None });
        let s = render(&app, 120, 30);
        assert!(s.contains("PROCESSES · 3"), "{s}");
        assert!(!s.contains("looking…"), "what was read shows meanwhile");
        let row = |s: &str, name: &str| {
            s.lines()
                .find(|l| l.contains(name))
                .unwrap_or_default()
                .to_string()
        };
        assert!(s.contains("│claude-1 · 107.3% · 709 MiB "), "{s}");
        let root = row(&s, "  claude ");
        assert!(
            root.contains("  claude           500   12.3%  300 MiB"),
            "{root:?}"
        );
        assert!(!root.contains('▶'));
        let relay = row(&s, "  imece   ");
        assert!(
            relay.contains("    imece          501    0.0%  9 MiB"),
            "{relay:?}"
        );
        let cmd = row(&s, "▶");
        assert!(
            cmd.contains("▶   cargo [2J      502   95.0%  400 MiB"),
            "{cmd:?}"
        );
        let column = |l: &str, pid: &str| l[..l.find(pid).unwrap()].chars().count();
        assert_eq!(
            column(&root, "500"),
            column(&cmd, "502"),
            "the columns line up"
        );
        assert!(s.contains("end this one"), "{s}");
        app.ps = Some(crate::app::PsWin {
            at: 2,
            sure: Some(crate::app::Sure::One(crate::app::Whose::Agent(1), 502)),
        });
        let s = render(&app, 120, 30);
        assert!(
            s.contains("end cargo [2J (502) under claude-1? y ends it, any other key keeps it"),
            "{s}"
        );
        assert!(!s.contains("end this one"), "{s}");
        app.world.procs.failed = Some("ps did not answer".into());
        let s = render(&app, 120, 30);
        assert!(
            s.contains("the processes could not be read: ps did not answer"),
            "{s}"
        );
        // On a short screen the row at hand stays in view.
        app.ps = Some(crate::app::PsWin { at: 2, sure: None });
        let s = render(&app, 120, MIN_ROWS);
        assert!(s.contains("▶"), "{s}");
        // What an earlier run left, under its own heading, `a` named.
        app.world.procs.failed = None;
        assert!(!render(&app, 120, 30).contains("end all left over"));
        app.world.procs.left = vec![
            crate::procs::Left {
                place: "imece-b6-notes/claude-3".into(),
                procs: vec![
                    p(700, 1, 40, 200 * 1024, "claude"),
                    p(701, 700, 900, 100 * 1024, "node"),
                ],
            },
            crate::procs::Left {
                place: String::new(),
                procs: vec![p(800, 1, 0, 1024, "sh")],
            },
        ];
        app.ps = Some(crate::app::PsWin { at: 4, sure: None });
        let s = render(&app, 120, 30);
        assert!(s.contains("PROCESSES · 6"), "{s}");
        let at = |s: &str, text: &str| s.lines().position(|l| l.contains(text));
        let head = at(&s, "LEFT FROM AN EARLIER RUN · 3").expect("its heading");
        assert!(at(&s, "  cargo").unwrap() < head, "{s}");
        assert!(
            s.contains("│imece-b6-notes/claude-3 · 94.0% · 300 MiB"),
            "{s}"
        );
        assert!(s.contains("│? · 0.0% · 1 MiB"), "{s}");
        let node = row(&s, "node");
        assert!(node.contains("▶   node"), "{node:?}");
        assert!(at(&s, "▶").unwrap() > head);
        assert!(s.contains("end all left over"), "{s}");
        app.ps = Some(crate::app::PsWin {
            at: 4,
            sure: Some(crate::app::Sure::One(crate::app::Whose::Left(0), 701)),
        });
        let s = render(&app, 120, 30);
        assert!(
            s.contains("end node (701), left from an earlier run? y ends it"),
            "{s}"
        );
        app.ps = Some(crate::app::PsWin {
            at: 4,
            sure: Some(crate::app::Sure::AllLeft),
        });
        let s = render(&app, 120, 30);
        assert!(
            s.contains("end all 3 processes left from an earlier run? y ends them"),
            "{s}"
        );
        // No agent's session, and what was left: both said.
        app.world.procs.agents.clear();
        app.world.procs.looking = false;
        let s = render(&app, 120, 30);
        assert!(s.contains("no agent has a session running"), "{s}");
        assert!(s.contains("PROCESSES · 3"), "{s}");
        assert_eq!(overlay::cpu(0), "0.0%");
        assert_eq!(overlay::cpu(1009), "100.9%");
    }

    #[test]
    fn the_tidy_window_lists_the_worktrees_no_agent_owns_and_asks_before_work_goes() {
        let mut app = app_after(1, 1, 0, Lang::En);
        app.open_tidy();
        let s = render(&app, 120, 30);
        assert!(s.contains("WORKTREES NO AGENT OWNS · 0"), "{s}");
        assert!(s.contains("looking…"), "{s}");
        app.step();
        let s = render(&app, 120, 30);
        assert!(s.contains("WORKTREES NO AGENT OWNS · 2"), "{s}");
        assert!(!s.contains("looking…"), "{s}");
        let row = |s: &str, name: &str| {
            s.lines()
                .find(|l| l.contains(name))
                .map(str::to_string)
                .unwrap_or_default()
        };
        let first = row(&s, "claude-31");
        assert!(first.contains("▶ claude-31"), "{first}");
        assert!(first.contains("~/src/"), "{first}");
        assert!(first.contains("clean"), "{first}");
        let second = row(&s, "claude-32");
        assert!(second.contains("  claude-32 "), "{second}");
        assert!(!second.contains('▶'), "{second}");
        assert!(
            second.contains("files changed: 3 · commits of its own: 1"),
            "{second}"
        );
        assert!(s.contains("remove the clean ones"), "{s}");
        app.tidy = Some(crate::app::TidyWin { at: 1, sure: true });
        let s = render(&app, 140, 30);
        assert!(row(&s, "▶").contains("claude-32"), "{s}");
        assert!(
            s.contains("remove claude-32 with what it holds (files changed: 3 · commits of its own: 1)? y removes it"),
            "{s}"
        );
        assert!(!s.contains("remove the clean ones"), "{s}");
        // Going: said; all gone: said so.
        app.tidy = Some(crate::app::TidyWin::default());
        app.world.orphans.busy = true;
        assert!(render(&app, 120, 30).contains("removing…"));
        app.world.orphans = crate::model::Orphans {
            found: Some(Vec::new()),
            ..Default::default()
        };
        let s = render(&app, 120, 30);
        assert!(
            s.contains("none: every worktree under imece's state has its agent"),
            "{s}"
        );
        assert!(!s.contains("removing…"), "{s}");
    }

    #[test]
    fn the_lonca_window_lists_what_waits_for_a_human_and_what_is_held() {
        use crate::app::LoncaWin;
        use crate::model::{Role, StoreLease, StorePending, StoreProposal};
        let mut app = app_after(2, 2, 0, Lang::En);
        let (name, repo) = (
            app.world.projects[1].name.clone(),
            app.world.projects[1].name.clone(),
        );
        let id = (app.world.agents.iter())
            .find(|a| a.project == 1 && a.role == Role::Worker)
            .map(|a| a.id)
            .unwrap();
        let worker = app.world.agents[id].name.clone();
        app.world.agents[id].workspace = Some(repo.clone());
        let mut change = a_change(id);
        change.files = vec![ChangedFile {
            path: "x.kt".into(),
            added: 1,
            removed: 0,
            binary: false,
        }];
        app.world.change_proposed(
            change,
            StoreProposal {
                seq: 4,
                change: "demo2".into(),
                older: Vec::new(),
                why: Vec::new(),
                redacted: Vec::new(),
                cut: false,
                verifies: Runs::default(),
                on_trunk: true,
            },
        );
        app.selected = crate::app::Sel::Project(1);
        app.step();
        let lease = |symbol: &str, holder, who: &str| StoreLease {
            file: "app/cart/CartViewModel.kt".into(),
            symbol: symbol.into(),
            holder,
            name: who.into(),
            shared: false,
        };
        app.world.store_leases(
            1,
            &repo,
            vec![
                lease("CartViewModel.add", Some(id), "x"),
                lease("CartViewModel.total\u{1b}[31m", None, "someone"),
            ],
        );
        app.lonca_win = Some(LoncaWin::default());
        let s = render(&app, 120, 40);
        assert!(s.contains(&format!("LONCA · {repo}")), "{s}");
        assert!(
            s.contains(&format!(
                "{name} is on Lonca repository {repo}: trunk 9d3f1c2, 2 waiting for a human"
            )),
            "{s}"
        );
        assert!(s.contains("WAITING FOR A HUMAN · 2"), "{s}");
        let row = |s: &str, word: &str| {
            (s.lines()
                .find(|l| l.contains(word))
                .map(|l| l.trim_end().to_string()))
            .unwrap_or_else(|| panic!("no row with {word}: {s}"))
        };
        let shown = row(&s, "Δ shown");
        assert!(
            shown.contains(&format!("▶    4  {worker}  x.kt  Δ shown")),
            "{shown}"
        );
        assert!(!shown.contains("based before trunk"));
        let stranger = row(&s, "ayse");
        let pad = " ".repeat(worker.chars().count() - 4);
        assert!(
            stranger.contains(&format!(
                "     1  ayse{pad}  app/cart/CartViewModel.kt  based before trunk"
            )),
            "{stranger}"
        );
        assert!(!stranger.contains('▶'));
        assert!(s.contains("HELD · 2"), "{s}");
        assert!(
            s.contains(&format!(
                "  app/cart/CartViewModel.kt · CartViewModel.add · {worker}"
            )),
            "{s}"
        );
        let total = clean("CartViewModel.total\u{1b}[31m");
        assert!(
            s.contains(&format!("  app/cart/CartViewModel.kt · {total} · someone")),
            "cleaned: {s}"
        );
        assert!(s.contains("move") && s.contains("open the change") && s.contains("back"));
        // The row at hand marked.
        app.lonca_win = Some(LoncaWin { at: 1 });
        let s = render(&app, 120, 40);
        assert!(
            row(&s, "ayse").starts_with('▶') || row(&s, "ayse").contains("▶    1"),
            "{s}"
        );
        assert!(!row(&s, "Δ shown").contains('▶'));
        // In Turkish.
        app.lang = Lang::Tr;
        let s = render(&app, 120, 40);
        for word in [
            "İNSAN BEKLEYEN · 2",
            "TUTULAN · 2",
            "Δ gösteriliyor",
            "temeli gövdenin gerisinde",
            "değişikliği aç",
        ] {
            assert!(s.contains(word), "{word}: {s}");
        }
        // Nothing waiting, nothing held: said.
        app.lang = Lang::En;
        app.world.store_pending(1, &repo, Vec::new());
        app.world.store_leases(1, &repo, Vec::new());
        app.lonca_win = Some(LoncaWin::default());
        let s = render(&app, 120, 40);
        assert!(
            s.contains("WAITING FOR A HUMAN · 0") && s.contains("nothing waits for a human"),
            "{s}"
        );
        assert!(
            s.contains("HELD · 0") && s.contains("nothing is held"),
            "{s}"
        );
        // On a short screen the row at hand stays in view.
        let many = (10..=15)
            .map(|seq| StorePending {
                seq,
                change: format!("c{seq}"),
                name: format!("p{seq}"),
                holder: None,
                paths: Vec::new(),
                on_trunk: true,
            })
            .collect();
        app.world.store_pending(1, &repo, many);
        app.lonca_win = Some(LoncaWin { at: 5 });
        let s = render(&app, 100, 12);
        assert!(s.contains("▶   15  p15"), "{s}");
        assert!(!s.contains("  10  p10"), "scrolled away: {s}");
        app.lonca_win = Some(LoncaWin { at: 0 });
        let s = render(&app, 100, 12);
        assert!(s.contains("▶   10  p10") && !s.contains("p15"), "{s}");
    }

    #[test]
    fn the_ledger_window_says_each_project_s_figures_and_its_attempts_newest_first() {
        use crate::ledger::{Attempt, Outcome};
        let mut app = app_after(2, 1, 0, Lang::En);
        let w = &mut app.world;
        let p = w.salon_order()[0];
        let other = w.salon_order()[1];
        let lead = w.projects[p].leader;
        let name = w.agents[lead].name.clone();
        w.agents[lead].cost = 2_500;
        let c = crate::live::testing::with_card(w, lead);
        w.cards[c].project = p;
        w.ledger = vec![
            Attempt {
                project: p,
                card: Some(c),
                attempt: 2,
                agent: lead,
                model: "claude-opus-5-5".into(),
                cost: 8_400,
                rounds: 3,
                sent_back: 2,
                outcome: Outcome::Late,
                began: 0,
                ended: 12,
                version: Some("2.1.4 (Claude Code)".into()),
                ..Attempt::default()
            },
            Attempt {
                project: p,
                agent: lead,
                cost: 700,
                unpriced: true,
                rounds: 1,
                outcome: Outcome::Applied,
                began: 0,
                ended: 11,
                ..Attempt::default()
            },
        ];
        app.command("ledger");
        assert_eq!(app.ledger_win, Some(0));
        let s = render(&app, 140, 30);
        assert!(s.contains("LEDGER · 2 attempts"), "{s}");
        assert!(!s.contains("API-equivalent"), "{s}");
        assert!(
            s.contains("applied $0.07+? · wasted $0.84 · at work $0.00 · lead $0.25"),
            "{s}"
        );
        let row = |s: &str, word: &str| {
            s.lines()
                .find(|l| l.contains(word))
                .unwrap_or("")
                .to_string()
        };
        let late = row(&s, "late done");
        assert!(
            late.contains(&format!(
                "  #{} try 2  {name}  claude-opus-5-5  3 Δ · 2 sent back  $0.84  1 min  late done",
                c + 1
            )),
            "{s}"
        );
        assert!(
            late.ends_with(" · 2.1.4") || late.contains(" · 2.1.4 "),
            "{late}"
        );
        assert!(!late.contains("(Claude Code)"), "{late}");
        let applied = row(&s, "  applied  ");
        assert!(
            applied.contains(&format!("▶ no card  {name}  1 Δ  $0.07+?  55 s  applied")),
            "{s}"
        );
        assert!(
            !late.contains('▶') && !applied.contains("sent back"),
            "{applied}"
        );
        // Newest first: the row with no card is above.
        assert!(
            s.find("no card").unwrap() < s.find("late done").unwrap(),
            "{s}"
        );
        // A project nothing was spent in says so.
        let other_name = app.world.projects[other].name.clone();
        assert!(s.contains(&format!("{other_name}  applied $0.00")), "{s}");
        assert!(s.contains("nothing spent on a card yet"), "{s}");
        assert!(s.contains("open the card"), "{s}");
        // On a plan: API-equivalent, every figure ≈.
        app.world.estimated = true;
        let s = render(&app, 140, 30);
        assert!(s.contains("LEDGER · 2 attempts · API-equivalent"), "{s}");
        assert!(
            s.contains("≈$0.84") && s.contains("applied ≈$0.07+?"),
            "{s}"
        );
        // In Turkish.
        app.lang = Lang::Tr;
        let s = render(&app, 140, 30);
        for word in [
            "HARCAMALAR · 2 deneme · API karşılığı",
            "uygulanan ≈$0.07+? · boşa ≈$0.84 · süren ≈$0.00 · lider ≈$0.25",
            "deneme 2",
            "kartsız",
            "2 geri döndü",
            "geç done",
            "uygulandı",
            "henüz bir karta harcama yok",
            "kartı aç",
        ] {
            assert!(s.contains(word), "{word}: {s}");
        }
        // On a short screen the row at hand stays in view.
        app.lang = Lang::En;
        app.world.estimated = false;
        let older = (0..8).map(|i| Attempt {
            project: p,
            agent: lead,
            model: format!("m-{i}"),
            ..Attempt::default()
        });
        app.world.ledger.splice(0..0, older);
        app.ledger_win = Some(9);
        let s = render(&app, 140, 12);
        assert!(row(&s, "m-0").contains('▶'), "the oldest, at hand: {s}");
        assert!(!s.contains("late done"), "scrolled away: {s}");
        app.ledger_win = Some(0);
        let s = render(&app, 140, 12);
        assert!(s.contains("lead $0.25") && !s.contains("m-0"), "{s}");
        app.world.ledger.drain(0..8);
        // How it ended says it in its colour; the row at hand in bold;
        // a blank line between projects.
        app.ledger_win = Some(1);
        let mut term = ratatui::Terminal::new(TestBackend::new(140, 30)).unwrap();
        term.draw(|f| draw(f, &app)).unwrap();
        let buf = term.backend().buffer().clone();
        let s = render(&app, 140, 30);
        let lines: Vec<&str> = s.lines().collect();
        let cell = |word: &str| {
            let y = lines.iter().position(|l| l.contains(word)).unwrap();
            let x = lines[y].split(word).next().unwrap().chars().count();
            buf[(x as u16, y as u16)].clone()
        };
        assert_eq!(cell("late done").fg, pal().warn);
        let applied = lines.iter().position(|l| l.contains("no card")).unwrap();
        let x = (lines[applied].find("55 s  applied"))
            .map(|b| lines[applied][..b].chars().count() + "55 s  ".len())
            .unwrap();
        assert_eq!(buf[(x as u16, applied as u16)].fg, pal().ok);
        assert!(
            cell("#3 try 2").modifier.contains(Modifier::BOLD),
            "at hand"
        );
        assert!(!cell("no card").modifier.contains(Modifier::BOLD));
        let other_at = lines
            .iter()
            .position(|l| l.contains(&format!("{other_name}  applied")))
            .unwrap();
        assert!(
            lines[other_at - 1].contains("│ ")
                && lines[other_at - 1]
                    .trim_matches(|c| c == ' ' || c == '│')
                    .is_empty(),
            "{s}"
        );
        app.world.ledger.push(Attempt {
            project: other,
            agent: lead,
            model: "m-open".into(),
            ..Attempt::default()
        });
        let s = render(&app, 140, 30);
        let lines: Vec<&str> = s.lines().collect();
        let mut term = ratatui::Terminal::new(TestBackend::new(140, 30)).unwrap();
        term.draw(|f| draw(f, &app)).unwrap();
        let buf = term.backend().buffer().clone();
        let y = lines.iter().position(|l| l.contains("m-open")).unwrap();
        let x = lines[y]
            .find("at work")
            .map(|b| lines[y][..b].chars().count())
            .unwrap();
        assert_eq!(buf[(x as u16, y as u16)].fg, pal().info);
        app.world.ledger.pop();
        // The card's screen lists its tries.
        app.ledger_win = None;
        app.open_card(c);
        let s = render(&app, 140, 40);
        assert!(s.contains("ATTEMPTS"), "{s}");
        assert!(row(&s, "late done").contains("try 2"), "{s}");
        assert!(!s.contains("no card"), "only the card's: {s}");
        // A card no try spent on lists none.
        let d = crate::live::testing::with_card(&mut app.world, lead);
        app.world.cards[d].project = p;
        app.open_card(d);
        let s = render(&app, 140, 40);
        assert!(!s.contains("ATTEMPTS"), "{s}");
    }

    #[test]
    fn how_long_an_attempt_took_is_seconds_under_a_minute() {
        use crate::ui::ledger::lasted;
        assert_eq!(lasted(Lang::En, 11), "55 s");
        assert_eq!(lasted(Lang::En, 12), "1 min");
        assert_eq!(lasted(Lang::Tr, 0), "0 sn");
        // Every outcome has its own words.
        let t = Lang::En.text();
        let words: std::collections::HashSet<&str> = [
            crate::ledger::Outcome::Open,
            crate::ledger::Outcome::Applied,
            crate::ledger::Outcome::Done,
            crate::ledger::Outcome::Failed,
            crate::ledger::Outcome::Withdrawn,
            crate::ledger::Outcome::HandedOn,
            crate::ledger::Outcome::Late,
        ]
        .map(|o| crate::ui::ledger::outcome(t, o))
        .into_iter()
        .collect();
        assert_eq!(words.len(), 7);
        assert_eq!(
            crate::ui::ledger::outcome(t, crate::ledger::Outcome::HandedOn),
            "handed on"
        );
        assert_eq!(
            crate::ui::ledger::outcome(t, crate::ledger::Outcome::Withdrawn),
            "withdrawn"
        );
        assert_eq!(
            crate::ui::ledger::outcome(t, crate::ledger::Outcome::Failed),
            "failed"
        );
        assert_eq!(
            crate::ui::ledger::outcome(t, crate::ledger::Outcome::Done),
            "done, no change"
        );
        assert_eq!(
            crate::ui::ledger::outcome(t, crate::ledger::Outcome::Open),
            "at work"
        );
    }

    #[test]
    fn a_teardown_script_asked_about_shows_its_bytes_over_the_tidy_window() {
        let mut app = app_after(1, 1, 0, Lang::En);
        app.open_tidy();
        app.step();
        let checkout = std::path::PathBuf::from("/src/shop");
        let script = crate::sim::teardown_script(&checkout);
        let sha: String = script.sha.chars().take(16).collect();
        app.teardown = Some(crate::app::TeardownAsk {
            script: script.clone(),
            scroll: 0,
            at: vec![0],
            force: false,
        });
        let s = render(&app, 160, 30);
        assert!(
            s.contains(&format!("TEARDOWN SCRIPT · shop · sha256 {sha} ")),
            "{s}"
        );
        assert!(s.contains("before :tidy removes it"), "{s}");
        assert!(
            s.contains("file: /src/shop/.imece/worktree-teardown.sh"),
            "{s}"
        );
        assert!(s.contains("1 #!/bin/sh"), "{s}");
        assert!(s.contains("5 npm run db:drop"), "{s}");
        assert!(s.contains("yes, then remove"), "{s}");
        assert!(s.contains("remove nothing"), "{s}");
        app.teardown.as_mut().unwrap().scroll = 4;
        let s = render(&app, 160, 30);
        assert!(!s.contains("1 #!/bin/sh"), "{s}");
        assert!(s.contains("5 npm run db:drop"), "{s}");
        app.lang = Lang::Tr;
        let s = render(&app, 160, 30);
        assert!(s.contains("SÖKÜM BETİĞİ · shop"), "{s}");
        assert!(s.contains("dosya: /src/shop/"), "{s}");
        assert!(s.contains("evet, sonra sil"), "{s}");
    }

    #[test]
    fn the_tidy_window_keeps_the_row_at_hand_on_a_short_screen() {
        let mut app = app_after(1, 1, 0, Lang::En);
        app.open_tidy();
        app.step();
        let mut found = app.world.orphans.found.clone().unwrap();
        for i in 0..30 {
            let mut o = found[0].clone();
            o.name = format!("w-{i:02}");
            found.push(o);
        }
        app.world.orphans.found = Some(found);
        app.tidy = Some(crate::app::TidyWin {
            at: 31,
            sure: false,
        });
        let s = render(&app, 120, 16);
        assert!(s.contains("▶ w-29"), "{s}");
        assert!(!s.contains("claude-31"), "{s}");
        app.tidy = Some(crate::app::TidyWin { at: 0, sure: false });
        let s = render(&app, 120, 16);
        assert!(s.contains("▶ claude-31"), "{s}");
        assert!(s.contains("under imece's state"), "{s}");
        // While a removal runs, its line is above the rows: the row at
        // hand, the last, still shows, the one above it too.
        app.world.orphans.busy = true;
        app.tidy = Some(crate::app::TidyWin {
            at: 31,
            sure: false,
        });
        let s = render(&app, 120, 16);
        assert!(s.contains("▶ w-29"), "{s}");
        assert!(s.contains("  w-28 "), "{s}");
        assert!(!s.contains("removing…"), "scrolled away: {s}");
        // Scrolled exactly: the row at hand is the last the window shows.
        let shown: Vec<&str> = s.lines().filter(|l| l.contains("w-2")).collect();
        assert!(shown.last().unwrap().contains("▶ w-29"), "{s}");
        assert!(!s.contains(" w-19 "), "one row too few scrolled: {s}");
    }

    #[test]
    fn waiting_cleared_off_the_screen_is_counted_in_its_title() {
        let mut app = app_after(1, 4, 0, Lang::En);
        app.world.inbox.clear();
        for agent in [1, 2, 3] {
            app.world.inbox.push(crate::model::InboxItem {
                kind: InboxKind::Question,
                agent,
                detail: "x".into(),
                researcher: None,
                open: true,
                tick: 0,
                card: None,
            });
        }
        app.world.cleared.inbox = [0, 1].into();
        let s = render(&app, 140, 40);
        assert!(
            s.contains("TO DECIDE 1 question · 2 hidden (:unclear)"),
            "{s}"
        );
        app.world.cleared.inbox.insert(2);
        let s = render(&app, 140, 40);
        assert!(s.contains("TO DECIDE 3 hidden (:unclear)"), "{s}");
        // A card's screen lists them, so says nothing of it.
        app.world.inbox[0].card = Some(0);
        app.open_card(0);
        let s = render(&app, 140, 40);
        assert!(!s.contains("hidden"), "{s}");
        app.leave_card();
        app.world.inbox[1].card = Some(0);
        app.world.inbox.iter_mut().for_each(|i| i.open = false);
        let s = render(&app, 140, 40);
        assert!(!s.contains("TO DECIDE") && !s.contains("hidden"), "{s}");
        // The grace counts down in the bottom line, for each kind.
        let at = std::time::Instant::now() + crate::app::UNDO_GRACE;
        for (what, words) in [
            (
                crate::app::Clear::All,
                "clearing TO DECIDE and the journal off the screen in 10 s",
            ),
            (
                crate::app::Clear::Inbox,
                "clearing TO DECIDE off the screen in 10 s",
            ),
            (
                crate::app::Clear::Journal,
                "clearing the journal off the screen in 10 s",
            ),
        ] {
            app.grace = Some((crate::app::Undoable::Clear(what), at));
            let s = render(&app, 140, 40);
            assert!(s.contains(words), "{s}");
        }
    }

    /// A demo team after a while, card #1's screen opened by typing it.
    fn on_card_one(lang: Lang) -> App {
        let mut app = App::new(1, 4, 4, 2, lang);
        for _ in 0..140 {
            app.step();
        }
        app.open_card(0);
        assert_eq!(app.card, Some(0));
        app
    }

    #[test]
    fn a_card_s_screen_is_its_owner_s_detail_under_the_frame() {
        let app = on_card_one(Lang::En);
        let owner = app.world.cards[0].owner.unwrap();
        assert_eq!((app.pane, app.zoom), (Pane::Salon, Zoom::Focus(owner)));
        assert_eq!(app.detail(), Some((owner, crate::app::Tab::Log)));
        assert_eq!(
            app.detail_card(owner),
            Some(0),
            "the card, not the owner's next"
        );
        // The card's block has the keys' colour and no title of its own:
        // the detail's head names the card.
        let buf = buffer(&app, 140, 36);
        let s = render(&app, 140, 36);
        let y = s.lines().position(|l| l.starts_with('┌')).unwrap();
        assert!(y > 0, "under the head: {s}");
        assert_eq!(buf[(0, y as u16)].style().fg, Some(pal().accent));
        assert!(s.lines().nth(y).unwrap().starts_with("┌───"), "{s}");
    }

    #[test]
    fn a_card_s_screen_shows_where_it_stands_who_had_a_part_and_its_lines() {
        let app = on_card_one(Lang::En);
        let s = render(&app, 140, 38);
        let card = &app.world.cards[0];
        assert!(!card.symbols.is_empty() && card.column == Column::Review);
        let owner = &app.world.agents[card.owner.unwrap()].name;
        let owns = format!("owns   {}", crate::paths::listed(&card.paths));
        assert!(!card.paths.is_empty());
        for want in [
            &format!("#1 {}", card.title),
            "shop-api · TO REVIEW · ",
            " min",
            "editing  ",
            &owns,
            "WHO",
            "posted    ! ▎lead ",
            "owner     ",
            "talked    ",
            "STORY",
            &format!("◎ intent     {owner}: {}", card.title),
            "n p  next / previous card",
        ] {
            assert!(s.contains(want), "{want} missing:\n{s}");
        }
        assert!(
            !s.contains("TEAM") && !s.contains("TO DECIDE · #1") && !s.contains("JOURNAL · #1"),
            "the card is the detail's body, no panes beside it:\n{s}"
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
            format!("shop-api · TO REVIEW · {minutes} min\n\n"),
            "editing  ".to_string(),
            "## WHO\n\n- posted: ".to_string(),
            format!("- owner: {owner}\n"),
            "- talked: ".to_string(),
            "\n## STORY\n\n- ".to_string(),
            format!("{owner}: {}", card.title),
            "\n## To decide\n\n- ".to_string(),
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
            "sahibi    ",
            "sonraki / önceki iş",
        ] {
            assert!(s.contains(want), "{want} missing:\n{s}");
        }
    }

    #[test]
    fn the_salon_offers_cards_and_says_what_is_typed_after_a_hash() {
        let mut app = app_after(1, 4, 140, Lang::En);
        assert!(render(&app, 140, 36).contains(" home · "));
        app.on_key(KeyEvent::from(KeyCode::Char('#')));
        let s = render(&app, 140, 36);
        assert!(s.contains(" › card: #"), "{s}");
        assert!(s.contains("#5 "), "the menu offers the cards:\n{s}");
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
        let s = render(&app, 140, 80);
        assert!(s.contains("KARAR BEKLEYEN"), "{s}");
        assert!(!s.contains("TO DECIDE") && !s.contains("AT WORK"));
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

    /// One project whose agents are all calm and working for a long time:
    /// the leader, the workers in `w`, the researchers in `r`.
    fn calm_team() -> (App, Vec<AgentId>, Vec<AgentId>) {
        let mut app = app_after(1, 4, 0, Lang::En);
        let id = app.world.agents.len();
        app.world.agents.push(Agent::new(
            id,
            0,
            "res-1".into(),
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
    fn the_focus_of_a_helper_names_its_worker_and_a_helper_that_left_is_journaled() {
        let mut app = app_after(1, 4, 50, Lang::En);
        let (worker, [h1, _]) = with_helpers(&mut app);
        app.world.note(Event::Left { agent: h1 });
        let name = app.world.agents[worker].name.clone();
        app.open_window(Window::Journal);
        let s = render(&app, 140, 45);
        assert!(s.contains(&format!("{name}.1 finished and left")), "{s}");
        app.close_window();
        app.zoom = Zoom::Focus(h1);
        let s = render(&app, 140, 45);
        assert!(s.contains(&format!("helper of {name}")), "{s}");
    }

    #[test]
    fn a_focus_footer_names_the_keys_that_decide_what_its_agent_waits_for() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let foot = |app: &App| render(app, 160, 30).lines().last().unwrap().to_string();
        let id = app.world.projects[0].leader + 1;
        app.zoom = Zoom::Focus(id);
        let calm = foot(&app);
        assert!(
            calm.contains("   enter  trace   @  write to this agent   esc  back   ?  help"),
            "{calm}"
        );
        for gone in [" a ", " A ", " r "] {
            assert!(!calm.contains(gone), "{calm}");
        }
        // An ask of another agent says nothing here.
        app.world.asks_you(id + 1, "Bash: make");
        assert_eq!(foot(&app), calm);
        app.world.asks_you(id, "Bash: make");
        let asked = foot(&app);
        // Three keys for the ask, the way back and the help; `A` is in the help.
        assert!(
            asked.contains("   a  allow once   r  refuse   e  yes this turn   esc  back   ?  help"),
            "{asked}"
        );
        assert!(!asked.contains(" A "), "{asked}");
        app.zoom = Zoom::Trace(id);
        let traced = foot(&app);
        assert!(
            traced
                .contains("   a  allow once   r  refuse   e  yes this turn   esc  back   ?  help"),
            "{traced}"
        );
        // A window over it has keys of its own.
        app.peek = Some(id);
        assert!(!foot(&app).contains("allow once"), "{}", foot(&app));
        app.peek = None;
        // Its change, once its ask is decided.
        app.world.decided(1, true);
        app.world.change_ready(crate::model::Change {
            agent: id,
            title: "did it".into(),
            files: Vec::new(),
            patch: String::new(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
        app.zoom = Zoom::Focus(id);
        let change = foot(&app);
        assert!(
            change.contains("   a  approve   r  send back   esc  back   ?  help"),
            "{change}"
        );
        app.lang = Lang::Tr;
        app.world.asks_you(id, "Bash: make");
        let tr = foot(&app);
        assert!(
            tr.contains("a  bir kez izin ver   r  reddet   e  bu tur evet   esc  geri   ?  yardım"),
            "{tr}"
        );
    }

    #[test]
    fn mail_that_waits_with_an_agent_shows_on_its_row_in_both_languages() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let id = app.world.projects[0].leader + 1;
        let text = |app: &App| {
            status_spans(app.lang, &app.world, &app.world.agents[id])
                .iter()
                .map(|s| s.content.to_string())
                .collect::<String>()
        };
        let plain = text(&app);
        app.world.agents[id].held_mail = 2;
        assert_eq!(text(&app), format!("✉ 2 queued {plain}"));
        let spans = status_spans(app.lang, &app.world, &app.world.agents[id]);
        assert_eq!(spans[0].style, fg(pal().warn));
        app.world.agents[id].turn_yes = Some(0);
        assert_eq!(text(&app), format!("✓ turn ✉ 2 queued {plain}"));
        app.world.agents[id].turn_yes = None;
        app.lang = Lang::Tr;
        assert!(text(&app).starts_with("✉ 2 kuyrukta "));
    }

    #[test]
    fn a_yes_for_the_turn_shows_first_on_the_agents_status_while_it_holds() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let id = app.world.projects[0].leader + 1;
        let text = |app: &App| {
            status_spans(app.lang, &app.world, &app.world.agents[id])
                .iter()
                .map(|s| s.content.to_string())
                .collect::<String>()
        };
        let plain = text(&app);
        assert!(!plain.contains("✓ turn"), "{plain}");
        app.world.agents[id].turn_yes = Some(3);
        let spans = status_spans(app.lang, &app.world, &app.world.agents[id]);
        assert_eq!(spans[0].content, "✓ turn ");
        assert_eq!(spans[0].style, bold(pal().warn));
        assert_eq!(text(&app), format!("✓ turn {plain}"));
        app.zoom = Zoom::Focus(id);
        assert!(render(&app, 160, 30).contains("status ✓ turn "));
        app.lang = Lang::Tr;
        assert!(text(&app).starts_with("✓ tur "));
    }

    #[test]
    fn the_salon_and_the_focus_say_the_folder_they_work_in() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let name = app.world.projects[0].name.clone();
        let header = |app: &App| {
            let l = salon::project_line(app, 0, &app.rungs());
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        };
        assert!(
            header(&app).ends_with(&format!("  ~/src/{name}")),
            "{}",
            header(&app)
        );
        // Folded, it ends with its loudest member instead.
        app.collapsed[0] = true;
        assert!(!header(&app).contains("~/src"), "{}", header(&app));
        app.collapsed[0] = false;
        let lead = app.world.projects[0].leader;
        app.zoom = Zoom::Focus(lead);
        let s = render(&app, 160, 30);
        assert!(s.contains(&format!("folder ~/src/{name} ")), "{s}");
        assert!(!s.contains("worktree of"), "{s}");
        assert!(!s.contains("scratch "), "a leader writes nothing: {s}");
        app.zoom = Zoom::Focus(lead + 1);
        let s = render(&app, 160, 30);
        let worker = app.world.agents[lead + 1].name.clone();
        assert!(
            s.contains(&format!(
                "folder ~/.local/state/imece/worktrees/{name}-5e1f0a2c/{worker} · worktree of ~/src/{name}"
            )),
            "{s}"
        );
        assert!(
            s.contains(&format!(
                "scratch ~/.local/state/imece/worktrees/{name}-5e1f0a2c-scratch/{worker} "
            )),
            "{s}"
        );
        // A worker on Lonca: the folder is where it was, the word is not.
        app.world.agents[lead + 1].workspace = Some("kule".into());
        let s = render(&app, 160, 30);
        assert!(s.contains("· workspace of lonca kule"), "{s}");
        assert!(!s.contains("worktree of"), "{s}");
        app.lang = Lang::Tr;
        let s = render(&app, 160, 30);
        assert!(s.contains("· lonca kule için workspace"), "{s}");
        app.world.agents[lead + 1].workspace = None;
        let s = render(&app, 160, 30);
        assert!(s.contains(&format!("· ~/src/{name} için worktree")), "{s}");
        assert!(s.contains("klasör ~/.local"), "{s}");
        assert!(s.contains("karalama ~/.local"), "{s}");
        // A solo's goes by its project, as live's does.
        app.world.agents[lead + 1].role = Role::Solo;
        assert_eq!(
            app.scratch_of(lead + 1),
            Some(format!(
                "~/.local/state/imece/worktrees/{name}-5e1f0a2c-scratch/solo-0"
            ))
        );
    }

    /// An open answer of `agent` in TO DECIDE.
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
        assert!(render(&app, 140, 40).contains(&format!("{one} is pinned to the top of the team")));
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

    fn plan_of(app: &mut App, a: AgentId) {
        app.world.agents[a].plan = vec![
            Step {
                text: "Read the code".into(),
                state: StepState::Done,
            },
            Step {
                text: "Run the tests".into(),
                state: StepState::Doing,
            },
            Step {
                text: "Write it up".into(),
                state: StepState::Doing,
            },
            Step {
                text: "Ship".into(),
                state: StepState::Todo,
            },
        ];
    }

    #[test]
    fn the_step_is_the_first_in_progress_and_none_without_one() {
        let (mut app, w, _) = calm_team();
        let a = w[0];
        assert_eq!(app.world.agents[a].step(), None);
        plan_of(&mut app, a);
        assert_eq!(app.world.agents[a].step(), Some("Run the tests"));
        app.world.agents[a].plan[1].text = "  ".into();
        app.world.agents[a].plan.remove(2);
        assert_eq!(
            app.world.agents[a].step(),
            None,
            "a blank step says nothing"
        );
        for s in &mut app.world.agents[a].plan {
            s.state = StepState::Done;
        }
        assert_eq!(app.world.agents[a].step(), None);
    }

    #[test]
    fn a_salon_s_rows_stand_in_columns_whatever_their_depth_and_parts() {
        let (mut app, w, _) = calm_team();
        app.world.inbox.clear();
        let lead = app.world.projects[app.world.agents[w[0]].project].leader;
        let rungs = app.rungs();
        for &id in &[lead, w[0], w[1]] {
            let a = &mut app.world.agents[id];
            a.status = Status::Idle;
            a.talk = None;
            a.activity = None;
            a.plan.clear();
            a.context = None;
            a.cost = 0;
            a.stand = Stand::default();
        }
        app.world.agents[lead].cost = 550_000;
        app.world.agents[w[0]].cost = 12_290_000;
        app.world.agents[w[0]].stand.added = 727;
        app.world.agents[w[1]].stand.added = 1090;
        app.world.agents[w[1]].stand.removed = 134;
        plan_of(&mut app, w[0]);
        for id in [lead, w[0], w[1]] {
            app.world.agents[id].status = Status::Idle;
            app.world.agents[id].changed_at = 0;
        }
        app.world.tick = 240;
        app.world.agents[w[1]].changed_at = 228;
        let row = |app: &App, id: AgentId, prefix: &'static str, width: usize| -> String {
            let line = salon::agent_line(app, &app.world.agents[id], prefix, &rungs, width);
            (line.spans.iter()).map(|s| s.content.as_ref()).collect()
        };
        let at = |r: &str, what: &str| {
            let cut = r.find(what).unwrap_or_else(|| panic!("{what} in {r:?}"));
            r[..cut].chars().count()
        };
        let lead_row = row(&app, lead, "  ", 200);
        let one = row(&app, w[0], "  ├ ", 200);
        let two = row(&app, w[1], "  └ ", 200);
        let helper = row(&app, w[1], "    └ ", 200);
        for r in [&one, &two, &helper] {
            assert_eq!(at(r, "idle"), at(&one, "idle"), "the status: {r:?}");
        }
        assert_eq!(at(&lead_row, "coordinating"), at(&one, "idle"));
        // The columns stand under one another, a part a row lacks kept blank.
        assert_eq!(at(&lead_row, "·"), at(&one, "·"), "{lead_row:?}\n{one:?}");
        assert_eq!(at(&one, "·"), at(&two, "·"), "{one:?}\n{two:?}");
        assert_eq!(at(&one, "+727"), at(&two, "+1090"), "{one:?}\n{two:?}");
        assert_eq!(
            at(&lead_row, "$") + dollars(550_000).len(),
            at(&one, "$") + dollars(12_290_000).len(),
            "the money is right aligned: {lead_row:?}\n{one:?}"
        );
        // A narrow screen gives up the blanks, never what the row says.
        for width in 80..130 {
            for (id, prefix) in [(lead, "  "), (w[0], "  ├ "), (w[1], "  └ ")] {
                let r = row(&app, id, prefix, width);
                assert!(r.chars().count() <= width, "{width}: {r:?}");
            }
            let r = row(&app, w[0], "  ├ ", width);
            assert!(
                r.contains(&dollars(12_290_000)) && r.contains("+727"),
                "{r:?}"
            );
        }
        // Who has gone away widens nothing, and a row alone is not padded.
        app.world.agents[lead].cost = 1_000_000_000;
        let wider = row(&app, w[0], "  ├ ", 200);
        assert!(at(&wider, "$") > at(&one, "$"), "{wider:?}");
        app.world.agents[lead].gone = true;
        assert_eq!(row(&app, w[0], "  ├ ", 200), one, "gone");
        app.world.agents[lead].gone = false;
        app.world.projects[app.world.agents[lead].project].closed = true;
        assert_eq!(row(&app, w[0], "  ├ ", 200).len(), one.len(), "closed");
        app.world.projects[app.world.agents[lead].project].closed = false;
        app.world.agents[lead].cost = 550_000;
        let alone = row(&app, w[1], "", 200);
        assert!(alone.chars().count() < two.chars().count(), "{alone:?}");
        assert!(!alone.contains("  ·"), "{alone:?}");
        // A helper's name gives way, a shallower one takes the room.
        app.world.agents[w[1]].name = "a-helper-with-a-long-name".into();
        assert!(row(&app, w[1], "    └ ", 200).contains("a-helper-with…"));
        assert!(row(&app, w[1], "  └ ", 200).contains("a-helper-with-a…"));
        assert!(row(&app, w[1], "  ", 200).contains("a-helper-with-a-l…"));
    }

    #[test]
    fn a_step_is_cut_to_what_the_row_leaves_it_between_its_bounds() {
        let (mut app, w, _) = calm_team();
        app.world.inbox.clear();
        let a = w[0];
        plan_of(&mut app, a);
        app.world.agents[a].plan[1].text = "Run the tests with coverage on".into();
        app.world.agents[a].cost = 20_000;
        app.world.agents[a].stand = Stand::default();
        let rungs = app.rungs();
        let line = |app: &App, width: usize| {
            salon::agent_line(app, &app.world.agents[a], "", &rungs, width)
        };
        let full = line(&app, 1000);
        let at = (full.spans.iter())
            .position(|s| s.content.starts_with("▸ "))
            .expect("a step");
        let base: usize = full.spans[..at].iter().map(Span::width).sum();
        let tail = 2 + dollars(20_000).chars().count();
        let step = |app: &App, width: usize| {
            line(app, width)
                .spans
                .iter()
                .find(|s| s.content.starts_with("▸ "))
                .map(|s| s.content.to_string())
                .unwrap_or_default()
        };
        assert_eq!(step(&app, 1000), "▸ Run the tests with cove…");
        assert_eq!(step(&app, base + tail + 2 + 12), "▸ Run the tes…");
        assert_eq!(step(&app, base + tail + 2 + 3), "▸ Run the…");
        assert_eq!(
            status_spans(Lang::En, &app.world, &app.world.agents[a])[0].content,
            "▸ Run the tests with cove…"
        );
    }

    #[test]
    fn what_a_step_does_takes_what_the_status_and_row_leave_or_nothing() {
        let mut a = Agent::new(0, 0, "w".into(), Role::Worker, Status::Working, 0);
        let text = |s: Vec<Span>| s.iter().map(|s| s.content.to_string()).collect::<String>();
        a.activity = Some("x".repeat(50));
        let x = |n: usize| "x".repeat(n);
        assert_eq!(
            text(doing_spans(Lang::En, &a, 0, usize::MAX)),
            format!("  {}…", x(37))
        );
        assert_eq!(
            text(doing_spans(Lang::En, &a, 30, usize::MAX)),
            format!("  {}…", x(7))
        );
        assert!(doing_spans(Lang::En, &a, 31, usize::MAX).is_empty());
        assert_eq!(
            text(doing_spans(Lang::En, &a, 0, 10)),
            format!("  {}…", x(7))
        );
        assert!(doing_spans(Lang::En, &a, 0, 9).is_empty());
        a.activity = Some(String::new());
        assert_eq!(text(doing_spans(Lang::Tr, &a, 0, 80)), "  düşünüyor…");
        a.activity = None;
        assert!(doing_spans(Lang::En, &a, 0, 80).is_empty());
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
        assert!(!open.contains("■"), "{open}");
        // Folded: the loudest member last; the lead counts as a member,
        // with its status's glyph like anyone (record 125).
        app.collapsed[0] = true;
        let h = header(&app);
        assert!(h.ends_with(&format!("  ● {}", name(&app, lead))), "{h}");
        app.world.agents[lead].status = Status::Idle;
        let h = header(&app);
        assert!(h.ends_with(&format!("  ● {}", name(&app, w[0]))), "{h}");
        app.world.agents[w[3]].status = Status::Stopped;
        app.world.agents[w[1]].status = Status::Blocked;
        let h = header(&app);
        // Blocked and stopped are one count of stuck.
        assert!(h.contains("  ■2"), "{h}");
        assert!(!h.contains("■1"), "{h}");
        assert!(h.ends_with(&format!("  ■ {}", name(&app, w[1]))), "{h}");
        app.world.agents[w[1]].status = Status::Working;
        let h = header(&app);
        assert!(!h.contains("■2") && h.contains("  ■1"), "{h}");
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
        // A leader's row waits like anyone's (record 125: no glyph of its own).
        let lead = app.world.projects[0].leader;
        assert_eq!(row(&app, lead).spans[1].content, "● ");
        waiting(&mut app, lead);
        assert_eq!(row(&app, lead).spans[1].content, "! ");
    }

    /// The screen of `app` drawn whole on a terminal of `width` columns:
    /// its cells and the cursor.
    fn whole(app: &App, width: u16) -> (Buffer, Option<Position>) {
        let mut t = Terminal::new(TestBackend::new(width, 40)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        let cursor = app.caret_at.get().map(|_| t.get_cursor_position().unwrap());
        (t.backend().buffer().clone(), cursor)
    }

    /// The screen of `app` patched from `last` on a terminal of `width`
    /// columns, if it could be.
    fn patched(app: &App, last: &Buffer, width: u16) -> Option<(Buffer, Option<Position>)> {
        let mut t = Terminal::new(TestBackend::new(width, 40)).unwrap();
        let mut done = false;
        t.draw(|f| done = patch(f, app, last)).unwrap();
        let cursor = app.caret_at.get().map(|_| t.get_cursor_position().unwrap());
        done.then(|| (t.backend().buffer().clone(), cursor))
    }

    #[test]
    fn a_frame_patched_for_the_breath_is_the_frame_drawn_whole() {
        use crate::motion::Motion;
        let ms = std::time::Duration::from_millis;
        let mut app = app_after(2, 10, 0, Lang::En);
        app.motion = Motion::Calm;
        let (mut checked, mut refused) = (0, 0);
        for round in 0..120 {
            app.step();
            // Folded projects, a selected row, the person writing, a narrow
            // screen: each in some rounds.
            app.collapsed[1] = round % 3 == 1;
            app.selected = match (app.world.agents.iter()).find(|a| breathes(a, app.rungs()[a.id]))
            {
                Some(a) if round % 4 == 2 => Sel::Agent(a.id),
                _ => Sel::Project(0),
            };
            if round == 30 {
                for c in ['/', 'h', 'i'] {
                    app.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
                }
            }
            let width = if round % 5 == 4 { 70 } else { 160 };
            // Drawn whole in one tone; the next frame, in the other, is
            // patched where the main loop would patch it. Two seconds on,
            // when the glows the step lit are over.
            let since = std::time::Instant::now() + ms(2000) - app.after(ms(0));
            let beat = since.as_millis() as u64 / 600 * 600 + 600;
            for (from, to) in [(beat, beat + 600), (beat + 600, beat + 1200)] {
                app.frame_after(ms(from));
                let (first, _) = whole(&app, width);
                app.drawn(app.after(ms(from)));
                let glyphs = app.breathing.borrow().clone();
                let due = app.patch_due(app.after(ms(to)));
                app.frame_after(ms(to));
                let got = patched(&app, &first, width);
                let want = whole(&app, width);
                assert_eq!(*app.breathing.borrow(), glyphs, "the same glyphs");
                if due {
                    assert_eq!(got, Some(want), "round {round}, {from} ms → {to} ms");
                    checked += 1;
                } else {
                    refused += 1;
                }
            }
        }
        // Enough of each: the demo lights rows and starts sessions often.
        assert!(checked > 40, "{checked} patched, {refused} not");
        assert!(refused > 10, "{checked} patched, {refused} not");
    }

    #[test]
    fn a_working_owner_breathes_on_the_board_s_window_and_nothing_is_patched() {
        use crate::motion::Motion;
        let ms = std::time::Duration::from_millis;
        let (mut app, w, _) = calm_team();
        app.motion = Motion::Calm;
        app.window = Some(Window::Board);
        // The team under the window tells of cards the test replaces.
        app.world.journal.clear();
        app.world.inbox.clear();
        let card = |id, owner, column| Card {
            id,
            project: 0,
            title: format!("card {id}"),
            owner,
            column,
            story: Vec::new(),
            symbols: Vec::new(),
            paths: Vec::new(),
            checks: Vec::new(),
            started: 0,
            finished: 0,
            attempt: 1,
            breaker: Default::default(),
            after: Vec::new(),
        };
        app.world.cards = vec![
            card(0, Some(w[0]), Column::Working),
            card(1, Some(w[1]), Column::Working),
            card(2, None, Column::Waiting),
            card(3, Some(w[3]), Column::Done),
            card(4, None, Column::Waiting),
        ];
        app.world.agents[w[1]].status = Status::Idle;
        app.world.agents[w[3]].status = Status::Working;
        let width = 160;
        // The window starts four columns and two rows in, its border one
        // more; in progress is the second of five columns of 30, its
        // border at 35, its rows under its own border (y 4); the glyph
        // after four cells, two wide, under the card's title.
        let (x, y) = (40, 5);
        app.frame_after(ms(600));
        let (first, _) = whole(&app, width);
        assert_eq!(
            *app.breathing.borrow(),
            None,
            "a window over the screen: no patch"
        );
        assert!(!app.patchable());
        assert_eq!(first[(x, y)].symbol(), "●");
        assert!(
            first[(x, y)].modifier.contains(Modifier::DIM),
            "the second tone"
        );
        assert_eq!(first[(x, y + 2)].symbol(), "○", "the idle owner");
        // A waiting card nobody holds and that waits on nothing takes one
        // row: the next card's title right under it.
        assert_eq!(first[(6, 4)].symbol(), "#");
        assert_eq!(first[(7, 4)].symbol(), "3");
        assert_eq!(first[(6, 5)].symbol(), "#");
        assert_eq!(first[(7, 5)].symbol(), "5");
        assert!(!first[(x, y + 2)].modifier.contains(Modifier::DIM));
        // The next beat draws the first tone, whole.
        app.drawn(app.after(ms(600)));
        assert!(!app.patch_due(app.after(ms(1200))));
        app.frame_after(ms(1200));
        assert!(patched(&app, &first, width).is_none());
        let (second, _) = whole(&app, width);
        assert_eq!(second[(x, y)].symbol(), "●");
        assert!(
            !second[(x, y)].modifier.contains(Modifier::DIM),
            "the first tone"
        );
    }

    #[test]
    fn a_working_row_breathes_in_its_colour_never_its_shape() {
        use crate::motion::Motion;
        let ms = std::time::Duration::from_millis;
        let (mut app, w, _) = calm_team();
        let lead = app.world.projects[0].leader;
        let glyph = |app: &App, id: AgentId| {
            let line = salon::agent_line(app, &app.world.agents[id], "", &app.rungs(), 200);
            (line.spans[1].content.to_string(), line.spans[1].style)
        };
        let ok = fg(pal().ok);
        let out = ok.add_modifier(Modifier::DIM);
        assert_eq!(glyph(&app, w[0]), ("● ".into(), ok), "off: still");
        app.motion = Motion::Calm;
        app.frame_after(ms(600));
        assert_eq!(glyph(&app, w[0]), ("● ".into(), out), "the second tone");
        assert_eq!(glyph(&app, lead), ("● ".into(), ok), "a leader's does not");
        app.frame_after(ms(1200));
        assert_eq!(glyph(&app, w[0]), ("● ".into(), ok));
        app.frame_after(ms(600));
        app.world.agents[w[1]].status = Status::Idle;
        assert_eq!(glyph(&app, w[1]), ("○ ".into(), fg(pal().dim)), "idle");
        waiting(&mut app, w[2]);
        assert_eq!(glyph(&app, w[2]), ("! ".into(), fg(pal().warn)), "waiting");
        // A folded project's loudest member breathes with it.
        app.world.inbox.clear();
        app.world.agents[lead].status = Status::Idle;
        app.collapsed[0] = true;
        let header = salon::project_line(&app, 0, &app.rungs());
        let member = &header.spans[header.spans.len() - 2];
        assert_eq!((member.content.as_ref(), member.style), ("  ● ", out));
        app.frame_after(ms(0));
        let header = salon::project_line(&app, 0, &app.rungs());
        assert_eq!(header.spans[header.spans.len() - 2].style, ok);
    }

    #[test]
    fn a_lit_row_blinks_or_settles_and_the_selection_stays_reversed() {
        use crate::motion::{Glow, GlowOn, Lit, Motion};
        let ms = std::time::Duration::from_millis;
        let reversed = Style::new().add_modifier(Modifier::REVERSED);
        assert_eq!(lit_style(Lit::Reversed), reversed);
        // Settling: the theme's `ok` toward its background, step by step.
        assert!(crate::theme::set("gruvbox-dark-medium"));
        let (Color::Rgb(r, g, b), Color::Rgb(br, bg, bb)) = (pal().ok, pal().bg) else {
            panic!("an RGB theme");
        };
        let shade = |percent| {
            let c = crate::theme::Rgb(br, bg, bb).blend(crate::theme::Rgb(r, g, b), percent);
            Style::new().bg(Color::Rgb(c.0, c.1, c.2))
        };
        assert_eq!(lit_style(Lit::Settle(0)), shade(35));
        assert_eq!(lit_style(Lit::Settle(1)), shade(20));
        assert_eq!(lit_style(Lit::Settle(2)), shade(8));
        assert_eq!(lit_style(Lit::Settle(9)), shade(8), "no step past the last");
        assert_ne!(shade(35), shade(20));
        // A theme of the terminal's own colours: reversed, then calmer.
        assert!(crate::theme::set("terminal"));
        assert_eq!(lit_style(Lit::Settle(0)), reversed);
        assert_eq!(lit_style(Lit::Settle(1)), changed());
        assert_eq!(lit_style(Lit::Settle(2)), changed());
        let (mut app, w, _) = calm_team();
        let row = |app: &App, id: AgentId| {
            salon::agent_line(app, &app.world.agents[id], "", &app.rungs(), 200).style
        };
        app.motion = Motion::Calm;
        app.frame_after(ms(0));
        app.light_now(GlowOn::Agent(w[0]), Glow::Arrive);
        app.light_now(GlowOn::Agent(w[1]), Glow::Settle);
        assert_eq!(row(&app, w[0]), reversed, "a blink");
        assert_eq!(row(&app, w[1]), lit_style(Lit::Settle(0)));
        assert_eq!(row(&app, w[2]), Style::default());
        app.frame_after(ms(300));
        assert_eq!(row(&app, w[0]), Style::default(), "between blinks");
        assert_eq!(row(&app, w[1]), lit_style(Lit::Settle(1)));
        app.world.agents[w[0]].changed_at = app.world.tick;
        assert_eq!(row(&app, w[0]), changed(), "a change shows as before");
        app.frame_after(ms(600));
        assert_eq!(row(&app, w[0]), reversed, "lit over a change");
        app.pane = Pane::Salon;
        app.selected = Sel::Agent(w[0]);
        app.frame_after(ms(300));
        assert_eq!(row(&app, w[0]), reversed, "the selection, not a blink");
        app.motion = Motion::Off;
        app.frame_after(ms(0));
        assert_eq!(row(&app, w[1]), Style::default(), "off: nothing lit");
    }

    #[test]
    fn starting_counts_its_dots_while_motion_moves() {
        use crate::motion::Motion;
        let ms = std::time::Duration::from_millis;
        let (mut app, w, _) = calm_team();
        app.world.agents[w[0]].starting = true;
        let row = |app: &App| {
            let line = salon::agent_line(app, &app.world.agents[w[0]], "", &app.rungs(), 200);
            text(&[line]).remove(0)
        };
        let name = app.world.agents[w[0]].name.clone();
        assert!(row(&app).contains(" starting…"), "{}", row(&app));
        app.motion = Motion::Calm;
        for (at, dots) in [(0, "   "), (600, ".  "), (1200, ".. "), (1800, "...")] {
            app.frame_after(ms(at));
            let r = row(&app);
            assert!(r.contains(&format!(" starting{dots}")), "{at}: {r}");
            assert!(!r.contains('…') && r.contains(&name), "{at}: {r}");
        }
        app.world.agents[w[0]].starting = false;
        assert!(!row(&app).contains("starting"));
    }

    #[test]
    fn the_bottom_line_spins_while_an_answer_is_waited_for_and_full_spins_by_the_fleet() {
        use crate::motion::{Motion, SPIN};
        let ms = std::time::Duration::from_millis;
        let (mut app, _, _) = calm_team();
        let foot = |app: &App| render(app, 160, 30).lines().last().unwrap().to_string();
        let head = |app: &App| render(app, 160, 30).lines().next().unwrap().to_string();
        app.asking = Some("asking claude for a commit message".into());
        assert!(foot(&app).starts_with(" … asking claude"), "{}", foot(&app));
        app.motion = Motion::Calm;
        app.frame_after(ms(300));
        assert!(foot(&app).starts_with(&format!(" {} asking claude", SPIN[1])));
        app.flash = Some(("said".into(), std::time::Instant::now()));
        assert!(foot(&app).starts_with(" said"), "a flash first");
        app.flash = None;
        app.asking = None;
        assert!(!foot(&app).contains("asking"));
        // Full: by the fleet while anyone works.
        assert!(!head(&app).contains(SPIN[1]), "calm: none");
        app.motion = Motion::Full;
        app.frame_after(ms(100));
        assert!(
            head(&app).contains(&format!("{} ", SPIN[1])),
            "{}",
            head(&app)
        );
        let at = head(&app).find(SPIN[1]).unwrap();
        let name = app.world.projects[0].name.clone();
        assert!(head(&app)[at..].contains(&name), "before the projects");
    }

    #[test]
    fn the_first_run_window_spins_while_claude_code_is_checked() {
        use crate::motion::{Motion, SPIN};
        use crate::welcome::Seen;
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.seen = Seen::Checking;
        app.welcome = true;
        assert!(render(&app, 120, 40).contains("… claude"));
        app.motion = Motion::Calm;
        app.frame_after(std::time::Duration::from_millis(600));
        let screen = render(&app, 120, 40);
        assert!(screen.contains(&format!("{} claude", SPIN[2])), "{screen}");
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
            ("● ", fg(pal().ok))
        );
        assert!(text(&line).contains(" starting…"), "{}", text(&line));
        assert!(!text(&line).contains("thinking…"));
        app.lang = Lang::Tr;
        assert!(text(&row(&app)).contains(" başlıyor…"));
        app.world.agents[w[0]].starting = false;
        assert!(text(&row(&app)).contains("düşünüyor…"));
    }

    #[test]
    fn a_bare_colon_solo_opens_the_pick_screen_which_asks_model_and_effort() {
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
            s.contains("Which model?") && s.contains("default (Claude Code's own)"),
            "{s}"
        );
        assert!(s.contains("esc back"), "{s}");
        // Only the chosen row is reversed.
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
        let default = cell(" 1 default");
        assert!(
            default.modifier.contains(Modifier::REVERSED),
            "the chosen row"
        );
        let fable = cell(" 2 fable");
        assert_eq!(fable.fg, pal().strong);
        assert!(!fable.modifier.contains(Modifier::REVERSED), "not chosen");
        // The keys go to the screen, not the salon: j moves its row.
        key(&mut app, KeyCode::Char('j'));
        assert_eq!(app.solo_pick.as_ref().unwrap().sel(), 1);
        key(&mut app, KeyCode::Char('k'));
        assert!(s.contains(" claude"), "what was picked so far: {s}");
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
        assert_eq!(app.model_of(head).as_deref(), Some("@high"));
        // Turkish words too.
        app.lang = Lang::Tr;
        for c in ":solo".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        let s = render(&app, 140, 40);
        assert!(s.contains("Hangi model?"), "{s}");
        key(&mut app, KeyCode::Esc);
        assert!(app.solo_pick.is_none());
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
        app.open_window(Window::Journal);
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
        app.open_window(Window::Journal);
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
        app.window = Some(Window::Board);
        let s = render(&app, 140, 40);
        assert!(s.contains("IN PROGRESS"), "{s}");
        let cards = app.world.shown_cards().count();
        assert!(s.contains(&format!(" BOARD · cards: {cards} ")), "{s}");
        app.window = Some(Window::Code);
        assert!(render(&app, 140, 40).contains("CODE MAP"));
        app.window = None;
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
        assert!(screen.contains("answered: Short first line."), "{screen}");
        app.at_home.at = Some(HomeRow::Item(*app.world.open_inbox().last().unwrap()));
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
    fn a_vendor_error_shows_its_retry_on_the_row_then_waits_for_the_person_in_the_inbox() {
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
        let words = |app: &App, lang| {
            status_spans(lang, &app.world, &app.world.agents[worker])
                .iter()
                .map(|s| s.content.to_string())
                .collect::<String>()
        };
        // A retry 2 of 3, five ticks (25 s) and then 25 ticks off.
        let now = app.world.tick;
        app.world.agents[worker].retry = Some(Retry {
            attempt: 2,
            at: now + 5,
        });
        assert_eq!(words(&app, Lang::En), "↻ 2/3 in 25 s");
        assert_eq!(words(&app, Lang::Tr), "↻ 2/3 · 25 sn sonra");
        app.world.agents[worker].retry = Some(Retry {
            attempt: 3,
            at: now + 25,
        });
        assert_eq!(words(&app, Lang::En), "↻ 3/3 in 2 min 5 s");
        app.world.agents[worker].retry = Some(Retry {
            attempt: 1,
            at: now.saturating_sub(1),
        });
        assert!(
            words(&app, Lang::En).starts_with("↻ 1/3 in 0"),
            "{}",
            words(&app, Lang::En)
        );
        // A mid-turn agent's row is its work, a retry or not.
        app.world.agents[worker].status = Status::Working;
        assert!(!words(&app, Lang::En).contains('↻'));
        app.world.agents[worker].status = Status::Idle;
        // Then the person's: on the row, in the inbox, its keys write to it.
        app.world.agents[worker].retry = None;
        app.world.turn_failed(
            worker,
            "Invalid API key · Fix external API key",
            Some(Fault::Account),
        );
        assert_eq!(
            words(&app, Lang::En),
            "■ API error \"Invalid API key · Fix ext…\"",
            "cut to the status's width"
        );
        assert!(words(&app, Lang::Tr).starts_with("■ API hatası \"Invalid"));
        let i = *app.world.open_inbox().last().unwrap();
        assert_eq!(inbox_line(&app, i).spans[0].content, "■ ");
        assert_eq!(
            words::inbox(Lang::En, &app.world, &app.world.inbox[i]),
            format!(
                "{name} API error, write to it once mended: Invalid API key · Fix external API key"
            )
        );
        app.at_home.at = Some(HomeRow::Item(*app.world.open_inbox().last().unwrap()));
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
        assert!(app.world.open_item(worker, InboxKind::Fault).is_some());
        app.world.said(worker, "Mended.");
        assert!(app.world.open_item(worker, InboxKind::Fault).is_none());
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
            options: Vec::new(),
        });
        app.world.asked_you(worker, "Which API?\nTHE-REST");
        let screen = render(&app, 140, 40);
        assert!(screen.contains("Which API?"), "{screen}");
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
        app.at_home.at = Some(HomeRow::Item(i));
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
            check: Check::None,
            spend: None,
        }
    }

    #[test]
    fn a_change_s_line_ends_with_what_it_cost_once_known() {
        let mut app = app_after(1, 4, 50, Lang::En);
        app.world.inbox.clear();
        app.world.change_ready(a_change(1));
        let open = app.world.open_inbox();
        let line = inbox_line(&app, *open.last().unwrap());
        let last = line.spans.last().unwrap();
        assert!(!last.content.contains('$'), "unsettled: {}", last.content);
        app.world.changes[0].spend = Some(Spend {
            cost: 8_400,
            ticks: 144,
            rounds: 2,
            unpriced: false,
        });
        let line = inbox_line(&app, *open.last().unwrap());
        let last = line.spans.last().unwrap();
        assert_eq!(last.content, " · $0.84 · 12 min · 2 rounds");
        assert_eq!(last.style, dim());
    }

    #[test]
    fn a_change_s_line_starts_with_how_the_check_stands() {
        let mut app = app_after(1, 4, 50, Lang::En);
        app.world.inbox.clear();
        // The repository's check stands before the runs, in its colour;
        // none, or one that could not run, puts nothing there.
        let checked = |ok| {
            Check::Ran(Checked {
                ok,
                end: "exit 0, 1.0 s".into(),
                lines: Vec::new(),
            })
        };
        for (check, mark, style) in [
            (checked(true), "✓ check · ", fg(pal().ok)),
            (checked(false), "✗ check · ", fg(pal().bad)),
            (Check::Running, "● check · ", dim()),
            (Check::Asked, "▶ check · ", dim()),
        ] {
            app.world.change_ready(Change {
                check,
                ..a_change(1)
            });
            let open = app.world.open_inbox();
            let line = inbox_line(&app, *open.last().unwrap());
            let before_last = &line.spans[line.spans.len() - 2];
            assert_eq!(
                (before_last.content.as_ref(), before_last.style),
                (mark, style)
            );
        }
        for check in [Check::None, Check::Failed("x".into())] {
            app.world.change_ready(Change {
                check,
                ..a_change(1)
            });
            let open = app.world.open_inbox();
            let line = inbox_line(&app, *open.last().unwrap());
            let before_last = &line.spans[line.spans.len() - 2];
            assert!(!before_last.content.contains("check"), "{:?}", line.spans);
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
                check: Check::None,
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

    fn spans_text(spans: &[Span<'_>]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn the_header_lets_the_least_wanted_part_go_first_and_never_a_kept_one() {
        let part = |text: &str, glued: bool, keep: u8| HeadPart {
            spans: vec![Span::raw(text.to_string())],
            glued,
            keep,
        };
        let parts = || {
            vec![
                part("AAAA", false, KEEP),
                part("BB", false, 2),
                part("CCC", false, 5),
                part("dd", true, 2),
                part("E", false, 1),
            ]
        };
        let text = |width: usize| spans_text(&fit_header(parts(), width).spans);
        // The brand's eight, twelve of text, three separators of three.
        assert_eq!(text(29), " imece  AAAA │ BB │ CCCdd │ E");
        assert_eq!(text(28), " imece  AAAA │ BB │ CCCdd", "the lowest goes");
        assert_eq!(text(25), " imece  AAAA │ BB │ CCCdd");
        assert_eq!(text(24), " imece  AAAA │ BB │ CCC", "of equals the last");
        assert_eq!(text(23), " imece  AAAA │ BB │ CCC");
        assert_eq!(text(22), " imece  AAAA │ CCC", "with its separator");
        assert_eq!(text(18), " imece  AAAA │ CCC");
        assert_eq!(text(17), " imece  AAAA");
        assert_eq!(text(12), " imece  AAAA", "the brand while it fits");
        assert_eq!(text(11), " AAAA", "then the brand goes, a kept part never");
        assert_eq!(text(3), " AAAA", "a kept part never goes");
    }

    #[test]
    fn the_header_puts_what_waits_first_and_keeps_a_low_pot_and_a_full_window() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let keeps = |app: &App, words: &str| -> Option<(usize, u8)> {
            let parts = header_parts(app);
            (parts.iter().enumerate())
                .find(|(_, p)| spans_text(&p.spans).contains(words))
                .map(|(i, p)| (i, p.keep))
        };
        // Nothing waits, the demo's clock at its own speed; the last ten
        // minutes, the cards, the researchers and the rate are no parts at
        // all (record 125: the header has four things).
        app.world.inbox.clear();
        app.world.max_researchers = 2;
        app.speed = 1;
        let to = app.world.agents[0].id;
        app.world.journal.push(Entry {
            tick: app.world.tick,
            event: Event::Sent {
                from: 1,
                to,
                kind: MsgKind::Inform,
                text: "x".into(),
                id: 1,
                re: None,
            },
        });
        for words in [
            "waiting for you",
            "10 min:",
            "cards",
            "researchers",
            "×",
            "/h",
        ] {
            assert_eq!(keeps(&app, words), None, "{words}");
        }
        app.speed = 2;
        assert_eq!(keeps(&app, "×2").map(|k| k.1), Some(3));
        waiting(&mut app, 1);
        // The projects first, then what is to decide.
        let name = app.world.projects[0].name.clone();
        assert_eq!(keeps(&app, &name), Some((0, 8)), "the projects");
        assert_eq!(keeps(&app, "to decide"), Some((1, KEEP)));
        assert_eq!(keeps(&app, " of $").map(|k| k.1), Some(KEEP));
        let window = |used| Limit {
            minutes: 300,
            used,
            resets_at: None,
        };
        // A window short of loud is the `:budget` note's, not the header's.
        app.world.limits = vec![window(LIMIT_LOUD - 1)];
        assert_eq!(keeps(&app, "claude"), None);
        app.world.limits = vec![window(LIMIT_LOUD)];
        assert_eq!(keeps(&app, "claude").map(|k| k.1), Some(KEEP));
        // A pot running low stays, with how long it lasts.
        for a in &mut app.world.agents {
            a.cost = 0;
        }
        app.world.tick = 1;
        app.world.agents[0].cost = 100;
        app.world.watch_spending();
        app.world.budget = 100 + 299;
        assert!(app.world.pot_low());
        assert_eq!(keeps(&app, " of $").map(|k| k.1), Some(KEEP));
        assert_eq!(keeps(&app, "left").map(|k| k.1), Some(KEEP));
        assert_eq!(keeps(&app, "/h"), None);
        // On a narrow screen the projects go first; what waits and the
        // pot are still there, and the brand goes before they would be
        // cut (record 131).
        let s = render(&app, 70, 20);
        let top = s.lines().next().unwrap();
        assert!(top.starts_with("  ! 1 to decide"), "{top}");
        assert!(top.contains(" of $") && top.contains("10% left"), "{top}");
        assert!(!top.contains(&name) && !top.contains("imece"), "{top}");
        let s = render(&app, 90, 20);
        let top = s.lines().next().unwrap();
        assert!(
            top.starts_with(" imece  ") && top.contains("! 1 to decide"),
            "{top}"
        );
    }

    #[test]
    fn a_line_belongs_to_the_card_its_agent_was_on_then() {
        let (mut app, w) = story_team();
        let mut a = a_card(0, 0, Some(w[0]), Column::Done);
        a.finished = 10;
        let mut b = a_card(1, 0, Some(w[0]), Column::Done);
        b.finished = 20;
        let c = a_card(2, 0, Some(w[0]), Column::Working);
        let waiting = a_card(3, 0, Some(w[0]), Column::Waiting);
        let other = a_card(4, 0, Some(w[1]), Column::Working);
        app.world.cards = vec![a, b, c, waiting, other];
        let held = story::Held::of(&app.world);
        let at = |tick| held.at(&app.world, w[0], tick);
        assert_eq!(
            [at(5), at(10), at(11), at(20), at(21)],
            [Some(0), Some(0), Some(1), Some(1), Some(2)]
        );
        assert_eq!(held.at(&app.world, w[1], 5), Some(4));
        assert_eq!(held.at(&app.world, w[2], 5), None);
        // A helper's is its worker's.
        let (worker, [h, _]) = with_helpers(&mut app);
        let held = story::Held::of(&app.world);
        assert!(held.at(&app.world, worker, 5).is_some());
        assert_eq!(held.at(&app.world, h, 5), held.at(&app.world, worker, 5));
    }

    fn a_card(id: CardId, project: ProjectId, owner: Option<AgentId>, column: Column) -> Card {
        Card {
            id,
            project,
            title: format!("card {}", id + 1),
            owner,
            column,
            story: Vec::new(),
            symbols: Vec::new(),
            paths: Vec::new(),
            checks: Vec::new(),
            started: 0,
            finished: 0,
            attempt: 1,
            breaker: Default::default(),
            after: Vec::new(),
        }
    }

    fn sent(from: AgentId, to: AgentId, kind: MsgKind, id: MsgId, re: Option<MsgId>) -> Event {
        Event::Sent {
            from,
            to,
            kind,
            text: format!("m{id}"),
            id,
            re,
        }
    }

    /// A calm team with no journal, no inbox, no cards, at tick 1000.
    fn story_team() -> (App, Vec<AgentId>) {
        let (mut app, w, _) = calm_team();
        app.world.journal.clear();
        app.world.inbox.clear();
        app.world.cards.clear();
        app.world.tick = 1000;
        for &a in &w {
            app.world.agents[a].card = None;
            app.world.agents[a].plan.clear();
        }
        (app, w)
    }

    #[test]
    fn what_was_said_goes_to_the_asker_s_card_and_setting_up_to_none() {
        let (mut app, w) = story_team();
        app.world.cards = vec![
            a_card(0, 0, Some(w[0]), Column::Working),
            a_card(1, 0, Some(w[1]), Column::Working),
        ];
        let held = story::Held::of(&app.world);
        let lead = app.world.projects[0].leader;
        let of = |event: Event| story::card_for(&app.world, &held, &Entry { tick: 1000, event });
        assert_eq!(
            of(Event::Claimed {
                agent: w[1],
                card: 0
            }),
            Some(0),
            "named"
        );
        assert_eq!(of(sent(w[0], w[1], MsgKind::Ask, 3, None)), Some(0));
        assert_eq!(of(sent(w[1], w[0], MsgKind::Answer, 4, Some(3))), Some(0));
        assert_eq!(of(sent(lead, w[1], MsgKind::Inform, 5, None)), Some(1));
        assert_eq!(of(sent(w[1], lead, MsgKind::Answer, 6, Some(5))), Some(1));
        assert_eq!(of(sent(lead, w[3], MsgKind::Inform, 7, None)), None);
        assert_eq!(
            of(Event::Replied {
                agent: w[1],
                text: "x".into()
            }),
            Some(1),
            "to the person"
        );
        assert_eq!(
            of(Event::ChangeReady {
                agent: w[1],
                files: 1,
                added: 1,
                removed: 0,
                spend: None
            }),
            Some(1)
        );
        assert_eq!(
            of(Event::Stopped {
                agent: w[0],
                why: "x".into()
            }),
            Some(0)
        );
        assert_eq!(of(Event::Failed { agent: w[0] }), Some(0));
        assert_eq!(
            of(Event::Copied {
                agent: w[0],
                paths: vec![".env".into()],
                bytes: 1,
                included: false
            }),
            None
        );
        assert_eq!(
            of(Event::CopySkipped {
                agent: w[0],
                path: ".env".into(),
                why: "x".into(),
                included: false
            }),
            None,
            "a failure of setting up is no card's"
        );
    }

    #[test]
    fn the_person_holds_up_who_waits_for_them_and_who_waits_for_those() {
        let (mut app, w) = story_team();
        assert!(story::held(&app.world).is_empty());
        waiting(&mut app, w[2]);
        waiting(&mut app, w[2]);
        app.world.agents[w[3]].status = Status::WaitingHuman;
        app.world.agents[w[0]].status = Status::Asking { to: w[1], until: 0 };
        app.world.agents[w[1]].status = Status::Asking { to: w[2], until: 0 };
        assert_eq!(
            story::held(&app.world),
            [
                (w[2], None),
                (w[3], None),
                (w[1], Some(w[2])),
                (w[0], Some(w[1]))
            ]
        );
        app.world.agents[w[3]].gone = true;
        assert!(!story::held(&app.world).contains(&(w[3], None)));
        // Asking one nobody holds up holds nobody up.
        let lead = app.world.projects[0].leader;
        app.world.agents[lead].status = Status::Asking { to: w[3], until: 0 };
        assert!(story::held(&app.world).iter().all(|&(a, _)| a != lead));
        app.story = true;
        app.open_window(Window::Journal);
        let s = render(&app, 240, 50);
        assert!(
            s.contains(&format!(
                "! needs you (3): {}, {} via {}, {} via {}",
                app.world.agents[w[2]].name,
                app.world.agents[w[1]].name,
                app.world.agents[w[2]].name,
                app.world.agents[w[0]].name,
                app.world.agents[w[1]].name
            )),
            "{s}"
        );
    }

    /// The screen with the story in the side panel, wide enough for its lines.
    fn render_story(app: &mut App) -> String {
        app.open_window(Window::Journal);
        assert!(app.story_on());
        render(app, 200, 50)
    }

    #[test]
    fn the_story_tells_each_card_newest_first_and_folds_the_rest_by_kind() {
        let (mut app, w) = story_team();
        app.story = true;
        let mut done = a_card(0, 0, Some(w[0]), Column::Done);
        done.finished = 995;
        app.world.cards = vec![done, a_card(1, 0, Some(w[1]), Column::Working)];
        app.world.agents[w[1]].card = Some(1);
        plan_of(&mut app, w[1]);
        let lead = app.world.projects[0].leader;
        let at = |tick, event| Entry { tick, event };
        let copied = |agent| Event::Copied {
            agent,
            paths: vec!["node_modules".into()],
            bytes: 1,
            included: false,
        };
        app.world.journal = vec![
            at(900, sent(w[0], lead, MsgKind::Progress, 1, None)),
            at(950, sent(w[0], lead, MsgKind::Progress, 2, None)),
            at(960, sent(w[0], lead, MsgKind::Progress, 3, None)),
            at(990, sent(w[0], lead, MsgKind::Done, 4, None)),
            at(991, copied(w[2])),
            at(992, copied(w[3])),
            at(976, sent(w[1], w[3], MsgKind::Ask, 6, None)),
            at(999, sent(w[1], w[2], MsgKind::Ask, 7, None)),
            at(1000, sent(w[2], w[1], MsgKind::Answer, 8, Some(7))),
        ];
        // The journal is in time order; the asks of card 2 came last.
        app.world.journal.sort_by_key(|e| e.tick);
        let s = render_story(&mut app);
        let at_line = |words: &str| s.lines().position(|l| l.contains(words));
        let two = at_line("#2 card 2").expect("card 2");
        let one = at_line("#1 card 1").expect("card 1");
        assert!(two < one, "newest first: {s}");
        assert!(s.contains("#1 card 1 ·") && s.contains("✓ done"), "{s}");
        // Three newest lines, oldest first; the oldest left out.
        assert!(!s.contains("[progress]: m1"), "{s}");
        let m2 = at_line("[progress]: m2").unwrap();
        let m4 = at_line("[done]: m4").unwrap();
        assert!(one < m2 && m2 < m4, "{s}");
        // An ask answered says nothing more; one not: since when.
        let ask = |id: &str| {
            s.lines()
                .find(|l| l.contains(&format!("[ask]: {id}")))
                .unwrap_or("")
                .to_string()
        };
        assert!(!ask("m7").contains("no answer"), "{s}");
        assert!(ask("m6").contains("no answer · 2 min"), "{s}");
        // Its owner now, and how far its list is.
        assert!(s.contains("#2 card 2 ·") && s.contains("≡ 1/4"), "{s}");
        assert!(s.contains("now ▸ Run the tests"), "{s}");
        // The rest by kind, the newest of each with how many.
        let elsewhere = at_line("elsewhere").expect("elsewhere");
        assert!(elsewhere > one, "{s}");
        assert!(
            s.lines()
                .any(|l| l.contains(&app.world.agents[w[3]].name) && l.contains("×2")),
            "{s}"
        );
        // A change waiting on a done card says both.
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Change,
            agent: w[0],
            detail: "x".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: Some(0),
        });
        assert!(render_story(&mut app).contains("✓ done · change ready"));
        // Cleared lines are not told, a marked one still is.
        app.world.inbox.clear();
        app.world.cleared.journal = app.world.journal.len();
        let s = render_story(&mut app);
        assert!(s.contains("Nothing has happened yet"), "{s}");
        app.world.marks.insert(app.world.journal.len() - 1);
        assert!(render_story(&mut app).contains("[answer]"));
        // Cleared up to the last line: the last is still told.
        app.world.marks.clear();
        app.world.cleared.journal = app.world.journal.len() - 1;
        let s = render_story(&mut app);
        assert!(
            s.contains("[answer] ↩: m8") && !s.contains("[ask]: m7"),
            "{s}"
        );
        app.world.cleared.journal = 0;
        // An archived project's lines are its archive's.
        app.world.projects[0].archived = Some(Archived {
            at: 0,
            title: "old".into(),
        });
        assert!(render_story(&mut app).contains("Nothing has happened yet"));
    }

    #[test]
    fn the_story_marks_only_numbered_asks_folds_four_kinds_and_says_a_change() {
        let (mut app, w) = story_team();
        app.story = true;
        app.world.cards = vec![a_card(0, 0, Some(w[0]), Column::Working)];
        app.world.agents[w[0]].card = Some(0);
        let lead = app.world.projects[0].leader;
        let at = |tick, event| Entry { tick, event };
        app.world.journal = vec![
            // An ask from before numbers (0) is not one that waits.
            at(990, sent(w[0], w[1], MsgKind::Ask, 0, None)),
            // One under a minute old waits "yet".
            at(996, sent(w[0], w[1], MsgKind::Ask, 9, None)),
            at(991, Event::Failed { agent: lead }),
            at(
                992,
                Event::Stopped {
                    agent: lead,
                    why: "x".into(),
                },
            ),
            at(
                993,
                Event::Copied {
                    agent: w[2],
                    paths: vec!["x".into()],
                    bytes: 1,
                    included: false,
                },
            ),
            at(
                994,
                Event::Hired {
                    by: lead,
                    agent: w[3],
                    model: None,
                    from: None,
                },
            ),
            at(
                995,
                Event::Retrying {
                    agent: w[2],
                    why: "x".into(),
                    attempt: 1,
                    seconds: 30,
                },
            ),
        ];
        let s = render_story(&mut app);
        let ask = s.lines().find(|l| l.contains("[ask]: m0")).unwrap();
        assert!(!ask.contains("no answer"), "{s}");
        let ask = s.lines().find(|l| l.contains("[ask]: m9")).unwrap();
        assert!(ask.contains("no answer yet"), "{s}");
        let start = s.lines().position(|l| l.contains("elsewhere")).unwrap();
        let folded = (s.lines().skip(start + 1))
            .take_while(|l| l.contains(":") && l.contains("│  1"))
            .count();
        assert_eq!(folded, story::STORY_OTHER, "{s}");
        assert!(!s.contains("×1"), "{s}");
        // Another card's change, or this card's item that is no change,
        // says nothing on its header.
        plan_of(&mut app, w[0]);
        for (kind, card) in [(InboxKind::Change, Some(1)), (InboxKind::Reply, Some(0))] {
            app.world.inbox = vec![InboxItem {
                kind,
                agent: w[0],
                detail: "x".into(),
                researcher: None,
                open: true,
                tick: 0,
                card,
            }];
            let s = render_story(&mut app);
            // The window's line, under Home's preview that names the card.
            let head = s.lines().rev().find(|l| l.contains("#1 card 1")).unwrap();
            assert!(!head.contains("Δ") && head.contains('≡'), "{head}");
        }
        // A done card says done, not its owner's list.
        app.world.inbox.clear();
        app.world.cards[0].column = Column::Done;
        app.world.cards[0].finished = 1000;
        let s = render_story(&mut app);
        let head = s.lines().rev().find(|l| l.contains("#1 card 1")).unwrap();
        assert!(head.contains("✓ done") && !head.contains('≡'), "{head}");
        // A failed one says so, and its owner has no line of now.
        app.world.cards[0].column = Column::Failed;
        let s = render_story(&mut app);
        let head = s.lines().rev().find(|l| l.contains("#1 card 1")).unwrap();
        assert!(
            head.contains("✕ failed") && !head.contains("✓ done"),
            "{head}"
        );
        assert!(!head.contains('≡') && !s.contains("now ▸"), "{s}");
        // A withdrawn one says so, dim (record 126).
        app.world.cards[0].column = Column::Withdrawn;
        let s = render_story(&mut app);
        let head = s.lines().rev().find(|l| l.contains("#1 card 1")).unwrap();
        assert!(
            head.contains("– withdrawn") && !head.contains("✕"),
            "{head}"
        );
        app.world.cards[0].column = Column::Working;
        // A change waiting on a card not done: that, not its list; the
        // owner off the card has no line of now.
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Change,
            agent: w[0],
            detail: "x".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: Some(0),
        });
        let s = render_story(&mut app);
        let head = s.lines().rev().find(|l| l.contains("#1 card 1")).unwrap();
        assert!(
            head.contains(" ·  change ready") || head.contains("  change ready"),
            "{head}"
        );
        assert!(!head.contains('≡') && !head.contains("✓ done"), "{head}");
        assert!(s.contains("now ▸ Run the tests"), "{s}");
        app.world.agents[w[0]].card = None;
        assert!(!render_story(&mut app).contains("now ▸"));
    }

    #[test]
    fn j_tells_the_story_and_its_keys_move_and_open_what_it_shows() {
        let (mut app, w) = story_team();
        app.world.cards = vec![
            a_card(0, 0, Some(w[0]), Column::Working),
            a_card(1, 0, Some(w[1]), Column::Working),
        ];
        app.world.agents[w[0]].card = Some(0);
        app.world.agents[w[1]].card = Some(1);
        let at = |tick, event| Entry { tick, event };
        app.world.journal = vec![
            at(
                990,
                Event::Copied {
                    agent: w[2],
                    paths: vec!["x".into()],
                    bytes: 1,
                    included: false,
                },
            ),
            at(995, sent(w[0], w[1], MsgKind::Inform, 1, None)),
            at(999, sent(w[1], w[0], MsgKind::Inform, 2, None)),
        ];
        assert!(!app.story);
        // `J` opens the journal's window; `J` there tells the story.
        key(&mut app, KeyCode::Char('J'));
        assert_eq!(app.window, Some(Window::Journal));
        assert!(!app.story);
        key(&mut app, KeyCode::Char('J'));
        assert!(app.story);
        let s = render(&app, 200, 50);
        assert!(s.contains("STORY · J journal"), "{s}");
        assert!(s.contains("the journal tells the story by card"), "{s}");
        // The window's pane is the journal's; no journal line is selected
        // under the story.
        assert_eq!((app.pane, app.journal_sel), (Pane::Journal, None));
        // Card 2, card 1, then the line of no card.
        render(&app, 200, 50);
        assert_eq!(app.story_items.borrow().len(), 3);
        key(&mut app, KeyCode::Char('j'));
        key(&mut app, KeyCode::Char('j'));
        key(&mut app, KeyCode::Char('j'));
        assert_eq!(app.story_sel, 2, "the last stays the last");
        key(&mut app, KeyCode::Char('k'));
        assert_eq!(app.story_sel, 1);
        let s = render(&app, 200, 50);
        let cell = |words: &str| {
            let y = s
                .lines()
                .collect::<Vec<_>>()
                .into_iter()
                .rposition(|l| l.contains(words))
                .unwrap();
            let x = s
                .lines()
                .nth(y)
                .unwrap()
                .chars()
                .position(|c| c == '#')
                .unwrap();
            buffer(&app, 200, 50)[(x as u16, y as u16)].modifier
        };
        assert!(cell("#1 card 1").contains(Modifier::REVERSED), "{s}");
        assert!(!cell("#2 card 2").contains(Modifier::REVERSED), "{s}");
        key(&mut app, KeyCode::Char('g'));
        assert_eq!(app.story_sel, 0);
        key(&mut app, KeyCode::Char('G'));
        assert_eq!(app.story_sel, 2);
        key(&mut app, KeyCode::Char('k'));
        // The agent at hand is the card's owner, or the line's agent; the
        // card at hand the item's.
        assert_eq!(app.agent_at_hand(), Some(w[0]));
        app.story_sel = 2;
        assert_eq!(app.agent_at_hand(), Some(w[2]));
        assert_eq!(app.card_at_hand(), None, "a line of no card");
        app.story_sel = 1;
        assert_eq!(app.card_at_hand(), Some(0));
        // enter on a card: its detail, the window closed; `J` there opens
        // the window again, the story still on.
        key(&mut app, KeyCode::Enter);
        assert_eq!((app.card, app.window), (Some(0), None));
        assert!(!app.story_on(), "a card's screen is its own");
        key(&mut app, KeyCode::Char('J'));
        assert_eq!(app.window, Some(Window::Journal));
        assert!(app.story);
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.window, None);
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.card, None);
        // enter on a line of no card: the journal, at that line, in the
        // story's place; what is kept says so.
        key(&mut app, KeyCode::Char('J'));
        render(&app, 200, 50);
        app.story_sel = 2;
        key(&mut app, KeyCode::Enter);
        assert!(app.story && app.story_away && !app.story_on());
        assert_eq!(app.journal_sel, Some(0));
        let shown = render(&app, 200, 50);
        assert!(shown.contains("JOURNAL · esc story"), "{shown}");
        assert!(shown.contains("the line in the journal · esc: back to the story"));
        // esc goes back to the story, at the item it left; the window stays.
        key(&mut app, KeyCode::Esc);
        assert!(app.story_on() && !app.story_away);
        assert_eq!(app.window, Some(Window::Journal));
        assert_eq!((app.journal_sel, app.story_sel), (None, 2));
        assert!(render(&app, 200, 50).contains("the journal tells the story by card"));
        // So does J, and it leaves the story on.
        key(&mut app, KeyCode::Enter);
        assert!(app.story_away);
        key(&mut app, KeyCode::Char('J'));
        assert!(app.story_on() && !app.story_away);
        // A second esc closes the window; a focus under it is untouched.
        app.zoom = Zoom::Focus(w[0]);
        key(&mut app, KeyCode::Enter);
        assert!(app.story_away);
        key(&mut app, KeyCode::Esc);
        assert!(!app.story_away && app.window.is_some());
        key(&mut app, KeyCode::Esc);
        assert_eq!((app.window, app.zoom), (None, Zoom::Focus(w[0])));
        app.zoom = Zoom::None;
        // :story opens the window and turns the story off and on.
        app.command("story");
        assert_eq!(app.window, Some(Window::Journal));
        assert!(!app.story);
        app.command("story");
        assert!(app.story);
        assert_eq!((app.journal_sel, app.story_sel), (None, 0));
        // Off again: a journal line is selected, and j k move it, not the
        // story's.
        key(&mut app, KeyCode::Char('J'));
        assert!(!app.story);
        assert!(app.journal_sel.is_some());
        let before = app.journal_sel;
        key(&mut app, KeyCode::Char('j'));
        assert_ne!(app.journal_sel, before);
        assert_eq!(app.story_sel, 0);
        let shown = render(&app, 200, 50);
        assert!(shown.contains("the journal line by line"));
        assert!(shown.contains("JOURNAL · J story"), "the key back, off");
        app.flash = None;
        // In Turkish too.
        app.lang = Lang::Tr;
        assert!(render(&app, 200, 50).contains("DEFTER · J hikâye"));
        app.story_away = true;
        assert!(render(&app, 200, 50).contains("DEFTER · esc hikâye"));
    }

    #[test]
    fn the_story_keeps_the_selected_card_in_view() {
        let (mut app, w) = story_team();
        app.story = true;
        app.pane = Pane::Journal;
        let n = 20;
        app.world.cards = (0..n)
            .map(|c| a_card(c, 0, Some(w[c % 4]), Column::Done))
            .collect();
        // Two lines a card: a block takes three rows.
        app.world.journal = (0..n)
            .flat_map(|c| {
                let (tick, agent) = (900 + c as u64, w[c % 4]);
                [
                    Entry {
                        tick,
                        event: Event::Claimed { agent, card: c },
                    },
                    Entry {
                        tick,
                        event: Event::Merged { agent, card: c },
                    },
                ]
            })
            .collect();
        app.open_window(Window::Journal);
        render(&app, 200, 30);
        let shown = app.story_items.borrow().len();
        // The window's 24 rows hold half as many cards and one more.
        assert_eq!(shown, 24 / 2 + 1);
        app.story_sel = shown - 1;
        let s = render(&app, 200, 30);
        let last = match app.story_items.borrow()[shown - 1] {
            story::Item::Card(c) => c,
            story::Item::Line(_) => unreachable!(),
        };
        assert!(
            s.contains(&format!("#{} card {}", last + 1, last + 1)),
            "{s}"
        );
        assert!(!s.contains("#20 card 20 ·"), "the first scrolled off: {s}");
    }

    use crate::model::{LANE_TICKS, Lane, LaneCell};

    #[test]
    fn a_lane_keeps_the_most_of_a_tick_fills_a_jump_and_forgets_past_an_hour() {
        use LaneCell::*;
        let mut lane = Lane::default();
        assert_eq!(lane.last(), Gap);
        assert_eq!(lane.at(10, 3), [Gap, Gap, Gap], "nothing seen yet");
        lane.mark(10, Reading);
        lane.mark(10, Thinking);
        assert_eq!(lane.last(), Reading, "the most of a tick");
        lane.mark(10, Writing);
        assert_eq!(lane.last(), Writing);
        // A jump of three ticks: the two between take what was last.
        lane.mark(13, Asking);
        assert_eq!(lane.at(13, 5), [Gap, Writing, Writing, Writing, Asking]);
        // An older tick counts as the newest.
        lane.mark(12, Stuck);
        assert_eq!(lane.at(13, 2), [Writing, Stuck]);
        // A turn's end is one tick: the ticks after it are idle.
        lane.mark(14, Done);
        lane.mark(17, Thinking);
        assert_eq!(lane.at(17, 4), [Done, Idle, Idle, Thinking]);
        // Ticks after the newest seen, and before tick 0, are gaps.
        assert_eq!(lane.at(19, 3), [Thinking, Gap, Gap]);
        assert_eq!(Lane::default().at(1, 3), [Gap, Gap, Gap]);
        let mut early = Lane::default();
        early.mark(1, Idle);
        assert_eq!(early.at(1, 4), [Gap, Gap, Gap, Idle]);
        // An hour is kept, no more.
        let mut long = Lane::default();
        long.mark(0, Writing);
        long.mark(LANE_TICKS as u64, Reading);
        let all = long.at(LANE_TICKS as u64, LANE_TICKS + 1);
        assert_eq!(all[0], Gap, "the first tick is gone");
        assert_eq!(all[1], Writing);
        assert_eq!(all[LANE_TICKS], Reading);
        // A jump of more than an hour keeps an hour of what was last.
        long.mark(10 * LANE_TICKS as u64, Idle);
        let all = long.at(10 * LANE_TICKS as u64, LANE_TICKS);
        assert_eq!((all[0], all[LANE_TICKS - 1]), (Reading, Idle));
        // In a turn: starting, thinking, reading, writing, asking.
        let busy: Vec<bool> = [
            Gap, Idle, Starting, Thinking, Reading, Writing, Asking, Done, Waiting, Stuck,
        ]
        .iter()
        .map(|c| c.busy())
        .collect();
        assert_eq!(
            busy,
            [
                false, false, true, true, true, true, true, false, false, false
            ]
        );
    }

    #[test]
    fn a_lane_says_what_the_agent_does_in_a_turn_and_waiting_only_out_of_one() {
        use crate::app::{cell_of, writes};
        let (mut app, w, _) = calm_team();
        app.world.inbox.clear();
        let a = w[0];
        let cell = |app: &App, waits| cell_of(&app.world, &app.world.agents[a], waits);
        let set = |app: &mut App, status: Status, doing: Option<&str>| {
            app.world.agents[a].status = status;
            app.world.agents[a].activity = doing.map(str::to_string);
        };
        set(&mut app, Status::Working, None);
        assert_eq!(cell(&app, true), LaneCell::Thinking, "in a turn, its work");
        set(&mut app, Status::Working, Some(""));
        assert_eq!(cell(&app, false), LaneCell::Thinking);
        set(&mut app, Status::Working, Some("Edit src/x.rs"));
        assert_eq!(cell(&app, false), LaneCell::Writing);
        set(&mut app, Status::Working, Some("Read src/x.rs"));
        assert_eq!(cell(&app, false), LaneCell::Reading);
        app.world.agents[a].starting = true;
        assert_eq!(cell(&app, false), LaneCell::Starting);
        app.world.agents[a].starting = false;
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Approval,
            agent: a,
            detail: "cargo test".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        });
        assert_eq!(cell(&app, true), LaneCell::Waiting, "a yes it waits for");
        app.world.inbox.clear();
        set(&mut app, Status::Asking { to: w[1], until: 0 }, None);
        assert_eq!(cell(&app, true), LaneCell::Asking);
        set(
            &mut app,
            Status::Researching {
                for_agent: w[1],
                until: 0,
            },
            None,
        );
        assert_eq!(cell(&app, true), LaneCell::Reading);
        set(&mut app, Status::Blocked, None);
        assert_eq!(cell(&app, false), LaneCell::Stuck);
        set(&mut app, Status::Stopped, None);
        assert_eq!(cell(&app, false), LaneCell::Stuck);
        set(&mut app, Status::WaitingHuman, None);
        assert_eq!(cell(&app, false), LaneCell::Waiting);
        for status in [Status::Idle, Status::Asleep] {
            set(&mut app, status, None);
            app.world.agents[a].lane = Lane::default();
            assert_eq!(
                cell(&app, true),
                LaneCell::Waiting,
                "out of a turn, what waits"
            );
            assert_eq!(cell(&app, false), LaneCell::Idle);
            app.world.agents[a].lane.mark(1, LaneCell::Reading);
            assert_eq!(cell(&app, false), LaneCell::Done, "the tick a turn ended");
        }
        for (doing, yes) in [
            ("Edit a", true),
            ("Write a", true),
            ("MultiEdit a", true),
            ("NotebookEdit a", true),
            ("Read a", false),
            ("run cargo test", false),
            ("Editor a", false),
            ("", false),
        ] {
            assert_eq!(writes(doing), yes, "{doing}");
        }
    }

    #[test]
    fn every_step_notes_each_agent_in_its_lane_but_a_gone_one() {
        let (mut app, w, _) = calm_team();
        app.world.inbox.clear();
        app.world.agents[w[0]].activity = Some("Edit x".into());
        app.world.agents[w[1]].gone = true;
        app.step();
        let t = app.world.tick;
        assert_eq!(app.world.agents[w[0]].lane.at(t, 1)[0], LaneCell::Writing);
        assert_eq!(app.world.agents[w[1]].lane.last(), LaneCell::Gap);
        waiting(&mut app, w[2]);
        app.world.agents[w[2]].status = Status::Idle;
        app.watch_lanes();
        let t = app.world.tick;
        assert_eq!(app.world.agents[w[2]].lane.at(t, 1)[0], LaneCell::Waiting);
    }

    #[test]
    fn columns_keep_the_most_of_their_ticks_and_messages_count_by_column() {
        use LaneCell::*;
        let mut lane = Lane::default();
        for (t, c) in [
            (1, Idle),
            (2, Writing),
            (3, Idle),
            (4, Thinking),
            (5, Idle),
            (6, Idle),
        ] {
            lane.mark(t, c);
        }
        assert_eq!(timeline::columns(&lane, 6, 3, 2), [Writing, Thinking, Idle]);
        assert_eq!(timeline::columns(&lane, 6, 2, 1), [Idle, Idle]);
        let (mut app, w, _) = calm_team();
        app.world.tick = 100;
        let lead = app.world.projects[0].leader;
        let at = |tick, from| Entry {
            tick,
            event: sent(from, lead, MsgKind::Inform, 1, None),
        };
        app.world.journal = vec![
            at(93, w[0]),
            at(94, w[0]),
            at(95, w[1]),
            at(95, w[1]),
            at(99, w[2]),
            at(100, w[3]),
            at(100, w[3]),
            at(100, w[3]),
        ];
        // Three columns of two ticks: 95-96, 97-98, 99-100.
        assert_eq!(timeline::messages(&app.world, 0, 100, 3, 2), [2, 0, 4]);
        assert_eq!(timeline::messages(&app.world, 0, 100, 4, 2), [2, 2, 0, 4]);
        assert_eq!(timeline::messages(&app.world, 1, 100, 3, 2), [0, 0, 0]);
        assert_eq!(timeline::messages(&app.world, 0, 100, 1, 1), [3]);
    }

    #[test]
    fn a_lane_s_width_leaves_room_for_the_name_and_the_status_or_drops_the_status() {
        // Name 20, lane at least 10, status 28, a space each side.
        assert_eq!(timeline::lane_width(60), (10, true));
        assert_eq!(timeline::lane_width(100), (50, true));
        assert_eq!(timeline::lane_width(59), (38, false));
        assert_eq!(timeline::lane_width(30), (9, false));
        assert_eq!(timeline::lane_width(5), (1, false));
        let glyphs: String = [
            LaneCell::Gap,
            LaneCell::Idle,
            LaneCell::Starting,
            LaneCell::Thinking,
            LaneCell::Reading,
            LaneCell::Writing,
            LaneCell::Asking,
            LaneCell::Done,
            LaneCell::Waiting,
            LaneCell::Stuck,
        ]
        .iter()
        .map(|&c| timeline::cell_glyph(c).0)
        .collect();
        assert_eq!(glyphs, " ·●░▒█?✓⚑■");
    }

    #[test]
    fn the_timeline_shows_each_open_project_s_agents_over_time_and_its_keys_work() {
        let mut app = app_after(2, 2, 30, Lang::En);
        let (worker, [h1, h2]) = with_helpers(&mut app);
        let p = app.world.agents[worker].project;
        let lead = app.world.projects[p].leader;
        let shown = timeline::agents(&app);
        let at = shown.iter().position(|&(id, _)| id == worker).unwrap();
        assert_eq!(&shown[at..at + 3], [(worker, 0), (h1, 1), (h2, 1)]);
        assert_eq!(shown.iter().filter(|&&(id, _)| id == h1).count(), 1, "once");
        assert!(shown.contains(&(lead, 0)));
        // Gone, settled, of a closed project: not there.
        app.world.agents[h2].gone = true;
        app.settled.insert(h1);
        assert!(
            !timeline::agents(&app)
                .iter()
                .any(|&(id, _)| id == h1 || id == h2)
        );
        let other = 1 - p;
        app.world.projects[other].closed = true;
        assert!(
            timeline::agents(&app)
                .iter()
                .all(|&(id, _)| app.world.agents[id].project == p)
        );
        app.world.projects[other].closed = false;
        // A project put out of the salon is not there either, open as it is.
        app.settled_projects.insert(other);
        assert!(
            timeline::agents(&app)
                .iter()
                .all(|&(id, _)| app.world.agents[id].project == p)
        );
        app.settled_projects.clear();
        assert!(
            timeline::agents(&app)
                .iter()
                .any(|&(id, _)| app.world.agents[id].project == other)
        );
        app.settled.clear();
        app.world.agents[h2].gone = false;
        // `]` in the salon is a panel, not the scale.
        key(&mut app, KeyCode::Char(']'));
        assert_eq!(app.timeline_scale, 0);
        for c in ":timeline".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.window, Some(Window::Timeline));
        assert_eq!(app.key_scope(), Some(crate::keys::Scope::Timeline));
        let s = render(&app, 160, 40);
        assert!(s.contains("TIMELINE · a column is 5 s"), "{s}");
        assert!(s.contains("█ writing"), "the legend: {s}");
        assert!(s.contains(" now"), "{s}");
        assert!(s.contains("  messages"), "{s}");
        assert!(
            s.lines().last().unwrap().trim_start().starts_with("[ ]"),
            "the footer: {s}"
        );
        let name = app.world.agents[h1].name.clone();
        let row = s.lines().find(|l| l.contains(&name)).unwrap();
        assert!(
            row.contains(&format!("  {name}")),
            "a helper is indented: {row}"
        );
        // The scale, held within its bounds.
        key(&mut app, KeyCode::Char(']'));
        assert!(render(&app, 160, 40).contains("a column is 30 s"));
        key(&mut app, KeyCode::Char(']'));
        key(&mut app, KeyCode::Char(']'));
        assert_eq!(app.timeline_scale, 2);
        assert!(render(&app, 160, 40).contains("a column is 1 min"));
        key(&mut app, KeyCode::Char('['));
        key(&mut app, KeyCode::Char('['));
        key(&mut app, KeyCode::Char('['));
        assert_eq!(app.timeline_scale, 0);
        // j k gg G, the selected row reversed; enter its focus, K a peek.
        let n = timeline::agents(&app).len();
        key(&mut app, KeyCode::Char('j'));
        assert_eq!(app.timeline_sel, 1);
        key(&mut app, KeyCode::Char('G'));
        assert_eq!(app.timeline_sel, n - 1);
        key(&mut app, KeyCode::Char('j'));
        assert_eq!(app.timeline_sel, n - 1, "the last stays the last");
        key(&mut app, KeyCode::Char('g'));
        key(&mut app, KeyCode::Char('g'));
        assert_eq!(app.timeline_sel, 0);
        key(&mut app, KeyCode::Char('k'));
        assert_eq!(app.timeline_sel, 0);
        app.timeline_sel = at;
        let s = render(&app, 160, 40);
        let wname = app.world.agents[worker].name.clone();
        // The lane, under Home's preview that names the worker too.
        let y = s
            .lines()
            .collect::<Vec<_>>()
            .into_iter()
            .rposition(|l| l.contains(&format!(" {wname} ")))
            .unwrap();
        let x = s
            .lines()
            .nth(y)
            .unwrap()
            .chars()
            .position(|c| c == '▎' || c.is_alphanumeric())
            .unwrap();
        assert!(
            buffer(&app, 160, 40)[(x as u16, y as u16)]
                .modifier
                .contains(Modifier::REVERSED)
        );
        let y2 = s
            .lines()
            .position(|l| l.contains(&format!("  {name}")))
            .unwrap();
        assert!(
            !buffer(&app, 160, 40)[(x as u16, y2 as u16)]
                .modifier
                .contains(Modifier::REVERSED)
        );
        // The window takes `K`; `enter` opens the focus in its place.
        key(&mut app, KeyCode::Char('K'));
        assert_eq!(app.peek, None);
        key(&mut app, KeyCode::Enter);
        assert_eq!((app.window, app.zoom), (None, Zoom::Focus(worker)));
        key(&mut app, KeyCode::Esc);
        assert_eq!((app.zoom), (Zoom::None));
        // A narrow screen: the status goes; Turkish words.
        app.window = Some(Window::Timeline);
        app.lang = Lang::Tr;
        let s = render(&app, 55, 30);
        assert!(s.contains("ZAMAN ŞERİDİ · bir sütun 5 sn"), "{s}");
        assert!(!s.contains("koordine ediyor"), "{s}");
        // :timeline typed from the salon.
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.window, None);
        for c in ":timeline".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.window, Some(Window::Timeline));
    }

    #[test]
    fn the_timeline_keeps_the_selected_agent_in_view() {
        let mut app = app_after(1, 12, 0, Lang::En);
        app.window = Some(Window::Timeline);
        let n = timeline::agents(&app).len();
        app.timeline_sel = n - 1;
        let s = render(&app, 120, 14);
        let last = timeline::agents(&app)[n - 1].0;
        assert!(s.contains(&app.world.agents[last].name), "{s}");
        let first = timeline::agents(&app)[0].0;
        assert!(
            !s.contains(&format!(" {} ", app.world.agents[first].name)),
            "{s}"
        );
        app.world.projects[0].closed = true;
        assert!(render(&app, 120, 14).contains("No agent to show."));
    }

    #[test]
    fn home_shows_imece_s_mark_while_no_project_has_a_worker() {
        let mut app = App::new(1, 0, 4, 7, Lang::En);
        let screen = render(&app, 120, 30);
        assert!(app.mark_drawn.get());
        let at = screen.lines().position(|l| l.contains("imece v")).unwrap();
        let rows: Vec<&str> = screen.lines().skip(at).take(4).collect();
        assert!(rows[0].contains("     ██        imece v"), "{screen}");
        assert!(rows[1].contains(" ██  ▄▄  ██    one terminal, many agents, one leader"));
        assert!(rows[2].contains("   ▄▄▀▀▄▄      shop-api · claude leads"));
        assert!(rows[3].contains("   ▀▀  ▀▀   "));
        // Under Home's list, mid the rows it leaves free.
        assert!(at > 2 && at < 20, "{screen}");
        // Drawn again, later: it goes on coming from where it began.
        let from = app.mark_from.get();
        assert!(from.is_some());
        render(&app, 120, 30);
        assert_eq!(app.mark_from.get(), from);
        // A worker on the list: the rows are its, the mark goes.
        let lead = app.world.projects[0].leader;
        let mut worker = app.world.agents[lead].clone();
        worker.id = app.world.agents.len();
        worker.name = "claude-1".into();
        worker.role = Role::Worker;
        worker.status = Status::Idle;
        app.world.agents.push(worker);
        let screen = render(&app, 120, 30);
        assert!(!screen.contains("imece v"), "{screen}");
        assert!(!app.mark_drawn.get());
    }
    #[test]
    fn the_welcome_and_news_windows_open_under_imece_s_mark_when_it_fits() {
        let mut app = App::new(1, 2, 4, 7, Lang::En);
        app.welcome = true;
        let screen = render(&app, 120, 40);
        let at = screen
            .lines()
            .position(|l| l.contains("WELCOME TO IMECE"))
            .unwrap();
        let rows: Vec<&str> = screen.lines().collect();
        assert!(
            rows[at + 2].contains("│       ██        imece v"),
            "{screen}"
        );
        assert!(rows[at + 5].contains("│     ▀▀  ▀▀   "), "{screen}");
        assert!(
            rows[at + 7].contains("One terminal for a team of Claude Code agents."),
            "{screen}"
        );
        assert!(app.mark_drawn.get());
        // The rows the window needs and the mark's six: 21 + 6 + 2.
        let lines = 2 + 2 + 1 + 1 + 1 + 9 + 2;
        assert!(render(&app, 120, lines + 6 + 2).contains("imece v"));
        let short = render(&app, 120, lines + 6 + 1);
        assert!(
            !short.contains("imece v") && short.contains("WELCOME TO IMECE"),
            "{short}"
        );
        // A narrow screen narrows the window: 53 inside, a column short of
        // the mark's 2 + 52, the words alone; 54, the mark.
        let narrow = render(&app, 55, 40);
        assert!(
            narrow.contains("imece v") && !narrow.contains("▀▀"),
            "{narrow}"
        );
        assert!(render(&app, 56, 40).contains("▀▀"));
        // What is new, under it too.
        app.welcome = false;
        app.news = vec![crate::welcome::VERSION];
        let screen = render(&app, 120, 40);
        assert!(
            screen.contains("imece v") && screen.contains("one terminal, many agents"),
            "{screen}"
        );
    }

    #[test]
    fn the_axis_says_when_its_first_and_middle_columns_were_and_now() {
        let t = 1000;
        let row = timeline::axis(Lang::En, t, 40, 1);
        assert_eq!(row.chars().count(), 40);
        assert!(row.starts_with(&clock(t - 39)), "{row}");
        assert_eq!(&row[18..23], clock(t - 21));
        assert!(row.ends_with("now"), "{row}");
        // Five minutes a column: the middle clock is 21 columns back, not 17.
        let row = timeline::axis(Lang::En, 10_000, 40, 60);
        assert_eq!(&row[18..23], clock(10_000 - 21 * 60), "{row}");
        let row = timeline::axis(Lang::Tr, t, 40, 6);
        assert!(
            row.starts_with(&clock(t - 39 * 6)) && row.ends_with("şimdi"),
            "{row}"
        );
        let short = timeline::axis(Lang::En, t, 23, 1);
        assert_eq!(short.trim(), "now", "too short for clocks");
        assert!(timeline::axis(Lang::En, t, 24, 1).starts_with(&clock(t - 23)));
        // A lane's glyphs in runs of one look.
        let spans = timeline::lane_spans(&[LaneCell::Writing, LaneCell::Writing, LaneCell::Idle]);
        let text: Vec<&str> = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, ["██", "·"]);
    }

    /// The last row of a drawing of `app`, as wide as `width`.
    fn foot(app: &App, width: u16) -> String {
        render(app, width, 40).lines().last().unwrap().to_string()
    }

    fn item(kind: InboxKind, agent: AgentId) -> InboxItem {
        InboxItem {
            kind,
            agent,
            detail: "the words".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        }
    }

    #[test]
    fn a_question_and_a_fault_name_the_keys_they_have() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let agent = app.world.agents[1].id;
        app.pane = Pane::Inbox;
        app.world.inbox.clear();
        app.world.inbox.push(item(InboxKind::Question, agent));
        let s = render(&app, 160, 40);
        assert!(s.contains(" a  answer   enter  open"), "{s}");
        assert!(!s.contains("a  approve"), "{s}");
        app.world.inbox[0] = item(InboxKind::Fault, agent);
        let s = render(&app, 160, 40);
        assert!(s.contains(" a  answer   enter  open"), "{s}");
        assert!(!s.contains("a  approve"), "{s}");
        app.world.inbox[0] = item(InboxKind::Sensitive, agent);
        assert!(render(&app, 160, 40).contains(" a  yes   r  no"));
    }

    #[test]
    fn an_idle_worker_with_a_change_waiting_says_so_and_not_idle() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let a = app.world.agents[1].id;
        app.world.agents[a].status = Status::Idle;
        let text = |app: &App| {
            let spans = doing_status(Lang::En, &app.world, &app.world.agents[a]);
            spans.iter().map(|s| s.content.as_ref()).collect::<String>()
        };
        assert_eq!(text(&app), "idle");
        app.world.inbox.push(item(InboxKind::Change, a));
        assert_eq!(text(&app), "change ready");
    }

    #[test]
    fn the_footer_leads_with_the_next_decision_while_one_waits() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let agent = app.world.agents[1].id;
        app.world.inbox.clear();
        assert!(!foot(&app, 200).contains("next for you"));
        app.world.inbox.push(item(InboxKind::Question, agent));
        let row = foot(&app, 200);
        assert!(
            row.contains("   a  answer   enter  open   s  later   ?  help"),
            "{row}"
        );
        // The demo's pause is not a key of the footer: the header says PAUSED.
        assert!(!row.contains("space"), "{row}");
    }

    #[test]
    fn the_board_lists_what_ended_newest_first_and_only_work_shows_its_owner_s_state() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let owner = app.world.agents[1].id;
        let name = app.world.agents[1].name.clone();
        app.world.agents[owner].status = Status::Working;
        let mut card = app.world.cards[0].clone();
        card.owner = Some(owner);
        let of = |id, column, finished, title: &str| {
            let mut c = card.clone();
            (c.id, c.column, c.finished) = (id, column, finished);
            c.title = title.into();
            c
        };
        app.world.cards = vec![
            of(0, Column::Done, 1, "older done"),
            of(1, Column::Done, 2, "newer done"),
            of(2, Column::Done, 3, "older review"),
            of(3, Column::Done, 4, "newer review"),
            of(4, Column::Verifying, 0, "checked"),
            of(5, Column::Working, 0, "at work"),
        ];
        app.world.inbox.clear();
        for c in [2, 3] {
            let mut change = item(InboxKind::Change, owner);
            change.card = Some(c);
            app.world.inbox.push(change);
        }
        app.window = Some(Window::Board);
        let s = render(&app, 160, 40);
        // Inside the window: Home's preview behind it names the cards too.
        let from = s.find("BOARD · cards").unwrap();
        let at = |word: &str| {
            s[from..]
                .find(word)
                .unwrap_or_else(|| panic!("{word}: {s}"))
        };
        assert!(s.contains("TO REVIEW (2)") && s.contains("DONE (2)"), "{s}");
        assert!(at("newer done") < at("older done"), "{s}");
        assert!(at("newer review") < at("older review"), "{s}");
        // At work on a card its owner has a state; past it, a name.
        let glyph = |word: &str| {
            let row = s.lines().find(|l| l.contains(word)).unwrap();
            let line = s.lines().skip_while(|l| !l.contains(word)).nth(1).unwrap();
            let cell = |l: &str, w: &str| l[..l.find(w).unwrap()].chars().count();
            let at = cell(row, word);
            line.chars().skip(at).take(16).collect::<String>()
        };
        assert!(glyph("checked").contains(&format!("● {name}")), "{s}");
        // Only a card at work has its progress.
        let bar = |word: &str| {
            let row = s.lines().skip_while(|l| !l.contains(word)).nth(1).unwrap();
            let at = s.lines().find(|l| l.contains(word)).unwrap();
            let from = at[..at.find(word).unwrap()].chars().count();
            let cell: String = row.chars().skip(from).take(28).collect();
            cell.contains('░') || cell.contains('▓')
        };
        assert!(bar("at work"), "{s}");
        assert!(!bar("checked"), "{s}");
        assert!(!bar("newer done"), "{s}");
        assert!(!glyph("newer review").contains('●'), "{s}");
        assert!(!glyph("newer done").contains('●'), "{s}");
        // Record 126: a card waiting for review stands there by its own
        // column, a failed one with the done ones marked, a later try says
        // which.
        app.world.inbox.clear();
        app.world.cards[2].column = Column::Review;
        app.world.cards[1].column = Column::Failed;
        app.world.cards[1].attempt = 2;
        let s = render(&app, 160, 40);
        assert!(s.contains("TO REVIEW (1)") && s.contains("DONE (3)"), "{s}");
        let row = s.lines().find(|l| l.contains("newer done")).unwrap();
        let cell = &row[row.find("#2").unwrap()..];
        let cell = cell.split('│').next().unwrap();
        assert!(cell.starts_with("#2  ✕ newer done "), "{cell}");
        assert!(
            cell.ends_with(" · try 2"),
            "the try at the column's end: {cell}"
        );
        let row = s.lines().find(|l| l.contains("older done")).unwrap();
        assert!(!row.contains('✕') && !row.contains("try"), "{row}");
    }

    #[test]
    fn under_a_conversation_the_footer_has_no_keys_of_the_salon() {
        let mut app = talking();
        assert!(foot(&app, 200).contains("open"));
        app.open_thread(4);
        let row = foot(&app, 200);
        assert!(row.contains("esc") && row.contains("help"), "{row}");
        assert!(!row.contains("write to the lead"), "{row}");
        assert!(!row.contains("space"), "{row}");
    }

    #[test]
    fn a_short_footer_loses_the_last_keys_first_and_never_the_way_back_or_the_help() {
        let hints = vec![("a", "one"), ("b", "two"), ("?", "help")];
        let all = spans_width(&hint_spans(&hints));
        let names = |h: &[(&str, &str)]| h.iter().map(|k| k.0.to_string()).collect::<Vec<_>>();
        // Exactly as wide as the room: everything stays.
        assert_eq!(names(&fit_hints(hints.clone(), all)), ["a", "b", "?"]);
        // A cell short: the one before the help goes.
        assert_eq!(names(&fit_hints(hints.clone(), all - 1)), ["a", "?"]);
        // The way back and the help stay, however short the room.
        assert_eq!(names(&fit_hints(hints, 0)), ["?"]);
        let h = fit_hints(vec![("esc", "back"), ("b", "two"), ("?", "help")], 0);
        assert_eq!(names(&h), ["esc", "?"]);
    }

    #[test]
    fn the_status_line_names_home_s_row_at_hand_and_elsewhere_the_agent_at_hand() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let agent = app.world.agents[1].id;
        let other = app.world.agents[2].id;
        // A card's id is its place in the list.
        let second_id = app.world.cards.len();
        let second_shown = second_id + 1;
        let mut second = app.world.cards[0].clone();
        second.id = second_id;
        second.title = "the other card".into();
        app.world.cards.push(second);
        app.world.agents[agent].card = Some(app.world.cards[0].id);
        app.world.agents[other].card = Some(second_id);
        let first = app.world.cards[0].id;
        app.world.inbox.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
            a.starting = false;
        }
        // A screen over Home is about its own agent, not the row's.
        app.zoom = Zoom::Focus(other);
        let row = foot(&app, 200);
        assert!(
            row.starts_with(&format!(" focus · shop-api · #{second_shown} ")),
            "{row}"
        );
        app.zoom = Zoom::None;
        // A row that is an agent, not an item, names its card too.
        app.world.inbox.clear();
        app.world.agents[agent].status = Status::Working;
        let row = foot(&app, 200);
        assert!(
            row.starts_with(&format!(" home · shop-api · #{} ", first + 1)),
            "{row}"
        );
    }

    #[test]
    fn the_footer_has_at_most_three_keys_for_the_selected_thing_then_the_way_back_and_the_help() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let agent = app.world.agents[1].id;
        app.world.agents[agent].card = Some(app.world.cards[0].id);
        app.world.inbox.clear();
        let badges = |row: &str| -> Vec<String> {
            // A key badge is ` k ` between two spaces of the label before it.
            let keys: Vec<&str> = row.split("   ").collect();
            keys.iter()
                .skip(1)
                .filter_map(|k| k.split("  ").next())
                .map(|k| k.trim().to_string())
                .filter(|k| !k.is_empty())
                .collect()
        };
        app.zoom = Zoom::Diff(agent);
        assert_eq!(badges(&foot(&app, 200)), ["a", "r", "c", "esc", "?"]);
        // A clash's `b` comes first and the note goes, so there are still three.
        app.world.change_ready(crate::model::Change {
            agent,
            title: "did it".into(),
            files: Vec::new(),
            patch: String::new(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
        app.world.change_clashed(
            agent,
            Clashes {
                head: "abc1234".into(),
                hunks: vec![Clash {
                    file: "x.rs".into(),
                    ..Clash::default()
                }],
                ..Clashes::default()
            },
        );
        assert_eq!(badges(&foot(&app, 200)), ["b", "a", "r", "esc", "?"]);
        app.world.change_clashed(agent, Clashes::default());
        app.world.inbox.clear();
        app.zoom = Zoom::None;
        // A window has no place before its keys: the badges start the line.
        let win_badges = |app: &App| badges(&format!("x {}", foot(app, 200)));
        app.window = Some(Window::Timeline);
        assert_eq!(win_badges(&app), ["[ ]", "enter", "@", "esc", "?"]);
        app.window = Some(Window::Board);
        assert_eq!(win_badges(&app), ["esc", "?"]);
        app.window = Some(Window::Code);
        assert_eq!(win_badges(&app), ["esc", "?"]);
        app.window = None;
        assert_eq!(badges(&foot(&app, 200)), ["enter", "s", "@", "?"]);
        app.world.inbox.push(item(InboxKind::Question, agent));
        assert_eq!(badges(&foot(&app, 200)), ["a", "enter", "s", "?"]);
        app.card = Some(app.world.card_of(agent).unwrap());
        assert_eq!(badges(&foot(&app, 200)), ["enter", "n p", "esc", "?"]);
        app.card = None;
        app.peek = Some(agent);
        assert_eq!(badges(&foot(&app, 200)), ["enter", "@", "esc", "?"]);
    }

    #[test]
    fn a_chord_s_first_key_lists_its_second_keys_as_which_key_does() {
        let mut app = app_after(1, 2, 0, Lang::En);
        assert!(!foot(&app, 200).contains('…'));
        app.pending = Some('g');
        assert_eq!(
            foot(&app, 200).trim_end(),
            " g…   g  first line   d  to decide   w  at work"
        );
        app.pending = Some('z');
        assert_eq!(
            foot(&app, 200).trim_end(),
            " z…   a  fold · unfold   M  fold all   R  unfold all"
        );
        app.pending = Some('q');
        assert_eq!(
            foot(&app, 200).trim_end(),
            " q…   q  quit   s  what quitting leaves"
        );
        app.lang = Lang::Tr;
        assert_eq!(
            foot(&app, 200).trim_end(),
            " q…   q  çık   s  çıkınca ne kalır"
        );
        app.lang = Lang::En;
        // The person's file: its pairs that start with the key, as what they act as.
        app.keys = crate::keys::Keymap::read("gt = G\ngn = j\nx = gg\n").unwrap();
        app.pending = Some('g');
        assert_eq!(
            foot(&app, 200).trim_end(),
            " g…   g  first line   d  to decide   w  at work   t  as G   n  as j"
        );
        // A first key of the file's alone, held: only the file's pairs.
        app.pending = None;
        app.keys = crate::keys::Keymap::read("xa = G\n").unwrap();
        app.keys_held = Some((
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
            std::time::Instant::now(),
        ));
        assert_eq!(foot(&app, 200).trim_end(), " x…   a  as G");
        app.keys_held = None;
        assert!(!foot(&app, 200).contains('…'));
    }

    #[test]
    fn home_s_footer_leads_with_what_the_row_at_hand_takes() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let agent = app.world.agents[1].id;
        for a in &mut app.world.agents {
            a.status = Status::Idle;
            a.starting = false;
        }
        let row = |app: &mut App, kind| {
            app.world.inbox.clear();
            app.world.inbox.push(item(kind, agent));
            foot(app, 200).trim_start().to_string()
        };
        assert!(
            row(&mut app, InboxKind::Change)
                .contains("   enter  read the diff   a  approve   r  send back   ?  help")
        );
        assert!(
            row(&mut app, InboxKind::Question)
                .contains("   a  answer   enter  open   s  later   ?  help")
        );
        assert!(
            row(&mut app, InboxKind::Fault)
                .contains("   a  answer   enter  open   s  later   ?  help")
        );
        assert!(
            row(&mut app, InboxKind::Approval)
                .contains("   a  allow once   r  refuse   e  yes this turn   ?  help")
        );
        assert!(
            row(&mut app, InboxKind::Reply)
                .contains("   enter  open   r  dismiss   s  later   ?  help")
        );
        assert!(
            row(&mut app, InboxKind::Plan).contains("   enter  open   a  yes   r  no   ?  help")
        );
        for kind in [InboxKind::Tries, InboxKind::Rounds] {
            assert!(
                row(&mut app, kind).contains("   enter  open   a  one more   r  fail it   ?  help"),
                "{kind:?}"
            );
        }
        app.world.inbox.clear();
        let agent_row = foot(&app, 200);
        assert!(
            agent_row.contains("   enter  open   s  hide   @  write to this agent   ?  help"),
            "{agent_row}"
        );
    }

    #[test]
    fn a_change_of_several_files_lists_them_beside_its_patch_from_120_columns() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let id = app.world.agents[1].id;
        let file = |path: &str| ChangedFile {
            path: path.into(),
            added: 1,
            removed: 0,
            binary: false,
        };
        app.world.change_ready(Change {
            agent: id,
            title: "Paging".into(),
            files: vec![file("src/a.rs"), file("src/b.rs")],
            patch: "diff --git a/src/a.rs b/src/a.rs\n@@ -1 +1 @@\n+x\n".into(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
        app.zoom = Zoom::Diff(id);
        // Width less the window's two borders.
        let wide = render(&app, 122, 30);
        assert!(wide.contains("▸ src/a.rs +1 −0"), "{wide}");
        assert!(!wide.contains("diff --git"), "{wide}");
        let narrow = render(&app, 121, 30);
        assert!(!narrow.contains('▸'), "{narrow}");
        assert!(narrow.contains(" src/b.rs +1 −0"), "{narrow}");
    }

    #[test]
    fn a_change_says_first_what_its_worker_said_of_it() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let id = app.world.agents[1].id;
        app.world.note(Event::Sent {
            from: id,
            to: 0,
            kind: MsgKind::Done,
            text: "Paging\n\nKept the old shape; next is a new field.".into(),
            id: 1,
            re: None,
        });
        app.world.change_ready(Change {
            agent: id,
            title: "Paging".into(),
            files: Vec::new(),
            patch: "+x\n".into(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
        app.zoom = Zoom::Diff(id);
        let s = render(&app, 120, 30);
        let rows: Vec<&str> = s.lines().collect();
        // The detail's head, then the tabs, then the change's own frame
        // with the verdict first and the worker's words under it.
        assert!(
            rows[1].contains("Paging") && rows[1].contains("· 0 files +0 −0"),
            "{s}"
        );
        assert!(rows[3].contains("diff · talk · log"), "{s}");
        let v = rows
            .iter()
            .position(|l| l.contains("! look first · nothing ran"))
            .unwrap();
        assert_eq!(v, 5, "{s}");
        assert!(
            rows[v + 1].contains("Kept the old shape; next is a new field."),
            "the worker's words under the verdict: {s}"
        );
    }

    #[test]
    fn the_files_stand_beside_a_wide_change_only_when_there_are_two() {
        let file = |path: &str| ChangedFile {
            path: path.into(),
            added: 7,
            removed: 5,
            binary: false,
        };
        let shown = |files: Vec<ChangedFile>| {
            let mut app = app_after(1, 2, 0, Lang::En);
            let id = app.world.agents[1].id;
            app.world.change_ready(Change {
                agent: id,
                title: "Paging".into(),
                patch: "diff --git a/src/one.rs b/src/one.rs\n+x\n".into(),
                files,
                runs: Vec::new(),
                check: Check::None,
                spend: None,
            });
            app.zoom = Zoom::Diff(id);
            render(&app, 160, 30)
        };
        // The tree's row: the file at hand, its counts, then the rule.
        let beside = |s: &str| s.lines().any(|l| l.contains("▸ src/one.rs +7 −5  "));
        let two = shown(vec![file("src/one.rs"), file("src/two.rs")]);
        assert!(beside(&two), "{two}");
        let one = shown(vec![file("src/one.rs")]);
        assert!(!beside(&one), "{one}");
    }

    #[test]
    fn a_change_s_footer_keeps_the_help_at_any_width() {
        let mut app = app_after(1, 2, 0, Lang::En);
        app.zoom = Zoom::Diff(app.world.agents[1].id);
        assert!(foot(&app, 120).contains("help"), "{}", foot(&app, 120));
        assert!(foot(&app, 60).contains("help"), "{}", foot(&app, 60));
    }

    #[test]
    fn a_project_header_shows_how_far_its_own_cards_are() {
        assert!(salon::cards_spans(0, 0).is_empty());
        let bar = |done, all| spans_text(&salon::cards_spans(done, all));
        assert_eq!(bar(3, 8), "  ▓▓▓░░░░░ 3/8");
        assert_eq!(bar(1, 9), "  ░░░░░░░░ 1/9", "down to the cell");
        assert_eq!(bar(7, 7), "  ▓▓▓▓▓▓▓▓ 7/7");
        let mut app = app_after(2, 2, 0, Lang::En);
        let card = |project, column| Card {
            id: 0,
            project,
            title: "c".into(),
            owner: None,
            column,
            story: Vec::new(),
            symbols: Vec::new(),
            paths: Vec::new(),
            checks: Vec::new(),
            started: 0,
            finished: 0,
            attempt: 1,
            breaker: Default::default(),
            after: Vec::new(),
        };
        app.world.cards = vec![
            card(0, Column::Done),
            card(0, Column::Working),
            card(0, Column::Waiting),
            card(1, Column::Done),
        ];
        let rungs = app.rungs();
        let line = |app: &App, p| spans_text(&salon::project_line(app, p, &rungs).spans);
        assert!(
            line(&app, 0).contains("  ▓▓░░░░░░ 1/3  "),
            "{}",
            line(&app, 0)
        );
        assert!(
            line(&app, 1).contains("  ▓▓▓▓▓▓▓▓ 1/1  "),
            "{}",
            line(&app, 1)
        );
        app.world.cards.clear();
        assert!(!line(&app, 0).contains('░'), "{}", line(&app, 0));
    }

    #[test]
    fn which_key_lists_gd_and_gw_on_home_s_list_alone() {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        app.pending = Some('g');
        let line = which_key(&app).unwrap();
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains(" g  first line"), "{text}");
        assert!(text.contains(" d  to decide"), "{text}");
        assert!(text.contains(" w  at work"), "{text}");
        app.lang = Lang::Tr;
        let line = which_key(&app).unwrap();
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains(" d  karar bekleyen") && text.contains(" w  çalışan"),
            "{text}"
        );
        // Over a screen on Home, or off Home, `g…` has the one key.
        app.lang = Lang::En;
        app.zoom = Zoom::Focus(1);
        let line = which_key(&app).unwrap();
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains(" g  first line") && !text.contains("to decide"),
            "{text}"
        );
        app.zoom = Zoom::None;
        app.pending = Some('z');
        let line = which_key(&app).unwrap();
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(!text.contains("to decide"), "{text}");
    }

    #[test]
    fn the_status_line_names_the_card_an_item_came_on_not_the_one_its_agent_has_now() {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        let agent = app.world.agents[1].id;
        let second_id = app.world.cards.len();
        let mut second = app.world.cards[0].clone();
        second.id = second_id;
        second.title = "the other card".into();
        app.world.cards.push(second);
        app.world.agents[agent].card = Some(second_id);
        app.world.inbox.clear();
        let mut it = item(InboxKind::Check, agent);
        it.card = Some(0);
        app.world.inbox.push(it);
        let row = foot(&app, 200);
        assert!(row.starts_with(" home · shop-api · #1 "), "{row}");
        assert!(!row.contains("the other card"), "{row}");
        // Without a card of its own, the agent's.
        app.world.inbox[0].card = None;
        let row = foot(&app, 200);
        assert!(
            row.starts_with(&format!(
                " home · shop-api · #{} the other card",
                second_id + 1
            )),
            "{row}"
        );
    }

    #[test]
    fn the_projects_stand_in_the_header_and_the_one_home_is_narrowed_to_is_lit() {
        let text = |app: &App| -> String {
            projects_spans(app)
                .iter()
                .map(|s| s.content.as_ref())
                .collect()
        };
        let mut app = App::new(2, 2, 0, 1, Lang::En);
        let names: Vec<String> = app.world.projects.iter().map(|p| p.name.clone()).collect();
        assert_eq!(text(&app), format!("{} · {}", names[0], names[1]));
        let lit = |app: &App, n: usize| {
            projects_spans(app)[n * 2]
                .style
                .add_modifier
                .contains(Modifier::REVERSED)
        };
        assert!(!lit(&app, 0) && !lit(&app, 1));
        app.at_home.scope = Some(1);
        assert!(!lit(&app, 0) && lit(&app, 1));
        // A closed project is not named.
        app.world.projects[0].closed = true;
        assert_eq!(text(&app), names[1]);
        assert_eq!(projects_spans(&app)[0].content, names[1]);
    }

    #[test]
    fn a_row_done_and_not_seen_says_so_for_a_leader_too() {
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
        assert_eq!(line.spans[1].content, "✓ ");
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
        assert!(salon::stand_spans(&Stand::default(), None).is_empty());
        let all = salon::stand_spans(&stand(Some("fix-parser"), 2, 1, 12, 3), None);
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
        assert_eq!(
            text(&salon::stand_spans(&stand(None, 1, 0, 0, 0), None)),
            "  ↑1"
        );
        assert_eq!(
            text(&salon::stand_spans(&stand(None, 0, 4, 0, 0), None)),
            "  ↓4"
        );
        assert_eq!(
            text(&salon::stand_spans(&stand(None, 0, 0, 5, 0), None)),
            "  +5"
        );
        assert_eq!(
            text(&salon::stand_spans(&stand(None, 0, 0, 0, 6), None)),
            "  −6"
        );
        assert_eq!(
            text(&salon::stand_spans(&stand(None, 0, 0, 5, 6), None)),
            "  +5 −6"
        );
        // An agent's branch name is cleaned and cut.
        let long = salon::stand_spans(
            &stand(Some("a-very-long-branch-name\u{1b}[2J"), 0, 0, 0, 0),
            None,
        );
        assert_eq!(text(&long), "  ⎇ a-very-long-bra…");
        // The start point it was hired from, when no branch is named; a
        // branch the agent made wins.
        let from = salon::stand_spans(&stand(None, 0, 3, 0, 0), Some("#41"));
        assert_eq!(text(&from), "  ⎇ #41 ↓3");
        let both = salon::stand_spans(&stand(Some("wip"), 0, 0, 0, 0), Some("#41"));
        assert_eq!(text(&both), "  ⎇ wip");

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
    fn a_setup_ask_s_window_shows_how_it_runs_and_its_bytes_from_the_line_scrolled_to() {
        let (mut app, item, id) = crate::app::tests::asking_setup();
        let text = app.script_text(item).unwrap().text();
        let sha = crate::sha::hex(text.as_bytes());
        app.pane = crate::app::Pane::Inbox;
        let row = render(&app, 240, 30);
        let name = app.world.agents[id].name.clone();
        assert!(
            row.contains("run .imece/worktree-setup.sh in its new worktrees?"),
            "{row}"
        );
        assert!(row.contains(" a  yes   r  no"), "{row}");
        app.script = Some((item, 0));
        let s = render(&app, 140, 30);
        assert!(
            s.contains(&format!("SETUP SCRIPT · {name} · sha256 {} ", &sha[..16])),
            "{s}"
        );
        assert!(!s.contains(&sha[..17]), "the hash's head only");
        assert!(s.contains("runs as /bin/sh in each new worktree"), "{s}");
        assert!(
            s.contains("file: /demo/shop-api/.imece/worktree-setup.sh"),
            "{s}"
        );
        assert!(s.contains("1 #!/bin/sh"), "{s}");
        assert!(s.contains("5 npm ci --no-audit"), "{s}");
        assert!(s.contains(" a  yes "), "{s}");
        // Scrolled: from that line on.
        app.script = Some((item, 4));
        let s = render(&app, 140, 30);
        assert!(!s.contains("1 #!/bin/sh"), "{s}");
        assert!(s.contains("5 npm ci --no-audit"), "{s}");
        // Answered: the window says it is no longer asked about.
        app.script = None;
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        app.on_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
        app.script = Some((item, 0));
        assert!(render(&app, 140, 30).contains("that setup script is no longer asked about"));
    }

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

    /// Whether the cell at `(x, y)` links to `url` (the web addresses' id).
    pub(crate) fn links_to(buf: &Buffer, x: u16, y: u16, url: &str) -> bool {
        let open = format!("\x1b]8;id=imece-{LINK_ID};{url}\x1b\\");
        buf[(x, y)].symbol().starts_with(&open)
    }

    #[test]
    fn web_addresses_shown_whole_are_links_and_no_others() {
        let url = "https://a.b/c";
        let n = url.len() as u16;
        // At 3 on the first row; up to the area's last cell.
        let rows = vec![
            Line::from(vec![Span::raw("ab "), Span::styled(url, link_style())]),
            Line::from(vec![Span::raw("abcd"), Span::styled(url, link_style())]),
            Line::from(vec![Span::raw("x"), Span::styled(url, fg(pal().info))]),
            Line::from(vec![Span::styled("not a url", link_style())]),
            Line::from(vec![Span::styled(url, link_style())]),
        ];
        let area = Rect::new(2, 1, 3 + n, 4);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 8));
        ratatui::widgets::Widget::render(Paragraph::new(rows.clone()), area, &mut buf);
        link_rows(&mut buf, area, &rows);
        assert!(!links_to(&buf, 4, 1, url), "before it");
        assert!(links_to(&buf, 5, 1, url), "its first cell");
        assert!(
            links_to(&buf, 4 + n, 1, url),
            "its last cell, the area's last"
        );
        assert!(!links_to(&buf, 6, 2, url), "one cell past the area: none");
        assert!(!links_to(&buf, 3, 3, url), "not in a link's style");
        assert!(!buf[(2, 4)].symbol().starts_with("\x1b"), "not an address");
        // A row below the area is not drawn there, nor linked.
        assert!(!links_to(&buf, 2, 5, url));
        // Drawn from the area's top row on: the second row on the area's
        // second.
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 8));
        link_rows(&mut buf, Rect::new(0, 1, 40, 2), &rows[3..]);
        assert!(links_to(&buf, 0, 2, url) && links_to(&buf, n - 1, 2, url));
        assert!(!links_to(&buf, n, 2, url));
        assert!(!links_to(&buf, 0, 1, url) && !links_to(&buf, 0, 0, url));
    }

    #[test]
    fn an_address_wider_than_a_row_is_cut_and_never_a_link() {
        let url = "https://a.b/cdef"; // 16 cells
        let line = || Line::from(vec![Span::raw("  "), Span::styled(url, link_style())]);
        // Room for it after the indent: one row, still a link.
        let rows = rows_of(vec![line()], 18);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].spans[1].style, link_style());
        // One cell short: cut, in the link's colour without the underline.
        let rows = rows_of(vec![line()], 17);
        assert!(rows.len() > 1);
        assert!(
            rows.iter()
                .flat_map(|r| &r.spans)
                .all(|s| s.style != link_style())
        );
        assert!(rows[0].spans.iter().any(|s| s.style == fg(pal().info)));
    }

    #[test]
    fn linked_takes_each_address_that_starts_a_word() {
        let base = fg(pal().ok);
        assert_eq!(
            parts(&linked("a http://x.y b https://z.w", base)),
            [
                ("a ".into(), base),
                ("http://x.y".into(), link_style()),
                (" b ".into(), base),
                ("https://z.w".into(), link_style()),
            ]
        );
        assert_eq!(
            parts(&linked("xhttp://x.y httpd http://", base)),
            [("xhttp://x.y httpd http://".into(), base)],
            "inside a word, no address, nothing after the scheme"
        );
        assert_eq!(
            parts(&linked("(http://x.y)", base)),
            [
                ("(".into(), base),
                ("http://x.y".into(), link_style()),
                (")".into(), base),
            ]
        );
        assert_eq!(
            parts(&linked("http://x.y", base)),
            [("http://x.y".into(), link_style())]
        );
        assert_eq!(
            parts(&linked("http://a.b/?u=http://c.d z", base)),
            [
                ("http://a.b/?u=http://c.d".into(), link_style()),
                (" z".into(), base),
            ],
            "an address inside one is part of it"
        );
        assert!(linked("", base).is_empty());
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
            let mut a = Agent::new(3, 0, "w".into(), role, Status::Idle, 0);
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
    fn a_proposal_says_so_in_its_view_and_its_row_says_its_keys() {
        let mut app = app_after(1, 4, 50, Lang::En);
        app.world.inbox.clear();
        app.pane = Pane::Inbox;
        app.world.change_ready(a_change(1));
        let s = render(&app, 140, 40);
        assert!(s.contains(" a  approve   r  send back"), "{s}");
        let proposal = crate::model::StoreProposal {
            seq: 5,
            change: "c5".into(),
            older: Vec::new(),
            why: vec!["unobserved_session".into()],
            redacted: Vec::new(),
            cut: false,
            verifies: Runs::default(),
            on_trunk: true,
        };
        app.world.change_proposed(a_change(1), proposal);
        let s = render(&app, 140, 40);
        assert!(s.contains(" a  approve   r  send back"), "{s}");
        app.zoom = Zoom::Diff(1);
        let s = render(&app, 140, 40);
        assert!(
            s.contains("nobody watched this session's tools · proposal 5"),
            "{s}"
        );
    }

    #[test]
    fn a_change_of_no_proposal_says_nothing_of_one() {
        let mut app = app_after(1, 4, 50, Lang::En);
        app.world.change_ready(a_change(1));
        app.zoom = Zoom::Diff(1);
        assert!(!render(&app, 140, 40).contains("proposal"));
    }

    #[test]
    fn a_change_shows_its_review_above_its_files_or_who_reads_it() {
        let mut app = app_after(1, 4, 50, Lang::En);
        app.world.inbox.clear();
        app.world.change_ready(a_change(1));
        app.zoom = Zoom::Diff(1);
        let s = render(&app, 140, 40);
        assert!(!s.contains("✎"), "{s}");
        for c in ":review".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        assert!(app.reviewing(1), "asked");
        let s = render(&app, 140, 40);
        assert!(s.contains("✎ claude reads this change…"), "{s}");
        let patch = crate::model::patch_sum(&app.world.change_of(1).unwrap().patch);
        app.world.reviewed(
            1,
            Review {
                patch,
                model: "m1".into(),
                text: "1. parse.rs: no test".into(),
                cost: 420,
            },
        );
        let s = render(&app, 140, 40);
        let head = "✎ review · claude · m1 · $0.04";
        assert!(
            s.contains(head) && s.contains("1. parse.rs: no test"),
            "{s}"
        );
        let (review, files) = (s.find(head).unwrap(), s.find("2 files").unwrap_or(0));
        assert!(review > files, "under the title, above the list: {s}");

        // Whole, it takes up to half the screen and counts the rest.
        let long: Vec<String> = (1..=40).map(|i| format!("point-{i:02}")).collect();
        app.world.reviews.get_mut(&1).unwrap().text = long.join("\n");
        app.review_whole = true;
        let s = render(&app, 140, 40);
        assert!(s.contains("point-10") && !s.contains("point-40"), "{s}");
        assert!(s.contains("lines · R shows all"), "{s}");
    }

    #[test]
    fn a_change_that_clashed_says_b_sends_it_back_in_its_journal_line_view_and_foot() {
        let t = Lang::En.text();
        let mut app = app_after(1, 4, 50, Lang::En);
        app.world.inbox.clear();
        app.world.change_ready(a_change(1));
        app.world
            .change_failed(1, "conflicts with the checkout in x".into());
        let name = app.world.agents[1].name.clone();
        let line = |app: &App| {
            words::event(
                app.lang,
                &app.world,
                &app.world.journal.last().unwrap().event,
            )
        };
        let plain = fill(
            t.ev_apply_failed,
            &[&name, "conflicts with the checkout in x"],
        );
        assert_eq!(line(&app), plain);
        app.zoom = Zoom::Diff(1);
        let s = render(&app, 220, 40);
        assert!(
            !s.contains("send the clash back") && !s.contains("sends it back"),
            "{s}"
        );
        app.world.change_clashed(
            1,
            Clashes {
                head: "abc1234".into(),
                hunks: vec![Clash {
                    file: "x.rs".into(),
                    ..Clash::default()
                }],
                ..Clashes::default()
            },
        );
        assert_eq!(
            line(&app),
            format!("{plain} · b in the change sends it back")
        );
        let s = render(&app, 220, 40);
        assert!(
            s.contains(" b  send the clash back   a  approve   r  send back   esc  back"),
            "{s}"
        );
        assert!(
            s.contains(&format!(
                "✗ does not fit your checkout at abc1234: x.rs · b sends it back to {name}"
            )),
            "{s}"
        );
    }

    #[test]
    fn a_change_waits_in_the_inbox_and_enter_shows_its_diff() {
        let mut app = app_after(1, 4, 50, Lang::En);
        app.world.inbox.clear();
        app.world.change_ready(a_change(1));
        let name = app.world.agents[1].name.clone();
        let s = render(&app, 220, 40);
        assert!(
            s.contains("Fix the parser") && s.contains("2 files +3 −1"),
            "{s}"
        );
        app.pane = Pane::Inbox;
        app.inbox_sel = 0;
        let s = render(&app, 220, 40);
        assert!(s.contains(" a  approve   r  send back"), "{s}");
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.zoom, Zoom::Diff(1));
        let s = render(&app, 118, 40);
        // The head names the card, the project and the worker, then the
        // files and lines; the tabs under it.
        let head = s.lines().nth(1).unwrap();
        assert!(
            head.contains(&format!(" · {name} · ")) && head.contains("2 files +3 −1"),
            "{s}"
        );
        assert!(
            s.lines().nth(3).unwrap().contains("diff · talk · log"),
            "{s}"
        );
        assert!(!s.contains("CHANGE ·"), "{s}");
        assert!(s.contains("-OLD-LINE") && s.contains("+NEW-LINE"), "{s}");
        // The verdict heads the diff, its files under that, then the
        // patch. The demo's card owns the files of its leases, which this
        // change is not of: the verdict says so (record 114).
        let row = |needle: &str| s.lines().position(|l| l.contains(needle)).unwrap();
        let card = app.world.agents[1].card.unwrap();
        assert!(!app.world.cards[card].paths.is_empty());
        let verdict = format!(
            "! look first · nothing ran · 2 files · 2 outside #{}: src/p.rs, src/q.rs",
            card + 1
        );
        let ran = row(&verdict);
        assert_eq!(ran + 1, row("│ src/p.rs +"), "{s}");
        assert_eq!(ran + 3, row("│src/p.rs "), "{s}");
        assert!(
            s.contains("+Q-LINE \n") || s.contains("+Q-LINE "),
            "control characters go: {s}"
        );
        assert!(
            s.contains(" a  approve   r  send back   c  note   esc  back"),
            "{s}"
        );
        let wide = render(&app, 200, 36);
        // Movement and the rest are in the help, however wide the screen.
        assert!(
            !wide.contains(" j k ") && !wide.contains(" >  quote to it"),
            "{wide}"
        );
        // The agent at hand is the change's: `@` writes to it.
        key(&mut app, KeyCode::Char('@'));
        assert_eq!(app.input, format!("@{name} "));
        key(&mut app, KeyCode::Esc);
        // Scrolling: lines, pages, ends, files.
        key(&mut app, KeyCode::Char('j'));
        key(&mut app, KeyCode::Char('j'));
        assert_eq!(app.diff_scroll, 4, "over git's lines");
        key(&mut app, KeyCode::Char('k'));
        assert_eq!(app.diff_scroll, 3);
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
    fn the_demo_decides_an_ask_as_live_does() {
        let mut app = app_after(1, 4, 0, Lang::En);
        let name = app.world.agents[1].name.clone();
        for (k, allowed) in [('a', true), ('r', false)] {
            app.world.asks_you(1, "Bash: make");
            let item = app.world.inbox.len() - 1;
            app.at_home.at = Some(HomeRow::Item(item));
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
    fn c_copies_a_line_and_capital_c_the_conversation_as_shown() {
        let mut app = talking();
        app.world.note(Event::Replied {
            agent: 0,
            text: "token sk-abc123 \u{1b}[31mred".into(),
        });
        app.open_window(Window::Journal);
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
        let mut app = talking_over_two_days();
        app.open_window(Window::Journal);
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
        app.open_window(Window::Journal);
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
            lines[keys - 1].contains("⇄") && lines[keys - 1].contains("claude-1"),
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
        app.open_window(Window::Journal);
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

    /// The row above the footer, where the completions are.
    fn menu_row(app: &App, width: u16) -> String {
        let s = render(app, width, 40);
        let mut rows = s.lines().rev();
        rows.next();
        rows.next().unwrap_or_default().to_string()
    }

    #[test]
    fn every_command_says_what_it_does_in_both_languages_in_the_menus_order() {
        for lang in [Lang::En, Lang::Tr] {
            let names: Vec<&str> = lang.text().command_hints.iter().map(|(c, _)| *c).collect();
            assert_eq!(names, crate::complete::COMMANDS, "{lang:?}");
            for (c, what) in lang.text().command_hints {
                assert!(
                    !what.is_empty() && what.chars().count() <= 80,
                    "{c}: {what}"
                );
            }
        }
    }

    #[test]
    fn the_colon_menu_says_how_many_are_out_of_view_and_what_the_one_put_in_does() {
        let mut app = app_after(1, 1, 0, Lang::En);
        let foot = |app: &App| render(app, 140, 40).lines().last().unwrap().to_string();
        let n = crate::complete::COMMANDS.len();
        key(&mut app, KeyCode::Char(':'));
        let row = menu_row(&app, 140);
        assert!(row.contains(" more › "), "more than fit, said: {row}");
        assert!(!row.contains("‹"), "{row}");
        let f = foot(&app);
        assert!(
            f.contains(&format!("tab next · enter take · esc hide · 1 of {n}")),
            "{f}"
        );
        assert!(!f.contains("yes from now on"), "nothing put in yet: {f}");
        // tab puts the first one in: what it does follows the keys.
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.input, ":allow ");
        let f = foot(&app);
        assert!(
            f.contains(&format!(
                "esc hide · 1 of {n}   the project's yes from now on; PROGRAM [WORD] adds or drops one"
            )),
            "{f}"
        );
        key(&mut app, KeyCode::Tab);
        let f = foot(&app);
        assert!(
            f.contains(&format!("2 of {n}   put the project at hand away")),
            "{f}"
        );
        // Far on, the ones before are counted too.
        for _ in 0..40 {
            key(&mut app, KeyCode::Tab);
        }
        let row = menu_row(&app, 140);
        assert!(row.starts_with(" ‹ "), "{row}");
        // In Turkish.
        app.lang = Lang::Tr;
        let f = foot(&app);
        let (c, what) = Lang::Tr.text().command_hints[41];
        assert_eq!(app.input, format!(":{c} "));
        assert!(f.contains(&format!("42/{n}   {what}")), "{f}");
        app.lang = Lang::En;
        // Writing on: gone.
        key(&mut app, KeyCode::Char('x'));
        assert!(
            !foot(&app).contains(Lang::En.text().command_hints[41].1),
            "{}",
            foot(&app)
        );
        // One item: no count.
        app.input.clear();
        app.after = 0;
        for c in ":unse".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        assert_eq!(app.menu.as_ref().map(|m| m.items.len()), Some(1));
        let f = foot(&app);
        assert!(f.contains("esc hide") && !f.contains(" of 1"), "{f}");
        // An argument's menu and an @ menu are not commands.
        app.input.clear();
        app.after = 0;
        for c in ":lang ".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.input, ":lang en");
        assert!(!foot(&app).contains("Türkçe / English"), "{}", foot(&app));
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Char('@'));
        key(&mut app, KeyCode::Tab);
        assert!(
            app.menu
                .as_ref()
                .is_some_and(|m| m.inserted && m.start == 1)
        );
        let f = foot(&app);
        assert!(f.contains("esc hide · 1 of "), "{f}");
        assert!(!f.contains("   the "), "{f}");
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
        assert!(
            !render(&app, 140, 40).contains("enter send"),
            "the keys to send go once something is written"
        );
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
        // The window counts the notes: the highlighted item stays in view,
        // the one before it out, and said.
        let (row, _) = completion_row(&m, 19);
        assert_eq!(row, " ‹ 1  open $  help ", "an exact fit");
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
            ["peek", "pending", "pin", "plan", "ps"],
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
    fn a_colon_offers_the_commands_to_the_end() {
        let mut app = app_after(1, 2, 0, Lang::En);
        key(&mut app, KeyCode::Char(':'));
        // The demo's own commands too.
        assert_eq!(app.menu.as_ref().unwrap().items, crate::complete::COMMANDS);
        let row = menu_row(&app, 140);
        assert!(row.contains("budget") && row.contains("board"), "{row}");
        key(&mut app, KeyCode::Char('b'));
        key(&mut app, KeyCode::Char('u'));
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.input, ":budget ");
        key(&mut app, KeyCode::Enter);
        assert!(!app.typing);
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
        for open in 0..6 {
            match open {
                0 => app.help = true,
                1 => app.digest = Some(Default::default()),
                2 => app.left_off = Some(Default::default()),
                3 => app.news = vec!["0.1.0"],
                4 => app.welcome = true,
                _ => app.leaving = true,
            }
            app.flash = None;
            app.paste("x");
            assert_eq!(app.input, line, "overlay {open}");
            assert!(app.flash.is_some(), "overlay {open}");
            (app.help, app.digest, app.left_off, app.leaving) = (false, None, None, false);
            (app.news, app.welcome) = (Vec::new(), false);
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
        // Nothing written yet: the keys to send, after who it goes to.
        let empty = render(&app, 140, 40);
        let row = empty.lines().nth(39).unwrap();
        assert!(
            row.starts_with(&format!(
                "{to}   enter send · \\ enter new line · esc cancel"
            )),
            "{row}"
        );
        app.shift_enter = true;
        assert!(render(&app, 140, 40).contains("enter send · ⇧enter new line · esc cancel"));
        app.shift_enter = false;
        write(&mut app, "one");
        with(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
        write(&mut app, "two");
        let s = render(&app, 140, 40);
        let rows: Vec<&str> = s.lines().collect();
        let (first, last) = (rows[38], rows[39]);
        assert!(first.starts_with(&format!("{to}one")), "{first}");
        let indent = " ".repeat(to.chars().count());
        assert!(last.starts_with(&format!("{indent}two")), "{last}");
        assert!(!s.contains("enter send"), "gone once something is written");
        assert!(!s.contains("/2"), "all lines fit: no count");

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
    fn a_row_is_cut_cell_by_cell_and_the_cursor_found_in_it() {
        let cut = |t: &str, room, caret| wrap_cells(t, room, caret);
        assert_eq!(cut("", 4, Some(0)), (vec![String::new()], (0, 0)));
        assert_eq!(cut("", 4, None), (vec![String::new()], (0, 0)));
        assert_eq!(cut("abc", 4, Some(3)), (vec!["abc".into()], (0, 3)));
        assert_eq!(cut("abcd", 4, None), (vec!["abcd".into()], (0, 0)));
        // At the end of a full row the cursor is on the next, made for it.
        assert_eq!(
            cut("abcd", 4, Some(4)),
            (vec!["abcd".into(), String::new()], (1, 0))
        );
        assert_eq!(
            cut("abcdefghij", 4, Some(5)),
            (vec!["abcd".into(), "efgh".into(), "ij".into()], (1, 1))
        );
        assert_eq!(
            cut("abcdefgh", 4, Some(4)),
            (vec!["abcd".into(), "efgh".into()], (1, 0)),
            "before e: the start of its row"
        );
        assert_eq!(cut("abcde", 4, Some(3)).1, (0, 3));
        // A wide character that does not fit goes on to the next row.
        assert_eq!(
            cut("abc中d", 4, Some(3)),
            (vec!["abc".into(), "中d".into()], (1, 0))
        );
        assert_eq!(
            cut("中中中", 4, Some(9)),
            (vec!["中中".into(), "中".into()], (1, 2))
        );
        // Too narrow for one: one character a row, never none.
        assert_eq!(cut("中a", 1, None).0, ["中", "a"]);
        assert_eq!(
            cut("ab", 0, Some(1)),
            (vec!["a".into(), "b".into()], (1, 0))
        );
    }

    #[test]
    fn a_long_line_goes_on_in_the_next_row_and_the_footer_grows() {
        let mut app = app_after(1, 3, 0, Lang::En);
        let lead = app.world.agents[app.world.projects[0].leader].name.clone();
        let to = format!(" › to {lead}: ");
        let indent = " ".repeat(to.chars().count());
        let room = 60 - to.chars().count();
        key(&mut app, KeyCode::Char('/'));
        let long: String = (0..room * 2 + 5)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        write(&mut app, &long);
        let s = render(&app, 60, 30);
        let rows: Vec<&str> = s.lines().collect();
        assert_eq!(rows[27], format!("{to}{}", &long[..room]));
        assert_eq!(
            rows[28].trim_end(),
            format!("{indent}{}", &long[room..room * 2])
        );
        assert_eq!(
            rows[29].trim_end(),
            format!("{indent}{}", &long[room * 2..])
        );
        assert_eq!(
            caret_of(&app, 60, 30),
            Some((u16::try_from(to.chars().count() + 5).unwrap(), 29))
        );
        assert_eq!(input_rows(&app, 60, 30), 3);
        // A row exactly full puts the cursor on a new one below it.
        let mut app = app_after(1, 3, 0, Lang::En);
        key(&mut app, KeyCode::Char('/'));
        write(&mut app, &long[..room]);
        assert_eq!(input_rows(&app, 60, 30), 2);
        assert_eq!(
            caret_of(&app, 60, 30),
            Some((u16::try_from(to.chars().count()).unwrap(), 29))
        );
        key(&mut app, KeyCode::Left);
        assert_eq!(input_rows(&app, 60, 30), 1, "not at its end: one row");
        // More rows than the footer has: the cursor's stays, counted.
        let mut app = app_after(1, 3, 0, Lang::En);
        key(&mut app, KeyCode::Char('/'));
        write(&mut app, &long.repeat(5));
        let s = render(&app, 60, 30);
        let last = s.lines().nth(29).unwrap();
        assert!(last.contains("11/11"), "{last}");
        assert_eq!(input_rows(&app, 60, 30), 8);
        // The rows above went out of view: the cursor is on the last row.
        assert_eq!(caret_of(&app, 60, 30).map(|(_, y)| y), Some(29));
    }

    /// Where the terminal's cursor is after `app` is drawn on a screen of
    /// `width` by `height`; none when it is hidden.
    fn caret_of(app: &App, width: u16, height: u16) -> Option<(u16, u16)> {
        let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        let b = t.backend();
        b.cursor_visible()
            .then(|| (b.cursor_position().x, b.cursor_position().y))
    }

    /// The column `needle` starts at on row `y` of `app` drawn 140 by 40;
    /// every character here is one cell wide.
    fn column_of(app: &App, y: usize, needle: &str) -> u16 {
        let screen = render(app, 140, 40);
        let row = screen.lines().nth(y).unwrap();
        let at = row
            .find(needle)
            .unwrap_or_else(|| panic!("{needle} in {row}"));
        u16::try_from(row[..at].chars().count()).unwrap()
    }

    #[test]
    fn the_terminal_s_cursor_stands_on_its_own_line_at_its_own_character() {
        let mut app = app_after(1, 3, 0, Lang::En);
        assert_eq!(
            caret_of(&app, 140, 40),
            None,
            "hidden while nothing is typed"
        );
        key(&mut app, KeyCode::Char('/'));
        app.input = "one\ntwo\nthree".into();
        app.after = 4; // before the h of three
        assert_eq!(
            caret_of(&app, 140, 40),
            Some((column_of(&app, 39, "three") + 1, 39))
        );
        app.after = 8; // before the w of two
        assert_eq!(
            caret_of(&app, 140, 40),
            Some((column_of(&app, 38, "two") + 1, 38))
        );
        app.after = 0; // the end
        assert_eq!(
            caret_of(&app, 140, 40),
            Some((column_of(&app, 39, "three") + 5, 39))
        );
        // No reversed letter or block of imece's own any more.
        let buf = buffer(&app, 140, 40);
        let reversed = (37..40)
            .flat_map(|y| (0..140).map(move |x| (x, y)))
            .filter(|&(x, y)| buf[(x, y)].modifier.contains(Modifier::REVERSED))
            .count();
        assert_eq!(reversed, 0);
        assert!(!render(&app, 140, 40).contains('█'));
    }

    #[test]
    fn the_cursor_stands_before_its_byte_and_after_a_hidden_token() {
        assert_eq!(caret_line("açb", 1), ("açb".to_string(), 1));
        assert_eq!(
            caret_line("açb", 3),
            ("açb".to_string(), 3),
            "ç is two bytes"
        );
        assert_eq!(
            caret_line("a中b", 4),
            ("a中b".to_string(), 4),
            "中 is three bytes"
        );
        assert_eq!(caret_line("ab", 2), ("ab".to_string(), 2));
        assert_eq!(caret_line("ab", 0), ("ab".to_string(), 0));
        // A line `clean` changes is drawn cleaned, the cursor at its end
        // wherever it is.
        assert_eq!(caret_line("x sk-abc", 3), ("x [***]".to_string(), 7));
        assert_eq!(caret_line("x sk-abc", 0), ("x [***]".to_string(), 7));
    }

    #[test]
    fn a_cursor_past_the_edge_stays_on_the_last_column_and_none_is_off_the_area() {
        let area = Rect::new(2, 3, 5, 2);
        assert_eq!(caret_in(area, 0, 0), Some(Position::new(2, 3)));
        assert_eq!(caret_in(area, 1, 4), Some(Position::new(6, 4)));
        assert_eq!(caret_in(area, 1, 5), Some(Position::new(6, 4)), "cut there");
        assert_eq!(caret_in(area, 0, 99), Some(Position::new(6, 3)));
        assert_eq!(caret_in(area, 2, 0), None, "a row it does not have");
        assert_eq!(caret_in(Rect::new(2, 3, 0, 2), 0, 0), None, "no column");
        assert_eq!(caret_in(Rect::new(2, 3, 5, 0), 0, 0), None, "no row");
    }

    #[test]
    fn the_cursor_s_look_says_what_is_typed() {
        let mut app = app_after(1, 3, 0, Lang::En);
        assert_eq!(look(&app), None);
        app.typing = true;
        app.input = "hello".into();
        let bar = Look {
            shape: Shape::BlinkingBar,
            colour: None,
        };
        assert_eq!(look(&app), Some(bar));
        app.input = ":theme".into();
        assert_eq!(
            look(&app),
            Some(Look {
                shape: Shape::SteadyBar,
                colour: rgb(pal().accent),
            })
        );
        app.input = "#3".into();
        assert_eq!(
            look(&app),
            Some(Look {
                shape: Shape::BlinkingUnderline,
                colour: None,
            })
        );
        assert_eq!(rgb(Color::Rgb(1, 2, 3)), Some([1, 2, 3]));
        assert_eq!(rgb(Color::Indexed(4)), None, "the terminal's own");
        assert_eq!(rgb(Color::Cyan), None);
        assert_eq!(rgb(Color::Reset), None);
    }

    #[test]
    fn the_filters_of_the_help_and_the_archives_have_the_cursor_after_them() {
        let mut app = app_after(1, 3, 0, Lang::En);
        key(&mut app, KeyCode::Char('?'));
        key(&mut app, KeyCode::Char('/'));
        write(&mut app, "esc");
        let screen = render(&app, 140, 40);
        let (y, row) = (screen.lines().enumerate())
            .find(|(_, r)| r.contains(" /esc"))
            .unwrap_or_else(|| panic!("{screen}"));
        let at = row.find(" /esc").unwrap();
        let x = u16::try_from(row[..at].chars().count() + 5).unwrap();
        let y = u16::try_from(y).unwrap();
        assert_eq!(caret_of(&app, 140, 40), Some((x, y)));
        assert!(!screen.contains('▏'), "no bar of imece's own");
        key(&mut app, KeyCode::Enter);
        assert!(!app.help_typing);
        assert_eq!(caret_of(&app, 140, 40), None, "kept, not typed");
        // A short screen: the line saying how much is below comes first.
        let mut app = app_after(1, 3, 0, Lang::En);
        key(&mut app, KeyCode::Char('?'));
        key(&mut app, KeyCode::Char('/'));
        let screen = render(&app, 70, 20);
        let (y, row) = (screen.lines().enumerate())
            .find(|(_, r)| r.contains(" more · "))
            .unwrap_or_else(|| panic!("{screen}"));
        let at = row.find(" more · ").unwrap();
        let at = at + row[at..].find(" /").unwrap();
        let x = u16::try_from(row[..at].chars().count() + 2).unwrap();
        assert_eq!(caret_of(&app, 70, 20), Some((x, u16::try_from(y).unwrap())));
        // The archives.
        let mut app = app_after(1, 3, 0, Lang::En);
        let mut b = crate::archives::Browser::new(&app.world, Vec::new());
        b.typing = true;
        b.find = "ab".into();
        app.archives = Some(b);
        let screen = render(&app, 140, 40);
        let (y, row) = (screen.lines().enumerate())
            .find(|(_, r)| r.contains(" /ab"))
            .unwrap_or_else(|| panic!("{screen}"));
        let at = row.find(" /ab").unwrap();
        let x = u16::try_from(row[..at].chars().count() + 4).unwrap();
        assert_eq!(
            caret_of(&app, 140, 40),
            Some((x, u16::try_from(y).unwrap()))
        );
        assert!(!screen.contains('▏'));
        app.archives.as_mut().unwrap().typing = false;
        assert_eq!(caret_of(&app, 140, 40), None);
    }

    #[test]
    fn the_cursor_is_not_shown_under_a_conversation_or_a_peek() {
        let mut app = app_after(1, 4, 50, Lang::En);
        app.open_thread(4);
        assert!(app.thread.is_some());
        app.typing = true;
        app.input = "one".into();
        assert_eq!(
            caret_of(&app, 140, 40).map(|(_, y)| y),
            Some(39),
            "the foot is clear of the window"
        );
        app.input = "one\ntwo\nthree".into();
        app.after = 8; // on the middle line: the window stays above the composer
        assert_eq!(caret_of(&app, 140, 40).map(|(_, y)| y), Some(38));
        app.thread = None;
        assert_eq!(caret_of(&app, 140, 40).map(|(_, y)| y), Some(38));
        app.peek = Some(1);
        app.after = 0;
        assert_eq!(caret_of(&app, 140, 40).map(|(_, y)| y), Some(39));
        // The peek leaves three rows below it: a footer of five lines has
        // its first under it.
        app.input = "1\n2\n3\n4\n5".into();
        app.after = 8; // the first line
        let rows = caret_of(&app, 140, 40);
        assert_eq!(rows, None, "under the peek");
    }

    /// `completions` alone: a row of `width` cells with `m` drawn in it,
    /// and whether the cell at `x` stands out.
    fn completion_row(m: &crate::complete::Menu, width: u16) -> (String, Vec<bool>) {
        let mut t = Terminal::new(TestBackend::new(width, 1)).unwrap();
        t.draw(|f| completions(f, f.area(), m, Lang::En)).unwrap();
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
        assert_eq!(
            row, " aa  bbb  cccc  dd  e ",
            "an exact fit shows all, no marks"
        );
        assert!(out[1] && out[2], "the highlighted one stands out");
        assert!(out[..4].iter().all(|&o| o));
        assert!(out[4..].iter().all(|&o| !o), "{out:?}");
        // One cell short: what does not fit is counted, never cut.
        let (row, _) = completion_row(&menu_of(&items, 0), 21);
        assert_eq!(row, " aa  bbb  +3 more ›  ");
        let (row, _) = completion_row(&menu_of(&items, 0), 20);
        assert_eq!(row, " aa  bbb  +3 more › ", "the mark fits exactly");
        let (row, _) = completion_row(&menu_of(&items, 0), 19);
        assert_eq!(row, " aa  +4 more ›     ", "one cell short of it");
        // The last one highlighted: the window starts where the earlier
        // ones still fit before it, the ones before counted.
        let (row, out) = completion_row(&menu_of(&items, 4), 21);
        assert_eq!(row, " ‹ 2  cccc  dd  e    ");
        assert!(out[15] && out[16] && out[17] && !out[14] && !out[18]);
        // In the middle: both marks.
        let seven = ["x1", "x2", "x3", "x4", "x5", "x6", "x7"];
        let (row, out) = completion_row(&menu_of(&seven, 3), 20);
        assert_eq!(row, " ‹ 3  x4  +3 more › ");
        assert!(out[5] && out[8] && !out[4] && !out[9]);
        // Pulling the window back looks at the item before, not the one
        // counted already: a big one before a small highlighted one.
        let (row, out) = completion_row(&menu_of(&["xxxxxxxx", "a"], 1), 12);
        assert_eq!(row, " ‹ 1  a     ");
        assert!(out[5] && out[6] && out[7] && !out[4] && !out[8]);
        // The marks' widths are those of their counts: a count of two
        // digits takes a cell more (7 items, 4 after: one digit).
        let (row, _) = completion_row(&menu_of(&seven, 0), 23);
        assert_eq!(row, " x1  x2  x3  +4 more › ");
        let twelve: Vec<String> = (1..=12).map(|i| format!("y{i}")).collect();
        let twelve: Vec<&str> = twelve.iter().map(String::as_str).collect();
        let (row, _) = completion_row(&menu_of(&twelve, 0), 19);
        assert_eq!(row, " y1  +11 more ›    ", "10 after would not fit");
        let (row, _) = completion_row(&menu_of(&twelve, 0), 20);
        assert_eq!(row, " y1  y2  +10 more › ");
        // Pulled back to the first: no mark before, its room is the items'.
        let (row, _) = completion_row(&menu_of(&seven, 1), 19);
        assert_eq!(row, " x1  x2  +5 more › ");
        // Turkish says it in its words.
        let mut t = Terminal::new(TestBackend::new(21, 1)).unwrap();
        t.draw(|f| completions(f, f.area(), &menu_of(&items, 0), Lang::Tr))
            .unwrap();
        let row: String = (0..21)
            .map(|x| t.backend().buffer()[(x, 0)].symbol().to_string())
            .collect();
        assert_eq!(row, " aa  +4 tane daha ›  ");
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
        app.open_window(Window::Journal);
        let s = render(&app, 140, 40);
        assert!(s.contains("of $80.00"), "{s}");
        assert!(s.contains("the budget is $80.00 now"), "{s}");
        let said = format!("you set the budget to $80.00 (was {})", dollars(was));
        assert!(s.contains(&said), "{s}");
        app.close_window();
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
    fn the_key_file_s_first_key_shows_while_it_waits_for_its_second() {
        let mut app = app_after(2, 12, 300, Lang::En);
        app.keys_held = Some((
            KeyEvent::new(
                KeyCode::Char('o'),
                ratatui::crossterm::event::KeyModifiers::CONTROL,
            ),
            std::time::Instant::now(),
        ));
        let s = render(&app, 140, 40);
        let foot = s.lines().last().unwrap();
        assert!(foot.starts_with(" ^o…   "), "{foot}");
        app.keys_held = None;
        let s = render(&app, 140, 40);
        assert!(!s.lines().last().unwrap().contains('…'), "nothing held");
    }

    #[test]
    fn gg_and_capital_g_jump_in_every_list_and_the_chord_shows() {
        let mut app = app_after(2, 12, 300, Lang::En);
        render(&app, 140, 40);
        key(&mut app, KeyCode::Char('g'));
        let s = render(&app, 140, 40);
        let foot = s.lines().last().unwrap();
        assert!(
            foot.starts_with(" g…   "),
            "the pending key shows first, never cut off: {foot}"
        );
        key(&mut app, KeyCode::Char('g'));
        assert_eq!(app.at_home.row, 0);
        key(&mut app, KeyCode::Char('G'));
        assert_eq!(app.at_home.row, app.home_rows().len() - 1);
        app.pending = None;
        // The journal, newest first: gg is the newest line.
        app.open_window(Window::Journal);
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
        app.zoom = Zoom::Focus(0);
        key(&mut app, KeyCode::Char('t'));
        assert_eq!(app.zoom, Zoom::Trace(0));
        key(&mut app, KeyCode::Char('v'));
        let th = app.thread.as_ref().unwrap();
        assert_eq!(th.entries, [0, 1, 3, 4], "you ⇄ lead, from the trace too");
        app.thread = None;
        app.zoom = Zoom::None;
        app.at_home.at = Some(HomeRow::Agent(2));
        key(&mut app, KeyCode::Char('v'));
        assert!(app.thread.is_none());
        let screen = render(&app, 140, 40);
        assert!(screen.contains("no conversation with"), "{screen}");
        // In the leader's focus, the leader is at hand.
        app.zoom = Zoom::Focus(0);
        key(&mut app, KeyCode::Char('v'));
        assert!(app.thread.is_some());
    }

    #[test]
    fn a_note_or_a_search_that_starts_with_a_bang_keeps_its_own_lead() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let t = Lang::En.text();
        app.typing = true;
        app.input = "!ls".into();
        assert_eq!(writing_to(&app).content, t.typing_shell);
        assert_eq!(writing_to(&app).style, shell());
        app.searching = true;
        assert_eq!(writing_to(&app).content, t.typing_history);
        assert_ne!(writing_to(&app).style, shell());
        app.recalling = true;
        assert_eq!(writing_to(&app).content, t.typing_recall);
        app.searching = false;
        assert_eq!(
            writing_to(&app).content,
            t.typing_shell,
            "recalling alone is no search"
        );
        app.recalling = false;
        app.noting = Some(Noting::Line(0, 0));
        assert_eq!(writing_to(&app).content, fill(t.typing_note, &[""]));
        assert_ne!(writing_to(&app).style, shell());
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
        // Its lead on the shell colour, the text after it plain; gone with
        // the `!`.
        let lead = |app: &App| {
            let buf = buffer(app, 140, 40);
            let at = |x: u16| buf[(x, 39)].style();
            (
                at(2),
                at(u16::try_from(" › shell: ".chars().count()).unwrap() + 1),
            )
        };
        let (on, text) = lead(&app);
        assert_eq!((on.bg, on.fg), (Some(pal().shell), Some(pal().on_badge)));
        assert!(on.add_modifier.contains(Modifier::BOLD));
        assert_ne!(text.bg, Some(pal().shell), "only the lead");
        key(&mut app, KeyCode::Backspace);
        assert!(app.typing && !app.typing_shell());
        assert_ne!(lead(&app).0.bg, Some(pal().shell), "a message again");
        key(&mut app, KeyCode::Char('!'));
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
                "cmd in your shell; O quotes it; ^x^e $EDITOR",
                "home, its first row to decide",
            ),
            (
                Lang::Tr,
                "kmt kabuğunda; O alıntılar; ^x^e $EDITOR'da",
                "ana ekran, karar bekleyen ilk satırı",
            ),
        ] {
            app.lang = lang;
            app.help = true;
            let s = render(&app, 140, 40);
            assert!(s.contains(bang), "{s}");
            let row = s.lines().find(|l| l.contains(w)).unwrap_or_default();
            assert!(row.contains(":waiting"), "{row}");
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
        ctrl(&mut app, 'd');
        assert_eq!(app.at_home.row, 5);
        ctrl(&mut app, 'u');
        assert_eq!(app.at_home.row, 0);
    }

    #[test]
    fn the_help_names_the_new_line_key_this_terminal_has_after_the_writing_keys() {
        for lang in [Lang::En, Lang::Tr] {
            let t = lang.text();
            for shift_enter in [true, false] {
                let keys = overlay::help_keys(lang, shift_enter);
                assert_eq!(keys[17].0, "^w ^u ^k ^y", "{lang:?}");
                let want = t.help_newline[usize::from(!shift_enter)];
                assert_eq!(keys[18], want, "{lang:?} {shift_enter}");
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
    fn the_bottom_line_counts_a_close_or_forget_down() {
        // A line written and sent, as the person does (the menu closed
        // first, so enter sends the line as written).
        let typed = |app: &mut App, line: &str| {
            for c in line.chars() {
                key(app, KeyCode::Char(c));
            }
            app.menu = None;
            key(app, KeyCode::Enter);
        };
        for (lang, close, forget) in [
            (
                Lang::En,
                "◷ closing shop in 10 s · u keeps it open",
                "◷ {} forgets its session in 10 s · u keeps it",
            ),
            (
                Lang::Tr,
                "◷ shop 10 sn sonra kapanıyor · u açık tutar",
                "◷ {} 10 sn sonra oturumunu unutuyor · u tutar",
            ),
        ] {
            let mut app = app_after(1, 1, 0, lang);
            app.home = None;
            typed(&mut app, ":open ~/work/shop");
            typed(&mut app, ":close shop");
            assert!(app.grace.is_some(), "{:?}", app.flash);
            let shown = render(&app, 140, 40);
            assert!(shown.contains(close), "{shown}");
            app.flash = Some(("something else".into(), std::time::Instant::now()));
            let shown = render(&app, 140, 40);
            assert!(
                shown.contains("something else") && !shown.contains("◷"),
                "{shown}"
            );
            app.flash = None;
            app.grace = None;
            let lead = app.world.agents[0].name.clone();
            typed(&mut app, &format!("/@{lead} hello"));
            typed(&mut app, &format!(":forget @{lead}"));
            let shown = render(&app, 140, 40);
            assert!(shown.contains(&forget.replace("{}", &lead)), "{shown}");
        }
    }

    #[test]
    fn archiving_puts_a_project_away_after_its_grace_and_a_fresh_one_opens() {
        let typed = |app: &mut App, line: &str| {
            for c in line.chars() {
                key(app, KeyCode::Char(c));
            }
            app.menu = None;
            key(app, KeyCode::Enter);
        };
        let flashed = |app: &App| app.flash.as_ref().map(|f| f.0.clone()).unwrap_or_default();
        for (lang, grace, cut, undone, done, line) in [
            (
                Lang::En,
                "◷ archiving shop as “first epic” in 10 s · u keeps it",
                "◷ archiving shop as “first epic” in 10 s, cutting 1 turns short · u keeps it",
                "shop stays as it is",
                "archived shop as “first epic”: its agents sleep with all they had; a fresh \
                 one opens",
                "you archived shop as “first epic”; a fresh shop takes its place",
            ),
            (
                Lang::Tr,
                "◷ shop “first epic” adıyla arşivleniyor, 10 sn sonra · u tutar",
                "◷ shop “first epic” adıyla arşivleniyor, 10 sn sonra; 1 tur yarıda kesilecek · \
                 u tutar",
                "shop olduğu gibi kalıyor",
                "shop “first epic” adıyla arşivlendi: agent'ları her şeyiyle uyuyor; yerine \
                 yenisi açıldı",
                "shop projesini “first epic” adıyla arşivledin; yerine yeni bir shop açıldı",
            ),
        ] {
            let mut app = app_after(1, 1, 0, lang);
            app.home = None;
            typed(&mut app, ":open ~/work/shop");
            let ours: Vec<AgentId> = (app.world.agents.iter())
                .filter(|a| a.project == 1)
                .map(|a| a.id)
                .collect();
            let worker = ours[1];
            let t = app.world.tick;
            for &id in &ours {
                app.world.agents[id].set_status(Status::Idle, t);
            }
            // Another project's turns are not this archive's to cut.
            for id in 0..ours[0] {
                app.world.agents[id].set_status(Status::Working, t);
            }
            app.at_home.scope = Some(1);
            typed(&mut app, ":archive first epic");
            assert_eq!(app.grace.map(|g| g.0), Some(Undoable::Archive(1)));
            assert_eq!(app.archive_title, "first epic");
            assert!(render(&app, 160, 40).contains(grace), "{lang:?}");
            app.world.agents[worker].set_status(Status::Working, t);
            assert!(render(&app, 160, 40).contains(cut), "{lang:?}");
            key(&mut app, KeyCode::Char('u'));
            assert!(app.grace.is_none());
            assert_eq!(flashed(&app), undone);
            assert!(app.world.projects[1].archived.is_none());

            app.world.agents[worker].set_status(Status::Idle, t);
            app.unseen.insert(worker);
            app.settled.insert(worker);
            // Another project's stay.
            app.unseen.insert(0);
            app.settled.insert(0);
            app.zoom = Zoom::Focus(worker);
            app.inbox_sel = 5;
            app.card = app.world.cards.iter().position(|c| c.project == 1);
            assert!(app.card.is_some());
            let theirs = |app: &App| {
                (journal_rows(&app.world).iter())
                    .filter(|&&(i, _)| {
                        app.world.agents[app.world.journal[i].event.agent()].project == 1
                    })
                    .count()
            };
            assert!(theirs(&app) > 0, "its lines are in sight before");
            typed(&mut app, ":archive first epic");
            app.end_grace();
            let w = &app.world;
            assert_eq!(
                w.projects[1].archived.as_ref().map(|a| a.title.as_str()),
                Some("first epic")
            );
            assert_eq!(w.projects.len(), 3);
            assert_eq!(
                (w.projects[2].name.as_str(), w.projects[2].closed),
                ("shop", false)
            );
            assert_eq!(app.selected, Sel::Agent(w.projects[2].leader));
            assert_eq!((app.zoom, app.card), (Zoom::None, None));
            assert_eq!(app.inbox_sel, app.inbox_rows().len().saturating_sub(1));
            assert_eq!(app.collapsed.len(), 3);
            assert!(!app.unseen.contains(&worker) && !app.settled.contains(&worker));
            assert!(app.unseen.contains(&0) && app.settled.contains(&0));
            assert_eq!(flashed(&app), done);
            assert_eq!(theirs(&app), 0, "its lines wait in the archive");
            assert!(w.shown_cards().all(|c| c.project != 1));
            app.open_window(Window::Journal);
            assert!(render(&app, 300, 40).contains(line), "{lang:?}");
            app.close_window();
            // Its agents sleep: the demo's steps leave them be.
            let before: Vec<(u8, u64)> = (ours.iter())
                .map(|&id| (app.world.agents[id].progress, app.world.agents[id].cost))
                .collect();
            for _ in 0..30 {
                app.step();
            }
            let after: Vec<(u8, u64)> = (ours.iter())
                .map(|&id| (app.world.agents[id].progress, app.world.agents[id].cost))
                .collect();
            assert_eq!(before, after);
            assert!(
                ours.iter()
                    .all(|&id| app.world.agents[id].status == Status::Asleep)
            );
            // A selected header moves to the fresh head too.
            app.at_home.scope = Some(2);
            typed(&mut app, ":archive again");
            app.end_grace();
            assert!(app.world.projects[2].archived.is_some());
            assert_eq!(app.selected, Sel::Agent(app.world.projects[3].leader));
            // Without a title: its name and days; a long one is cut.
            typed(&mut app, ":archive");
            assert_eq!(app.archive_title, app.world.archive_title(3));
            typed(&mut app, &format!(":archive {}", "x".repeat(70)));
            assert_eq!(app.archive_title, "x".repeat(60));
        }
    }

    #[test]
    fn restore_brings_an_archive_back_selects_its_head_and_says_what_was_missing() {
        let typed = |app: &mut App, line: &str| {
            for c in line.chars() {
                key(app, KeyCode::Char(c));
            }
            app.menu = None;
            key(app, KeyCode::Enter);
        };
        let flashed = |app: &App| app.flash.as_ref().map(|f| f.0.clone()).unwrap_or_default();
        for (lang, done, line, none, none_of) in [
            (
                Lang::En,
                "brought shop “first epic” back: its agents sleep until you write to them",
                "you brought back shop “first epic”",
                "no archive is titled or named nothing",
                "shop has no archive",
            ),
            (
                Lang::Tr,
                "shop “first epic” geri geldi: agent'ları sen yazana kadar uyuyor",
                "shop projesinin “first epic” arşivini geri getirdin",
                "nothing başlıklı ya da adlı arşiv yok",
                "shop projesinin arşivi yok",
            ),
        ] {
            let mut app = app_after(1, 1, 0, lang);
            app.home = None;
            typed(&mut app, ":open ~/work/shop");
            app.at_home.scope = Some(1);
            typed(&mut app, ":restore");
            assert_eq!(flashed(&app), none_of, "{lang:?}");
            typed(&mut app, ":restore nothing");
            assert_eq!(flashed(&app), none, "{lang:?}");
            typed(&mut app, ":archive first epic");
            app.end_grace();
            let fresh = 2;
            let head = app.world.projects[fresh].leader;
            app.zoom = Zoom::Focus(head);
            app.unseen.insert(head);
            // An `:archive` waiting acts first.
            typed(&mut app, ":archive x");
            assert!(app.grace.is_some());
            typed(&mut app, ":restore first epic");
            assert!(app.grace.is_none());
            let w = &app.world;
            assert!(
                w.projects[fresh].archived.is_some(),
                "the `:archive x` acted"
            );
            // `x` took 2's place as 3; it did nothing, so it is closed.
            assert!(w.projects[3].closed && w.projects[3].archived.is_none());
            assert!(!w.projects[1].closed && w.projects[1].archived.is_none());
            assert_eq!(app.selected, Sel::Agent(w.projects[1].leader));
            assert_eq!(flashed(&app), done, "{lang:?}");
            app.open_window(Window::Journal);
            assert!(render(&app, 300, 40).contains(line), "{lang:?}");
            app.close_window();
            // What gave way leaves the screen: its focus and its unseen.
            assert_eq!(app.zoom, Zoom::None);
            assert!(!app.unseen.contains(&head));
            // A bare `:restore` brings back the newest of the name at hand.
            typed(&mut app, ":restore");
            assert!(app.world.projects[fresh].archived.is_none());
            assert!(app.world.projects[1].archived.is_some(), "it was swapped");
            assert_eq!(app.selected, Sel::Agent(app.world.projects[fresh].leader));
            // A refusal is said: a solo has the name, the team is renamed.
            app.world.projects[fresh].name = "shop3".into();
            (app.world).open_project(None, "shop".into(), "/s".into(), Role::Solo);
            let row = app.at_home.row;
            typed(&mut app, ":restore shop");
            assert_eq!(flashed(&app), "a project named shop is open already");
            assert!(app.world.projects[1].archived.is_some());
            assert_eq!(app.at_home.row, row, "nothing moved");
        }
    }

    #[test]
    fn a_opens_the_archives_as_a_tree_and_r_brings_the_one_at_hand_back() {
        use crate::archives::Row;
        let typed = |app: &mut App, line: &str| {
            for c in line.chars() {
                key(app, KeyCode::Char(c));
            }
            app.menu = None;
            key(app, KeyCode::Enter);
        };
        let flashed = |app: &App| app.flash.as_ref().map(|f| f.0.clone()).unwrap_or_default();
        for (lang, none, title, checkout, open, row, keys, back) in [
            (
                Lang::En,
                "nothing is archived yet; :archive puts the project at hand away",
                " ARCHIVES ",
                " ▾ shop · 1 archived",
                "   ● shop · open now · 1 agents",
                "   ▸ first epic · $4.00 · 2 cards done · 2 agents · archived {}",
                " j k move · h l za fold · enter view · / filter · r restore · esc close",
                "brought shop “first epic” back",
            ),
            (
                Lang::Tr,
                "henüz arşiv yok; :archive elindeki projeyi kaldırır",
                " ARŞİVLER ",
                " ▾ shop · 1 arşiv",
                "   ● shop · şu an açık · 1 agent",
                "   ▸ first epic · $4.00 · 2 kart bitti · 2 agent · {} arşivlendi",
                " j k gez · h l za katla · enter bak · / süz · r geri getir · esc kapat",
                "shop “first epic” geri geldi",
            ),
        ] {
            let mut app = app_after(1, 1, 0, lang);
            app.home = None;
            key(&mut app, KeyCode::Char('A'));
            assert!(app.archives.is_none());
            assert_eq!(flashed(&app), none, "{lang:?}");
            typed(&mut app, ":open ~/work/shop");
            let ours: Vec<AgentId> = (app.world.agents.iter())
                .filter(|a| a.project == 1)
                .map(|a| a.id)
                .collect();
            let (lead, worker) = (ours[0], ours[1]);
            // What it spent counts its gone agents; its agents do not.
            app.world.agents[lead].cost = 15_000;
            app.world.agents[worker].cost = 25_000;
            for &id in &ours[2..] {
                app.world.agents[id].gone = true;
            }
            // A helper's row goes in further than a leader's.
            app.world.agents[worker].role = Role::Helper;
            // Its cards done count, not the others nor another's.
            let card = |id, project, column| Card {
                id,
                project,
                title: "t".into(),
                owner: None,
                column,
                story: Vec::new(),
                symbols: Vec::new(),
                paths: Vec::new(),
                checks: Vec::new(),
                started: 0,
                finished: 0,
                attempt: 1,
                breaker: Default::default(),
                after: Vec::new(),
            };
            app.world.cards.clear();
            app.world.cards.push(card(0, 1, Column::Done));
            app.world.cards.push(card(1, 1, Column::Working));
            app.world.cards.push(card(2, 0, Column::Done));
            app.world.cards.push(card(3, 1, Column::Done));
            app.at_home.scope = Some(1);
            typed(&mut app, ":archive first epic");
            // The `:archive` waiting acts first, and shows.
            key(&mut app, KeyCode::Char('A'));
            assert!(app.grace.is_none() && app.world.projects[1].archived.is_some());
            let b = app.archives.as_ref().expect("open");
            assert_eq!(b.groups.len(), 1);
            assert_eq!(b.at(&app.world), Some(Row::Archive(1)));
            let at = app.world.projects[1].archived.as_ref().unwrap().at;
            let s = render(&app, 160, 40);
            let row = row.replace("{}", &date(day(at)));
            for want in [title, checkout, open, row.as_str(), keys] {
                assert!(s.contains(want), "{lang:?} {want}\n{s}");
            }
            // `l` shows its agents: the leader first in bold, a helper two
            // cells in, each with what it spent.
            key(&mut app, KeyCode::Char('l'));
            let s = render(&app, 160, 40);
            let name = &app.world.agents[lead].name;
            assert!(s.contains(&format!("│       {name} · $1.50")), "{s}");
            let helper = &app.world.agents[worker];
            let name = &helper.name;
            assert!(s.contains(&format!("│         {name} · $2.50")), "{s}");
            let (buf, _) = whole(&app, 160);
            let bold_at = |name: &str| {
                // From the bottom: the browser's window stands under
                // Home's preview, which names the head too.
                let y = (0..40)
                    .rev()
                    .find(|&y| {
                        let row: String = (0..160).map(|x| buf[(x, y)].symbol()).collect();
                        row.contains(&format!(" {name} · "))
                    })
                    .unwrap_or_else(|| panic!("{name}: {s}"));
                let row: String = (0..160).map(|x| buf[(x, y)].symbol()).collect();
                // The cell, not the byte: glyphs before the name are wide.
                let byte = row.find(name).unwrap();
                let x = row[..byte].chars().count() as u16;
                buf[(x, y)].modifier.contains(Modifier::BOLD)
            };
            assert!(bold_at(&app.world.agents[lead].name), "the head in bold");
            assert!(!bold_at(name), "the rest plain");
            assert!(!s.contains(&app.world.agents[ours[2]].name), "gone: {s}");
            // `?` is the browser's to keep; esc closes it, `:archives` opens it.
            key(&mut app, KeyCode::Char('?'));
            assert!(!app.help && app.archives.is_some());
            key(&mut app, KeyCode::Esc);
            assert!(app.archives.is_none());
            typed(&mut app, ":archives");
            assert!(app.archives.is_some());
            // A paste goes into the filter only while it is typed.
            app.flash = None;
            app.paste("first");
            assert_eq!(flashed(&app), lang.text().paste_not_writing);
            assert_eq!(app.archives.as_ref().unwrap().find, "");
            assert!(!render(&app, 160, 40).contains("│ /"), "no filter line");
            key(&mut app, KeyCode::Char('/'));
            app.paste("first");
            assert_eq!(app.archives.as_ref().unwrap().find, "first");
            assert_eq!(app.scope_below(), None, "no place's keys in it");
            assert!(render(&app, 160, 40).contains("│ /first "));
            assert!(caret_of(&app, 160, 40).is_some(), "the cursor after it");
            key(&mut app, KeyCode::Enter);
            let s = render(&app, 160, 40);
            assert!(s.contains("│ /first ") && !s.contains("▏"), "kept: {s}");
            assert_eq!(caret_of(&app, 160, 40), None, "kept, not typed");
            key(&mut app, KeyCode::Char('/'));
            key(&mut app, KeyCode::Char('%'));
            let s = render(&app, 160, 40);
            assert!(s.contains(lang.text().help_none), "{s}");
            key(&mut app, KeyCode::Backspace);
            key(&mut app, KeyCode::Enter);
            key(&mut app, KeyCode::Char('r'));
            assert!(app.archives.is_none());
            assert!(app.world.projects[1].archived.is_none());
            assert_eq!(app.selected, Sel::Agent(app.world.projects[1].leader));
            assert!(flashed(&app).starts_with(back), "{lang:?}");
        }
    }

    #[test]
    fn an_archive_opens_read_only_under_its_banner_and_exports_what_it_shows() {
        let dir = scratch("archive-view");
        let typed = |app: &mut App, line: &str| {
            for c in line.chars() {
                key(app, KeyCode::Char(c));
            }
            if let Some(v) = app.viewing.as_mut() {
                v.menu = None;
            }
            key(app, KeyCode::Enter);
        };
        let mut app = app_after(2, 2, 120, Lang::En);
        app.selected = Sel::Agent(app.world.projects[1].leader);
        typed(&mut app, ":archive first epic");
        app.end_grace();
        // Something waits on the screen behind: the banner counts it.
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Question,
            agent: app.world.projects[0].leader,
            detail: "x".into(),
            researcher: None,
            open: true,
            tick: app.world.tick,
            card: None,
        });
        let waiting = app.world.open_inbox().len().to_string();
        key(&mut app, KeyCode::Char('A'));
        key(&mut app, KeyCode::Enter);
        let at = app.world.projects[1].archived.as_ref().unwrap().at;
        let (name, other) = (
            app.world.projects[1].name.clone(),
            app.world.projects[0].name.clone(),
        );
        let s = render(&app, 160, 40);
        let banner = format!(
            " ARCHIVE · {name} “first epic” · archived {} · {waiting} to decide now · esc back ",
            date(day(at))
        );
        let top = s.lines().next().unwrap();
        assert!(top.starts_with(&banner), "{s}");
        for (i, l) in s.lines().enumerate() {
            if l.contains(other.as_str()) {
                eprintln!("DBGL {i}: {l}");
            }
        }
        assert!(!s.contains(other.as_str()), "only the archive: {s}");
        // A conversation of it, exported.
        let v = app.viewing.as_mut().unwrap();
        let said = (v.world.journal.iter())
            .position(|e| words::said(&e.event).is_some())
            .expect("its agents talked");
        v.open_thread(said);
        assert!(v.thread.is_some());
        let file = dir.join("archived.md");
        typed(&mut app, &format!(":export {}", file.display()));
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.starts_with("# Conversation: "), "{text}");
        // In Turkish.
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Char('L'));
        let s = render(&app, 160, 40);
        let banner = format!(
            " ARŞİV · {name} “first epic” · {} arşivlendi · şu an {waiting} karar bekliyor · esc geri ",
            date(day(at))
        );
        assert!(s.lines().next().unwrap().starts_with(&banner), "{s}");
        // Back: the screen behind as it was.
        key(&mut app, KeyCode::Esc);
        assert!(app.viewing.is_none());
        let s = render(&app, 160, 40);
        assert!(
            !s.contains("ARCHIVE ·") && s.contains(other.as_str()),
            "{s}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_archives_scroll_to_keep_the_selected_row_in_sight() {
        let mut app = app_after(1, 1, 0, Lang::En);
        for n in 0..8 {
            app.end_grace();
            app.selected = Sel::Agent(app.world.projects[app.world.projects.len() - 1].leader);
            app.on_key(KeyEvent::from(KeyCode::Char(':')));
            for c in format!("archive epic{n}").chars() {
                key(&mut app, KeyCode::Char(c));
            }
            app.menu = None;
            key(&mut app, KeyCode::Enter);
        }
        key(&mut app, KeyCode::Char('A'));
        // A checkout, the project open on it, eight archives newest first.
        let rows = app.archives.as_ref().unwrap().rows(&app.world).len();
        assert_eq!(rows, 10);
        key(&mut app, KeyCode::Char('G'));
        // Twelve rows: the frame, a blank line above and below the rows and
        // the keys leave seven for the rows.
        let buf = buffer(&app, 120, 12);
        let row = |y: u16| -> String { (0..120).map(|x| buf[(x, y)].symbol()).collect() };
        let all: Vec<String> = (0..12).map(row).collect();
        let at = |title: &str| all.iter().position(|r| r.contains(&format!("▸ {title} ")));
        assert_eq!(at("epic0"), Some(8), "the last row, at the foot\n{all:#?}");
        assert_eq!(at("epic6"), Some(2), "seven rows up from it");
        assert_eq!(at("epic7"), None, "the one before is out of sight");
        let x = all[8]
            .find("▸ epic0")
            .map(|b| all[8][..b].chars().count())
            .unwrap();
        let x = u16::try_from(x).unwrap();
        assert!(
            buf[(x, 8)].modifier.contains(Modifier::REVERSED),
            "selected"
        );
        assert!(!buf[(x, 7)].modifier.contains(Modifier::REVERSED));
        // However short the screen, down to the floor, the selected row
        // is drawn; under it nothing is (record 131).
        assert!(!render(&app, 120, MIN_ROWS - 1).contains("epic0"));
        assert!(render(&app, 120, MIN_ROWS).contains("▸ epic0 "));
        // At the top again, the first rows are in sight.
        key(&mut app, KeyCode::Char('g'));
        key(&mut app, KeyCode::Char('g'));
        let s = render(&app, 120, 12);
        assert!(s.contains("· 8 archived") && !s.contains("epic1 "), "{s}");
    }

    #[test]
    fn a_live_checkout_is_written_with_a_tilde_for_the_home() {
        let mut w = crate::live::testing::team(1);
        let home = std::env::temp_dir().join("imece-archives-home");
        let repo = home.join("src").join("kule");
        let live = crate::live::testing::idle_live_in(&repo, 2);
        w.projects[0].closed = true;
        w.projects[0].archived = Some(Archived {
            at: 0,
            title: "epic".into(),
        });
        let mut app = App::with(w, crate::app::Engine::Live(Box::new(live)), Lang::En);
        app.home = Some(home.clone());
        key(&mut app, KeyCode::Char('A'));
        let checkout = |app: &App| app.archives.as_ref().unwrap().groups[0].checkout.clone();
        assert_eq!(checkout(&app), "~/src/kule");
        key(&mut app, KeyCode::Esc);
        // Not under the home: the whole path.
        app.home = Some(home.join("other"));
        key(&mut app, KeyCode::Char('A'));
        assert_eq!(checkout(&app), repo.display().to_string());
    }

    #[test]
    fn restore_completes_the_archives_titles_newest_first_with_name_and_day() {
        let mut app = app_after(1, 1, 0, Lang::En);
        app.world.tick = 10;
        app.world.archive_project(0, "first epic".into());
        app.world.tick = 20;
        app.world.archive_project(1, "second".into());
        let day = crate::model::date(crate::model::day(20));
        let menu = |input: &str| {
            crate::complete::suggest(input, &app.world, 2, None)
                .map(|m| (m.start, m.items, m.notes))
        };
        let name = app.world.projects[0].name.clone();
        let then = crate::model::date(crate::model::day(10));
        assert_eq!(
            menu(":restore "),
            Some((
                9,
                vec!["second".to_string(), "first epic".to_string()],
                vec![format!("{name} · {day}"), format!("{name} · {then}")]
            ))
        );
        assert_eq!(
            menu(":restore FI"),
            Some((
                9,
                vec!["first epic".to_string()],
                vec![format!("{name} · {then}")]
            ))
        );
        assert_eq!(menu(":restore first epic"), None, "typed whole");
        assert_eq!(menu(":restore z"), None);
    }

    #[test]
    fn an_archive_s_cards_leave_the_board_the_queue_and_the_header() {
        let mut app = app_after(1, 2, 0, Lang::En);
        let name = app.world.projects[0].name.clone();
        let theirs: Vec<String> = (app.world.cards.iter()).map(|c| c.title.clone()).collect();
        assert!(theirs.len() >= 2, "the demo has cards");
        app.world.cards[0].column = Column::Done;
        app.world.cards[1].column = Column::Verifying;
        app.window = Some(Window::Board);
        let before = render(&app, 200, 40);
        assert!(before.contains(&theirs[0]));
        let fresh = app.world.archive_project(0, "x".into());
        app.collapsed.push(false);
        let lead = app.world.projects[fresh].leader;
        let c = app.world.cards.len();
        app.world.cards.push(Card {
            id: c,
            project: fresh,
            title: "the next epic".into(),
            owner: Some(lead),
            column: Column::Working,
            story: Vec::new(),
            symbols: Vec::new(),
            paths: Vec::new(),
            checks: Vec::new(),
            started: 0,
            finished: 0,
            attempt: 1,
            breaker: Default::default(),
            after: Vec::new(),
        });
        let board = render(&app, 200, 40);
        for title in &theirs {
            assert!(!board.contains(title.as_str()), "{title}: {board}");
        }
        assert!(board.contains("the next epic"));
        // One project in sight: the cards do not name it.
        let row = (board.lines())
            .skip_while(|l| !l.contains("the next epic"))
            .nth(1)
            .unwrap();
        assert!(!row.contains(&name), "{row}");
        assert!(
            board.contains("IN PROGRESS (1)") && board.contains("DONE (0)"),
            "{board}"
        );
        app.world.cards.last_mut().unwrap().column = Column::Working;
        // Two projects in sight: each card names its own.
        app.world.projects[0].archived = None;
        app.window = Some(Window::Board);
        let board = render(&app, 200, 40);
        let row = (board.lines())
            .skip_while(|l| !l.contains("the next epic"))
            .nth(1)
            .unwrap();
        assert!(row.contains(&name), "{row}");
    }

    #[test]
    fn the_help_shows_the_persons_own_keys_first_and_in_place() {
        let none = crate::keys::Keymap::default();
        for lang in [Lang::En, Lang::Tr] {
            for shift_enter in [true, false] {
                let plain = overlay::help_keys(lang, shift_enter);
                let rows = overlay::help_rows(lang, shift_enter, &none, None, false);
                let want: Vec<(String, String)> = (plain.iter())
                    .map(|(k, w)| (k.to_string(), w.to_string()))
                    .collect();
                assert_eq!(rows, want, "no key file: the help as it is");
            }
            // The rows whose keys are written stay as they are.
            let plain = overlay::help_keys(lang, false);
            let typed: Vec<&str> = overlay::TYPED.iter().map(|&i| plain[i].0).collect();
            assert_eq!(
                typed,
                [
                    "$",
                    "tab ↑ ↓ ^r",
                    "^a ^e ⌥b ⌥f",
                    "^w ^u ^k ^y",
                    "\\enter ⌥enter"
                ],
                "{lang:?}"
            );
        }
        let keys = crate::keys::Keymap::read("^n = j\nj = G\n↑ = k\n^w = q").unwrap();
        let rows = overlay::help_rows(Lang::En, false, &keys, None, false);
        let row = |k: &str| -> Vec<&str> {
            (rows.iter().filter(|r| r.1 == k))
                .map(|r| r.0.as_str())
                .collect()
        };
        assert_eq!(
            rows[..4],
            [
                ("^n".to_string(), "your key: as j".to_string()),
                ("j".into(), "your key: as G".into()),
                ("↑".into(), "your key: as k".into()),
                ("^w".into(), "your key: as q".into()),
            ]
        );
        assert_eq!(row("select"), ["– ↓  ^n ↑/k"]);
        assert_eq!(
            row("first / last line · Home: to decide · at work"),
            ["gg  j/G  gd  gw"]
        );
        assert_eq!(row("quit (q or :quit asks first)"), ["^w/q ^w/q  :quit"]);
        assert_eq!(
            row("writing: complete · sent lines · :history"),
            ["tab ↑ ↓ ^r"],
            "written keys are not turned"
        );
        assert_eq!(
            row("writing: cut a word / before / after; paste"),
            ["^w ^u ^k ^y"]
        );
        let tr = overlay::help_rows(Lang::Tr, false, &keys, None, false);
        assert_eq!(tr[0], ("^n".to_string(), "senin tuşun: j gibi".to_string()));
        assert_eq!(tr.len(), rows.len());
        // Drawn, and found by the filter.
        let mut app = app_after(1, 4, 0, Lang::En);
        app.keys = keys;
        app.help = true;
        let shown = render(&app, 140, 50);
        assert!(shown.contains("your key: as G"), "{shown}");
        app.help_find = "your key".into();
        let found = render(&app, 140, 50);
        assert!(found.contains("your key: as q"), "{found}");
        assert!(!found.contains("first / last line"), "{found}");
        // Keys of two, on either side.
        let two = crate::keys::Keymap::read("gt = G\nx = gg\n^t = z M").unwrap();
        let rows = overlay::help_rows(Lang::En, false, &two, None, false);
        let row = |k: &str| -> Vec<&str> {
            (rows.iter().filter(|r| r.1 == k))
                .map(|r| r.0.as_str())
                .collect()
        };
        assert_eq!(
            rows[..3],
            [
                ("gt".to_string(), "your key: as G".to_string()),
                ("x".into(), "your key: as gg".into()),
                ("^t".into(), "your key: as zM".into()),
            ]
        );
        assert_eq!(
            row("first / last line · Home: to decide · at work"),
            ["x/gg  gt/G  gd  gw"]
        );
        // A place's keys: said where, and the rows as they are there.
        let placed = crate::keys::Keymap::read("J = G\n[change]\n^n = j\nJ = gg").unwrap();
        let rows = |lang, place| overlay::help_rows(lang, false, &placed, place, false);
        let change = rows(Lang::En, Some(crate::keys::Scope::Change));
        assert_eq!(
            change[..3],
            [
                ("J".to_string(), "your key: as G".to_string()),
                ("^n".into(), "your key in a change: as j".into()),
                ("J".into(), "your key in a change: as gg".into()),
            ]
        );
        let row = |rows: &[(String, String)], k: &str| -> Vec<String> {
            (rows.iter().filter(|r| r.1 == k))
                .map(|r| r.0.clone())
                .collect()
        };
        let first_last = "first / last line · Home: to decide · at work";
        assert_eq!(row(&change, first_last), ["J/gg  G  gd  gw"]);
        assert_eq!(row(&change, "select"), ["↑ ↓  ^n/j k"]);
        let salon = rows(Lang::En, Some(crate::keys::Scope::Salon));
        assert_eq!(row(&salon, first_last), ["gg  J/G  gd  gw"]);
        assert_eq!(row(&salon, "select"), ["↑ ↓  j k"]);
        assert_eq!(rows(Lang::En, None), salon);
        let tr = rows(Lang::Tr, Some(crate::keys::Scope::Change));
        assert_eq!(tr[1].1, "senin tuşun, değişiklikte: j gibi");
        // Filtered to a screen, the file's lines of another place go.
        let scoped = |place| overlay::help_rows(Lang::En, false, &placed, Some(place), true);
        let salon = scoped(crate::keys::Scope::Salon);
        assert_eq!(salon[0], ("J".to_string(), "your key: as G".to_string()));
        assert!(
            !salon.iter().any(|r| r.1.contains("in a change")),
            "{salon:?}"
        );
        let change = scoped(crate::keys::Scope::Change);
        assert_eq!(change[1].1, "your key in a change: as j");
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
        // Two columns: a line of the first half carries one of the second
        // (the split is by rows drawn, so which one depends on wrapping).
        let keys = overlay::help_keys(Lang::En, false);
        // A long text wraps under its key, so the key is what the line
        // surely carries.
        // A key of one letter (r) is in every line's words: left out.
        let second: Vec<&str> = (keys[keys.len() / 2..].iter())
            .map(|k| k.0)
            .filter(|k| k.chars().count() > 1)
            .collect();
        let line = wide.lines().find(|l| l.contains("gg  G")).unwrap();
        assert!(
            second.iter().any(|o| line.contains(o)),
            "two columns: {line}"
        );
        let narrow = render(&app, 80, 40);
        let line = narrow.lines().find(|l| l.contains("gg  G")).unwrap();
        assert!(
            !second.iter().any(|o| line.contains(o)),
            "one column: {line}"
        );
    }

    #[test]
    fn a_colon_lead_is_no_command_any_more() {
        let mut app = app_after(1, 2, 0, Lang::En);
        for c in ":lead claude".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        assert!(render(&app, 140, 40).contains("no command lead"));
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
        let first_last = (
            "gg  G  gd  gw",
            "first / last line · Home: to decide · at work",
        );
        assert_eq!(got, [&first_last], "every word");
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
        assert!(s.contains(" /fold ") && s.contains("enter keep"), "{s}");
        assert!(caret_of(&app, 140, 40).is_some(), "the cursor after it");
        // Enter keeps it; / goes back to it; any other key closes.
        key(&mut app, KeyCode::Enter);
        assert!(app.help && !app.help_typing);
        assert_eq!(app.help_find, "fold");
        let s = render(&app, 140, 40);
        assert!(s.contains(" /fold ") && !s.contains("▏"), "{s}");
        assert_eq!(caret_of(&app, 140, 40), None, "kept, not typed");
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
        key(&mut app, KeyCode::Char('x'));
        assert!(!app.help, "closed");
        key(&mut app, KeyCode::Char('?'));
        assert!(app.help && app.help_find.is_empty(), "{:?}", app.help_find);
    }

    #[test]
    fn the_help_opens_with_this_screens_keys_and_question_mark_again_with_every_key() {
        use crate::keys::Scope;
        let settle = "hide the row at hand until it needs you";
        let hunk = "the hunk at the top, quoted in a message to its worker";
        let wrap = "in a change: wrap long lines";
        let mut app = app_after(1, 2, 0, Lang::En);
        let t = Lang::En.text();
        key(&mut app, KeyCode::Char('?'));
        assert!(app.help && app.help_scoped());
        let s = render(&app, 188, 60);
        assert!(s.contains(t.help_here.trim()), "{s}");
        assert!(s.contains(settle) && s.contains(":budget USD"), "{s}");
        assert!(
            !s.contains(hunk) && !s.contains(wrap),
            "a change's keys: {s}"
        );
        // `?` again: every key; again: this screen's.
        key(&mut app, KeyCode::Char('?'));
        assert!(app.help && !app.help_scoped());
        let s = render(&app, 188, 60);
        assert!(s.contains(t.help_every.trim()), "{s}");
        assert!(s.contains(hunk) && s.contains(settle), "{s}");
        key(&mut app, KeyCode::Char('?'));
        assert!(app.help && app.help_scoped());
        assert!(!render(&app, 188, 60).contains(hunk));
        // Opened anew, this screen's again; Turkish words the title.
        key(&mut app, KeyCode::Esc);
        app.help_all = true;
        app.lang = Lang::Tr;
        key(&mut app, KeyCode::Char('?'));
        assert!(app.help_scoped());
        assert!(render(&app, 188, 60).contains(Lang::Tr.text().help_here.trim()));
        // Over a window that is no place (the first run's), every key.
        app.lang = Lang::En;
        app.welcome = true;
        app.help = true;
        app.help_all = false;
        assert_eq!(app.scope_below(), None);
        let s = render(&app, 188, 60);
        assert!(s.contains(t.help_title.trim()) && !s.contains(t.help_here.trim()));
        assert!(s.contains(hunk) && s.contains(settle), "{s}");
        // The rows as the table places them.
        let keys = overlay::help_keys(Lang::En, false);
        assert_eq!(keys[13].0, ":pin NAME");
        assert_eq!(keys[27].0, "s  :settle");
        assert_eq!(keys[44].0, ">");
        assert_eq!(keys[47].0, "w  R");
        assert!(
            keys[18].1.starts_with("writing: a new line"),
            "{:?}",
            keys[18]
        );
        assert!(
            overlay::help_shown(18, Scope::Board),
            "the new-line line, everywhere"
        );
        assert!(overlay::help_shown(0, Scope::Board) && overlay::help_shown(13, Scope::Salon));
        assert!(!overlay::help_shown(13, Scope::Board), ":pin is the team's");
        assert!(overlay::help_shown(44, Scope::Change) && !overlay::help_shown(44, Scope::Salon));
        assert!(overlay::help_shown(27, Scope::Salon) && !overlay::help_shown(27, Scope::Change));
    }

    #[test]
    fn the_help_in_a_change_and_in_the_windows_lists_their_keys() {
        let settle = "hide the row at hand until it needs you";
        let hunk = "the hunk at the top, quoted in a message to its worker";
        let copy = "copy the message · the conversation";
        let scale = "home: one project · change: file · timeline: scale";
        let mut app = app_after(1, 2, 0, Lang::En);
        app.zoom = Zoom::Diff(app.world.agents[1].id);
        key(&mut app, KeyCode::Char('?'));
        let s = render(&app, 188, 60);
        assert!(s.contains(hunk) && s.contains(scale), "{s}");
        assert!(
            !s.contains(settle) && !s.contains(copy) && !s.contains(":pin NAME"),
            "{s}"
        );
        key(&mut app, KeyCode::Esc);
        app.zoom = Zoom::None;
        app.command("journal");
        key(&mut app, KeyCode::Char('?'));
        let s = render(&app, 188, 60);
        assert!(
            s.contains(copy) && s.contains("mark the line at hand"),
            "{s}"
        );
        assert!(
            !s.contains(hunk) && !s.contains(scale) && !s.contains(settle),
            "{s}"
        );
        key(&mut app, KeyCode::Esc);
        app.command("timeline");
        key(&mut app, KeyCode::Char('?'));
        let s = render(&app, 188, 60);
        assert!(s.contains(scale) && s.contains(":budget USD"), "{s}");
        assert!(!s.contains(settle) && !s.contains(copy), "{s}");
    }

    #[test]
    fn the_help_s_filter_searches_every_key_and_esc_narrows_it_again() {
        let hunk = "the hunk at the top, quoted in a message to its worker";
        let mut app = app_after(1, 2, 0, Lang::En);
        let t = Lang::En.text();
        key(&mut app, KeyCode::Char('?'));
        key(&mut app, KeyCode::Char('/'));
        assert!(
            !app.help_scoped(),
            "typing a filter is a search over every key"
        );
        for c in "hunk".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        let s = render(&app, 188, 60);
        assert!(s.contains(hunk) && s.contains(t.help_every.trim()), "{s}");
        key(&mut app, KeyCode::Enter);
        assert!(!app.help_scoped() && !app.help_find.is_empty());
        assert!(render(&app, 188, 60).contains(hunk));
        // Back to the filter, dropped: this screen's keys again.
        key(&mut app, KeyCode::Char('/'));
        key(&mut app, KeyCode::Esc);
        assert!(
            app.help && app.help_scoped(),
            "the filter dropped, this screen's keys"
        );
        let s = render(&app, 188, 60);
        assert!(!s.contains(hunk) && s.contains(t.help_here.trim()), "{s}");
    }

    #[test]
    fn a_filtered_help_shows_what_matches_and_keeps_its_size() {
        let t = Lang::En.text();
        let mut app = app_after(1, 4, 50, Lang::En);
        app.help = true;
        app.help_all = true;
        // The rows of the frame's top and bottom corners, on a screen the
        // whole help fits.
        let frame = |app: &App, width: u16| {
            let buf = buffer(app, width, 50);
            let ys: Vec<u16> = (0..50)
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
            let s = render(&app, 140, 50);
            for k in keys {
                assert!(s.contains(k), "{find}: {k} {s}");
            }
            assert_eq!(s.contains("gg  G"), find.is_empty(), "{find}: {s}");
            assert_eq!(s.contains("≈ tainted"), legend, "{find}: {s}");
            assert_eq!(s.contains("▴ pinned"), find.is_empty(), "{find}: {s}");
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
    fn the_help_scrolls_where_the_screen_is_too_short_for_it() {
        let t = Lang::En.text();
        let mut app = app_after(1, 4, 50, Lang::En);
        app.on_key(KeyEvent::from(KeyCode::Char('?')));
        app.help_all = true;
        // One column: the keys (a long text on two lines), a blank, the
        // legend, a blank: more lines than twenty rows less the frame and
        // the last line.
        let rows = overlay::help_rows(Lang::En, false, &app.keys, None, false);
        let rows: Vec<_> = rows.iter().collect();
        let keys = overlay::help_columns(&rows, false, overlay::help_widths(80, false)).len();
        assert!(keys > rows.len(), "some wrap");
        let (key_cells, text_cells) = overlay::help_widths(80, false);
        let legend: usize = (t.help_legend.iter())
            .map(|l| overlay::legend_wrap(l, 1 + key_cells + text_cells).len())
            .sum();
        assert!(legend > t.help_legend.len(), "the legend wraps");
        let lines = keys + legend + 2;
        let room = 20 - 3;
        let first = |app: &App| {
            let s = render(app, 80, 20);
            assert!(s.contains(t.help_close.trim()), "the last line stays: {s}");
            s
        };
        let s = first(&app);
        assert!(s.contains(" /  i "), "the top: {s}");
        let below = lines - room;
        assert!(
            s.contains(&format!(" ↓ {below} more · j k scroll · / filter")),
            "{s}"
        );
        assert_eq!(app.help_room.get(), (room, below));
        // `j` scrolls a line and keeps the help open; `k` back.
        key(&mut app, KeyCode::Char('j'));
        assert!(app.help);
        assert_eq!(app.help_scroll, 1);
        let s = first(&app);
        assert!(!s.contains(" /  i ") && s.contains(" @name text"), "{s}");
        assert!(s.contains(&format!(" ↓ {} more", below - 1)), "{s}");
        key(&mut app, KeyCode::Up);
        key(&mut app, KeyCode::Char('k'));
        assert_eq!(app.help_scroll, 0, "not above the top");
        // Half a page, a page, back; never past the last.
        let ctrl =
            |app: &mut App, c| app.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL));
        ctrl(&mut app, 'd');
        assert_eq!(app.help_scroll, room / 2);
        ctrl(&mut app, 'u');
        assert_eq!(app.help_scroll, 0);
        ctrl(&mut app, 'f');
        assert_eq!(app.help_scroll, room);
        ctrl(&mut app, 'b');
        assert_eq!(app.help_scroll, 0);
        key(&mut app, KeyCode::PageDown);
        assert_eq!(app.help_scroll, room);
        key(&mut app, KeyCode::PageUp);
        assert_eq!(app.help_scroll, 0);
        key(&mut app, KeyCode::Down);
        key(&mut app, KeyCode::Char('G'));
        assert_eq!(app.help_scroll, below);
        let s = first(&app);
        let foot: String = (t.help_legend[t.help_legend.len() - 1].chars())
            .take(24)
            .collect();
        assert!(s.contains(&foot), "the foot: {s}");
        assert!(
            s.contains(" ↑ j k scroll · / filter") && !s.contains("more · j k"),
            "{s}"
        );
        for _ in 0..3 {
            key(&mut app, KeyCode::Char('j'));
            ctrl(&mut app, 'd');
            ctrl(&mut app, 'f');
            key(&mut app, KeyCode::PageDown);
            key(&mut app, KeyCode::End);
        }
        assert_eq!(app.help_scroll, below, "not past the last");
        // `g` waits for the second; `gg` and home go to the top.
        key(&mut app, KeyCode::Char('g'));
        assert_eq!(app.help_scroll, below);
        key(&mut app, KeyCode::Char('g'));
        assert_eq!(app.help_scroll, 0);
        key(&mut app, KeyCode::End);
        key(&mut app, KeyCode::Home);
        assert_eq!(app.help_scroll, 0);
        // A key after `g` that is not `g` acts as itself.
        key(&mut app, KeyCode::Char('g'));
        key(&mut app, KeyCode::Char('j'));
        assert_eq!(app.help_scroll, 1);
        // The filter shows from its top; what it finds fits.
        key(&mut app, KeyCode::Char('/'));
        assert_eq!(app.help_scroll, 1, "not before a letter");
        key(&mut app, KeyCode::Char('z'));
        assert_eq!(app.help_scroll, 0);
        key(&mut app, KeyCode::Enter);
        // Any other key closes it, ctrl ones too; it opens at the top.
        ctrl(&mut app, 'x');
        assert!(!app.help);
        // The page keys are ctrl's: the letters alone close it, and ctrl-j
        // is not j.
        for close in [
            KeyEvent::from(KeyCode::Char('d')),
            KeyEvent::from(KeyCode::Char('u')),
            KeyEvent::from(KeyCode::Char('f')),
            KeyEvent::from(KeyCode::Char('b')),
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
        ] {
            key(&mut app, KeyCode::Char('?'));
            app.help_all = true;
            app.help_scroll = 2;
            app.on_key(close);
            assert!(!app.help, "{close:?}");
        }
        app.help_scroll = 4;
        key(&mut app, KeyCode::Char('?'));
        app.help_all = true;
        assert!(app.help && app.help_scroll == 0);
        // However short the screen, down to the floor, its lines less the
        // frame and the last line are in sight and ^d moves half of them.
        render(&app, 80, MIN_ROWS);
        let least = usize::from(MIN_ROWS) - 3;
        assert_eq!(app.help_room.get(), (least, lines - least));
        ctrl(&mut app, 'd');
        assert_eq!(app.help_scroll, least / 2);
        // Under the floor nothing is drawn, so no key moves it (record 131).
        render(&app, 80, MIN_ROWS - 1);
        ctrl(&mut app, 'd');
        assert_eq!(app.help_scroll, least / 2);
        app.help_scroll = 0;
        // A screen it fits: nothing to say, nothing to scroll.
        let s = render(&app, 80, 100);
        assert!(!s.contains("more · j k") && !s.contains("↑ j k"), "{s}");
        assert_eq!(app.help_room.get().1, 0);
        key(&mut app, KeyCode::Char('j'));
        assert!(app.help && app.help_scroll == 0);
        key(&mut app, KeyCode::Char('x'));
        assert!(!app.help);
    }

    #[test]
    fn the_help_window_is_as_tall_as_its_lines_and_their_frame() {
        let mut app = app_after(1, 4, 50, Lang::En);
        app.help = true;
        app.help_all = true;
        let t = Lang::En.text();
        // Forty rows: the help fits a screen that tall whole where it is
        // wide enough for every row on one line (188: keys 16, texts 74).
        let buf = buffer(&app, 188, 40);
        let row = |y: u16| -> String { (0..188).map(|x| buf[(x, y)].symbol()).collect() };
        let top = (0..40).find(|&y| row(y).contains(t.help_title)).unwrap();
        // Two columns of keys, a blank line, the legend, a blank line, close.
        let keys = overlay::help_keys(Lang::En, false).len();
        assert_eq!(keys, t.help_keys.len() + 1, "and the new-line line");
        let lines = u16::try_from(keys.div_ceil(2) + t.help_legend.len() + 3).unwrap();
        let x = (188 - 186) / 2;
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
        // Whole on forty rows, with no row to spare.
        assert_eq!(top + lines + 2, 40, "it fills the forty rows");
        assert!(
            (top..top + lines).any(|y| row(y).contains("⎇ branch  ↑ own commits")),
            "the legend says what a worker's row counts"
        );
        assert!(
            (top..top + lines).any(|y| row(y).contains("↗ a local address the worker wrote")),
            "and what its link is"
        );
        // Narrower, its long texts take two lines and it scrolls.
        let s = render(&app, 140, 40);
        assert!(s.contains(" more · j k scroll"), "{s}");
        // One column: a blank, the key, a blank, the text and the frame.
        let buf = buffer(&app, 80, 100);
        let line = |y: u16| -> String { (0..80).map(|x| buf[(x, y)].symbol()).collect() };
        let top = (0..100).find(|&y| line(y).contains(t.help_title)).unwrap();
        let corners: Vec<u16> = (0..80)
            .filter(|&x| matches!(buf[(x, top)].symbol(), "┌" | "┐"))
            .collect();
        assert_eq!(corners, [9, 9 + (2 + 13 + 44 + 2) - 1], "61 wide");
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
        assert_eq!(
            shell(),
            Style::new()
                .fg(p.on_badge)
                .bg(p.shell)
                .add_modifier(Modifier::BOLD)
        );
        assert_eq!(speaker_color(0), p.speakers[0]);
        assert_eq!(speaker_color(13), speaker_color(1), "colours wrap by id");
        crate::theme::set("mono");
        assert_eq!(dim(), Style::new().add_modifier(Modifier::DIM));
        assert_eq!(
            brand(),
            Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        );
        assert_eq!(attention(), brand());
        assert_eq!(shell(), brand(), "reversed: plain without colour");
        assert_eq!(key_style(), Style::new().add_modifier(Modifier::REVERSED));
        assert_eq!(changed(), Style::new().add_modifier(Modifier::BOLD));
        assert_eq!(border(), dim());
        crate::theme::set("terminal");
    }

    #[test]
    fn every_glyph_icon_and_status_has_its_own_mark() {
        let mut a = Agent::new(1, 0, "w".into(), Role::Worker, Status::Working, 0);
        assert_eq!(glyph(&a).0, "●");
        a.role = Role::Leader;
        assert_eq!(glyph(&a), ("●", pal().ok), "a leader's is its status's");
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
        // Five glyphs (record 125): the colour tells asking from working
        // and blocked from stopped, the status words do without it.
        assert_eq!(marks, ["○", "●", "●", "■", "!", "●", "■", "○"]);
        let colours: Vec<Color> = [
            Status::Asking { to: 0, until: 1 },
            Status::Blocked,
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
            glyph(&a).1
        })
        .collect();
        let p = pal();
        assert_eq!(colours, [p.warn, p.bad, p.research, p.stopped, p.dim]);
        // On the ladder: a rung's own glyph for waiting, done and
        // starting, the status's for the rest.
        assert!(!a.starting, "new: not starting");
        a.status = Status::Working;
        assert_eq!(rung_glyph(&a, Rung::Waiting), ("!", pal().warn));
        assert_eq!(rung_glyph(&a, Rung::Done), ("✓", pal().ok));
        assert_eq!(rung_glyph(&a, Rung::Starting), ("●", pal().ok));
        assert_eq!(rung_glyph(&a, Rung::Working), ("●", pal().ok));
        a.status = Status::Stopped;
        assert_eq!(rung_glyph(&a, Rung::Stuck), ("■", pal().stopped));
        a.status = Status::Blocked;
        assert_eq!(rung_glyph(&a, Rung::Stuck), ("■", pal().bad));
        a.status = Status::Asleep;
        assert_eq!(rung_glyph(&a, Rung::Idle), ("○", pal().dim));
        // A leader's row is on the ladder like anyone's.
        a.role = Role::Leader;
        a.status = Status::Working;
        assert_eq!(rung_glyph(&a, Rung::Waiting), ("!", pal().warn));
        assert_eq!(rung_glyph(&a, Rung::Done), ("✓", pal().ok));
        assert_eq!(rung_glyph(&a, Rung::Working), ("●", pal().ok));
        a.role = Role::Worker;
        for lang in [Lang::En, Lang::Tr] {
            let legend = lang.text().help_legend.join(" ");
            for g in ["!", "●", "○", "✓", "■"] {
                assert!(legend.contains(&format!(" {g} ")), "{lang:?} {g}");
            }
            for g in ["◆", "◌", "◐", "◇", "✕", "◦", "~"] {
                assert!(!legend.contains(g), "{lang:?} {g}: gone from the legend");
            }
        }
        a.taint = Taint::Clean;
        assert_eq!(taint_span(&a).content, " ");
        a.taint = Taint::Soft;
        assert_eq!(taint_span(&a).content, " ", "soft-tainted says nothing");
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
        // The leader's acts are an arrow like an assignment, in the plan
        // colour; blocked and failed are stuck, told apart by colour.
        assert_eq!(entry_icon(EntryKind::Plan), ("→", fg(pal().plan)));
        assert_eq!(entry_icon(EntryKind::Assign), ("→", dim()));
        assert_eq!(entry_icon(EntryKind::Blocked), ("■", fg(pal().bad)));
        assert_eq!(entry_icon(EntryKind::Failed), ("■", fg(pal().stopped)));
        // Every status says something, in words.
        let w = app_after(1, 4, 50, Lang::En).world;
        let mut idle = Agent::new(9, 0, "w".into(), Role::Worker, Status::Idle, 0);
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
    fn the_header_says_the_pot_and_how_long_it_lasts_only_once_it_runs_low() {
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
        let row = head(&app);
        assert!(row.contains("$0.00 of $1.00  "), "{row}");
        assert!(!row.contains("spent"), "{row}");
        assert!(!row.contains("/h") && !row.contains("left"), "{row}");
        app.world.tick = 1;
        app.world.agents[0].cost = 100;
        app.world.watch_spending();
        let row = head(&app);
        assert!(row.contains("$0.01 of $1.00  "), "{row}");
        assert!(
            !row.contains("left"),
            "the rate and the time left are :budget's: {row}"
        );
        assert_eq!(at(&app, "$0.01"), looks(Style::default()));
        app.world.budget = 100 + 299;
        let row = head(&app);
        assert!(row.contains("$0.01 of $0.03, ~29 min left"), "{row}");
        assert_eq!(at(&app, "$0.01"), looks(attention()));
        assert_eq!(at(&app, "left"), looks(attention()));
        app.lang = Lang::Tr;
        let row = head(&app);
        assert!(row.contains("$0.01 / $0.03, ~29 dk kaldı"), "{row}");
    }

    #[test]
    fn the_header_shows_what_is_spent_of_the_budget_and_the_footer_no_longer_names_the_views() {
        let mut app = app_after(1, 2, 0, Lang::En);
        app.world.budget = 10_000;
        for a in &mut app.world.agents {
            a.cost = 0;
        }
        app.world.agents[0].cost = 2_500;
        app.world.agents[1].cost = 2_500;
        let screen = render(&app, 170, 40);
        assert!(screen.contains("imece"));
        assert!(screen.contains("│ $0.50 of $1.00"), "{screen}");
        // On a plan, with a turn never priced: what it spent at least, its
        // API-equivalent (record 132); the budget is the person's own.
        app.world.estimated = true;
        app.world.agents[1].unpriced.gaps = 1;
        let screen = render(&app, 170, 40);
        assert!(screen.contains("│ ≈$0.50+? of $1.00"), "{screen}");
        // The footer names no view and no pause: the status line says where
        // you are, the header says PAUSED.
        let footer = screen.lines().last().unwrap().to_string();
        assert!(
            !footer.contains("space") && !footer.contains("1 salon"),
            "{footer}"
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
        assert!(screen.contains(" ■ 1 blocks"), "{screen}");
        assert!(screen.contains(" ✓ 2 cards merged"), "{screen}");
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
    fn the_welcome_window_says_what_claude_code_answered_and_the_keys() {
        use crate::welcome::Seen;
        use imece_agent::Login;
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let here = |version: &str, login| Seen::Here {
            version: version.into(),
            login,
        };
        app.seen = here("2.1.3 (Claude Code)", Login::Yes { subscription: true });
        app.welcome = true;
        let screen = render(&app, 120, 40);
        for want in [
            &format!("WELCOME TO IMECE {}", crate::welcome::VERSION),
            "One terminal for a team of Claude Code agents.",
            "Claude Code on this machine:",
            "✓ claude  2.1.3 (Claude Code) · signed in, a plan · ≈ API prices",
            "Keys to start with:",
            "   /  i            write to the leader",
            "   ?               every key; / in it filters",
            "any key: close · :welcome shows this again",
        ] {
            assert!(screen.contains(want), "{want} missing:\n{screen}");
        }
        // An API key's figures are what it pays.
        app.seen = here(
            "2.1.3 (Claude Code)",
            Login::Yes {
                subscription: false,
            },
        );
        let screen = render(&app, 120, 40);
        assert!(screen.contains("✓ claude  2.1.3 (Claude Code) · signed in "));
        assert!(!screen.contains("a plan"), "{screen}");
        // Not signed in, in a CLI's words, cleaned.
        app.seen = here("2.1.3 \u{1b}[31mred", Login::No);
        let screen = render(&app, 120, 40);
        assert!(!screen.contains('\u{1b}'), "a CLI's words are cleaned");
        assert!(
            screen.contains("✗ claude  2.1.3")
                && screen.contains("red · not signed in: run claude auth login"),
            "{screen}"
        );
        app.seen = here("0.65.0", Login::Unknown);
        let screen = render(&app, 120, 40);
        assert!(
            screen.contains("? claude  0.65.0 · its sign-in is checked when it first starts"),
            "{screen}"
        );
        // Before the answer, and not there.
        app.seen = Seen::Checking;
        let screen = render(&app, 120, 40);
        assert!(screen.contains("… claude  checking…"), "{screen}");
        app.seen = Seen::Missing;
        let screen = render(&app, 120, 40);
        assert!(screen.contains("· claude  not on PATH"), "{screen}");
        // In Turkish.
        app.lang = Lang::Tr;
        let screen = render(&app, 120, 40);
        assert!(screen.contains("Bu makinedeki Claude Code:"), "{screen}");
        assert!(screen.contains("PATH'te yok"), "{screen}");
    }

    #[test]
    fn the_news_window_lists_each_version_newest_first() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.news = vec!["0.1.0"];
        let screen = render(&app, 120, 40);
        for want in [
            &format!("WHAT'S NEW IN IMECE {}", crate::welcome::VERSION),
            " 0.1.0",
            "· The first release, for macOS and Linux",
            "any key: close · :news shows this again",
        ] {
            assert!(screen.contains(want), "{want} missing:\n{screen}");
        }
        // The first version right under imece's mark (a blank row, four,
        // a blank row), no blank line more.
        let lines: Vec<&str> = screen.lines().collect();
        let top = lines.iter().position(|l| l.contains("WHAT'S NEW")).unwrap();
        assert!(lines[top + 2].contains("imece v"), "{screen}");
        assert!(lines[top + 7].contains("│ 0.1.0"), "{screen}");
        // The news come before the welcome, which comes before where the
        // person left off.
        app.welcome = true;
        app.left_off = Some(Default::default());
        assert!(render(&app, 120, 40).contains("WHAT'S NEW"));
        app.news.clear();
        let screen = render(&app, 120, 40);
        assert!(screen.contains("WELCOME TO IMECE") && !screen.contains("WHAT'S NEW"));
        app.welcome = false;
        assert!(render(&app, 120, 40).contains("WHERE YOU LEFT OFF"));
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
            "! 2 to decide:",
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
        assert_eq!(at("14:00 ⇄ claude-1"), at("─ 1970-01-01") + 1, "{screen}");
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
        // Panels cycle only on the salon, outside a focus: a window over
        // it takes the keys.
        app.window = Some(Window::Board);
        let pane = app.pane;
        for code in [
            KeyCode::Tab,
            KeyCode::BackTab,
            KeyCode::Char('['),
            KeyCode::Char(']'),
            KeyCode::Char('w'),
        ] {
            key(&mut app, code);
            assert_eq!(app.pane, pane, "{code:?} in a window");
        }
        app.window = None;
        app.zoom = Zoom::Focus(1);
        // `tab` in a focus is the detail's: it moves to another tab of
        // the same agent's work, never a pane.
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.pane, pane, "tab in a focus");
        assert_ne!(
            app.detail().map(|d| d.1),
            Some(crate::app::Tab::Log),
            "another tab"
        );
        app.thread = None;
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
        // On Home the chords fold the row at hand's project, or all.
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('a'));
        assert_ne!(app.collapsed, [false, false], "za on Home folds");
        app.collapsed = vec![false, false];
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('M'));
        assert_eq!(app.collapsed, [true, true], "zM on Home folds all");
        app.collapsed = vec![false, false];
        app.collapsed = vec![true, true];
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('R'));
        assert_eq!(app.collapsed, [false, false], "zR on Home unfolds too");
        app.zoom = Zoom::Focus(1);
        app.collapsed = vec![true, true];
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('R'));
        assert_eq!(app.collapsed, [true, true], "zR in a focus over Home");
        app.collapsed = vec![false, false];
        key(&mut app, KeyCode::Char('z'));
        key(&mut app, KeyCode::Char('M'));
        assert_eq!(app.collapsed, [false, false], "zM in a focus over Home");
        app.zoom = Zoom::None;
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
        let before = app.at_home.row;
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
            app.at_home.row, before,
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
        ctrl(&mut app, 'f');
        let pos = |app: &App| app.at_home.row;
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
        app.window = Some(Window::Code);
        assert!(render(&app, 140, 40).contains("no shared trunk yet"));
        // Before any card exists there is no "0/0 cards" either.
        app.window = None;
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
        assert_eq!(words(&app, Lang::En), "⚑ asks to run \"cargo test ↵ -q\"");
        // The forty columns: the mark and the words take 13, a space and
        // the quotes 3, so 24 letters show whole and 25 are cut.
        app.world.inbox.clear();
        app.world.asks_you(w, &"c".repeat(24));
        assert!(words(&app, Lang::En).ends_with(&format!("\"{}\"", "c".repeat(24))));
        app.world.inbox.clear();
        app.world.asks_you(w, &"c".repeat(25));
        assert!(words(&app, Lang::En).ends_with(&format!("\"{}…\"", "c".repeat(23))));
        app.world.inbox.clear();
        // Blocked: what it waits for, then on whom.
        app.world.agents[w].status = Status::Blocked;
        app.world.agents[w].topic = "JWT clock skew".into();
        // The queue's words leave eight of the forty.
        assert_eq!(
            words(&app, Lang::En),
            "BLOCKED \"JWT c…\" in the researcher queue"
        );
        let r = app.world.agents.len();
        app.world.agents.push(Agent::new(
            r,
            0,
            "researcher-1".into(),
            Role::Researcher,
            Status::Idle,
            0,
        ));
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
        assert_eq!(words(&app, Lang::En), "■ session ended: exited with code 1");
        assert_eq!(words(&app, Lang::Tr), "■ oturum bitti: exited with code 1");
        app.world.agents[w].topic = "x".repeat(50);
        let said = words(&app, Lang::En);
        assert_eq!(said.chars().count(), 40, "{said}");
        assert!(
            said.starts_with("■ session ended: xxx") && said.ends_with('…'),
            "{said}"
        );
        app.world.agents[w].topic.clear();
        assert_eq!(
            words(&app, Lang::En),
            "■ session ended · write to it to restart"
        );
    }

    #[test]
    fn a_row_quotes_its_newest_exchange_only_when_the_status_says_no_text() {
        let mut app = quiet_rows();
        let (w, other) = (1, 2);
        let lead = app.world.projects[0].leader;
        app.world.agents[other].name = "bob".into();
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
            r.contains("■ session ended · write to it to restart") && !r.contains('"'),
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
        assert!(r.contains("needs you → you \"parser fixed\""), "{r}");
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
            t.help_legend[4].ends_with("  ≡ 3/7 its to-do steps done, ▸ the one in progress"),
            "{}",
            t.help_legend[4]
        );
        assert!(
            Lang::Tr.text().help_legend[4]
                .ends_with("  ≡ 3/7 yapılacaklarından biten, ▸ şu an yaptığı"),
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
    fn the_activity_bar_shows_the_last_ticks_and_gives_way_first() {
        use crate::motion::Motion;
        let mut app = quiet_rows();
        app.motion = Motion::Calm;
        let (worker, _) = with_helpers(&mut app);
        for a in &mut app.world.agents {
            a.stirs = Stirs::default();
        }
        let a = &mut app.world.agents[worker];
        a.status = Status::Idle;
        a.changed_at = 64;
        a.local = Some("http://localhost:5173/".into());
        a.plan = steps(3, 1);
        told(a, Party::Person, true, "parser fixed");
        // Nothing written lately: no bar.
        let without = row(&app, worker, 200);
        assert!(!without.contains('▁'), "{without}");
        // One line four ticks ago, three now.
        let a = &mut app.world.agents[worker];
        a.trace(96, "tool_use Read");
        for _ in 0..3 {
            a.trace(100, "tool_use Edit");
        }
        let whole = row(&app, worker, 200);
        assert!(
            whole.contains("idle → you \"parser fixed\"  ▂▁▁▁▄ · 3 min  ≡ 1/3  ▾"),
            "{whole}"
        );
        // The bar's seven: at the row's width exactly it stays.
        let n = whole.chars().count();
        assert_eq!(n, without.chars().count() + 7, "{whole}");
        assert_eq!(row(&app, worker, n), whole, "fits exactly");
        // One short: the bar goes, the quote keeps all of its words.
        let r = row(&app, worker, n - 1);
        assert_eq!(r, without, "the bar gives way before the quote");
        // A quote cut short leaves no room for the bar either.
        let r = row(&app, worker, n - 9);
        assert!(
            r.contains("\"parser fi…\" · 3 min") && !r.contains('▁'),
            "{r}"
        );
        assert!(r.chars().count() <= n - 9, "{r}");
        // A tick on, the columns move left; five on, it is gone.
        app.world.tick = 101;
        assert!(row(&app, worker, 200).contains("\"  ▁▁▁▄▁ · 3 min"));
        app.world.tick = 104;
        assert!(row(&app, worker, 200).contains("\"  ▄▁▁▁▁ · "));
        app.world.tick = 105;
        assert_eq!(row(&app, worker, 200), without);
        // Motion off: no bar, whatever was written.
        app.world.tick = 100;
        app.motion = Motion::Off;
        assert_eq!(row(&app, worker, 200), without);
        app.motion = Motion::Full;
        assert!(row(&app, worker, 200).contains("  ▂▁▁▁▄ · "));
        let spans = salon::stir_spans(&app, &app.world.agents[worker]);
        assert_eq!(spans[0].style, dim(), "quiet");
        // One scale for every row, a column per count.
        let bar: String = [0, 1, 2, 3, 4, 5, 6, 7, 9, 10, 14, 15, 200, u16::MAX]
            .into_iter()
            .map(salon::stir_glyph)
            .collect();
        assert_eq!(bar, "▁▂▃▄▄▅▅▆▆▇▇███");
        // The legend names the bar in both languages.
        assert!(
            Lang::En.text().help_legend[5].ends_with("  ▁▃▅ what it did, 5 s a column"),
            "{}",
            Lang::En.text().help_legend[5]
        );
        assert!(
            Lang::Tr.text().help_legend[5].ends_with("  ▁▃▅ yaptıkları, sütun başı 5 sn"),
            "{}",
            Lang::Tr.text().help_legend[5]
        );
    }

    #[test]
    fn the_demo_s_working_agents_wear_the_activity_bar() {
        use crate::motion::Motion;
        let mut app = app_after(1, 4, 60, Lang::En);
        app.motion = Motion::Calm;
        let tick = app.world.tick;
        let busy: Vec<AgentId> = (app.world.agents.iter())
            .filter(|a| a.stirs.at(tick).iter().any(|&n| n > 0))
            .map(|a| a.id)
            .collect();
        assert!(!busy.is_empty(), "the demo writes traces");
        for id in busy {
            let r = row(&app, id, 300);
            assert!(r.chars().any(|c| ('▁'..='█').contains(&c)), "{r}");
        }
        app.motion = Motion::Off;
        assert!(!render(&app, 200, 45).contains("▁"));
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
            help.contains("tab ↑ ↓ ^r    yazarken: tamamla · gönderilenler · :history"),
            "{help}"
        );
        app.lang = Lang::En;
        let help = render(&app, 140, 40);
        assert!(
            help.contains("tab ↑ ↓ ^r    writing: complete · sent lines · :history"),
            "{help}"
        );
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
        assert_eq!((app.zoom, app.pane), (Zoom::None, Pane::Salon));
        assert_eq!(app.at_home.at, Some(crate::app::HomeRow::Agent(h1)));
        let s = render(&app, 160, 40);
        assert!(
            s.contains(&format!("└ ○ {helper}")),
            "under its worker: {s}"
        );
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
            check: Check::None,
            spend: None,
        });
        app.zoom = Zoom::Diff(worker);
        app.diff_scroll = 3;
        let screen = render(&app, 120, 20);
        assert!(screen.contains(" c  note   esc  back"), "{screen}");
        key(&mut app, KeyCode::Char('c'));
        let screen = render(&app, 120, 20);
        assert!(screen.contains("note on x:1 ▸"), "{screen}");
        assert!(screen.contains("enter keep · "), "{screen}");
        assert!(!screen.contains("enter send"), "{screen}");
        app.lang = Lang::Tr;
        assert!(render(&app, 120, 20).contains("x:1 için not ▸"));
    }

    #[test]
    fn the_header_shows_the_tightest_window_loud_when_nearly_full() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let at = |minutes, used| Limit {
            minutes,
            used,
            resets_at: None,
        };
        app.world.limits = vec![at(300, 52), at(10_080, 90)];
        let screen = render(&app, 200, 20);
        let top = screen.lines().next().unwrap();
        assert!(top.contains(" │ claude 7d: 10% left"), "{top}");
        assert!(!top.contains("claude 5h"), "{top}");
        // Short of loud, no window is in the header at all.
        app.world.limits = vec![at(300, 52), at(10_080, 89)];
        let top = render(&app, 200, 20).lines().next().unwrap().to_string();
        assert!(!top.contains("claude"), "{top}");
        let span = |used| limit_span(Lang::En, &at(300, used), 0);
        assert_eq!(span(89).style, Style::new());
        assert_eq!(span(90).style, attention());
        assert_eq!(span(90).content, "claude 5h: 10% left");
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
    fn a_table_in_a_conversation_wraps_to_its_width_and_copy_mode_moves_over_what_is_drawn() {
        let mut app = app_after(1, 4, 0, Lang::En);
        app.world.journal.clear();
        let id = app.world.number();
        let wide = "| # | What |\n|---|---|\n| 1 | a cell of many words that cannot fit in the window at all |";
        app.world.note(Event::Sent {
            from: 1,
            to: 0,
            kind: MsgKind::Inform,
            text: wide.into(),
            id,
            re: None,
        });
        app.open_thread(0);
        assert_eq!(
            app.thread.as_ref().unwrap().room.get(),
            usize::MAX,
            "not drawn yet"
        );
        // On a narrow screen the window is as wide as it (record 131):
        // 56, its border and gutter take 4.
        let s = render(&app, 56, 30);
        let th = app.thread.as_ref().unwrap();
        assert_eq!(th.room.get(), 52);
        assert!(
            s.contains("▌   1 │ a cell of many words that cannot fit in the   │"),
            "{s}"
        );
        assert!(s.contains("▌     │ window at all     "), "{s}");
        // Copy mode's lines are the drawn ones: one each.
        let lines = thread_lines(Lang::En, &app.world, &th.entries, th.room.get());
        let (_, _, drawn) = overlay::thread_layout(Lang::En, &app.world, &[], th, 54);
        assert_eq!(lines.len(), drawn.len());
        assert!(drawn.iter().all(|&(a, b)| b == a + 1), "{drawn:?}");
        assert_eq!(lines.len(), 5, "the head, the header, the rule, two");
    }

    #[test]
    fn a_conversation_s_copy_lines_are_heads_reply_lines_and_what_was_written() {
        let app = replies();
        let worker = app.world.agents[1].name.clone();
        let th = app.thread.as_ref().unwrap();
        let lines = thread_lines(Lang::En, &app.world, &th.entries, usize::MAX);
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
        let tr = thread_lines(Lang::Tr, &app.world, &th.entries, usize::MAX);
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
            !s.contains("COPY ·") && s.contains(" c  note   esc  back"),
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
        let src: Vec<String> = markdown_src(md, "  ", usize::MAX)
            .into_iter()
            .map(|(_, s)| s)
            .collect();
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
        assert_eq!(
            markdown_src(md, "  ", usize::MAX).len(),
            markdown(md, "  ").len()
        );
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
        let lines = thread_lines(Lang::En, &app.world, &th.entries, usize::MAX);
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
        // overlap (20 rows: 2 bars, 3 of the detail's frame, 2 border, 1
        // hints: 12 of text).
        let th = app.thread.as_ref().unwrap();
        assert_eq!(th.page.get(), 11);
        for _ in 0..9 {
            key(&mut app, KeyCode::PageDown);
        }
        render(&app, 100, 20);
        assert_eq!(
            usize::from(app.thread.as_ref().unwrap().scroll.get()),
            rows.len() - 12,
            "{} rows, 12 shown",
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
                (" (".into(), dim()),
                ("http://x".into(), link_style()),
                (")".into(), dim()),
                (" g".into(), base),
            ]
        );
        // A link without its words is text, its address still an address.
        assert_eq!(
            parts(&inline("[](http://x)", base)),
            [
                ("[](".into(), base),
                ("http://x".into(), link_style()),
                (")".into(), base),
            ]
        );
        // A link's address that is no web address stays dim.
        assert_eq!(
            parts(&inline("[f](ftp://x)", base)),
            [
                ("f".into(), under),
                (" (".into(), dim()),
                ("ftp://x".into(), dim()),
                (")".into(), dim()),
            ]
        );
        // A bare address is a span of its own; in code it is code.
        assert_eq!(
            parts(&inline("see https://a.b/c. `http://d.e`", base)),
            [
                ("see ".into(), base),
                ("https://a.b/c".into(), link_style()),
                (". ".into(), base),
                ("http://d.e".into(), code),
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
                (" (".into(), dim()),
                ("http://x/y".into(), link_style()),
                (")".into(), dim()),
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
    fn a_table_s_columns_give_way_widest_first_to_fit() {
        // They fit, the equal case too: as they are.
        assert_eq!(column_widths(&[3, 10, 5], 18), [3, 10, 5]);
        assert_eq!(column_widths(&[3, 10, 5], 100), [3, 10, 5]);
        // One cell short: the widest gives one.
        assert_eq!(column_widths(&[3, 10, 5], 17), [3, 9, 5]);
        // The widest down to the next, then both.
        assert_eq!(column_widths(&[3, 10, 5], 13), [3, 5, 5]);
        assert_eq!(column_widths(&[3, 10, 5], 11), [3, 4, 4]);
        assert_eq!(column_widths(&[2, 20, 30], 26), [2, 12, 12]);
        // No room: a cell each, an empty column too.
        assert_eq!(column_widths(&[3, 10, 0], 2), [1, 1, 1]);
        assert_eq!(column_widths(&[], 0), Vec::<usize>::new());
    }

    #[test]
    fn a_table_wider_than_its_room_wraps_its_cells_in_their_columns() {
        use unicode_width::UnicodeWidthStr;
        let line = |l: &Line| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        };
        let md = "| # | What | Priority |
|---|---|---|
| #28 | Config file and a baseline run on both | high |
| #29 | see https://example.com/a/long/path/here | medium |";
        // Room for it all: one line a row, copied as written.
        let wide = markdown_src(md, "  ", 200);
        assert_eq!(wide.len(), 4);
        assert_eq!(
            wide[1].1, "|---|---|---|",
            "the rule copies as the separator"
        );
        assert_eq!(
            wide[2].1,
            "| #28 | Config file and a baseline run on both | high |"
        );
        // 40 cells, the indent's 2 and the gaps' 6 counted: 32 for the
        // cells, the narrow columns kept (3 + 8), the widest the 21 left,
        // wrapped in its own.
        let lines = markdown_src(md, "  ", 40);
        let shown: Vec<String> = lines.iter().map(|(l, _)| line(l)).collect();
        assert_eq!(
            shown,
            [
                "  #   │ What                  │ Priority",
                "  ────┼───────────────────────┼─────────",
                "  #28 │ Config file and a     │ high",
                "      │ baseline run on both  │ ",
                "  #29 │ see                   │ medium",
                "      │ https://example.com/a │ ",
                "      │ /long/path/here       │ ",
            ]
        );
        assert_eq!(shown[0].width(), 40, "just full");
        // One cell less: the widest gives it.
        let narrower = markdown_src(md, "  ", 39);
        assert_eq!(
            line(&narrower[0].0),
            "  #   │ What                 │ Priority"
        );
        // Copied as read, so a wrapped row copies its lines, not twice.
        assert_eq!(lines[3].1, "│ baseline run on both  │");
        assert_eq!(lines[1].1, "────┼───────────────────────┼─────────");
        assert_eq!(lines[2].1, "#28 │ Config file and a     │ high");
        // The header still bold, the address too long for its column no
        // link: a link of part of it would open somewhere else.
        assert_eq!(lines[0].0.spans[1].style, bold(pal().strong));
        let parts: Vec<Style> = (lines[5..7].iter())
            .flat_map(|(l, _)| l.spans.iter())
            .filter(|s| s.content.contains("example") || s.content.contains("/long"))
            .map(|s| s.style)
            .collect();
        assert_eq!(parts, [fg(pal().info), fg(pal().info)]);
        // An address that fits its column stays a link.
        let md = "| a | b |\n|---|---|\n| https://a.b | x y z w v u t |";
        let lines = markdown_src(md, "", 26);
        let link = (lines.iter())
            .flat_map(|(l, _)| l.spans.iter())
            .find(|s| s.content == "https://a.b")
            .map(|s| s.style);
        assert_eq!(link, Some(link_style()));
        assert_eq!(line(&lines[2].0), "https://a.b │ x y z w v u ");
        assert_eq!(line(&lines[3].0), "            │ t");
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
        let journaled = |app: &App, needle: &str| {
            (app.world.journal.iter())
                .any(|e| words::event(Lang::En, &app.world, &e.event).contains(needle))
        };
        // The header names the open projects (record 125).
        let named = |app: &App| -> Vec<String> {
            let header = render(app, 140, 40).lines().next().unwrap().to_string();
            (app.world.open_projects())
                .filter(|p| header.contains(&p.name))
                .map(|p| p.name.clone())
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
        assert!(journaled(&app, "you opened api at"));
        assert_eq!(named(&app).len(), 2, "both in the header");

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

        // Rename the one at hand (Home narrowed to it).
        app.at_home.scope = Some(1);
        typed(&mut app, ":rename shop");
        assert_eq!(app.world.projects[1].name, "shop");
        assert_eq!(app.world.agents[lead_api].name, "lead-shop");
        assert_eq!(flashed(&app), "api is now shop; write to lead-shop");
        assert!(journaled(&app, "you renamed api to shop"));
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
        app.end_grace();
        assert!(app.world.projects[1].closed);
        assert_eq!(app.zoom, Zoom::None);
        assert_eq!(app.selected, Sel::Agent(lead3), "not in the closed project");
        assert_eq!(app.inbox_sel, 0);
        assert_eq!(
            flashed(&app),
            "closed shop: its agents left; their worktrees stay on disk"
        );
        assert_eq!(named(&app).len(), 2, "the two left, in the header");
        assert!(journaled(&app, "you closed shop"));
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
        app.end_grace();
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
        app.end_grace();
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

    #[test]
    fn a_folder_without_git_is_said_in_the_journal_and_on_its_header() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let lead = app.world.agents[0].name.clone();
        let e = Event::NoGit {
            leader: 0,
            path: "/n".into(),
        };
        for lang in [Lang::En, Lang::Tr] {
            assert_eq!(
                words::event(lang, &app.world, &e),
                fill(lang.text().ev_no_git, &[&lead, "/n"])
            );
        }
        assert!(
            words::event(Lang::En, &app.world, &e)
                .starts_with(&format!("{lead} works straight in /n: no git here"))
        );
        let s = render(&app, 140, 30);
        assert!(!s.contains("⌀ no git"), "{s}");
        app.world.projects[0].no_git = true;
        let s = render(&app, 140, 30);
        assert!(s.contains("⌀ no git"), "{s}");
        app.lang = Lang::Tr;
        let s = render(&app, 140, 30);
        assert!(s.contains("⌀ git yok"), "{s}");
    }

    /// A demo team, everyone idle and nothing waiting: the lead (0) and
    /// workers 1 to 4, agent 3 a helper of worker 1 and agent 4
    /// researching for it.
    fn row_of_three() -> App {
        let mut app = App::new(1, 4, 0, 1, Lang::En);
        app.world.inbox.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
        }
        let a = &mut app.world.agents;
        a[3].role = Role::Helper;
        a[3].parent = Some(1);
        a[4].role = Role::Researcher;
        a[4].status = Status::Researching {
            for_agent: 1,
            until: 9,
        };
        app
    }

    #[test]
    fn a_peek_is_the_focus_in_a_window_over_the_view() {
        let mut app = row_of_three();
        app.peek = Some(2);
        let t = Lang::En.text();
        let s = render(&app, 100, 40);
        let rows: Vec<Vec<char>> = s.lines().map(|l| l.chars().collect()).collect();
        // Four columns and three rows of the view show around it.
        assert_eq!(rows[3][4], '┌', "{s}");
        assert_ne!(rows[3][3], '┌', "{s}");
        assert_ne!(rows[2][4], '┌', "{s}");
        assert_eq!(rows[36][95], '┘', "{s}");
        assert_eq!(rows[36].get(96), Some(&' '), "{s}");
        let name = app.world.agents[2].name.clone();
        assert!(
            s.lines()
                .nth(3)
                .unwrap()
                .contains(&fill(t.focus_title, &[&name]))
        );
        // The footer has the peek's keys.
        let foot = s.lines().last().unwrap();
        assert!(
            foot.starts_with(&format!(" {} · ", t.places[5]))
                && foot.contains(t.focus)
                && foot.contains(t.write_this),
            "{foot}"
        );
        app.peek = None;
        let foot = render(&app, 100, 40).lines().last().unwrap().to_string();
        assert!(!foot.contains(t.places[5]), "{foot}");
    }

    #[test]
    fn a_plan_s_window_lists_its_cards_with_tier_paths_and_body_and_the_pot_beside_the_cost() {
        let (mut app, item, leader) = crate::app::tests::asking_plan();
        let name = app.world.agents[leader].name.clone();
        let row = render(&app, 240, 30);
        assert!(row.contains("plans 2 cards for $1.25"), "{row}");
        assert!(row.contains(" a  yes   r  no"), "{row}");
        app.plan_window = Some((item, 0));
        let s = render(&app, 140, 30);
        assert!(
            s.contains(&format!(" PLAN · {name} · 2 cards · $1.25 ")),
            "{s}"
        );
        assert!(
            s.contains("the leader hires a second worker only once you say yes"),
            "{s}"
        );
        let left = dollars(app.world.budget.saturating_sub(app.world.spent()));
        assert!(
            s.contains(&format!("expected $1.25 · left in the pot {left}")),
            "{s}"
        );
        assert!(s.contains("1. Parser · cheap"), "{s}");
        assert!(s.contains("   owns src/p.rs, docs (shared)"), "{s}");
        assert!(s.contains("   Fix it."), "{s}");
        assert!(s.contains("   Then test."), "{s}");
        assert!(s.contains("2. Docs · standard"), "{s}");
        assert!(s.contains("   Write them."), "{s}");
        assert!(s.contains(" a  yes ") && s.contains(" n  notes "), "{s}");
        // The footer says the window's keys too, not the pane's.
        let foot = s.lines().last().unwrap();
        assert!(
            foot.contains("a  yes") && foot.contains("j k  scroll"),
            "{foot}"
        );
        assert!(!foot.contains("enter shows the plan"), "{foot}");
        // Scrolled past the first card.
        app.plan_window = Some((item, 5));
        let s = render(&app, 140, 30);
        assert!(!s.contains("1. Parser"), "{s}");
        assert!(s.contains("2. Docs · standard"), "{s}");
        // The plan gone (answered, then the item reopened by hand): said so.
        app.world.projects[0].plan.proposal = None;
        let s = render(&app, 140, 30);
        assert!(s.contains("that plan is no longer waiting"), "{s}");
        // The lines a card takes.
        let p = Proposal {
            cards: vec![
                Planned {
                    title: "a".into(),
                    tier: "cheap".into(),
                    paths: Vec::new(),
                    checks: Vec::new(),
                    body: "one".into(),
                    after: Vec::new(),
                },
                Planned {
                    title: "b".into(),
                    tier: "strong".into(),
                    paths: vec![Owned::new("x")],
                    checks: vec!["api".into(), "e2e".into()],
                    body: "one\ntwo\nthree".into(),
                    after: vec![1],
                },
            ],
            cost: 0,
            tick: 0,
        };
        let lines = overlay::plan_lines(&app, &p);
        let texts: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        assert_eq!(
            texts,
            [
                "1. a · cheap",
                "   one",
                "",
                "2. b · strong",
                "   owns x",
                "   checks api, e2e",
                "   after 1 above",
                "   one",
                "   two",
                "   three",
                ""
            ]
        );
        assert_eq!(lines[0].spans[0].style, bold(pal().strong));
        assert_eq!(lines[4].spans[0].style, dim());
        assert_eq!(lines[5].spans[0].style, dim());
        assert_eq!(lines[6].spans[0].style, dim());
    }

    #[test]
    fn a_card_s_screen_says_the_checks_it_asks_for() {
        let mut app = on_card_one(Lang::En);
        app.world.cards[0].checks = Vec::new();
        assert!(!render(&app, 140, 38).contains("checks "));
        app.world.cards[0].checks = vec!["api".into(), "e2e".into()];
        let s = render(&app, 140, 38);
        assert!(s.contains("checks api, e2e"), "{s}");
        app.lang = Lang::Tr;
        assert!(render(&app, 140, 38).contains("denetim api, e2e"));
    }

    #[test]
    fn the_board_says_what_a_card_waits_on_and_r_leaves_out_what_is_not_ready() {
        let (mut app, w, _) = calm_team();
        app.window = Some(Window::Board);
        let name = |app: &App, id: AgentId| app.world.agents[id].name.clone();
        let card = |id, owner, column, after: Vec<CardId>| Card {
            id,
            project: 0,
            title: format!("card {id}"),
            owner,
            column,
            story: Vec::new(),
            symbols: Vec::new(),
            paths: Vec::new(),
            checks: Vec::new(),
            started: 0,
            finished: 0,
            attempt: 1,
            breaker: Default::default(),
            after,
        };
        app.world.cards = vec![
            card(0, Some(w[0]), Column::Working, vec![]),
            card(1, None, Column::Waiting, vec![0]),
            card(2, Some(w[1]), Column::Waiting, vec![0]),
            card(3, Some(w[2]), Column::Failed, vec![]),
            card(4, None, Column::Waiting, vec![0, 3]),
            card(5, None, Column::Waiting, vec![]),
            card(6, Some(w[3]), Column::Waiting, vec![]),
        ];
        let s = render(&app, 200, 40);
        assert!(s.contains(" BOARD · cards: 7 · r ready only "), "{s}");
        assert!(s.contains("⧗ after #1 "), "{s}");
        assert!(
            s.contains(&format!("⧗ after #1 → {}", name(&app, w[1]))),
            "{s}"
        );
        assert!(s.contains("✕ #4 failed "), "{s}");
        assert!(s.contains(&format!("    → {}", name(&app, w[3]))), "{s}");
        assert!(s.contains("#6  card 5"), "{s}");
        // Ready only: the three that wait are gone, and the title says so.
        app.board_ready = true;
        let s = render(&app, 200, 40);
        assert!(
            s.contains(" BOARD · cards: 7 · ready only: 3 waiting hidden · r all "),
            "{s}"
        );
        for gone in ["card 1", "card 2", "card 4", "⧗", "✕ #4"] {
            assert!(!s.contains(gone), "{gone}:\n{s}");
        }
        for kept in ["card 0", "card 3", "card 5", "card 6"] {
            assert!(s.contains(kept), "{kept}:\n{s}");
        }
        assert_eq!(board::hidden(&app.world), 3);
        // Their marks in colour: failed in the bad one, waiting dim.
        let t = Lang::En.text();
        let spans = board_waiting_spans(t, &app.world, 4);
        assert_eq!(spans[0].style, fg(pal().bad));
        let spans = board_waiting_spans(t, &app.world, 1);
        assert_eq!(spans[0].style, dim());
        assert!(board_waiting_spans(t, &app.world, 5).is_empty());
        // A withdrawn card stands with the done ones, marked dim; one that
        // waits on it, through another, is held back for good.
        app.board_ready = false;
        app.world.cards[3].column = Column::Withdrawn;
        app.world.cards[5].after = vec![4];
        let s = render(&app, 200, 40);
        assert!(s.contains("#4  – card 3"), "{s}");
        assert!(s.contains("✕ #4 withdrawn"), "{s}");
        let spans = board_waiting_spans(t, &app.world, 5);
        assert_eq!(spans[0].content, "✕ #4 withdrawn");
        assert_eq!(spans[0].style, fg(pal().bad));
        let line = |s: &str| {
            s.lines()
                .find(|l| l.contains("– card 3"))
                .unwrap()
                .to_string()
        };
        assert!(!line(&s).contains("✕"), "{}", line(&s));
        // The mark is drawn in the withdrawn style, not the failed one.
        let (buf, _) = whole(&app, 200);
        let at = (buf.content.iter())
            .position(|c| c.symbol() == "–")
            .expect("the mark");
        let style = Style::default().patch(dim());
        assert_eq!(Some(buf.content[at].fg), style.fg.or(Some(Color::Reset)));
        assert_ne!(Some(buf.content[at].fg), fg(pal().bad).fg);
        assert_eq!(
            crate::ui::card::state(t, &app.world, &app.world.cards[3]),
            "WITHDRAWN"
        );
        let spans = card::after_spans(t, &app.world, &app.world.cards[4]);
        assert_eq!(spans.last().unwrap().style, fg(pal().bad));
        assert_eq!(spans.last().unwrap().content, "✕");
    }

    fn board_waiting_spans(t: &crate::i18n::Text, w: &World, c: CardId) -> Vec<Span<'static>> {
        board::waiting_spans(t, w, &w.cards[c])
    }

    #[test]
    fn a_card_s_screen_says_what_it_waits_on_done_failed_or_not_yet() {
        let mut app = on_card_one(Lang::En);
        let n = app.world.cards.len();
        assert!(n >= 4, "{n}");
        app.world.cards[1].column = Column::Done;
        app.world.cards[2].column = Column::Failed;
        app.world.cards[3].column = Column::Working;
        app.world.cards[0].after = vec![1, 2, 3];
        let s = render(&app, 140, 38);
        assert!(s.contains("after  #2 ✓, #3 ✕, #4 ⧗"), "{s}");
        let t = Lang::En.text();
        let spans = card::after_spans(t, &app.world, &app.world.cards[0]);
        let styles: Vec<Style> = spans.iter().map(|s| s.style).collect();
        assert_eq!(
            styles,
            [
                dim(),
                Style::default(),
                fg(pal().ok),
                dim(),
                Style::default(),
                fg(pal().bad),
                dim(),
                Style::default(),
                dim()
            ]
        );
        app.world.cards[0].after.clear();
        assert!(!render(&app, 140, 38).contains("after  "));
    }

    #[test]
    fn a_stores_line_on_the_project_says_how_it_stands() {
        let mut app = App::with(
            crate::live::testing::team(0),
            crate::app::Engine::Live(Box::new(crate::live::testing::idle_live(1))),
            Lang::En,
        );
        let s = render(&app, 160, 30);
        assert!(!s.contains("lonca"), "{s}");
        app.world.on_lonca(0, Some("demo".into()));
        assert!(render(&app, 160, 30).contains("lonca demo …"));
        let trunk = "693d62952cd8f7ce58b0de780cf5d998b2f5c1a6".to_string();
        let seen = |pending| StoreSeen::Up {
            trunk: trunk.clone(),
            pending,
            behind: Behind::By(0),
        };
        app.world.store_seen(0, "demo", seen(0));
        let s = render(&app, 160, 30);
        assert!(s.contains("lonca demo 693d629"), "{s}");
        assert!(!s.contains("pending"), "{s}");
        app.world.store_seen(0, "demo", seen(2));
        assert!(render(&app, 160, 30).contains("lonca demo 693d629 · 2 pending"));
        app.lang = Lang::Tr;
        assert!(render(&app, 160, 30).contains("lonca demo 693d629 · 2 bekliyor"));
        app.world
            .store_seen(0, "demo", StoreSeen::Down(StoreDown::NoDaemon));
        assert!(render(&app, 160, 30).contains("⌀ lonca yok"));
        app.lang = Lang::En;
        assert!(render(&app, 160, 30).contains("⌀ no lonca"));
    }

    #[test]
    fn a_stores_events_are_worded_in_both_languages_with_their_reason() {
        let app = App::with(
            crate::live::testing::team(0),
            crate::app::Engine::Live(Box::new(crate::live::testing::idle_live(1))),
            Lang::En,
        );
        let project = app.world.projects[0].name.clone();
        let all = [
            (StoreDown::NoDaemon, "lonca daemon"),
            (StoreDown::NotTheDaemon, "not your daemon"),
            (StoreDown::Protocol, "another protocol"),
            (StoreDown::SignedOut, ":lonca login"),
            (StoreDown::Forbidden, "not enrolled"),
            (StoreDown::NoRepo, "no such repository"),
            (StoreDown::Refused, "refused"),
            (StoreDown::Busy, "busy"),
            (StoreDown::Frozen, "froze"),
        ];
        for (why, word) in all {
            let e = Event::StoreDown {
                leader: 0,
                repo: "demo".into(),
                why,
            };
            for lang in [Lang::En, Lang::Tr] {
                let t = lang.text();
                assert_eq!(
                    words::event(lang, &app.world, &e),
                    fill(
                        t.ev_store_down,
                        &[&project, "demo", words::store_down(t, why)]
                    )
                );
            }
            assert!(words::store_down(Lang::En.text(), why).contains(word));
        }
        let back = Event::StoreBack {
            leader: 0,
            repo: "demo".into(),
        };
        assert_eq!(
            words::event(Lang::En, &app.world, &back),
            format!("{project}'s Lonca repository demo answers again")
        );
    }

    #[test]
    fn the_import_window_says_what_goes_and_what_can_not_be_taken_back() {
        for (lang, word) in [
            (Lang::En, "imece can not take this back"),
            (Lang::Tr, "geri alamaz"),
        ] {
            let mut app = app_after(1, 1, 0, lang);
            app.importing = Some(crate::model::ImportAsk {
                project: 0,
                repo: "shop".into(),
                checkout: "/src/shop".into(),
                branch: "main".into(),
                head: "693d62952cd8".into(),
            });
            let s = render(&app, 120, 30);
            for shown in ["/src/shop", "main", "693d629", "shop", word] {
                assert!(s.contains(shown), "{shown}: {s}");
            }
            assert!(!s.contains("693d62952cd8"), "{s}");
        }
    }

    #[test]
    fn how_far_behind_trunk_is_a_word_on_the_project_line() {
        use crate::model::{Behind, StoreSeen};
        assert_eq!(words::behind(Behind::By(3)).as_deref(), Some("↓3"));
        assert_eq!(words::behind(Behind::Never).as_deref(), Some("↓?"));
        assert_eq!(words::behind(Behind::By(0)), None);
        assert_eq!(words::behind(Behind::Unknown), None);
        let mut app = app_after(1, 1, 0, Lang::En);
        app.world.on_lonca(0, Some("shop".into()));
        let up = StoreSeen::Up {
            trunk: "693d62952cd8".into(),
            pending: 2,
            behind: Behind::By(3),
        };
        app.world.store_seen(0, "shop", up);
        let s = render(&app, 160, 40);
        assert!(s.contains("lonca shop 693d629 ↓3 · 2 pending"), "{s}");
    }

    #[test]
    fn a_sync_says_how_the_checkout_stood() {
        use crate::model::{StoreAct, Synced};
        let app = app_after(1, 1, 0, Lang::En);
        let project = app.world.projects[0].name.clone();
        let line = |how| {
            let e = Event::StoreSynced {
                leader: 0,
                repo: "shop".into(),
                how,
            };
            words::event(Lang::En, &app.world, &e)
        };
        assert_eq!(
            line(Synced::Moved),
            format!("{project} moved forward to trunk of shop")
        );
        assert_eq!(
            line(Synced::Even),
            format!("{project} is level with trunk of shop")
        );
        assert_eq!(
            line(Synced::Apart),
            format!(
                "{project} has commits of its own: trunk of shop is in refs/lonca/trunk, nothing moved"
            )
        );
        let failed = Event::StoreFailed {
            leader: 0,
            act: StoreAct::Sync,
            why: Some("redacted".into()),
        };
        assert_eq!(
            words::event(Lang::En, &app.world, &failed),
            "bringing trunk to the checkout failed: redacted"
        );
    }

    #[test]
    fn a_lonca_commands_lines_say_how_it_went() {
        use crate::model::StoreAct;
        let app = app_after(1, 1, 0, Lang::En);
        let project = app.world.projects[0].name.clone();
        let line = |e: Event| words::event(Lang::En, &app.world, &e);
        let signed = |human: &str| Event::StoreSignedIn {
            leader: 0,
            human: human.into(),
        };
        assert_eq!(line(signed("")), "signed in to Lonca");
        assert_eq!(line(signed("ayse")), "signed in to Lonca as ayse");
        let imported = |warnings: Vec<String>, hardened| Event::StoreImported {
            leader: 0,
            repo: "demo".into(),
            warnings,
            hardened,
        };
        let went = format!("{project} went into Lonca as repository demo");
        assert_eq!(line(imported(vec![], false)), went);
        assert_eq!(
            line(imported(
                vec!["big.bin".into(), "x".into(), "y".into()],
                false
            )),
            format!("{went}; Lonca warns: big.bin (+2 more)")
        );
        let hard =
            format!("{went}, from Lonca's hardened profile (lonca doctor says what it asks for)");
        assert_eq!(line(imported(vec![], true)), hard);
        assert_eq!(
            line(imported(vec!["big.bin".into()], true)),
            format!("{hard}; Lonca warns: big.bin (+0 more)")
        );
        let verified = |checks: Vec<Attested>| Event::Verified {
            agent: 1,
            ok: false,
            checks,
        };
        let check = |name: &str, ok, end: &str, lines: &[&str]| Attested {
            check: name.into(),
            ok,
            end: end.into(),
            lines: lines.iter().map(|l| l.to_string()).collect(),
        };
        assert!(
            line(verified(vec![
                check("syntax", true, "", &[]),
                check("build", true, "exit 0, 1.1 s", &[]),
                check("tests", false, "exit 1, 2.0 s", &["first", "last"]),
            ]))
            .ends_with(
                ": Lonca's checks · ✓ syntax · ✓ build exit 0, 1.1 s · \
                 ✗ tests exit 1, 2.0 s · last"
            )
        );
        let kept = |why: Option<&str>| Event::StoreCriticalKept {
            leader: 0,
            repo: "demo".into(),
            why: why.map(str::to_string),
        };
        let stays =
            format!("{project} on demo: a worker's change to a critical path stays refused");
        assert_eq!(line(kept(None)), stays);
        assert_eq!(line(kept(Some("denied"))), format!("{stays}: denied"));
        let failed = |act, why: Option<&str>| Event::StoreFailed {
            leader: 0,
            act,
            why: why.map(str::to_string),
        };
        assert_eq!(
            line(failed(StoreAct::Login, Some("no key"))),
            "signing in to Lonca failed: no key"
        );
        assert_eq!(
            line(failed(StoreAct::Import, None)),
            "importing into Lonca failed: lonca did not answer: no such program, or it took too long"
        );
        // A refusal's reasons as a person says them (record 127); words
        // that are not one stay as they came.
        let yes = |why| line(failed(StoreAct::Decide, Some(why)));
        assert_eq!(
            yes("denied: unknown_check:api, invalid_checks"),
            format!(
                "{}: Lonca's policy has no api check, the card's checks are not a list \
                 Lonca takes · :checks NAMES changes what the card asks for",
                Lang::En.text().store_act_decide
            )
        );
        // Each refusal of the card's checks says how to change them, once;
        // one for checks given with no change does not.
        for why in ["denied: unknown_check:api", "denied: invalid_checks"] {
            assert!(
                yes(why).ends_with(" · :checks NAMES changes what the card asks for"),
                "{why}"
            );
        }
        assert_eq!(
            yes("denied: checks_not_a_change"),
            format!(
                "{}: checks go only with a proposed change",
                Lang::En.text().store_act_decide
            )
        );
        assert!(!yes("denied: unknown_checker").contains(":checks"));
        assert_eq!(
            yes("denied:unknown_check:api"),
            format!(
                "{}: denied:unknown_check:api",
                Lang::En.text().store_act_decide
            )
        );
        assert_eq!(
            yes("no answer"),
            format!("{}: no answer", Lang::En.text().store_act_decide)
        );
    }

    #[test]
    fn a_commit_is_read_by_its_first_digits() {
        assert_eq!(words::short_commit("693d62952cd8"), "693d629");
        assert_eq!(words::short_commit("693d629"), "693d629");
        assert_eq!(words::short_commit("69"), "69");
    }

    #[test]
    fn the_floor_is_forty_columns_by_twelve_rows() {
        assert!(fits_floor((MIN_COLUMNS, MIN_ROWS)));
        assert!(!fits_floor((MIN_COLUMNS - 1, MIN_ROWS)));
        assert!(!fits_floor((MIN_COLUMNS, MIN_ROWS - 1)));
        assert!(fits_floor((u16::MAX, MIN_ROWS)));
        assert_eq!((MIN_COLUMNS, MIN_ROWS, NARROW), (40, 12, 80));
    }

    #[test]
    fn under_the_floor_one_line_says_so_in_the_middle_and_nothing_else() {
        let app = app_after(1, 4, 0, Lang::En);
        assert!(!app.too_small(), "not drawn yet: keys act");
        let buf = buffer(&app, 39, 20);
        let row = |y: u16| -> String { (0..39).map(|x| buf[(x, y)].symbol()).collect() };
        let all: Vec<String> = (0..20).map(row).collect();
        // Two rows of words on twenty: from the tenth.
        assert_eq!(
            all[9].trim(),
            "this terminal is 39×20; imece needs",
            "{all:#?}"
        );
        assert_eq!(all[10].trim(), "40×12");
        let words = all.iter().filter(|r| !r.trim().is_empty());
        assert_eq!(words.count(), 2, "nothing else: {all:#?}");
        assert!(app.too_small());
        // One row short is under it too; the floor itself is not.
        assert!(render(&app, 120, MIN_ROWS - 1).contains("120×11"));
        assert!(app.too_small());
        let s = render(&app, MIN_COLUMNS, MIN_ROWS);
        assert!(s.starts_with(" imece ") || s.starts_with("  "), "{s}");
        assert!(!s.contains("this terminal"), "{s}");
        assert!(!app.too_small());
        // In Turkish too.
        let tr = app_after(1, 4, 0, Lang::Tr);
        assert!(render(&tr, 30, 30).contains("bu terminal 30×30; imece en az"));
        // A screen of no rows draws nothing and holds the keys.
        render(&app, 80, 0);
        assert!(app.too_small());
    }

    #[test]
    fn under_the_floor_no_key_acts_but_ctrl_c_and_no_paste_lands() {
        let mut app = app_after(1, 4, 0, Lang::En);
        render(&app, MIN_COLUMNS - 1, 30);
        key(&mut app, KeyCode::Char(':'));
        assert!(!app.typing, "what was not shown is not decided");
        key(&mut app, KeyCode::Char('?'));
        assert!(!app.help);
        render(&app, MIN_COLUMNS, MIN_ROWS);
        key(&mut app, KeyCode::Char(':'));
        assert!(app.typing, "at the floor keys act");
        app.paste("ab");
        assert_eq!(app.input, ":ab");
        render(&app, 80, MIN_ROWS - 1);
        app.paste("cd");
        assert_eq!(app.input, ":ab", "a paste neither");
        key(&mut app, KeyCode::Char('x'));
        assert_eq!(app.input, ":ab");
        ctrl(&mut app, 'c');
        assert!(app.quit, "ctrl-c always");
    }

    #[test]
    fn the_status_line_keeps_every_key_and_puts_the_place_in_what_they_leave() {
        let hints = vec![
            ("a", "approve"),
            ("r", "send back"),
            ("esc", "back"),
            ("?", "help"),
        ];
        let keys = spans_width(&hint_spans(&hints));
        let place = |words: &str| vec![Span::styled(words.to_string(), dim())];
        let words = "home · shop-api · #1 Shipping cost";
        let n = words.chars().count();
        let parts = |width: usize| status_parts(place(words), hints.clone(), width);
        // A blank, the place, three blanks, the keys.
        let (p, h) = parts(1 + n + 3 + keys);
        assert_eq!(p.unwrap().content, words, "whole where it fits");
        assert_eq!(h, hints);
        assert_eq!(
            p_style(&parts(1 + n + 3 + keys)),
            Some(dim()),
            "its style kept"
        );
        let (p, h) = parts(n + 3 + keys);
        assert_eq!(p.unwrap().content, "home · shop-api · #1 Shipping co…");
        assert_eq!(h, hints, "the place is cut, never a key");
        let (p, h) = parts(1 + PLACE_MIN + 3 + keys);
        assert_eq!(p.unwrap().content, "home · shop…", "at its least");
        assert_eq!(h, hints);
        let (p, h) = parts(PLACE_MIN + 3 + keys);
        assert!(p.is_none(), "under it the place goes");
        assert_eq!(h, hints, "every key still");
        // Then keys go from the end, never `esc` or `?`.
        let (p, h) = parts(1 + keys - 1);
        assert!(p.is_none());
        assert_eq!(h, vec![("a", "approve"), ("esc", "back"), ("?", "help")]);
        // No place: the keys alone.
        let (p, h) = status_parts(place(""), hints.clone(), 200);
        assert!(p.is_none());
        assert_eq!(h, hints);
        let (p, _) = status_parts(Vec::new(), hints.clone(), 200);
        assert!(p.is_none());
    }

    fn p_style(parts: &(Option<Span<'static>>, Vec<(&str, &str)>)) -> Option<Style> {
        parts.0.as_ref().map(|p| p.style)
    }

    #[test]
    fn a_narrow_screen_s_status_line_has_the_row_s_keys_not_the_place() {
        let mut app = app_after(1, 4, 0, Lang::En);
        let a = app.world.projects[0].leader + 1;
        app.world.inbox.clear();
        app.world.asked_you(a, "Which parser should the tests use?");
        app.home_settle();
        let s = render(&app, 48, 20);
        let foot = s.lines().last().unwrap();
        assert!(foot.contains(" a  ") && foot.contains("?  help"), "{foot}");
        assert!(!foot.contains("home ·"), "{foot}");
        let s = render(&app, 120, 20);
        let foot = s.lines().last().unwrap();
        assert!(foot.contains("home ·") && foot.contains(" a  "), "{foot}");
    }

    #[test]
    fn a_conversation_takes_the_whole_body_of_a_narrow_screen() {
        let app = replies();
        let left = |width: u16| {
            let buf = buffer(&app, width, 30);
            let y = (0..30)
                .find(|&y| {
                    (0..width)
                        .map(|x| buf[(x, y)].symbol())
                        .collect::<String>()
                        .contains("CONVERSATION")
                })
                .unwrap();
            let right = (0..width).rev().find(|&x| buf[(x, y)].symbol() == "┐");
            ((0..width).find(|&x| buf[(x, y)].symbol() == "┌"), right)
        };
        assert_eq!(left(79), (Some(0), Some(78)), "edge to edge");
        assert_eq!(left(80), (Some(2), Some(77)), "two columns either side");
    }
}
