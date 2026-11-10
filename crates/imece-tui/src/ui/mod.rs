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
mod footer;
mod header;
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
use crate::i18n::{Lang, clean, counted, fill};
use crate::model::*;
use crate::motion::{GlowOn, Lit};
use crate::screen::{Look, Shape};
pub(crate) use crate::theme::pal;
pub(crate) use diff::{
    QUOTE_LINES, Shown, clash_message, copy_patch, file_of, file_starts, head_path, hunk_at,
    notes_message, place, quote,
};
pub(crate) use footer::caret_in;
use footer::{completions, footer, input_rows};
use header::header;
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
    if let Some(win) = app.doctor {
        over.push(overlay::doctor(f, app, win));
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

/// What waits on the screen behind an archive's view: the decisions,
/// and the replies (only to read) apart.
fn behind_view(app: &App, lang: Lang) -> String {
    let t = lang.text();
    let w = &app.world;
    let decide = w.open_decisions();
    match w.open_inbox().len() - decide {
        0 => fill(t.view_waiting, &[&decide.to_string()]),
        n => fill(
            t.view_waiting_read,
            &[&decide.to_string(), &counted(n, t.home_counts[3])],
        ),
    }
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
            &behind_view(app, v.lang),
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
    use crate::blocked::Stage;
    match a.status {
        // Blocked between turns (record 143): its leader has it, in the
        // colour of what waits; the person has it, in the colour of
        // what is stuck.
        Status::Idle | Status::Asleep if let Some(b) = &a.block => match b.stage {
            Stage::Yours(_) => ("■", pal().bad),
            Stage::Leader | Stage::Asked { .. } => ("■", pal().warn),
        },
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
/// Lines pointed at light the same way from the theme's `accent`, in
/// five steps, and not at all where the colours cannot be blended: many
/// rows reversed would be loud, and what marks them then does not move.
pub(crate) fn lit_style(lit: Lit) -> Style {
    let p = pal();
    let (tone, shades, step): (Color, &[u8], u8) = match lit {
        Lit::Reversed => return Style::new().add_modifier(Modifier::REVERSED),
        Lit::Settle(step) => (p.ok, &SETTLE_SHADES, step),
        Lit::Point(step) => (p.accent, &POINT_SHADES, step),
    };
    match (tone, p.bg) {
        (Color::Rgb(r, g, b), Color::Rgb(br, bg, bb)) => {
            let percent = shades[usize::from(step).min(shades.len() - 1)];
            let c = crate::theme::Rgb(br, bg, bb).blend(crate::theme::Rgb(r, g, b), percent);
            Style::new().bg(Color::Rgb(c.0, c.1, c.2))
        }
        _ if matches!(lit, Lit::Point(_)) => Style::new(),
        _ if step == 0 => Style::new().add_modifier(Modifier::REVERSED),
        _ => changed(),
    }
}

/// How much of the theme's `accent` the background of lines pointed at
/// has, step by step.
const POINT_SHADES: [u8; 5] = [30, 24, 18, 12, 6];

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
        InboxKind::Fault | InboxKind::Blocked => ("■", bold(pal().bad)),
        InboxKind::Script | InboxKind::Check => ("▶", bold(pal().warn)),
        InboxKind::Plan => ("▤", bold(pal().warn)),
        InboxKind::Tries | InboxKind::Rounds => ("⊘", bold(pal().warn)),
        InboxKind::Close => ("!", bold(pal().warn)),
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

/// An agent's words as the Markdown they are (headings, lists, a table:
/// [`markdown_src`]), each drawn line wrapped at `width`: a worker's
/// `- **Added:** …` lists read as lists, not one run-on paragraph.
pub(crate) fn markdown_rows(text: &str, width: usize) -> Vec<Line<'static>> {
    (markdown_src(text, "", width).into_iter())
        .flat_map(|(line, _)| wrap(line, width))
        .collect()
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

/// The demo's made-up progress of `a` as a dim bar and a space, to stand
/// before its status; a live agent has no phases, so nothing.
pub(crate) fn demo_bar(w: &World, a: &Agent) -> Vec<Span<'static>> {
    match w.demo {
        true => vec![Span::styled(bar(a.progress), dim()), Span::raw(" ")],
        false => Vec::new(),
    }
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

/// A blocked worker's status (record 143): the word, what it said, and
/// who has it: its leader, the person its leader asked, or the person.
fn block_spans(
    t: &crate::i18n::Text,
    w: &World,
    a: &Agent,
    held: crate::blocked::Held,
) -> Vec<Span<'static>> {
    use crate::blocked::{Held, Stage};
    let Some(b) = &a.block else {
        return Vec::new();
    };
    let colour = match held {
        Held::Yours => pal().bad,
        Held::Leader | Held::Asked => pal().warn,
    };
    let mut spans = vec![Span::styled(t.blocked_word, bold(colour))];
    let first = b.said.lines().find(|l| !l.trim().is_empty());
    add_quote(&mut spans, first.unwrap_or_default(), fg(colour));
    let who = match (held, &b.stage) {
        (Held::Leader, _) => fill(t.blocked_leader, &[&clean(&w.agents[b.leader].name)]),
        (Held::Asked, _) => fill(t.blocked_asked, &[&clean(&w.agents[b.leader].name)]),
        (Held::Yours, Stage::Yours(why)) => words::unsolved(t, w, b.leader, *why),
        (Held::Yours, _) => String::new(),
    };
    if !who.is_empty() {
        spans.push(Span::styled(format!(" · {who}"), dim()));
    }
    spans
}

/// [`status_spans`] without the mark of a yes for the turn.
fn doing_status(lang: Lang, w: &World, a: &Agent) -> Vec<Span<'static>> {
    let t = lang.text();
    match a.status {
        Status::Idle | Status::Asleep if let Some(b) = &a.block => {
            block_spans(t, w, a, w.held_of(b))
        }
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
            let mut spans = demo_bar(w, a);
            spans.push(Span::styled(format!("? → {}", t.you), fg(pal().warn)));
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
            // The demo's made-up phases; a live agent has none: between
            // its calls it thinks.
            None if w.demo => vec![
                Span::styled(bar(a.progress), fg(pal().ok)),
                Span::raw(format!(" {}", Phase::of(a.progress).label())),
            ],
            None => vec![Span::styled(t.thinking, fg(pal().ok))],
        },
        Status::Asking { to, .. } => {
            let mut spans = demo_bar(w, a);
            spans.push(Span::styled(
                format!("? → {}", clean(&w.agents[to].name)),
                fg(pal().warn),
            ));
            spans
        }
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
mod tests;
