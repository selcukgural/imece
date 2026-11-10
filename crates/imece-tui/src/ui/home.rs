//! Home (records 124 and 125): the triage list. What is to decide,
//! grouped by what deciding costs (permissions, then questions, then
//! changes), then who is at work; every row says the whole ask, the keys
//! that decide it stand at its end, and a yes moves down. On a wide
//! screen the row at hand is read beside the list; on a narrow one the
//! list takes the width.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::*;
use crate::app::{App, Heading, HomeGroup, HomeRow};
use crate::i18n::Text;
use crate::ledger::Last;

/// From this wide the preview stands beside the list.
pub(crate) const WIDE: u16 = 120;
/// The list's narrowest beside the preview.
const LIST_MIN: u16 = 60;
/// How much of a row the title takes, at most.
const TITLE: usize = 24;
/// The title's narrowest, on a narrow screen.
const TITLE_MIN: usize = 10;
/// An ask gets at least this many cells on a line, under its title if
/// need be.
const ASK_MIN: usize = 20;
/// Files named in a preview.
const FILES: usize = 6;
/// Lines of a script shown in a preview.
const SCRIPT_ROWS: usize = 12;

/// How wide the list is on a screen `width` wide, the preview beside it.
fn list_width(width: u16) -> u16 {
    (width * 45 / 100).max(LIST_MIN)
}

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let lonca = lonca_line(app);
    let tall = u16::from(lonca.is_some());
    // The leader's line stands at the foot, where the line to write on
    // opens.
    let lead = lead_line(app, usize::from(area.width));
    let [store, body, foot] = Layout::vertical([
        Constraint::Length(tall),
        Constraint::Min(0),
        Constraint::Length(u16::from(lead.is_some())),
    ])
    .areas(area);
    if let Some(line) = lonca {
        f.render_widget(Paragraph::new(line), store);
    }
    if let Some(line) = lead {
        f.render_widget(Paragraph::new(line), foot);
    }
    let at = app.home_at();
    // Nothing listed: no row to read beside, the list takes the width so
    // what it says is said once, whole.
    if body.width < WIDE || at.is_none() {
        let drawn = list_drawn(app, usize::from(body.width), body.width < NARROW, at);
        let scroll = drawn.scroll(body.height);
        let used = drawn.lines.len() as u16;
        note_breath(app, &drawn.lines, &drawn.spots, scroll, body);
        f.render_widget(Paragraph::new(drawn.lines).scroll((scroll, 0)), body);
        mark_under(f, body, app, used.saturating_sub(scroll));
        return;
    }
    let [list, gap, preview] = Layout::horizontal([
        Constraint::Length(list_width(body.width)),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(body);
    let drawn = list_drawn(app, usize::from(list.width), false, at);
    let scroll = drawn.scroll(list.height);
    let used = drawn.lines.len() as u16;
    note_breath(app, &drawn.lines, &drawn.spots, scroll, list);
    f.render_widget(Paragraph::new(drawn.lines).scroll((scroll, 0)), list);
    mark_under(f, list, app, used.saturating_sub(scroll));
    let rule = vec![Line::from(Span::styled("│", dim())); usize::from(gap.height)];
    f.render_widget(Paragraph::new(rule), gap);
    let room = usize::from(preview.width).saturating_sub(1);
    if let Some(row) = at {
        let text = preview_lines(app, row, room, usize::from(preview.height));
        f.render_widget(Paragraph::new(text), preview);
    }
}

/// How far back in the journal the leader's line looks for its words.
pub(crate) const LEAD_BACK: usize = 2_000;

/// The leader's line (record 144): while its workers are at work the
/// leader of the project at hand has no row in the list, so its line
/// stands under it, always in the same place: its glyph and name, what it
/// does right now, else its newest words to the person and how long ago,
/// and at the right the key that writes to it. None for a solo (it has a
/// row), a leader gone or a project put away.
pub(crate) fn lead_line(app: &App, width: usize) -> Option<Line<'static>> {
    let w = &app.world;
    let t = app.lang.text();
    let p = w.projects.get(app.current_project())?;
    let a = w.agents.get(p.leader)?;
    if a.role != Role::Leader || a.gone || p.closed || w.put_away(a.id) {
        return None;
    }
    let (mark, colour) = glyph(a);
    let mut spans = vec![
        Span::raw(" "),
        Span::styled(format!("{mark} "), fg(colour)),
        Span::styled(clean(&a.name), bold(pal().strong)),
        Span::raw("  "),
    ];
    let key = [
        Span::styled(" / ", key_style()),
        Span::styled(format!(" {}", t.write_lead), dim()),
    ];
    let key_width: usize = key.iter().map(Span::width).sum();
    let room = width.saturating_sub(chars(&spans) + key_width + 3);
    let me = Party::Agent(a.id);
    // Every frame reads it: the journal's newest lines only. Past them
    // the line says nothing rather than that nothing was said.
    let last = (w.journal.iter().rev().take(LEAD_BACK))
        .find(|e| words::said(&e.event).is_some_and(|s| s.from == me && s.to == Party::Person));
    let all_read = w.journal.len() <= LEAD_BACK;
    let mut tail = Vec::new();
    if mark == "●" {
        spans.extend(status_spans(app.lang, w, a));
        tail = super::salon::since_spans(app, a);
    } else if let Some((e, s)) = last.and_then(|e| Some((e, words::said(&e.event)?))) {
        let seconds = w.tick.saturating_sub(e.tick) * SECONDS_PER_TICK;
        if seconds >= 60 {
            let ago = format!(" · {}", words::ago(app.lang, seconds));
            tail.push(Span::styled(ago, dim()));
        }
        let first = s.text.lines().find(|l| !l.trim().is_empty());
        let text = plain(&clean(first.unwrap_or_default()));
        spans.push(Span::raw(brief(&text, room.saturating_sub(chars(&tail)))));
    } else if all_read {
        spans.push(Span::styled(t.lead_quiet, dim()));
    }
    spans.extend(tail);
    let (line, _) = super::overlay::cut(Line::from(spans), width.saturating_sub(key_width + 3));
    let mut spans = line.spans;
    let gap = width.saturating_sub(chars(&spans) + key_width + 1);
    if gap >= 2 {
        spans.push(Span::raw(" ".repeat(gap)));
        spans.extend(key);
    }
    Some(Line::from(spans))
}

/// Whether a project in view has a worker (a solo, a helper, a
/// researcher too), listed or not: one with a change waiting has no
/// row of its own under AT WORK, yet the team is not empty.
fn staffed(app: &App) -> bool {
    let w = &app.world;
    let listed = (app.home_rows().iter()).any(|r| matches!(r.1, HomeRow::Agent(_)));
    listed
        || (w.agents.iter()).any(|a| {
            a.role != Role::Leader && !a.gone && !w.put_away(a.id) && !w.projects[a.project].closed
        })
}

/// While no project has a worker, imece's mark in the rows the list
/// leaves free under it (record 101).
fn mark_under(f: &mut Frame, area: Rect, app: &App, used: u16) {
    if staffed(app) {
        return;
    }
    let free = Rect {
        y: area.y + used.min(area.height),
        height: area.height.saturating_sub(used),
        ..area
    };
    brand::salon(f, free, app);
}

/// How the projects in view stand on Lonca; none when none is on it.
fn lonca_line(app: &App) -> Option<Line<'static>> {
    let w = &app.world;
    let t = app.lang.text();
    let mut spans = Vec::new();
    for p in w.open_projects() {
        if app.at_home.scope.is_some_and(|s| s != p.id) {
            continue;
        }
        if let Some(on) = &p.lonca {
            spans.push(salon::store_span(on, t));
        }
    }
    (!spans.is_empty()).then(|| Line::from(spans))
}

/// The list's rows with their groups, the group word of each decide
/// group, and the counts the two headings say.
struct Listed {
    rows: Vec<(HomeGroup, HomeRow)>,
}

impl Listed {
    fn many(&self, group: HomeGroup) -> usize {
        self.rows.iter().filter(|r| r.0 == group).count()
    }
}

/// `TO DECIDE  2 permissions · 1 question · 3 changes · 1 reply`: the heading and
/// what is under it, each kind counted, a count of none left out.
fn decide_heading(app: &App, listed: &Listed) -> Line<'static> {
    let t = app.lang.text();
    let w = &app.world;
    let mut spans = vec![Span::styled(format!(" {} ", t.home_turn), bold(pal().warn))];
    let stopped = match listed.many(HomeGroup::Stopped) {
        0 => None,
        n => Some(counted(n, t.halt_count)),
    };
    let counts: Vec<String> = [
        HomeGroup::Permission,
        HomeGroup::Question,
        HomeGroup::Change,
        HomeGroup::Reply,
        HomeGroup::Later,
    ]
    .into_iter()
    .zip(t.home_counts)
    .filter_map(|(group, (one, many))| match listed.many(group) {
        0 => None,
        1 => Some(one.to_string()),
        n => Some(fill(many, &[&n.to_string()])),
    })
    .collect();
    let mut words: Vec<String> = stopped.into_iter().chain(counts).collect();
    // What `:clear` took off the list, still open (record 90).
    let hidden = w.hidden_inbox();
    if hidden > 0 {
        words.push(fill(t.home_hidden, &[&hidden.to_string()]));
    }
    spans.push(Span::styled(words.join(" · "), dim()));
    Line::from(spans)
}

/// `AT WORK  12 · stuck 2 · finished 1 · idle 1 · 2 hidden (:unsettle)`:
/// who works (the projects' headings not counted), how many of them are
/// stuck, how many of the idle finished their work and how many did not,
/// and how many agents and projects `s` hid, when any are.
fn work_heading(app: &App, listed: &Listed) -> Line<'static> {
    let t = app.lang.text();
    let mut spans = vec![Span::styled(format!(" {} ", t.home_work), bold(pal().ok))];
    let agents = (listed.rows.iter())
        .filter(|r| r.0.heading() == Heading::Work && r.0 != HomeGroup::Team)
        .count();
    let mut words = vec![agents.to_string()];
    let quiet = listed.many(HomeGroup::Quiet);
    let finished = (listed.rows.iter())
        .filter(|r| match r {
            (HomeGroup::Quiet, HomeRow::Agent(id)) => rest_of(&app.world, *id).finished(),
            _ => false,
        })
        .count();
    for (n, word) in [
        (listed.many(HomeGroup::Stuck), t.home_stuck_count),
        (finished, t.home_finished_count),
        (quiet - finished, t.home_idle_count),
    ] {
        if n > 0 {
            words.push(fill(word, &[&n.to_string()]));
        }
    }
    let hidden = app.settled.len() + app.settled_projects.len();
    if hidden > 0 {
        let said = fill(t.settled_count, &[&hidden.to_string()]);
        words.push(said.trim_start_matches("· ").trim_end().to_string());
    }
    spans.push(Span::styled(words.join(" · "), dim()));
    Line::from(spans)
}

/// A decide group's own line under the heading: its word, and at the
/// right what makes its rows cheap (`one key each`, `3 ready to approve`).
fn group_line(app: &App, group: HomeGroup, listed: &Listed, width: usize) -> Line<'static> {
    let t = app.lang.text();
    let w = &app.world;
    let word = match group {
        HomeGroup::Stopped => t.halt_group,
        HomeGroup::Permission => t.home_groups[0],
        HomeGroup::Question => t.home_groups[1],
        HomeGroup::Change => t.home_groups[2],
        HomeGroup::Reply => t.home_groups[3],
        _ => t.home_groups[4],
    };
    let note = match group {
        HomeGroup::Permission => Some(t.home_one_key.to_string()),
        HomeGroup::Change => {
            let ready = (listed.rows.iter())
                .filter(|r| r.0 == group)
                .filter(|r| match r.1 {
                    HomeRow::Item(i) => {
                        let item = &w.inbox[i];
                        item.kind == InboxKind::Change
                            && w.verdict(item.agent).is_some_and(|v| v.ready)
                    }
                    HomeRow::Agent(_) | HomeRow::Project(_) | HomeRow::Stop(..) => false,
                })
                .count();
            (ready > 0).then(|| fill(t.home_ready, &[&ready.to_string()]))
        }
        _ => None,
    };
    let mut spans = vec![Span::styled(format!(" {word}"), dim())];
    if let Some(note) = note {
        let used = word.chars().count() + 1;
        let free = width.saturating_sub(used + note.chars().count() + 1);
        if free > 1 {
            spans.push(Span::raw(" ".repeat(free)));
            spans.push(Span::styled(note, dim()));
        }
    }
    Line::from(spans)
}

/// Where the agents' glyphs stand in the list's lines: the line, the
/// span and the agent, for the breath's patch (`note_glyph`).
fn glyph_spots(app: &App, lines_before: usize, row: HomeRow) -> Option<(usize, usize, AgentId)> {
    let HomeRow::Agent(id) = row else {
        return None;
    };
    let under = matches!(app.world.agents[id].role, Role::Helper | Role::Researcher);
    Some((lines_before, if under { 2 } else { 1 }, id))
}

/// Home's list as drawn: its lines, the lines the row at hand takes
/// (from, to), and where the agents' glyphs stand (the line, the span,
/// the agent) for the breath's patch.
struct Drawn {
    lines: Vec<Line<'static>>,
    at: Option<(usize, usize)>,
    spots: Vec<(usize, usize, AgentId)>,
}

/// Lines kept above the row at hand once the list scrolls.
const ABOVE: usize = 4;

impl Drawn {
    /// How far the list scrolls in `height` rows: not at all while the
    /// row at hand shows whole with a line under it, so the headings stay
    /// in view; past that the row at hand `ABOVE` lines from the top, the
    /// list's end never above the area's.
    fn scroll(&self, height: u16) -> u16 {
        let height = usize::from(height);
        let Some((from, to)) = self.at else {
            return 0;
        };
        if to + 1 < height {
            return 0;
        }
        let last = self.lines.len().saturating_sub(height);
        let scroll = from.saturating_sub(ABOVE).min(last);
        // A row taller than what is above it on the screen still ends in view.
        let scroll = scroll.max((to + 1).saturating_sub(height)).min(from);
        u16::try_from(scroll).unwrap_or(u16::MAX)
    }
}

/// The list in rows `width` wide, on a screen as wide, and how far to
/// scroll it in `height` rows for the row at hand to show.
#[cfg(test)]
fn list_lines(
    app: &App,
    width: usize,
    height: u16,
    at: Option<HomeRow>,
) -> (Vec<Line<'static>>, u16) {
    let drawn = list_drawn(app, width, width < usize::from(NARROW), at);
    let scroll = drawn.scroll(height);
    (drawn.lines, scroll)
}

/// The list drawn: see [`Drawn`]. `stacked` on a narrow screen (record
/// 131), not where the list is narrow beside the preview.
fn list_drawn(app: &App, width: usize, stacked: bool, at: Option<HomeRow>) -> Drawn {
    let t = app.lang.text();
    let listed = Listed {
        rows: app.home_rows(),
    };
    let mut lines = Vec::new();
    let mut at_lines = None;
    let mut spots = Vec::new();
    let mut last: Option<HomeGroup> = None;
    for &(group, row) in &listed.rows {
        if last != Some(group) {
            let heading = last.is_none_or(|l| l.heading() != group.heading());
            if heading {
                if last.is_some() {
                    lines.push(Line::default());
                }
                lines.push(match group.heading() {
                    Heading::Decide => decide_heading(app, &listed),
                    Heading::Work => work_heading(app, &listed),
                });
            }
            if group.heading() == Heading::Decide {
                lines.push(group_line(app, group, &listed, width));
            }
            last = Some(group);
        }
        let here = at == Some(row);
        let from = lines.len();
        spots.extend(glyph_spots(app, lines.len(), row));
        lines.extend(row_lines(app, row, width, stacked, here));
        if here {
            at_lines = Some((from, lines.len().saturating_sub(1).max(from)));
        }
    }
    // Nothing to decide but what `:clear` hid: the heading says so.
    if !listed.rows.iter().any(|r| r.0.heading() == Heading::Decide) && app.world.hidden_inbox() > 0
    {
        lines.insert(0, decide_heading(app, &listed));
        for spot in &mut spots {
            spot.0 += 1;
        }
        at_lines = at_lines.map(|(from, to)| (from + 1, to + 1));
    }
    // What stops a team is said first (record 146), then what is
    // finished in view (record 145); when nothing is listed under that,
    // it has said that nothing is left.
    let mut done = halt::banner(app, width);
    done.extend(round::banner(app, width));
    if listed.rows.is_empty() && !round::in_home(app).is_some_and(|f| f.all()) {
        // Whole: wrapped to the width, never cut.
        for row in overlay::wrap_words(t.home_nothing, width.saturating_sub(1).max(1)) {
            lines.push(Line::from(Span::styled(format!(" {row}"), dim())));
        }
    }
    if !done.is_empty() {
        let by = done.len();
        for spot in &mut spots {
            spot.0 += by;
        }
        at_lines = at_lines.map(|(from, to)| (from + by, to + by));
        lines.splice(0..0, done);
    }
    Drawn {
        lines,
        at: at_lines,
        spots,
    }
}

/// Notes where the glyphs that breathe are on the screen, so a frame
/// where only the breath changed is patched from this one (record 84).
fn note_breath(
    app: &App,
    lines: &[Line<'static>],
    spots: &[(usize, usize, AgentId)],
    scroll: u16,
    area: Rect,
) {
    let rungs = app.rungs();
    for &(line, span, id) in spots {
        let Some(row) = line.checked_sub(usize::from(scroll)) else {
            continue;
        };
        if row >= usize::from(area.height) {
            continue;
        }
        let a = &app.world.agents[id];
        if breathes(a, rungs[id]) {
            note_glyph(app, &lines[line], span, area, row);
        }
    }
}

/// The card a row is about.
fn card_of(w: &World, row: HomeRow) -> Option<&Card> {
    let id = match row {
        HomeRow::Item(i) => (w.inbox[i].card).or(w.agents[w.inbox[i].agent].card),
        HomeRow::Agent(a) => w.agents[a].card,
        HomeRow::Project(_) | HomeRow::Stop(..) => None,
    };
    id.and_then(|c| w.cards.get(c))
}

/// What a row is called: its change's title, else its card's, else the
/// person's line it works on with no card yet (record 126), else the
/// agent's name; a project's heading its name.
fn title_of(w: &World, row: HomeRow) -> String {
    if let HomeRow::Project(p) | HomeRow::Stop(p, _) = row {
        return clean(&w.projects[p].name);
    }
    let change = match row {
        HomeRow::Item(i) if w.inbox[i].kind == InboxKind::Change => w.change_of(w.inbox[i].agent),
        _ => None,
    };
    if let Some(c) = change {
        return clean(&c.title);
    }
    if let Some(c) = card_of(w, row) {
        return clean(&c.title);
    }
    // A leader's question about a worker's block is the blocked work's
    // row (record 143): called by that worker's work.
    let asked = match row {
        HomeRow::Item(i) if w.inbox[i].kind == InboxKind::Question => {
            w.asked_block(w.inbox[i].agent).map(|(worker, _)| worker)
        }
        _ => None,
    };
    let a = &w.agents[asked.unwrap_or_else(|| agent_of(w, row))];
    match &a.task.own {
        Some(own) => clean(&own.title),
        None => clean(&a.name),
    }
}

/// The agent a row belongs to; a project's heading its leader.
fn agent_of(w: &World, row: HomeRow) -> AgentId {
    match row {
        HomeRow::Item(i) => w.inbox[i].agent,
        HomeRow::Agent(a) => a,
        HomeRow::Project(p) | HomeRow::Stop(p, _) => w.projects[p].leader,
    }
}

/// A project's heading over its agents (record 125, step 5): the team's
/// project line, the cursor before it, the row at hand on the selection
/// colour.
fn project_row(app: &App, p: ProjectId, here: bool) -> Line<'static> {
    let rungs = app.rungs();
    let mut spans = vec![Span::styled(
        if here { "▸" } else { " " },
        bold(pal().strong),
    )];
    spans.extend(salon::project_line(app, p, &rungs).spans);
    let line = Line::from(spans);
    if here {
        line.style(Style::new().bg(pal().selection))
    } else {
        line
    }
}

/// The commands of a script, for the row that asks about it: its lines
/// that are not comments, one after the other; the whole is one `enter`
/// away.
pub(crate) fn commands_of(text: &str) -> String {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    clean(&lines.join("; "))
}

/// What a row asks or says, after its title: words that wrap, a
/// change's figures in their colours (a part that does not fit goes
/// under the first), or one line that is cut, an agent's.
enum Tail {
    Words(String),
    /// A question's words, then the answers it offers, one a line.
    Asked(String, Vec<String>),
    Spans(Vec<Span<'static>>),
    Cut(String),
    /// One line said in several ways, the longest first: the first that
    /// fits is drawn, the last cut when none does.
    Fits(Vec<Vec<Span<'static>>>),
}

impl Tail {
    /// How wide a one-line tail is, said whole; none for words that wrap.
    fn whole(&self) -> Option<usize> {
        match self {
            Tail::Cut(text) => Some(text.width()),
            Tail::Fits(ways) => ways.first().map(|way| way.iter().map(Span::width).sum()),
            Tail::Words(_) | Tail::Asked(..) | Tail::Spans(_) => None,
        }
    }
}

/// What became of the work an idle worker (or solo) last had; nothing is
/// said of anyone else's: a helper's work is its worker's.
fn rest_of(w: &World, id: AgentId) -> Last {
    match w.agents[id].role {
        Role::Worker | Role::Solo => w.last_work(id),
        Role::Leader | Role::Helper | Role::Researcher => Last::Unsaid,
    }
}

/// [`rest_of`] in words, in its colour: `finished, applied`, `#12
/// failed`, `no work yet`; none when nothing is said.
fn rest_span(lang: Lang, w: &World, id: AgentId) -> Option<Span<'static>> {
    let t = lang.text();
    Some(match rest_of(w, id) {
        Last::Applied => Span::styled(t.rest_applied, fg(pal().ok)),
        Last::Done => Span::styled(t.rest_done, fg(pal().ok)),
        Last::Failed(c) => Span::styled(fill(t.rest_failed, &[&words::card_ref(c)]), fg(pal().bad)),
        Last::Withdrawn(c) => Span::styled(fill(t.rest_withdrawn, &[&words::card_ref(c)]), dim()),
        Last::Nothing => Span::styled(t.rest_nothing, dim()),
        Last::Unsaid => return None,
    })
}

/// What an item's row says: the whole ask, in the person's words, without
/// the agent's name (the preview has it).
fn item_tail(app: &App, i: usize) -> Tail {
    let w = &app.world;
    let t = app.lang.text();
    let item = &w.inbox[i];
    let text = match item.kind {
        InboxKind::Approval => fill(t.row_run, &[&words::one_line(&item.detail)]),
        InboxKind::Script | InboxKind::Check => {
            let commands = app.script_text(i).map(|s| commands_of(&s.text()));
            let words = if item.kind == InboxKind::Check {
                t.row_check
            } else {
                t.row_script
            };
            fill(words, &[&item.detail, commands.as_deref().unwrap_or("")])
        }
        InboxKind::Question => {
            let options = w.question_of(item.agent).map(|q| q.options.to_vec());
            // A leader's question about a worker's block says so first
            // (record 143): the row is the blocked work's.
            let asked = match w.asked_block(item.agent) {
                Some((worker, b)) => fill(
                    (b.what).of([t.row_block_asks, t.row_ended_asks, t.row_failed_asks]),
                    &[
                        &w.agents[worker].name,
                        &w.agents[item.agent].name,
                        &plain(&item.detail),
                    ],
                ),
                None => plain(&item.detail),
            };
            return Tail::Asked(asked, options.unwrap_or_default());
        }
        InboxKind::Blocked => {
            let words = (w.block_what(item.agent)).of([t.row_blocked, t.row_ended, t.row_failed]);
            let said = fill(words, &[&plain(&item.detail)]);
            match w.agents[item.agent]
                .block
                .as_ref()
                .map(|b| (b.leader, &b.stage))
            {
                Some((leader, crate::blocked::Stage::Yours(why))) => {
                    format!("{said} · {}", words::unsolved(t, w, leader, *why))
                }
                _ => said,
            }
        }
        InboxKind::Research => {
            let by = item.researcher.map_or("?", |r| w.agents[r].name.as_str());
            fill(t.row_research, &[by, &item.detail])
        }
        InboxKind::Reply => fill(t.row_reply, &[&item.detail]),
        InboxKind::Fault => fill(t.row_fault, &[&item.detail]),
        InboxKind::Plan => {
            let (cards, cost) = words::plan_figures(w, item.agent);
            fill(
                t.row_plan,
                &[&counted(cards, t.n_cards), &dollars(cost), &item.detail],
            )
        }
        InboxKind::Tries | InboxKind::Rounds => {
            let words = match item.kind {
                InboxKind::Tries => t.row_tries,
                _ => t.row_rounds,
            };
            fill(words, &[&words::cap_count(w, item).to_string()])
        }
        InboxKind::Close => {
            let (n, titled) = words::item_named(w, item);
            let card = item.card.map_or(String::new(), words::card_ref);
            fill(t.row_close, &[&n, &titled, &card])
        }
        InboxKind::Sensitive => fill(t.row_sensitive, &[&item.detail]),
        InboxKind::Change => {
            return match (w.change_of(item.agent), w.verdict(item.agent)) {
                (Some(c), Some(v)) => {
                    let mut spans = change_spans(app.lang, c, &v);
                    // The issue its card closes (record 142), last.
                    if let Some((n, ..)) = w.change_closes(item.agent) {
                        spans.push(Span::styled(" · ", dim()));
                        let said = fill(t.verdict_closes, &[&n.to_string()]);
                        spans.push(Span::styled(said, dim()));
                    }
                    Tail::Spans(spans)
                }
                _ => Tail::Words(clean(&item.detail)),
            };
        }
    };
    Tail::Words(text)
}

/// A change's figures: its files and lines, what was proved on its tree
/// (the check, else the runs, else that nothing ran), and the files
/// outside its card's paths.
pub(crate) fn change_spans(lang: Lang, c: &Change, v: &Verdict) -> Vec<Span<'static>> {
    diff::verdict_spans(lang, v, false, Some(c.counts()))
}

/// What an agent's row says: the step it is on, else its status, then
/// how long, from a minute on; the row's own glyph is not said twice.
/// An idle worker's row also says what became of its last work (`idle ·
/// finished, applied · 6 min`); where that does not fit the time goes
/// first, then `idle`.
fn agent_tail(app: &App, id: AgentId, glyph: &str) -> Tail {
    let w = &app.world;
    let lang = app.lang;
    let a = &w.agents[id];
    let mut text = on_step(w, a).map_or_else(
        || {
            let text: String = status_spans(lang, w, a)
                .iter()
                .map(|s| s.content.as_ref())
                .collect();
            text
        },
        str::to_string,
    );
    if let Some(rest) = text.strip_prefix(glyph) {
        text = rest.trim_start().to_string();
    }
    if text.trim().is_empty() {
        text = lang.text().home_idle.to_string();
    }
    let since = w.tick.saturating_sub(app.since(id)) * SECONDS_PER_TICK;
    let ago = (since >= 60).then(|| format!(" · {}", words::ago(lang, since)));
    let idle = a.status == Status::Idle && text == lang.text().idle;
    if idle && let Some(rest) = rest_span(lang, w, id) {
        let said = vec![Span::raw(format!("{text} · ")), rest.clone()];
        let mut ways = Vec::new();
        if let Some(ago) = ago {
            let mut timed = said.clone();
            timed.push(Span::raw(ago));
            ways.push(timed);
        }
        ways.push(said);
        ways.push(vec![rest]);
        return Tail::Fits(ways);
    }
    if let Some(ago) = ago {
        text.push_str(&ago);
    }
    Tail::Cut(text)
}

/// The keys that decide the row at hand, at its end: the two that
/// decide, in a word each; the status line has the rest.
fn row_keys(app: &App) -> Vec<Span<'static>> {
    let t = app.lang.text();
    // `1-3` for a question that offers three answers, `1` for one.
    let pick = match app.home_at() {
        Some(HomeRow::Item(i)) => match app.world.question_of(app.world.inbox[i].agent) {
            Some(q) if q.options.len() > 1 => format!("1-{}", q.options.len()),
            Some(q) if q.options.len() == 1 => "1".to_string(),
            _ => String::new(),
        },
        _ => String::new(),
    };
    let keys: Vec<(&str, &str)> = match app.home_at() {
        Some(HomeRow::Item(i)) => match app.world.inbox[i].kind {
            InboxKind::Change => vec![("a", t.apply), ("r", t.reject)],
            InboxKind::Question if !pick.is_empty() => {
                vec![(pick.as_str(), t.home_pick), ("a", t.home_answer)]
            }
            InboxKind::Question | InboxKind::Fault | InboxKind::Blocked => {
                vec![("a", t.home_answer), ("enter", t.home_open)]
            }
            // A reply asks for nothing: `enter` reads it, `r` puts it away.
            InboxKind::Reply => vec![("enter", t.home_open), ("r", t.home_dismiss)],
            InboxKind::Tries | InboxKind::Rounds => {
                vec![("a", t.home_once_more), ("r", t.home_fail)]
            }
            InboxKind::Close => vec![("a", t.home_close), ("r", t.home_leave)],
            _ => vec![("a", t.home_yes), ("r", t.home_no)],
        },
        Some(HomeRow::Agent(_)) => vec![("enter", t.home_open)],
        // A heading decides nothing: the status line has its keys. A
        // stop's row draws its own (`halt::row`).
        Some(HomeRow::Project(_) | HomeRow::Stop(..)) | None => Vec::new(),
    };
    let mut spans = hint_spans(&keys);
    for s in &mut spans {
        if s.style != key_style() {
            s.style = dim();
        }
    }
    spans
}

/// How many cells past its column a title longer than it may take: what
/// a one-line row leaves free after its words whole (and, at hand, its
/// keys and the least its words keep beside them), so a long title is
/// cut only where the row is full; none for a row whose words wrap.
fn title_room(
    app: &App,
    head: &[Span<'static>],
    tail: &Tail,
    title: &str,
    width: usize,
    column: usize,
    here: bool,
) -> usize {
    let Some(text) = tail.whole() else {
        return 0;
    };
    let before: usize = head.iter().map(Span::width).sum();
    let keys: usize = if here {
        row_keys(app).iter().map(Span::width).sum()
    } else {
        0
    };
    // Keys that would not fit beside the column's title go anyway.
    let keys = if width.saturating_sub(before + column + 2 + keys + 2) >= ASK_MIN {
        keys
    } else {
        0
    };
    let said = if keys > 0 {
        text.max(ASK_MIN) + keys + 2
    } else {
        text + 1
    };
    let free = width.saturating_sub(before + column + 2 + said);
    free.min(title.width().saturating_sub(column))
}

/// The rows of one entry of the list: its glyph, project (when several
/// are open and the list is not narrowed), card and title, then what it
/// asks or says, whole: the words wrap under their own column, and the
/// row at hand ends with the keys that decide it. `narrow` (record 131):
/// the words stand under the title instead, the keys are the status
/// line's alone.
fn row_lines(
    app: &App,
    row: HomeRow,
    width: usize,
    narrow: bool,
    here: bool,
) -> Vec<Line<'static>> {
    let w = &app.world;
    if let HomeRow::Project(p) = row {
        return vec![project_row(app, p, here)];
    }
    if let HomeRow::Stop(p, halt) = row {
        let title_width = TITLE.min(width.saturating_sub(40)).max(TITLE_MIN);
        return halt::row(app, p, halt, width, narrow, here, title_width);
    }
    let agent = &w.agents[agent_of(w, row)];
    let (glyph, colour) = match row {
        HomeRow::Agent(_) => glyph(agent),
        HomeRow::Item(_) | HomeRow::Project(_) | HomeRow::Stop(..) => ("!", pal().warn),
    };
    let tail = match row {
        HomeRow::Item(i) => item_tail(app, i),
        HomeRow::Agent(id) => agent_tail(app, id, glyph),
        HomeRow::Project(_) | HomeRow::Stop(..) => unreachable!("a heading is its own row"),
    };
    let mut head = vec![Span::styled(
        if here { "▸" } else { " " },
        bold(pal().strong),
    )];
    // A helper or a researcher hangs under its worker's row.
    let under =
        matches!(row, HomeRow::Agent(_)) && matches!(agent.role, Role::Helper | Role::Researcher);
    if under {
        head.push(Span::styled("  └ ", dim()));
    }
    // A working row's glyph breathes as the team's did (record 84).
    let glyph_style = match row {
        HomeRow::Agent(id) => glyph_style(app, agent, app.rungs()[id], colour),
        HomeRow::Item(_) | HomeRow::Project(_) | HomeRow::Stop(..) => fg(colour),
    };
    head.push(Span::styled(glyph, glyph_style));
    head.push(Span::raw(" "));
    // On a narrow screen (record 131) the words stand under the title,
    // from where the card starts.
    let lead: usize = head.iter().map(Span::width).sum();
    let many = w.open_projects().count() > 1;
    if many && app.at_home.scope.is_none() {
        head.push(Span::styled(
            fit(&w.projects[agent.project].name, 11),
            dim(),
        ));
        head.push(Span::raw(" "));
    }
    // A helper's row has no card of its own: its branch takes the column.
    if !under {
        let card = card_of(w, row).map_or(String::new(), |c| words::card_ref(c.id));
        head.push(Span::styled(format!("{card:<5}"), dim()));
    }
    let title_width = TITLE.min(width.saturating_sub(40)).max(TITLE_MIN);
    let style = if here {
        Style::new().add_modifier(Modifier::BOLD)
    } else {
        Style::new()
    };
    let mut lines = Vec::new();
    let selection = |line: Line<'static>| {
        if here {
            line.style(Style::new().bg(pal().selection))
        } else {
            line
        }
    };
    if narrow {
        let title = clean(&title_of(w, row));
        let before: usize = head.iter().map(Span::width).sum();
        // A row that says only what it does keeps one line when it fits.
        let said = match &tail {
            Tail::Cut(text) => Some(vec![Span::raw(text.clone())]),
            Tail::Spans(spans) => Some(spans.clone()),
            Tail::Fits(ways) => ways.first().cloned(),
            Tail::Words(_) | Tail::Asked(..) => None,
        };
        if let Some(said) = said {
            let said_width: usize = said.iter().map(Span::width).sum();
            if before + title.width() + 2 + said_width < width {
                head.push(Span::styled(format!("{title}  "), style));
                head.extend(said);
                return vec![selection(Line::from(head))];
            }
        }
        let room = width.saturating_sub(before + 1);
        head.push(Span::styled(
            fit(&title, room).trim_end().to_string(),
            style,
        ));
        lines.push(selection(Line::from(std::mem::take(&mut head))));
        head.push(Span::raw(" ".repeat(lead)));
    } else {
        let title = title_of(w, row);
        let title_width =
            title_width + title_room(app, &head, &tail, &title, width, title_width, here);
        head.push(Span::styled(
            format!("{:<title_width$}  ", fit(&title, title_width)),
            style,
        ));
    }
    let used: usize = head.iter().map(Span::width).sum();
    // A narrow row's keys are the status line's alone.
    let keys = if here && !narrow {
        row_keys(app)
    } else {
        Vec::new()
    };
    let keys_width: usize = keys.iter().map(Span::width).sum();
    // The ask gets at least `ASK_MIN` on the first line: when the keys
    // would leave less, they go, the status line still has them.
    let keys = if width.saturating_sub(used + keys_width + 2) >= ASK_MIN {
        keys
    } else {
        Vec::new()
    };
    let keys_width: usize = keys.iter().map(Span::width).sum();
    let first_room = width.saturating_sub(used + keys_width + if keys_width > 0 { 2 } else { 1 });
    let end_first = |mut spans: Vec<Span<'static>>, lines: &mut Vec<Line<'static>>| {
        if !keys.is_empty() {
            let filled: usize = spans.iter().map(Span::width).sum();
            spans.push(Span::raw(
                " ".repeat(width.saturating_sub(filled + keys_width + 1)),
            ));
            spans.extend(keys.iter().cloned());
        }
        lines.push(selection(Line::from(spans)));
    };
    let rest_room = width.saturating_sub(used + 1);
    // Words wrap under their own column; the answers a question offers
    // follow, one a line, each with the key that picks it.
    let words_tail = |text: String, options: Vec<String>, lines: &mut Vec<Line<'static>>| {
        let mut rows = overlay::wrap_words(&text, first_room.max(1)).into_iter();
        let mut line = head.clone();
        line.push(Span::raw(rows.next().unwrap_or_default()));
        end_first(line, lines);
        let left: Vec<String> = rows.collect();
        if !left.is_empty() {
            // The rest of the words, as wide as the row lets them.
            let whole: String = left.join(" ");
            for row in overlay::wrap_words(&whole, rest_room.max(1)) {
                lines.push(selection(Line::from(vec![
                    Span::raw(" ".repeat(used)),
                    Span::raw(row),
                ])));
            }
        }
        for (n, option) in options.iter().enumerate() {
            let key = Span::styled(format!(" {} ", n + 1), key_style());
            let room = rest_room.saturating_sub(key.width() + 1).max(1);
            lines.push(selection(Line::from(vec![
                Span::raw(" ".repeat(used)),
                key,
                Span::raw(format!(" {}", fit(&clean(option), room).trim_end())),
            ])));
        }
    };
    match tail {
        Tail::Cut(text) => {
            let mut line = head;
            line.push(Span::raw(fit(&text, first_room).trim_end().to_string()));
            end_first(line, &mut lines);
        }
        Tail::Fits(ways) => {
            let wide = |way: &Vec<Span<'static>>| way.iter().map(Span::width).sum::<usize>();
            let way = (ways.iter().find(|way| wide(way) <= first_room))
                .or(ways.last())
                .cloned()
                .unwrap_or_default();
            let mut line = head;
            let mut room = first_room;
            for s in way {
                let s = if s.width() > room {
                    Span::styled(fit(&s.content, room).trim_end().to_string(), s.style)
                } else {
                    s
                };
                room -= s.width();
                line.push(s);
            }
            end_first(line, &mut lines);
        }
        Tail::Spans(spans) => {
            // Each part whole on a line, the first line's room first; a
            // part wider than a whole line is cut.
            let mut line = head;
            let mut room = first_room;
            let mut first = true;
            for s in spans {
                let n = s.width();
                // A narrow row's first line under its title holds only
                // its lead: a part goes on it, cut if need be.
                if n > room && !(first && line.len() == 3) && !(narrow && line.len() == 1) {
                    if first {
                        end_first(std::mem::take(&mut line), &mut lines);
                        first = false;
                    } else {
                        lines.push(selection(Line::from(std::mem::take(&mut line))));
                    }
                    line.push(Span::raw(" ".repeat(used)));
                    room = rest_room;
                }
                let s = if line.len() == 1 {
                    // First on its line: no separator before it.
                    let text = s.content.trim_start_matches(" · ").to_string();
                    Span::styled(fit(&text, room).trim_end().to_string(), s.style)
                } else if n > room {
                    Span::styled(fit(&s.content, room).trim_end().to_string(), s.style)
                } else {
                    s
                };
                room = room.saturating_sub(s.width());
                line.push(s);
            }
            if first {
                end_first(line, &mut lines);
            } else {
                lines.push(selection(Line::from(line)));
            }
        }
        Tail::Words(text) => words_tail(text, Vec::new(), &mut lines),
        Tail::Asked(text, options) => words_tail(text, options, &mut lines),
    }
    lines
}

/// A row of the preview: a dim label in its own column, then the text.
fn labelled(label: &str, text: Vec<Span<'static>>) -> Line<'static> {
    let mut spans = vec![Span::styled(format!("{label:<9}"), dim())];
    spans.extend(text);
    Line::from(spans)
}

/// What a change comes to: its why, its proof, cost and gate, its files
/// and the top of its patch.
fn change_lines(lang: Lang, w: &World, c: &Change, lines: &mut Vec<Line<'static>>, room: usize) {
    let t = lang.text();
    let [verdict, why, cost, gate, files] = t.home_labels;
    if let Some(v) = w.verdict(c.agent) {
        let said = Line::from(diff::verdict_spans(lang, &v, true, None));
        for (n, line) in wrap(said, room.saturating_sub(9).max(12))
            .into_iter()
            .enumerate()
        {
            lines.push(labelled(if n == 0 { verdict } else { "" }, line.spans));
        }
    }
    // The worker's words as the Markdown they are: its lists stay lists.
    let said = w.change_why(c.agent).unwrap_or(&c.title);
    for (n, line) in markdown_rows(said, room.saturating_sub(9).max(12))
        .into_iter()
        .enumerate()
    {
        lines.push(labelled(if n == 0 { why } else { "" }, line.spans));
    }
    if let Some(s) = c.spend {
        lines.push(labelled(
            cost,
            vec![Span::styled(words::spend(lang, w, &s), dim())],
        ));
    }
    if let Some(p) = w.proposal_of(c.agent) {
        for (n, line) in diff::proposal_lines(lang, p).into_iter().enumerate() {
            lines.push(labelled(if n == 0 { gate } else { "" }, line.spans));
        }
    }
    lines.push(Line::default());
    for (n, file) in c.files.iter().take(FILES).enumerate() {
        let path = fit(&file.path, room.saturating_sub(24).max(12));
        lines.push(labelled(
            if n == 0 { files } else { "" },
            vec![
                Span::raw(format!("{path} ")),
                Span::styled(format!("+{}", file.added), fg(pal().ok)),
                Span::styled(format!(" −{}", file.removed), fg(pal().bad)),
            ],
        ));
    }
    if c.files.len() > FILES {
        let rest = (c.files.len() - FILES).to_string();
        lines.push(labelled(
            "",
            vec![Span::styled(fill(t.home_more, &[&rest]), dim())],
        ));
    }
}

/// The patch's own lines, what git says around them left out.
fn patch_lines(patch: &str, room: usize, rows: usize) -> Vec<Line<'static>> {
    let git = |l: &str| {
        ["diff --git", "index ", "--- ", "+++ "]
            .iter()
            .any(|p| l.starts_with(p))
    };
    (patch.lines())
        .filter(|l| !git(l))
        .take(rows)
        .map(|l| {
            let style = match l.as_bytes().first() {
                Some(b'+') => fg(pal().ok),
                Some(b'-') => fg(pal().bad),
                Some(b'@') => dim(),
                _ => Style::new(),
            };
            Line::from(Span::styled(fit(l, room).trim_end().to_string(), style))
        })
        .collect()
}

/// A script's lines as the person reads them before a yes: numbered, the
/// first `SCRIPT_ROWS`, the rest counted; the window has them all.
fn script_lines(t: &Text, text: &str, room: usize) -> Vec<Line<'static>> {
    let all: Vec<&str> = text.lines().collect();
    let mut lines: Vec<Line<'static>> = all
        .iter()
        .take(SCRIPT_ROWS)
        .enumerate()
        .map(|(n, l)| {
            Line::from(vec![
                Span::styled(format!("{:>3} ", n + 1), dim()),
                Span::raw(
                    fit(&clean(l), room.saturating_sub(4))
                        .trim_end()
                        .to_string(),
                ),
            ])
        })
        .collect();
    if all.len() > SCRIPT_ROWS {
        let rest = (all.len() - SCRIPT_ROWS).to_string();
        lines.push(Line::from(Span::styled(fill(t.home_more, &[&rest]), dim())));
    }
    lines
}

/// What an agent is at: its status, its steps, its newest lines.
fn agent_lines(app: &App, id: AgentId, lines: &mut Vec<Line<'static>>, room: usize, rows: usize) {
    let w = &app.world;
    let a = &w.agents[id];
    let t = app.lang.text();
    lines.push(Line::from(status_spans(app.lang, w, a)));
    // Blocked between turns (record 143): what it said, whole, then
    // what its leader has done about it so far.
    if let Some(b) = a.block.as_ref().filter(|_| !a.status.busy()) {
        let leader = clean(&w.agents[b.leader].name);
        let told = fill(t.block_told, &[&leader, &crate::model::clock(b.at)]);
        lines.push(Line::from(Span::styled(told, dim())));
        lines.push(Line::default());
        said_lines(&b.said, "", lines, room);
        lines.push(Line::default());
        let so_far = fill(t.block_so_far, &[&leader]);
        lines.push(Line::from(Span::styled(so_far, dim())));
        let left = rows.saturating_sub(lines.len());
        let trace = &w.agents[b.leader].trace;
        lines.extend(newest_trace(trace, speaker_color(b.leader), room, left));
        return;
    }
    lines.push(Line::default());
    lines.extend(focus::plan_lines(t, &a.plan));
    if !a.plan.is_empty() {
        lines.push(Line::default());
    }
    let left = rows.saturating_sub(lines.len());
    lines.extend(newest_trace(&a.trace, speaker_color(id), room, left));
}

/// The newest of a trace in `rows` lines, as the trace window follows
/// it: whole entries, the newest last, as many as fit (a line may wrap
/// to several); the newest alone, its first lines, when it is taller.
fn newest_trace(
    trace: &std::collections::VecDeque<String>,
    colour: Color,
    room: usize,
    rows: usize,
) -> Vec<Line<'static>> {
    let mut kept: Vec<Vec<Line<'static>>> = Vec::new();
    let mut used = 0;
    for raw in trace.iter().rev() {
        let entry = focus::trace_lines(raw, colour, room);
        if used + entry.len() > rows {
            if kept.is_empty() {
                kept.push(entry.into_iter().take(rows).collect());
            }
            break;
        }
        used += entry.len();
        kept.push(entry);
    }
    kept.into_iter().rev().flatten().collect()
}

/// A question whole, past its first line (the row has that), then the
/// answers it offers with their keys, then a line before what its agent
/// did so far.
fn question_lines(t: &Text, q: &Question, lines: &mut Vec<Line<'static>>, room: usize) {
    // Cleaned line by line: a newline is a control character too.
    let whole: Vec<String> = (q.text.lines())
        .map(|l| clean(l.trim_end()))
        .filter(|l| !l.trim().is_empty())
        .collect();
    if whole.len() > 1 {
        lines.push(Line::default());
        for text in &whole {
            for row in overlay::wrap_words(text, room) {
                lines.push(Line::from(Span::styled(row, bold(pal().strong))));
            }
        }
    }
    option_lines(q, lines, room);
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(t.home_so_far.to_string(), dim())));
}

/// The answers a question offers, each with the key that picks it, a
/// line before them; nothing when it offers none.
fn option_lines(q: &Question, lines: &mut Vec<Line<'static>>, room: usize) {
    if q.options.is_empty() {
        return;
    }
    lines.push(Line::default());
    for (n, option) in q.options.iter().enumerate() {
        let key = Span::styled(format!(" {} ", n + 1), key_style());
        let left = room.saturating_sub(key.width() + 1).max(1);
        lines.push(Line::from(vec![
            key,
            Span::raw(format!(" {}", fit(&clean(option), left).trim_end())),
        ]));
    }
}

/// An agent's words whole, cleaned line by line (a newline is a control
/// character too) and wrapped behind `lead`; blank lines left out.
fn said_lines(text: &str, lead: &str, lines: &mut Vec<Line<'static>>, room: usize) {
    let room = room.saturating_sub(lead.width()).max(1);
    for line in text.lines().map(|l| clean(l.trim_end())) {
        if line.trim().is_empty() {
            continue;
        }
        for row in overlay::wrap_words(&line, room) {
            lines.push(Line::from(vec![
                Span::raw(lead.to_string()),
                Span::raw(row),
            ]));
        }
    }
}

/// A leader's question about a worker's block, read whole (record 143):
/// who asks and why (it is risky, or the leader could not solve it),
/// then the worker's words, what the leader tried, what it asks and the
/// answers it offers, then a line before what the leader did so far.
pub(crate) fn block_question_lines(
    t: &Text,
    leader: &str,
    b: &crate::blocked::Block,
    q: &Question,
    lines: &mut Vec<Line<'static>>,
    room: usize,
) {
    let (tried, risk) = match &b.stage {
        crate::blocked::Stage::Asked { tried, risk } => (tried.as_str(), risk.as_deref()),
        _ => ("", None),
    };
    let why = match risk {
        Some(risk) => fill(t.block_risky, &[&plain(risk)]),
        None => t.block_could_not.to_string(),
    };
    let head = Line::from(vec![
        Span::styled(fill(t.asks_you_head, &[leader]), bold(pal().warn)),
        Span::styled(format!(" · {why}"), dim()),
    ]);
    lines.extend(super::wrap(head, room.max(1)));
    lines.push(Line::default());
    let mut part = |label: String, text: &str| {
        if text.trim().is_empty() {
            return;
        }
        lines.push(Line::from(Span::styled(label, dim())));
        said_lines(text, "  ", lines, room);
    };
    let word = (b.what).of([t.blocked_word, t.ended_word, t.failed_word]);
    part(word.to_string(), &b.said);
    part(fill(t.block_tried, &[leader]), tried);
    part(t.block_asks.to_string(), q.text);
    option_lines(q, lines, room);
    lines.push(Line::default());
    let so_far = fill(t.block_so_far, &[leader]);
    lines.push(Line::from(Span::styled(so_far, dim())));
}

/// What a row holds, read without a key: a heading, then what it is.
fn preview_lines(app: &App, row: HomeRow, room: usize, rows: usize) -> Vec<Line<'static>> {
    if let HomeRow::Stop(p, halt) = row {
        return halt::preview(app, p, halt, room);
    }
    let w = &app.world;
    let t = app.lang.text();
    let id = agent_of(w, row);
    let a = &w.agents[id];
    let project = clean(&w.projects[a.project].name);
    let mut head = Vec::new();
    if let Some(c) = card_of(w, row) {
        head.push(Span::styled(format!("{} ", words::card_ref(c.id)), dim()));
    }
    head.push(Span::styled(
        fit(&title_of(w, row), room.saturating_sub(30).max(10))
            .trim_end()
            .to_string(),
        bold(pal().strong),
    ));
    // A heading is the project: its leader and cost follow the name.
    let tail = match row {
        HomeRow::Project(_) => format!("  {} · {}", clean(&a.name), w.spent_by(a.id)),
        _ => format!("  {project} · {} · {}", clean(&a.name), w.spent_by(a.id)),
    };
    head.push(Span::styled(tail, dim()));
    let mut lines = vec![Line::from(head), Line::default()];
    match row {
        HomeRow::Project(p) => {
            // Where the project is and how it stands on Lonca, then its
            // leader's work.
            if let Some(folder) = app.project_folder(p) {
                lines.push(Line::from(Span::styled(folder, dim())));
            }
            if w.projects[p].lonca.is_some() {
                lines.push(Line::from(Span::styled(app.lonca_state(p), dim())));
            }
            agent_lines(app, id, &mut lines, room, rows);
        }
        HomeRow::Item(i) => {
            let item = &w.inbox[i];
            let change = (item.kind == InboxKind::Change)
                .then(|| w.change_of(item.agent))
                .flatten();
            if let Some(c) = change {
                change_lines(app.lang, w, c, &mut lines, room);
                lines.push(Line::default());
                let left = rows.saturating_sub(lines.len());
                lines.extend(patch_lines(&c.patch, room, left));
            } else if let (InboxKind::Question, Some((_, b)), Some(q)) = (
                item.kind,
                w.asked_block(item.agent),
                w.question_of(item.agent),
            ) {
                // The leader's question about a worker's block.
                block_question_lines(t, &clean(&a.name), b, &q, &mut lines, room);
                let left = rows.saturating_sub(lines.len());
                lines.extend(newest_trace(&a.trace, speaker_color(id), room, left));
            } else {
                let said = words::inbox(app.lang, w, item);
                for row in overlay::wrap_words(&said, room) {
                    lines.push(Line::from(Span::raw(row)));
                }
                if item.kind == InboxKind::Blocked {
                    // What the worker said, whole, and what its leader did.
                    lines.push(Line::default());
                    agent_lines(app, id, &mut lines, room, rows);
                } else if let Some(q) = w.question_of(item.agent) {
                    question_lines(t, &q, &mut lines, room);
                    agent_lines(app, id, &mut lines, room, rows);
                } else if !item.detail.is_empty() && !said.contains(item.detail.as_str()) {
                    lines.push(Line::default());
                    for row in overlay::wrap_words(&clean(&item.detail), room) {
                        lines.push(Line::from(Span::styled(row, bold(pal().strong))));
                    }
                }
                // A script's bytes are what the yes is to: read here.
                if matches!(item.kind, InboxKind::Script | InboxKind::Check)
                    && let Some(script) = app.script_text(i)
                {
                    lines.push(Line::default());
                    lines.extend(script_lines(t, &script.text(), room));
                }
            }
        }
        HomeRow::Agent(_) => agent_lines(app, id, &mut lines, room, rows),
        HomeRow::Stop(..) => {}
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Engine;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render(app: &App, width: u16, height: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
        t.draw(|f| crate::ui::draw(f, app)).unwrap();
        let buf = t.backend().buffer();
        buf.content
            .chunks(usize::from(width))
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn home(lang: Lang) -> App {
        let (world, sim) = crate::sim::world(2, 4, 0, 1, lang);
        let mut app = App::with(world, Engine::Sim(sim), lang);
        app.world.inbox.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
            a.starting = false;
        }
        app
    }

    fn workers(app: &App) -> Vec<AgentId> {
        (app.world.agents.iter())
            .filter(|a| a.role == Role::Worker)
            .map(|a| a.id)
            .collect()
    }

    fn worker(app: &App) -> AgentId {
        workers(app)[0]
    }

    fn item(kind: InboxKind, agent: AgentId, detail: &str) -> InboxItem {
        InboxItem {
            kind,
            agent,
            detail: detail.into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        }
    }

    fn ran(line: &str, code: i32) -> Ran {
        Ran {
            line: line.into(),
            code: Some(code),
            timed_out: false,
        }
    }

    fn change(agent: AgentId, runs: Vec<Ran>) -> Change {
        Change {
            agent,
            title: "Paging in product search".into(),
            files: vec![ChangedFile {
                path: "src/query.rs".into(),
                added: 19,
                removed: 6,
                binary: false,
            }],
            patch: "diff --git a/x b/x\nindex 1..2\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old line\n+new line\n"
                .into(),
            runs,
            check: Check::None,
            spend: None,
        }
    }

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn lines(app: &App, width: usize) -> Vec<String> {
        list_lines(app, width, 40, app.home_at())
            .0
            .iter()
            .map(text)
            .collect()
    }

    /// The list `width` wide in columns, as it stands beside the preview
    /// on a wide screen.
    fn beside(app: &App, width: usize) -> Vec<String> {
        list_drawn(app, width, false, app.home_at())
            .lines
            .iter()
            .map(text)
            .collect()
    }

    /// The demo stepped until the repository's check asks for its yes.
    fn demo_with_check() -> (App, usize) {
        let (world, sim) = crate::sim::world(1, 3, 0, 1, Lang::En);
        let mut app = App::with(world, Engine::Sim(sim), Lang::En);
        for _ in 0..400 {
            app.step();
            if let Some(i) =
                (app.world.inbox.iter()).position(|i| i.kind == InboxKind::Check && i.open)
            {
                return (app, i);
            }
        }
        panic!("the demo never asked about its check");
    }

    /// Home with every card of both projects done at tick 90, but the
    /// first project's second, which failed; now is tick 500.
    fn all_done(lang: Lang) -> App {
        let mut app = home(lang);
        for c in &mut app.world.cards {
            c.column = Column::Done;
            c.finished = 90;
        }
        app.world.cards[1].column = Column::Failed;
        app.world.tick = 500;
        app
    }

    fn rows(app: &App, width: u16) -> Vec<String> {
        (render(app, width, 14).lines())
            .map(|r| r.trim_end().to_string())
            .collect()
    }

    #[test]
    fn all_the_work_done_is_home_s_first_line_with_what_is_next_under_it() {
        let mut app = all_done(Lang::En);
        app.at_home.scope = Some(0);
        let got = rows(&app, 100);
        assert!(got[0].contains("shop-api ✓ · mobile-app ✓"), "{}", got[0]);
        assert_eq!(
            got[1],
            " ✓ ALL DONE  shop-api · 3 cards · 2 landed · ✕ 1 failed · 7 min"
        );
        assert_eq!(
            got[2],
            " Nothing is left for the team or for you. / tells the lead what is next."
        );
        assert_eq!(got[3], "");
        assert_eq!(got[4], " AT WORK 2 · idle 2");
        // The row at hand is still the one under the cursor.
        assert!(got[5].starts_with("▸○"), "{}", got[5]);
        // Every project in view: counted together.
        app.at_home.scope = None;
        let got = rows(&app, 100);
        let line = (got.iter()).find(|r| r.contains("ALL DONE")).unwrap();
        assert_eq!(
            line,
            " ✓ ALL DONE  2 projects · 6 cards · 5 landed · ✕ 1 failed · 7 min"
        );
        let mut app = all_done(Lang::Tr);
        app.at_home.scope = Some(0);
        let got = rows(&app, 100);
        assert!(
            got[1].starts_with(" ✓ HEPSİ BİTTİ  shop-api · 3 kart · 2 tamamlandı · ✕ 1 başarısız"),
            "{}",
            got[1]
        );
        assert_eq!(
            got[2],
            " Ekibe de size de kalan iş yok. / lidere sıradakini söyler."
        );
    }

    #[test]
    fn a_change_waiting_makes_it_the_team_finished_and_counts_what_waits() {
        let mut app = all_done(Lang::En);
        app.at_home.scope = Some(0);
        app.world.cards[0].column = Column::Review;
        app.world.cards[2].column = Column::Review;
        let got = rows(&app, 100);
        assert!(got[0].contains("shop-api · mobile-app ✓"), "{}", got[0]);
        assert_eq!(
            got[1],
            " ◐ TEAM FINISHED  shop-api · 3 cards · ✕ 1 failed · 2 wait for you · 41 min"
        );
        assert_eq!(got[2], "", "what is next is the person's: nothing under it");
        // One project all done and one whose team finished: the team's.
        app.at_home.scope = None;
        let got = rows(&app, 100);
        assert!(
            (got.iter())
                .any(|r| r.starts_with(" ◐ TEAM FINISHED  2 projects · 6 cards · 3 landed")),
            "{got:#?}"
        );
        // Too narrow for one line: the figures go on under the word,
        // whole.
        let got = rows(&app, 50);
        let at = (got.iter())
            .position(|r| r.contains("TEAM FINISHED"))
            .unwrap();
        assert_eq!(got[at], " ◐ TEAM FINISHED  2 projects · 6 cards · 3 landed");
        assert_eq!(got[at + 1], "   ✕ 1 failed · 2 wait for you · 41 min");
        // One card and one waiting are said in the singular.
        app.at_home.scope = Some(0);
        app.world.cards.retain(|c| c.project != 0 || c.id == 0);
        let got = rows(&app, 100);
        assert_eq!(
            got[1],
            " ◐ TEAM FINISHED  shop-api · 1 card · 1 waits for you · 41 min"
        );
    }

    #[test]
    fn while_a_card_is_open_or_somebody_works_home_says_nothing_of_it() {
        let mut app = all_done(Lang::En);
        app.at_home.scope = Some(0);
        app.world.cards[2].column = Column::Working;
        let got = rows(&app, 100);
        assert_eq!(got[1], " AT WORK 2 · idle 2");
        assert!(!got[0].contains("shop-api ✓"), "{}", got[0]);
        app.world.cards[2].column = Column::Done;
        let lead = app.world.projects[0].leader;
        app.world.agents[lead].status = Status::Working;
        assert!(rows(&app, 100)[1].starts_with(" AT WORK"));
        // The other project is still all done; both in view say nothing.
        app.at_home.scope = None;
        assert!(!render(&app, 100, 14).contains("ALL DONE"));
        app.at_home.scope = Some(1);
        assert!(
            rows(&app, 100)
                .iter()
                .any(|r| r.contains("✓ ALL DONE  mobile-app"))
        );
    }

    #[test]
    fn the_round_s_cost_is_on_the_line_when_its_cards_cost() {
        let mut app = all_done(Lang::En);
        app.at_home.scope = Some(0);
        app.world.ledger.push(crate::ledger::Attempt {
            project: 0,
            card: Some(0),
            cost: 41_200,
            ..Default::default()
        });
        assert_eq!(
            rows(&app, 100)[1],
            " ✓ ALL DONE  shop-api · 3 cards · 2 landed · ✕ 1 failed · $4.12 · 7 min"
        );
    }

    #[test]
    fn to_decide_counts_each_kind_and_the_groups_say_what_makes_them_cheap() {
        let mut app = home(Lang::En);
        let w = workers(&app);
        app.world
            .inbox
            .push(item(InboxKind::Approval, w[0], "cargo test"));
        app.world
            .inbox
            .push(item(InboxKind::Question, w[1], "Which?"));
        app.world.inbox.push(item(InboxKind::Reply, w[2], "done"));
        app.world
            .changes
            .push(change(w[3], vec![ran("cargo test", 0)]));
        app.world.inbox.push(item(InboxKind::Change, w[3], ""));
        let got = lines(&app, 100);
        // A reply asks for nothing: its own group, after the decisions.
        assert_eq!(
            got[0],
            " TO DECIDE 1 permission · 1 question · 1 change · 1 reply"
        );
        assert!(got[1].starts_with(" permissions"), "{}", got[1]);
        assert!(got[1].ends_with("one key each"), "{}", got[1]);
        assert_eq!(
            got[1].chars().count(),
            99,
            "the note a cell short of the edge"
        );
        assert_eq!(got[3], " questions");
        assert!(got[5].starts_with(" changes"), "{}", got[5]);
        assert!(got[5].ends_with("1 ready to approve"), "{}", got[5]);
        assert_eq!(got[7], " replies");
        assert!(got[8].contains("answered: done"), "{}", got[8]);
        // A change that was not proved clean is not ready: no note.
        app.world.changes[0].runs.clear();
        let got = lines(&app, 100);
        assert_eq!(got[5], " changes");
        // The note needs a cell between it and the word, and one at the
        // end: 12 + 1 + 12 + 1 + 1 columns.
        assert!(
            lines(&app, 27)[1].ends_with(" one key each"),
            "{}",
            lines(&app, 27)[1]
        );
        assert_eq!(lines(&app, 26)[1], " permissions");
        // What was put off stands last, in its own group, counted.
        app.deferred.push(0);
        let got = lines(&app, 100);
        assert_eq!(
            got[0],
            " TO DECIDE 1 question · 1 change · 1 reply · 1 put off"
        );
        assert_eq!(got[1], " questions");
        assert_eq!(got[5], " replies");
        assert_eq!(got[7], " later");
        assert!(got[8].contains("run cargo test?"), "{}", got[8]);
        app.deferred.clear();
        // One of a kind, many of another, in Turkish.
        app.lang = Lang::Tr;
        let got = lines(&app, 100);
        assert_eq!(
            got[0],
            " KARAR BEKLEYEN 1 izin · 1 soru · 1 değişiklik · 1 yanıt"
        );
        assert_eq!(got[7], " yanıtlar");
        assert!(got[1].ends_with("her biri tek tuş"), "{}", got[1]);
        app.deferred.push(0);
        let got = lines(&app, 100);
        assert!(got[0].ends_with("· 1 ertelenen"), "{}", got[0]);
        assert_eq!(got[7], " sonraya");
    }

    #[test]
    fn at_work_counts_who_is_stuck_and_idle_only_when_some_are() {
        let mut app = home(Lang::En);
        let w = workers(&app);
        app.world.agents[w[0]].status = Status::Working;
        app.world.agents[w[1]].status = Status::Working;
        let got = lines(&app, 100);
        assert_eq!(got[0], " AT WORK 4 · idle 2");
        app.world.agents[w[2]].status = Status::Blocked;
        assert_eq!(lines(&app, 100)[0], " AT WORK 4 · stuck 1 · idle 1");
        for &a in &w[2..] {
            app.world.agents[a].status = Status::Working;
        }
        assert_eq!(lines(&app, 100)[0], " AT WORK 4");
        app.lang = Lang::Tr;
        app.world.agents[w[2]].status = Status::Stopped;
        assert_eq!(lines(&app, 100)[0], " ÇALIŞAN 4 · takılan 1");
    }

    #[test]
    fn a_permission_row_says_the_whole_ask_and_a_check_s_row_its_commands() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.world
            .inbox
            .push(item(InboxKind::Approval, a, "Bash: cargo test\nmore"));
        let got = lines(&app, 100);
        assert!(
            got[2].contains("run Bash: cargo test ↵ more?"),
            "{}",
            got[2]
        );
        app.world.inbox[0] = item(InboxKind::Script, a, ".imece/worktree-setup.sh");
        let got = lines(&app, 100);
        // The row at hand ends with its keys; the ask goes on under it.
        let whole: Vec<String> = got[2..4]
            .iter()
            .map(|l| l.replace("a  yes   r  no", ""))
            .flat_map(|l| l.split_whitespace().map(str::to_string).collect::<Vec<_>>())
            .collect();
        assert!(
            whole
                .join(" ")
                .contains("run .imece/worktree-setup.sh in its new worktrees?"),
            "{got:?}"
        );
        let (mut app, i) = demo_with_check();
        let got = lines(&app, 120);
        let at = got
            .iter()
            .position(|l| l.contains("run .imece/check on its changes?"))
            .expect("the check's row");
        // With the row's keys at its end, the commands go on under it.
        let row = got[at..at + 2]
            .iter()
            .map(|l| l.replace("a  yes   r  no", "").trim().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            row.contains("set -e; cd \"$IMECE_WORKTREE\"; npm run build; npm test -- --silent"),
            "{row}"
        );
        assert!(
            !row.contains("#!/bin/sh") && !row.contains("Builds"),
            "{row}"
        );
        // The card the change was made for, not the one its worker has now.
        let card = app.world.inbox[i].card.expect("about a card");
        assert!(row.contains(&format!("#{}", card + 1)), "{row}");
        app.lang = Lang::Tr;
        let got = lines(&app, 120);
        assert!(
            got.iter()
                .any(|l| l.contains("değişikliklerinde çalışsın mı?")),
            "{got:?}"
        );
    }

    #[test]
    fn commands_of_a_script_leave_out_its_comments_and_blanks() {
        let script = "#!/bin/sh\n# says\n\n  set -e  \ncargo build\n\ncargo test\n";
        assert_eq!(commands_of(script), "set -e; cargo build; cargo test");
        assert_eq!(commands_of("# only\n"), "");
        assert!(!commands_of("echo \u{7}x").contains('\u{7}'), "cleaned");
    }

    #[test]
    fn a_row_not_at_hand_gets_the_whole_first_line_but_its_last_cell() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.world
            .inbox
            .push(item(InboxKind::Question, a, "first, at hand"));
        app.world.inbox.push(item(InboxKind::Question, a, "QQQ"));
        let got = lines(&app, 80);
        let row = got.iter().position(|l| l.contains("QQQ")).unwrap();
        let column = got[row].find("QQQ").unwrap();
        let c = got[row][..column].chars().count();
        let room = 80 - c - 1;
        let i = app.world.inbox.len() - 1;
        // Words that fill the room to its last cell stay on one line.
        let fits = format!("{} b", "a".repeat(room - 2));
        app.world.inbox[i].detail = fits.clone();
        let got = lines(&app, 80);
        assert!(got[row].trim_end().ends_with(&fits), "{}", got[row]);
        assert_eq!(got[row].trim_end().chars().count(), 79);
        assert_ne!(got[row + 1].trim(), "b", "{}", got[row + 1]);
        // One cell more and the last word goes under.
        app.world.inbox[i].detail = format!("{} b", "a".repeat(room - 1));
        let got = lines(&app, 80);
        assert!(!got[row].contains(" b"), "{}", got[row]);
        assert_eq!(got[row + 1].trim(), "b", "{}", got[row + 1]);
        let next: Vec<char> = got[row + 1].chars().collect();
        assert_eq!(next[c], 'b', "under its own column: {}", got[row + 1]);
    }

    #[test]
    fn a_question_s_row_lists_the_answers_it_offers_under_it_with_their_keys() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        let ask = |app: &mut App, text: &str, options: &[&str]| {
            app.world.note(Event::ToYou {
                from: a,
                kind: MsgKind::Ask,
                text: text.into(),
                options: options.iter().map(|o| o.to_string()).collect(),
            });
            app.world.asked_you(a, text);
        };
        ask(
            &mut app,
            "Which card?",
            &["the card", "store \u{1b}[31mcredit"],
        );
        let got = lines(&app, 120);
        let row = got.iter().position(|l| l.contains("Which card?")).unwrap();
        assert!(
            got[row].contains(" 1-2 ") && got[row].contains("pick"),
            "{}",
            got[row]
        );
        assert!(
            got[row].contains(" a ") && got[row].contains("answer"),
            "{}",
            got[row]
        );
        assert_eq!(got[row + 1].trim(), "1  the card");
        assert_eq!(got[row + 2].trim(), "2  store  [31mcredit", "cleaned");
        let column = got[row].find("Which").unwrap();
        let cells = got[row][..column].chars().count();
        let next: Vec<char> = got[row + 1].chars().collect();
        assert!(next[..cells].iter().all(|&c| c == ' '), "{}", got[row + 1]);
        assert_eq!(next[cells], ' ', "the key's badge starts the column");
        assert_eq!(next[cells + 1], '1', "{}", got[row + 1]);
        // A long answer is cut to the row.
        ask(&mut app, "Which card?", &[&"x".repeat(70)]);
        let got = lines(&app, 100);
        let row = got.iter().position(|l| l.contains("Which card?")).unwrap();
        assert!(
            got[row].contains(" 1 ") && !got[row].contains("1-"),
            "{}",
            got[row]
        );
        // Cut to the row's last cell but one, as the words are.
        assert_eq!(
            got[row + 1].trim_end().chars().count(),
            99,
            "{}",
            got[row + 1]
        );
        assert!(
            !got[row + 1].contains(&"x".repeat(70)),
            "cut: {}",
            got[row + 1]
        );
        assert!(
            got[row + 1].trim().starts_with("1  xxx"),
            "{}",
            got[row + 1]
        );
        // Without answers to pick: the keys as before, nothing under.
        ask(&mut app, "Which card?", &[]);
        let got = lines(&app, 120);
        let row = got.iter().position(|l| l.contains("Which card?")).unwrap();
        assert!(
            !got[row].contains("pick") && got[row].contains("open"),
            "{}",
            got[row]
        );
        assert!(!got[row + 1].trim().starts_with('1'), "{}", got[row + 1]);
        // Turkish keys.
        let mut app = home(Lang::Tr);
        ask(&mut app, "Hangi kart?", &["kart", "kredi"]);
        let got = lines(&app, 120);
        let row = got.iter().position(|l| l.contains("Hangi kart?")).unwrap();
        assert!(got[row].contains("seç"), "{}", got[row]);
    }

    #[test]
    fn the_preview_cuts_an_answer_to_its_room_and_numbers_them_from_one() {
        let t = Lang::En.text();
        let long = "x".repeat(70);
        let q = Question {
            text: "Which?",
            options: &[long.clone(), "b".into()],
        };
        let mut got = Vec::new();
        question_lines(t, &q, &mut got, 30);
        let texts: Vec<String> = got.iter().map(text).collect();
        assert_eq!(texts[0], "", "no paragraph for one line");
        assert_eq!(texts[1].chars().count(), 30, "{}", texts[1]);
        assert!(texts[1].starts_with(" 1  xxx"), "{}", texts[1]);
        assert_eq!(texts[2], " 2  b");
        assert_eq!(texts[3], "");
        assert_eq!(texts[4], "so far:");
        // A question whole, line by line, cleaned, before the answers.
        let q = Question {
            text: "Which?\nThe \u{1b}[31mred one?\n\n",
            options: &[],
        };
        let mut got = Vec::new();
        question_lines(t, &q, &mut got, 30);
        let texts: Vec<String> = got.iter().map(text).collect();
        assert_eq!(texts[..4], ["", "Which?", "The [31mred one?", ""]);
        assert_eq!(texts[4], "so far:");
    }

    #[test]
    fn the_preview_repeats_an_item_s_words_only_when_its_line_lacks_them() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        let preview = |app: &App| -> Vec<String> {
            let s = render(app, 150, 40);
            (s.lines())
                .filter_map(|l| l.split_once('│').map(|(_, right)| right.trim().to_string()))
                .collect()
        };
        // The leader's plan: its line counts cards and dollars, so the
        // words stand on their own under it.
        app.world
            .inbox
            .push(item(InboxKind::Plan, a, "Three cards, the parser first"));
        let said = words::inbox(Lang::En, &app.world, &app.world.inbox[0]);
        assert!(!said.contains("Three cards"), "{said}");
        let right = preview(&app);
        assert!(right.iter().any(|l| l == &said), "{right:?}");
        assert!(
            right.iter().any(|l| l == "Three cards, the parser first"),
            "{right:?}"
        );
        // An ask: its line says the command already, once.
        app.world.inbox.clear();
        app.world
            .inbox
            .push(item(InboxKind::Approval, a, "Bash: npm test"));
        let said = words::inbox(Lang::En, &app.world, &app.world.inbox[0]);
        assert!(said.contains("npm test"), "{said}");
        let right = preview(&app);
        assert_eq!(
            right.iter().filter(|l| l.contains("npm test")).count(),
            1,
            "{right:?}"
        );
        // Nothing to say: nothing under the line.
        app.world.inbox.clear();
        app.world.inbox.push(item(InboxKind::Approval, a, ""));
        let right = preview(&app);
        let said = words::inbox(Lang::En, &app.world, &app.world.inbox[0]);
        let at = right.iter().position(|l| l == said.trim()).unwrap();
        assert!(right[at + 1..].iter().all(|l| l.is_empty()), "{right:?}");
    }

    #[test]
    fn a_question_s_preview_reads_it_whole_its_answers_and_what_the_agent_did() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.world.note(Event::ToYou {
            from: a,
            kind: MsgKind::Ask,
            text: "Which card?\nThe refund one.".into(),
            options: vec!["the card".into()],
        });
        app.world.asked_you(a, "Which card?");
        app.world.agents[a].trace(0, "tool_use Read Money.cs".to_string());
        let s = render(&app, 150, 40);
        assert!(s.contains("The refund one."), "{s}");
        assert!(s.contains(" 1  the card"), "{s}");
        assert!(s.contains("so far:"), "{s}");
        assert!(s.contains("Money.cs"), "{s}");
        // In the preview, past the list's own column.
        let order = |needle: &str| s.find(needle).unwrap();
        assert!(order("The refund one.") < order("│ 1  the card"));
        assert!(order("│ 1  the card") < order("so far:"));
        assert!(order("so far:") < order("Money.cs"));
        // One line only: the row has it, the preview does not say it twice.
        app.world.note(Event::ToYou {
            from: a,
            kind: MsgKind::Ask,
            text: "Only this?".into(),
            options: Vec::new(),
        });
        app.world.asked_you(a, "Only this?");
        let s = render(&app, 150, 40);
        assert!(
            !s.lines()
                .any(|l| l.trim_start_matches(['│', ' ']).trim_end() == "Only this?"),
            "not read out again as a paragraph: {s}"
        );
        assert!(s.contains("so far:"), "{s}");
        let tr = {
            let mut app = home(Lang::Tr);
            app.world.asked_you(a, "Hangi?");
            render(&app, 150, 40)
        };
        assert!(tr.contains("şimdiye dek:"), "{tr}");
    }

    #[test]
    fn a_question_is_read_whole_and_wraps_under_its_own_column() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        let long = "May paging change the shape of the API's response, or must it keep every field as it is today?";
        app.world.inbox.push(item(InboxKind::Question, a, long));
        let got = lines(&app, 80);
        let first = got.iter().position(|l| l.contains("May paging")).unwrap();
        let joined: String = got[first..first + 3]
            .iter()
            .map(|l| l.trim().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(joined.contains(long), "{joined}");
        // The second line starts where the ask does.
        let column = got[first].find("May").unwrap();
        let cells = got[first][..column].chars().count();
        let next: Vec<char> = got[first + 1].chars().collect();
        assert!(
            next[..cells].iter().all(|&c| c == ' '),
            "{}",
            got[first + 1]
        );
        assert_ne!(next[cells], ' ', "{}", got[first + 1]);
        assert!(got[first + 1].chars().count() <= 80);
        let s = render(&app, 80, 24);
        assert!(s.contains("May paging change"), "{s}");
        assert!(
            !s.lines().skip(1).any(|l| l.contains('│')),
            "one column: {s}"
        );
    }

    #[test]
    fn an_issue_to_close_and_a_change_that_closes_one_say_its_number() {
        let mut app = home(Lang::En);
        assert!(!app.world.cards.is_empty(), "the demo has cards");
        let lead = app.world.projects[app.world.cards[0].project].leader;
        let project = app.world.cards[0].project;
        app.world.cards[0].closes = Some(58);
        let mut ask = item(InboxKind::Close, lead, "the card");
        ask.card = Some(0);
        app.world.inbox.push(ask);
        let all = |app: &App| lines(app, 160).join("\n");
        assert!(
            all(&app).contains("close #58? · card #1 landed"),
            "{}",
            all(&app)
        );
        app.world
            .items
            .insert((project, 58), Outside::Open("A 529 stops it".into()));
        assert!(
            all(&app).contains("close #58? “A 529 stops it” · card #1 landed"),
            "{}",
            all(&app)
        );
        assert_eq!(HomeGroup::of(InboxKind::Close), HomeGroup::Question);
        app.lang = Lang::Tr;
        assert!(
            all(&app).contains("#58 kapatılsın mı? “A 529 stops it” · #1 kartı girdi"),
            "{}",
            all(&app)
        );
        // A change whose card closes an issue ends with it; another's not.
        app.lang = Lang::En;
        app.world.inbox.clear();
        let w = worker(&app);
        app.world.agents[w].card = Some(0);
        let change = |agent| item(InboxKind::Change, agent, "Fix it");
        app.world.inbox.push(change(w));
        app.world.changes.push(crate::ui::tests::a_change(w));
        let row = all(&app);
        assert!(row.contains(" · closes #58"), "{row}");
        app.world.cards[0].closes = None;
        assert!(!all(&app).contains("closes #"), "{}", all(&app));
    }

    #[test]
    fn every_other_kind_of_item_says_what_it_is() {
        let mut app = home(Lang::En);
        let w = workers(&app);
        let researcher = app.world.projects[0].leader;
        let mut report = item(InboxKind::Research, w[0], "serde");
        report.researcher = Some(researcher);
        app.world.inbox.push(report);
        app.world
            .inbox
            .push(item(InboxKind::Reply, w[1], "yes it does"));
        app.world.inbox.push(item(InboxKind::Fault, w[2], "401"));
        app.world
            .inbox
            .push(item(InboxKind::Sensitive, w[3], "adds a crate"));
        let lead = app.world.projects[1].leader;
        app.world
            .inbox
            .push(item(InboxKind::Plan, lead, "First card"));
        let got = lines(&app, 140).join("\n");
        let by = app.world.agents[researcher].name.clone();
        for want in [
            format!("{by}'s report proposes a new package: serde"),
            "answered: yes it does".into(),
            "API error, write to it once mended: 401".into(),
            "sensitive change: adds a crate".into(),
            "plans 0 cards for $0.00: First card".into(),
        ] {
            assert!(got.contains(&want), "{want}:\n{got}");
        }
        assert!(
            got.contains("TO DECIDE 3 questions · 1 change · 1 reply"),
            "{got}"
        );
    }

    #[test]
    fn a_worker_s_row_is_called_by_its_card_else_the_person_s_line_else_its_name() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.world.agents[a].card = None;
        let name = app.world.agents[a].name.clone();
        assert_eq!(title_of(&app.world, HomeRow::Agent(a)), name);
        // No card for the person's line until a change shows (record
        // 126): the row says the line.
        app.world.agents[a].task.own = Some(crate::model::Own {
            title: "fix the footer\u{1b}[2J".into(),
            ..crate::model::Own::default()
        });
        assert_eq!(
            title_of(&app.world, HomeRow::Agent(a)),
            "fix the footer [2J",
            "cleaned"
        );
        app.world.agents[a].card = Some(0);
        let card = app.world.cards[0].title.clone();
        assert_eq!(title_of(&app.world, HomeRow::Agent(a)), card);
    }

    #[test]
    fn a_change_s_row_is_its_verdict_without_the_head_the_lines_with_the_files() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.world.agents[a].card = None;
        let said = |app: &App, lang: Lang| -> String {
            let c = app.world.change_of(a).unwrap();
            let v = app.world.verdict(a).unwrap();
            (change_spans(lang, c, &v).iter())
                .map(|s| s.content.as_ref())
                .collect()
        };
        app.world.changes.push(change(a, Vec::new()));
        assert_eq!(said(&app, Lang::En), "nothing ran · 1 file +19 −6");
        app.world.changes[0].runs = vec![ran("dotnet build", 0), ran("dotnet test", 0)];
        assert_eq!(
            said(&app, Lang::En),
            "✓ dotnet build · ✓ dotnet test · 1 file +19 −6"
        );
        app.world.changes[0].runs.push(ran("lint", 1));
        assert_eq!(
            said(&app, Lang::En),
            "✓ dotnet build · ✓ dotnet test · ✗ lint: exit 1 · 1 file +19 −6"
        );
        let v = app.world.verdict(a).unwrap();
        let spans = change_spans(Lang::En, app.world.change_of(a).unwrap(), &v);
        assert_eq!(spans[4].style, fg(pal().bad));
        // The repository's check comes first.
        app.world.changes[0].check = Check::Ran(Checked {
            ok: true,
            end: "exit 0".into(),
            lines: Vec::new(),
        });
        app.world.changes[0].runs.truncate(1);
        assert_eq!(
            said(&app, Lang::En),
            "✓ check · ✓ dotnet build · 1 file +19 −6"
        );
        // The card's paths: inside, else what is outside, by name.
        let mut card = app.world.cards[0].clone();
        card.id = app.world.cards.len();
        card.paths = crate::paths::parse(&serde_json::json!(["src/"])).unwrap();
        app.world.cards.push(card);
        let n = app.world.cards.len();
        app.world.agents[a].card = Some(n - 1);
        assert_eq!(
            said(&app, Lang::En),
            format!("✓ check · ✓ dotnet build · 1 file +19 −6, inside #{n}")
        );
        app.world.changes[0].files.push(ChangedFile {
            path: "tests/paging.rs".into(),
            added: 2,
            removed: 0,
            binary: false,
        });
        assert_eq!(
            said(&app, Lang::En),
            format!("✓ check · ✓ dotnet build · 2 files +21 −6 · 1 outside #{n}: tests/paging.rs")
        );
        assert_eq!(
            said(&app, Lang::Tr),
            format!(
                "✓ kontrol · ✓ dotnet build · 2 dosya +21 −6 · 1 kartın dışında (#{n}): tests/paging.rs"
            )
        );
        app.world.cards[n - 1].paths.clear();
        assert_eq!(
            said(&app, Lang::En),
            "✓ check · ✓ dotnet build · 2 files +21 −6",
            "a card without paths bounds nothing"
        );
    }

    #[test]
    fn a_part_of_a_change_s_row_that_does_not_fit_goes_under_without_its_dot() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.world.agents[a].card = None;
        app.world.changes.push(change(
            a,
            vec![ran("dotnet build", 0), ran("dotnet test", 0)],
        ));
        // A question comes first and is the row at hand: the change's row
        // has no keys at its end.
        app.world.inbox.push(item(InboxKind::Question, a, "Which?"));
        app.world.inbox.push(item(InboxKind::Change, a, ""));
        app.at_home.scope = Some(app.world.agents[a].project);
        let wide = lines(&app, 100);
        let row = wide.iter().position(|l| l.contains("+19 −6")).unwrap();
        assert!(wide[row].contains("✓ dotnet build · ✓ dotnet test · 1 file +19 −6"));
        let narrow = beside(&app, 66);
        assert!(
            narrow[row].ends_with("✓ dotnet build · ✓ dotnet test"),
            "{}",
            narrow[row]
        );
        assert_eq!(narrow[row + 1].trim(), "1 file +19 −6");
        let column = narrow[row].find("✓ dotnet").unwrap();
        let cells = narrow[row][..column].chars().count();
        let next: Vec<char> = narrow[row + 1].chars().collect();
        assert!(
            next[..cells].iter().all(|&c| c == ' '),
            "{}",
            narrow[row + 1]
        );
        assert_eq!(next[cells], '1', "{}", narrow[row + 1]);
        // A part wider than a whole line is cut, never dropped.
        let tiny = beside(&app, 50);
        assert!(
            tiny[row + 1].trim().starts_with("✓ dotnet test"),
            "{}",
            tiny[row + 1]
        );
        assert!(tiny[row + 1].chars().count() <= 50);
    }

    #[test]
    fn the_row_at_hand_ends_with_its_keys_unless_they_would_crowd_the_ask() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.world
            .inbox
            .push(item(InboxKind::Approval, a, "cargo test"));
        app.world.inbox.push(item(InboxKind::Question, a, "Which?"));
        app.home_settle();
        let got = lines(&app, 100);
        let row = &got[2];
        assert!(row.starts_with("▸!"), "{row}");
        assert!(row.contains("run cargo test?"), "{row}");
        assert!(row.ends_with(" a  yes   r  no  "), "{row}");
        assert_eq!(row.chars().count(), 99, "a cell short of the edge: {row}");
        assert!(!row.contains("help") && !row.contains("turn"), "{row}");
        assert!(!got[4].contains("answer"), "not at hand: {}", got[4]);
        // The keys dim, their badges as the status line's.
        let line = &list_lines(&app, 100, 40, app.home_at()).0[2];
        let badge = line.spans.iter().find(|s| s.content == " a ").unwrap();
        assert_eq!(badge.style, key_style());
        let word = line
            .spans
            .iter()
            .find(|s| s.content.contains("yes"))
            .unwrap();
        assert_eq!(word.style, dim());
        // Each kind its two words.
        app.home_go(1);
        assert!(lines(&app, 100)[4].ends_with(" a  answer   enter  open  "));
        app.world.changes.push(change(a, Vec::new()));
        app.world.inbox.push(item(InboxKind::Change, a, ""));
        app.home_go(2);
        assert!(lines(&app, 100)[6].ends_with(" a  approve   r  send back  "));
        app.world.agents[a].status = Status::Working;
        // Row 3 is the project's heading (two projects are open): the
        // team's project line, no keys at its end; row 4 the worker's.
        app.home_go(3);
        let heading = &lines(&app, 100)[9];
        assert!(heading.starts_with("▸▾ shop-api"), "{heading}");
        assert!(heading.trim_end().ends_with("~/src/shop-api"), "{heading}");
        app.home_go(4);
        assert!(
            lines(&app, 100)[10].ends_with(" enter  open  "),
            "{}",
            lines(&app, 100)[10]
        );
        // Too narrow for the ask beside them: the keys go, the status line
        // still has them.
        app.home_go(0);
        let narrow = beside(&app, 62);
        assert!(!narrow[2].contains("yes"), "{}", narrow[2]);
        assert!(narrow[2].contains("run cargo test?"), "{}", narrow[2]);
        assert!(render(&app, 62, 20).contains("allow once"));
    }

    #[test]
    fn the_keys_stay_while_the_ask_keeps_its_least_cells_and_each_kind_has_its_two() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.at_home.scope = Some(app.world.agents[a].project);
        app.world.inbox.push(item(InboxKind::Fault, a, "401"));
        app.home_settle();
        // The head is 34 cells: the ask keeps `ASK_MIN` at 34 + the keys
        // + 2 + ASK_MIN columns, one less drops the keys.
        let head = 34;
        let keys = " a  answer   enter  open  ";
        let t = Lang::En.text();
        let keys_width: usize = hint_spans(&[("a", t.home_answer), ("enter", t.home_open)])
            .iter()
            .map(Span::width)
            .sum();
        let at = |app: &App, width: usize| lines(app, width)[2].clone();
        let exact = head + keys_width + 2 + ASK_MIN;
        assert!(at(&app, exact).ends_with(keys), "{}", at(&app, exact));
        assert!(
            !at(&app, exact - 1).contains("answer"),
            "{}",
            at(&app, exact - 1)
        );
        assert!(
            at(&app, exact - 1).contains("API error"),
            "{}",
            at(&app, exact - 1)
        );
        for (kind, want) in [
            (InboxKind::Reply, " enter  open   r  dismiss  "),
            (InboxKind::Plan, " a  yes   r  no  "),
            (InboxKind::Sensitive, " a  yes   r  no  "),
            (InboxKind::Script, " a  yes   r  no  "),
            (InboxKind::Tries, " a  one more   r  fail it  "),
            (InboxKind::Rounds, " a  one more   r  fail it  "),
            (InboxKind::Close, " a  close   r  leave open  "),
        ] {
            app.world.inbox[0].kind = kind;
            assert!(at(&app, 120).ends_with(want), "{kind:?}: {}", at(&app, 120));
        }
        // A card at its breaker says what it counts (record 126, F9).
        assert!(!app.world.cards.is_empty(), "the demo has cards");
        app.world.cards[0].attempt = 3;
        app.world.cards[0].breaker.sent_back = 2;
        app.world.inbox[0].card = Some(0);
        let all = |app: &App| lines(app, 160).join("\n");
        app.world.inbox[0].kind = InboxKind::Tries;
        assert!(
            all(&app).contains("failed 3 tries: one more, or it fails for good"),
            "{}",
            all(&app)
        );
        app.world.inbox[0].kind = InboxKind::Rounds;
        assert!(
            all(&app).contains("its change came back 2 times: one more round, or it fails"),
            "{}",
            all(&app)
        );
    }

    #[test]
    fn the_list_scrolls_only_once_the_row_at_hand_would_leave_the_screen() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        for n in 0..12 {
            app.world
                .inbox
                .push(item(InboxKind::Question, a, &format!("q{n}")));
        }
        // TO DECIDE, questions, 12 rows, a blank, AT WORK and its rows.
        let total = list_lines(&app, 100, 40, None).0.len();
        let scroll = |app: &App, height| list_lines(app, 100, height, app.home_at()).1;
        assert_eq!(total, 21);
        // All of it fits: never scrolled, the headings stay.
        app.home_go(11);
        assert_eq!(scroll(&app, total as u16), 0);
        assert_eq!(scroll(&app, 40), 0);
        // Row 11 is line 13: in 15 rows it shows with a line under it.
        assert_eq!(scroll(&app, 15), 0);
        // In 14 it would be the last: it scrolls, as far as the list's
        // end lets it, else to stand four lines from the top.
        assert_eq!(scroll(&app, 14), 7, "the end at the foot");
        assert_eq!(scroll(&app, 10), 9, "four lines above it");
        // The first rows on a short screen: the heading stays.
        app.home_go(3);
        assert_eq!(scroll(&app, 7), 0, "row 3 is line 5");
        assert_eq!(scroll(&app, 6), 1);
        app.home_go(0);
        assert_eq!(scroll(&app, 4), 0);
        assert_eq!(scroll(&app, 3), 0);
        // Nothing at hand: nothing scrolls.
        assert_eq!(list_lines(&app, 100, 3, None).1, 0);
    }

    #[test]
    fn a_row_taller_than_the_lines_above_it_ends_in_view() {
        let drawn = Drawn {
            lines: vec![Line::default(); 30],
            at: Some((10, 19)),
            spots: Vec::new(),
        };
        // Ten lines from 10 to 19 in 12 rows: from line 8, its end shows.
        assert_eq!(drawn.scroll(12), 8);
        // Taller than the screen: its first line stays at the top.
        assert_eq!(drawn.scroll(5), 10);
        assert_eq!(drawn.scroll(20), 6);
        assert_eq!(drawn.scroll(30), 0);
    }

    #[test]
    fn an_agent_s_row_is_one_line_cut_with_how_long_and_its_glyph_said_once() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.at_home.scope = Some(app.world.agents[a].project);
        app.world.agents[a].status = Status::Working;
        app.world.agents[a].plan = vec![Step {
            text: "Write the tests for the paging of every product list there is".into(),
            state: StepState::Doing,
        }];
        app.world.tick = 36;
        app.world.agents[a].changed_at = 0;
        let got = beside(&app, 70);
        let row = got.iter().find(|l| l.contains("● ")).unwrap();
        assert!(row.contains("Write the tests for"), "{row}");
        assert!(row.contains("…  enter  open"), "cut, then the keys: {row}");
        assert!(row.chars().count() <= 70);
        assert!(
            !got.iter().any(|l| l.trim().starts_with("every")),
            "{got:?}"
        );
        app.world.agents[a].plan.clear();
        app.world.agents[a].status = Status::Stopped;
        let got = lines(&app, 100);
        let row = got.iter().find(|l| l.contains("■ ")).unwrap();
        assert_eq!(row.matches('■').count(), 1, "{row}");
        assert!(row.contains("session ended"), "{row}");
        assert!(row.contains("· 3 min"), "{row}");
        app.world.tick = 11;
        let got = lines(&app, 100);
        assert!(
            !got.iter().any(|l| l.contains(" min")),
            "under a minute: {got:?}"
        );
        app.world.tick = 12;
        let got = lines(&app, 100);
        assert!(
            got.iter().any(|l| l.contains("· 1 min")),
            "from the minute: {got:?}"
        );
        // An idle one says so.
        app.world.agents[a].status = Status::Idle;
        let got = lines(&app, 100);
        let row = got.iter().find(|l| l.contains("○ ")).unwrap();
        assert!(row.contains("  idle · no work yet · 1 min"), "{row}");
    }

    /// Agent `a`'s newest row in the ledger: a try at card 0 that ended
    /// with `outcome`.
    fn ended(app: &mut App, a: AgentId, outcome: crate::ledger::Outcome) {
        app.world.agents[a].card = Some(0);
        app.world.ledger.push(crate::ledger::Attempt {
            agent: a,
            card: Some(0),
            attempt: 1,
            outcome,
            ..Default::default()
        });
    }

    #[test]
    fn an_idle_worker_s_row_says_what_became_of_its_last_work() {
        use crate::ledger::Outcome;
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.at_home.scope = Some(app.world.agents[a].project);
        app.world.cards[0].title = "Paging".into();
        let spans = |app: &App| row_lines(app, HomeRow::Agent(a), 100, false, false)[0].clone();
        let row = |app: &App| text(&spans(app));
        let colour = |app: &App, words: &str| {
            let line = spans(app);
            let span = line.spans.iter().find(|s| s.content == words);
            span.unwrap_or_else(|| panic!("{words}: {line:?}")).style
        };
        // Never given anything.
        app.world.agents[a].card = None;
        assert!(row(&app).ends_with("idle · no work yet"), "{}", row(&app));
        assert_eq!(colour(&app, "no work yet"), dim());
        // A card in hand, nothing ended: only idle.
        app.world.agents[a].card = Some(0);
        assert!(row(&app).ends_with("  idle"), "{}", row(&app));
        ended(&mut app, a, Outcome::Open);
        assert!(row(&app).ends_with("  idle"), "{}", row(&app));
        for (outcome, said, style) in [
            (Outcome::Applied, "finished, applied", fg(pal().ok)),
            (Outcome::Done, "finished", fg(pal().ok)),
            (Outcome::Failed, "#1 failed", fg(pal().bad)),
            (Outcome::Withdrawn, "#1 withdrawn", dim()),
        ] {
            ended(&mut app, a, outcome);
            let got = row(&app);
            assert!(got.ends_with(&format!("  idle · {said}")), "{got}");
            assert_eq!(colour(&app, said), style, "{said}");
        }
        // Handed on, or late: nothing is said.
        for outcome in [Outcome::HandedOn, Outcome::Late] {
            ended(&mut app, a, outcome);
            assert!(row(&app).ends_with("  idle"), "{}", row(&app));
        }
        // How long comes last.
        ended(&mut app, a, Outcome::Applied);
        app.world.tick = 12;
        app.world.agents[a].changed_at = 0;
        let got = row(&app);
        assert!(got.ends_with("idle · finished, applied · 1 min"), "{got}");
        // At work again it says what it does, not what it finished.
        app.world.agents[a].status = Status::Working;
        assert!(!row(&app).contains("finished"), "{}", row(&app));
        app.world.agents[a].status = Status::Idle;
        // A helper's work is its worker's: nothing of its own is said.
        app.world.agents[a].role = Role::Helper;
        assert!(!row(&app).contains("finished"), "{}", row(&app));
        app.world.agents[a].role = Role::Worker;
        // A solo's is its own.
        app.world.agents[a].role = Role::Solo;
        assert!(row(&app).contains("finished, applied"), "{}", row(&app));
        app.world.agents[a].role = Role::Worker;
        app.lang = Lang::Tr;
        let got = row(&app);
        assert!(got.ends_with("boşta · bitirdi, uygulandı · 1 dk"), "{got}");
    }

    #[test]
    fn a_finished_row_too_narrow_loses_the_time_then_idle_then_is_cut() {
        use crate::ledger::Outcome;
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.at_home.scope = Some(app.world.agents[a].project);
        app.world.cards[0].title = "Paging".into();
        ended(&mut app, a, Outcome::Applied);
        app.world.tick = 12;
        app.world.agents[a].changed_at = 0;
        // What the row says after its title, at each width from wide to
        // narrow: each way once, in this order.
        let mut said: Vec<String> = Vec::new();
        for width in (30..=100).rev() {
            let got = text(&row_lines(&app, HomeRow::Agent(a), width, false, false)[0]);
            assert!(got.chars().count() <= width, "{width}: {got}");
            let tail = got.split("Paging").nth(1).unwrap().trim().to_string();
            if said.last() != Some(&tail) {
                said.push(tail);
            }
        }
        assert!(said.len() > 3, "{said:?}");
        assert_eq!(
            said[..3],
            [
                "idle · finished, applied · 1 min",
                "idle · finished, applied",
                "finished, applied"
            ],
            "{said:?}"
        );
        assert!(
            said[3..]
                .iter()
                .all(|t| t.ends_with('…') && t.starts_with("finished")),
            "then cut: {said:?}"
        );
        assert!(said.len() > 3, "{said:?}");
        // The row at hand keeps its keys and still says it.
        app.at_home.at = Some(HomeRow::Agent(a));
        let got = text(&row_lines(&app, HomeRow::Agent(a), 100, false, true)[0]);
        assert!(got.contains("idle · finished, applied · 1 min"), "{got}");
        assert!(got.trim_end().ends_with("enter  open"), "{got}");
        // Narrow (record 131): on the title's line when it fits.
        let got = row_lines(&app, HomeRow::Agent(a), 60, true, false);
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(text(&got[0]).ends_with("Paging  idle · finished, applied · 1 min"));
    }

    #[test]
    fn at_work_counts_the_idle_who_finished_apart() {
        use crate::ledger::Outcome;
        let mut app = home(Lang::En);
        let w = workers(&app);
        app.world.agents[w[0]].status = Status::Working;
        app.world.agents[w[1]].status = Status::Working;
        ended(&mut app, w[2], Outcome::Applied);
        assert_eq!(lines(&app, 100)[0], " AT WORK 4 · finished 1 · idle 1");
        // A failed card is not finished work.
        ended(&mut app, w[3], Outcome::Failed);
        assert_eq!(lines(&app, 100)[0], " AT WORK 4 · finished 1 · idle 1");
        ended(&mut app, w[3], Outcome::Done);
        assert_eq!(lines(&app, 100)[0], " AT WORK 4 · finished 2");
        // One at work finished nothing yet, whatever its last row says.
        ended(&mut app, w[0], Outcome::Applied);
        assert_eq!(lines(&app, 100)[0], " AT WORK 4 · finished 2");
        app.lang = Lang::Tr;
        assert!(lines(&app, 100)[0].ends_with("4 · bitiren 2"));
    }

    #[test]
    fn an_agent_s_preview_follows_the_newest_of_its_trace() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.world.agents[a].status = Status::Working;
        app.world.agents[a].plan.clear();
        app.world.agents[a].trace.clear();
        for n in 0..40 {
            app.world.agents[a]
                .trace
                .push_back(format!("12:{n:02} tool_use Read src/file{n}.py"));
        }
        let preview = |app: &App, rows| -> Vec<String> {
            preview_lines(app, HomeRow::Agent(a), 60, rows)
                .iter()
                .map(text)
                .collect()
        };
        let got = preview(&app, 12);
        assert_eq!(got.len(), 12, "{got:?}");
        assert!(
            got[11].contains("src/file39.py"),
            "the newest last: {got:?}"
        );
        assert!(got.iter().any(|l| l.contains("src/file32.py")), "{got:?}");
        assert!(!got.iter().any(|l| l.contains("src/file31.py")), "{got:?}");
        assert!(!got.iter().any(|l| l.contains("src/file0.py")), "{got:?}");
        // An entry of several lines is kept whole, or left out whole: the
        // seven lines of the newest and the one before fill the eight.
        let words: Vec<String> = (0..7).map(|n| format!("- line {n}")).collect();
        let words = words.join("\\n");
        app.world.agents[a]
            .trace
            .push_back(format!("12:41 text \"{words}\""));
        let got = preview(&app, 12);
        assert!(got[4].contains("src/file39.py"), "{got:?}");
        assert!(!got.iter().any(|l| l.contains("src/file38.py")), "{got:?}");
        assert!(got[5].contains("12:41"), "{got:?}");
        assert!(got[11].trim_end().ends_with("line 6"), "{got:?}");
        // The newest alone taller than the room: its first lines.
        let got = preview(&app, 6);
        assert_eq!(got.len(), 6, "{got:?}");
        assert!(got[4].contains("12:41"), "{got:?}");
        assert!(got[5].trim_end().ends_with("line 1"), "{got:?}");
    }

    #[test]
    fn the_preview_stands_beside_the_list_from_120_columns() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.world.agents[a].card = Some(0);
        app.world
            .inbox
            .push(item(InboxKind::Question, a, "Which index?"));
        let wide = render(&app, WIDE, 30);
        let narrow = render(&app, WIDE - 1, 30);
        let rule = |s: &str| s.lines().skip(1).any(|l| l.contains('│'));
        assert!(rule(&wide), "{wide}");
        assert!(!rule(&narrow), "{narrow}");
        assert!(narrow.contains("Which index?"), "{narrow}");
        let name = app.world.agents[a].name.clone();
        assert!(wide.contains("asks you: Which index?"), "{wide}");
        assert!(wide.contains(&name), "the agent in the preview: {wide}");
        assert!(!narrow.contains(&name), "not on the row: {narrow}");
    }

    #[test]
    fn the_list_takes_under_half_and_never_less_than_its_narrowest() {
        assert_eq!(list_width(WIDE), LIST_MIN);
        assert_eq!(list_width(133), LIST_MIN);
        assert_eq!(list_width(134), 60);
        assert_eq!(list_width(150), 67);
        assert_eq!(list_width(200), 90);
    }

    #[test]
    fn the_preview_reads_a_script_s_lines_numbered_and_counts_the_rest() {
        let (app, _) = demo_with_check();
        let s = render(&app, 150, 40);
        assert!(s.contains("  1 #!/bin/sh"), "{s}");
        assert!(s.contains("  6 npm test -- --silent"), "{s}");
        let t = Lang::En.text();
        let long: String = (1..=SCRIPT_ROWS + 3)
            .map(|n| format!("echo {n}\n"))
            .collect();
        let got = script_lines(t, &long, 40);
        assert_eq!(got.len(), SCRIPT_ROWS + 1);
        assert_eq!(
            text(&got[SCRIPT_ROWS - 1]),
            format!(" {SCRIPT_ROWS} echo {SCRIPT_ROWS}")
        );
        assert_eq!(text(&got[SCRIPT_ROWS]), "… 3 more");
        let short = script_lines(t, "a\nb\n", 40);
        assert_eq!(short.len(), 2);
        assert_eq!(text(&short[1]), "  2 b");
    }

    #[test]
    fn a_change_s_preview_says_what_it_comes_to_and_its_patch_without_git_s_lines() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.world.changes.push(change(a, Vec::new()));
        app.world.inbox.push(item(InboxKind::Change, a, ""));
        let s = render(&app, 150, 40);
        assert!(s.contains("Paging in product search"), "{s}");
        assert!(s.contains("src/query.rs"), "{s}");
        assert!(s.contains("+new line") && s.contains("-old line"), "{s}");
        assert!(!s.contains("diff --git") && !s.contains("+++ b/x"), "{s}");
        assert!(!s.contains("index 1..2") && !s.contains("--- a/x"), "{s}");
        // What the worker said of it is read as the Markdown it is.
        app.world.journal.push(Entry {
            tick: app.world.tick,
            event: Event::Sent {
                from: a,
                to: app.world.projects[app.world.agents[a].project].leader,
                kind: MsgKind::Done,
                text: "Paging\n- **Added:** a page size\n- **Changed:** the query".into(),
                id: 9,
                re: None,
            },
        });
        let preview = preview_lines(&app, HomeRow::Item(0), 60, 40);
        let preview: Vec<String> = preview.iter().map(text).collect();
        let at = (preview.iter())
            .position(|l| l.starts_with("why"))
            .expect("the why");
        assert_eq!(preview[at].trim_end(), "why      • Added: a page size");
        assert_eq!(preview[at + 1].trim_end(), "         • Changed: the query");
    }

    #[test]
    fn the_patch_keeps_its_own_lines_and_only_as_many_as_there_is_room_for() {
        let patch = "diff --git a/x b/x\nindex 1\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n x\n-a\n+b\n";
        let got = |rows| {
            patch_lines(patch, 40, rows)
                .iter()
                .map(|l| l.spans[0].content.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(got(10), ["@@ -1 +1 @@", " x", "-a", "+b"]);
        assert_eq!(got(2), ["@@ -1 +1 @@", " x"]);
    }

    #[test]
    fn the_verdict_heads_the_preview_and_wraps_under_its_label() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.world.agents[a].card = None;
        app.world
            .changes
            .push(change(a, vec![ran("cargo test", 0)]));
        let c = app.world.change_of(a).unwrap();
        let mut got = Vec::new();
        change_lines(Lang::En, &app.world, c, &mut got, 80);
        assert_eq!(
            text(&got[0]),
            "verdict  ✓ ready to approve · ✓ cargo test · 1 file"
        );
        assert!(text(&got[1]).starts_with("why      "), "{}", text(&got[1]));
        let mut narrow = Vec::new();
        change_lines(Lang::En, &app.world, c, &mut narrow, 30);
        assert!(
            text(&narrow[0]).starts_with("verdict  ✓ ready"),
            "{}",
            text(&narrow[0])
        );
        assert!(
            text(&narrow[1]).starts_with("         "),
            "{}",
            text(&narrow[1])
        );
        assert!(
            text(&narrow[1]).contains("cargo test"),
            "{}",
            text(&narrow[1])
        );
        let mut tr = Vec::new();
        change_lines(Lang::Tr, &app.world, c, &mut tr, 80);
        assert!(
            text(&tr[0]).starts_with("hüküm    ✓ onaya hazır"),
            "{}",
            text(&tr[0])
        );
    }

    /// Two projects on Home, the first worker of project 0 with a helper
    /// under it.
    fn team() -> (App, AgentId, AgentId) {
        let mut app = home(Lang::En);
        let worker = (app.world.agents.iter())
            .find(|a| a.role == Role::Worker && a.project == 0)
            .map(|a| a.id)
            .unwrap();
        let n = app.world.agents.len();
        let mut helper = app.world.agents[worker].clone();
        helper.id = n;
        helper.name = "claude-9.1".into();
        helper.role = Role::Helper;
        helper.parent = Some(worker);
        helper.status = Status::Working;
        app.world.agents.push(helper);
        (app, worker, n)
    }

    #[test]
    fn the_team_stands_under_at_work_by_project_with_its_helpers_hung_under() {
        let (mut app, worker, helper) = team();
        app.home_go(0);
        assert_eq!(app.home_at(), Some(HomeRow::Project(0)));
        let got = lines(&app, 120);
        let name = clean(&app.world.projects[0].name);
        // The heading is the team's project line, the cursor before it,
        // no keys at its end; the other project's heading unmarked.
        assert!(got[1].starts_with(&format!("▸▾ {name}")), "{}", got[1]);
        assert!(
            got[1].contains("working") && got[1].contains("$0.00"),
            "{}",
            got[1]
        );
        assert!(!got[1].contains("open"), "{}", got[1]);
        let other = clean(&app.world.projects[1].name);
        assert!(
            got.iter().any(|l| l.starts_with(&format!(" ▾ {other}"))),
            "{got:?}"
        );
        // The helper hangs under its worker, indented.
        let at = |id: AgentId| {
            let name = clean(&app.world.agents[id].name);
            got.iter().position(|l| l.contains(&name)).unwrap()
        };
        assert_eq!(at(helper), at(worker) + 1);
        assert!(
            got[at(helper)].starts_with("   └ ● "),
            "{}",
            got[at(helper)]
        );
        assert!(got[at(worker)].starts_with(" ○ "), "{}", got[at(worker)]);
        // AT WORK counts the agents, not the headings; what `s` hid is
        // counted too.
        let heading = got.iter().find(|l| l.contains("AT WORK")).unwrap().clone();
        assert!(heading.contains(" 5 · idle 4"), "{heading}");
        app.settled.insert(helper);
        app.settled_projects.insert(1);
        let got = lines(&app, 120);
        let heading = got.iter().find(|l| l.contains("AT WORK")).unwrap();
        assert!(
            heading.contains(" 2 · idle 2 · 2 hidden (:unsettle)"),
            "{heading}"
        );
        assert!(!got.iter().any(|l| l.contains(&other)), "a hidden project");
        app.settled.clear();
        app.settled_projects.clear();
        // Folded: the heading alone, `▸`, ending with its loudest member
        // as the team's does; no row hangs under it.
        app.collapsed[0] = true;
        let got = lines(&app, 120);
        assert!(got[1].starts_with(&format!("▸▸ {name}")), "{}", got[1]);
        assert!(got[1].ends_with("● claude-9.1"), "{}", got[1]);
        assert!(!got.iter().any(|l| l.starts_with("   └")), "{got:?}");
        assert!(got[2].starts_with(&format!(" ▾ {other}")), "{}", got[2]);
        app.collapsed[0] = false;
        // The status line names the heading's keys.
        let s = render(&app, 120, 40);
        let foot = s.lines().last().unwrap();
        assert!(
            foot.contains("   enter  open   h l  fold · unfold   s  hide   ?  help"),
            "{foot}"
        );
        assert!(foot.starts_with(&format!(" home · {name}   ")), "{foot}");
        // Narrowed to one project: no heading, the rows as before.
        app.at_home.scope = Some(0);
        let got = lines(&app, 120);
        assert!(
            !got.iter().any(|l| l.contains(&format!("▾ {name}"))),
            "{got:?}"
        );
        assert!(got[1].starts_with("▸○ "), "{}", got[1]);
    }

    #[test]
    fn a_heading_s_preview_names_the_project_its_folder_and_its_leader() {
        let (mut app, _, _) = team();
        app.home_go(0);
        let s = render(&app, 150, 40);
        let name = clean(&app.world.projects[0].name);
        let lead = clean(&app.world.agents[app.world.projects[0].leader].name);
        let preview: Vec<&str> = s
            .lines()
            .map(|l| l.split('│').nth(1).unwrap_or(""))
            .collect();
        let head = preview.iter().find(|l| l.contains(&lead)).unwrap();
        assert!(head.contains(&name), "{head}");
        assert!(
            head.trim_start().starts_with(&format!("{name}  {lead} ·")),
            "the project once, then its leader: {head}"
        );
        let folder = app.project_folder(0).unwrap();
        assert!(preview.iter().any(|l| l.trim() == folder), "{preview:?}");
        let leader = &app.world.agents[app.world.projects[0].leader];
        let status: String = status_spans(app.lang, &app.world, leader)
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(
            preview.iter().any(|l| l.trim() == status.trim()),
            "the leader's status {status:?}: {preview:?}"
        );
    }

    #[test]
    fn nothing_listed_says_so() {
        let mut app = home(Lang::En);
        for a in &mut app.world.agents {
            a.gone = true;
        }
        assert!(render(&app, 150, 40).contains("Nothing needs you"));
        assert!(render(&app, 80, 24).contains("Nothing needs you"));
        // Said once and whole, where the preview would stand beside it
        // too, at the width the list alone would cut it (record 125).
        let said = Lang::En.text().home_nothing;
        for width in [120, 100] {
            let screen = render(&app, width, 40);
            assert_eq!(screen.matches("Nothing needs you").count(), 1, "{screen}");
            assert!(screen.contains(&format!(" {said}")), "{screen}");
        }
        // Too narrow for one line: it wraps, every word kept.
        let got = lines(&app, 30);
        let words: Vec<&str> = got.iter().flat_map(|l| l.split_whitespace()).collect();
        assert_eq!(words, said.split_whitespace().collect::<Vec<_>>());
        assert!(got.len() > 1 && got.iter().all(|l| l.chars().count() <= 30));
        // The status line offers nothing to write to an agent: none is at
        // hand; `/` writes to the lead, as the list says.
        let foot = render(&app, 120, 40);
        let foot = foot.lines().last().unwrap();
        assert!(!foot.contains("write to this agent"), "{foot}");
        assert!(foot.contains("/  write to the lead"), "{foot}");
    }

    #[test]
    fn a_row_names_its_project_only_where_that_tells_it_apart() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        let name = clean(&app.world.projects[app.world.agents[a].project].name);
        let row = |app: &App| {
            let line = row_lines(app, HomeRow::Agent(a), 100, false, false);
            text(&line[0])
        };
        assert!(row(&app).contains(&name));
        app.at_home.scope = Some(app.world.agents[a].project);
        assert!(!row(&app).contains(&name));
        app.at_home.scope = None;
        let other = 1 - app.world.agents[a].project;
        app.world.projects[other].closed = true;
        assert!(!row(&app).contains(&name));
    }

    #[test]
    fn the_title_column_shrinks_on_a_narrow_screen_but_never_under_its_least() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.at_home.scope = Some(app.world.agents[a].project);
        app.world.agents[a].card = Some(0);
        app.world.cards[0].title = "A title long enough to be cut at every width".into();
        app.world.inbox.push(item(InboxKind::Question, a, "Which?"));
        let title = |width| {
            let line = row_lines(&app, HomeRow::Item(0), width, false, false);
            let s = text(&line[0]);
            let at = s.find("A title").unwrap();
            let end = s[at..].find("  ").unwrap();
            s[at..at + end].chars().count()
        };
        assert_eq!(title(100), TITLE);
        assert_eq!(title(64), TITLE);
        assert_eq!(title(60), 20);
        assert_eq!(title(45), TITLE_MIN);
    }

    #[test]
    fn a_long_title_takes_what_the_row_leaves_free_after_its_words() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.at_home.scope = Some(app.world.agents[a].project);
        app.world.agents[a].card = Some(0);
        app.world.agents[a].status = Status::Idle;
        let long = "Case-insensitive filter for the product search page";
        app.world.cards[0].title = long.into();
        let row = |app: &App, width, here| {
            text(&row_lines(app, HomeRow::Agent(a), width, false, here)[0])
        };
        // 54 wide (the list beside the preview at 120): ` ○ #1   ` is 8,
        // the column 14 and 2, `idle` 4 and a cell: 25 more for the title.
        let got = row(&app, 54, false);
        assert_eq!(got, format!(" ○ #1   {}…  idle", &long[..38]));
        assert_eq!(got.chars().count(), 53);
        // A title that fits whole takes only what it needs: the column.
        app.world.cards[0].title = "Short one".into();
        assert!(row(&app, 54, false).contains(&format!("{:<16}idle", "Short one")));
        // At hand, too narrow for its keys: they go as before, the title
        // takes the row as one not at hand does.
        app.world.cards[0].title = long.into();
        app.at_home.at = Some(HomeRow::Agent(a));
        let got = row(&app, 54, true);
        assert_eq!(got, format!("▸○ #1   {}…  idle", &long[..38]));
        // Wide enough for them: they keep their place at the end and the
        // words their least cells; the title has what is left.
        let got = row(&app, 80, true);
        assert!(got.trim_end().ends_with("enter  open"), "{got}");
        // Just room for the keys beside the column's title (70 - 8 - 24 -
        // 2 - 14 - 2 is the words' least 20): they stay, nothing is free.
        let got = row(&app, 70, true);
        assert!(got.trim_end().ends_with("enter  open"), "{got}");
        assert!(got.contains(&format!("{}…  idle", &long[..23])), "{got}");
        let got = row(&app, 80, true);
        // 80 - 8 - 2 - (20 + 14 + 2): 34 cells, the column's 24 and 10.
        assert!(got.contains(&format!("{}…  idle ", &long[..33])), "{got}");
        assert!(got.chars().count() <= 80);
    }

    #[test]
    fn the_lonca_line_stays_above_the_list_for_a_project_on_it() {
        let mut app = home(Lang::En);
        app.world.on_lonca(0, Some("demo".into()));
        let s = render(&app, 100, 20);
        assert!(s.lines().nth(1).unwrap().contains("lonca demo"), "{s}");
        app.at_home.scope = Some(1);
        let s = render(&app, 100, 20);
        assert!(!s.contains("lonca demo"), "{s}");
    }

    #[test]
    fn a_narrow_screen_stacks_an_entry_s_words_under_its_title_without_keys() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.at_home.scope = Some(app.world.agents[a].project);
        app.world.agents[a].card = Some(0);
        app.world.cards[0].title = "Paging".into();
        app.world.inbox.push(item(
            InboxKind::Question,
            a,
            "Which parser should the tests use?",
        ));
        app.home_settle();
        let row = |app: &App, width: usize, narrow: bool| -> Vec<String> {
            row_lines(app, HomeRow::Item(0), width, narrow, true)
                .iter()
                .map(text)
                .collect()
        };
        let wide = row(&app, 100, false);
        assert!(wide[0].contains("Paging") && wide[0].contains("Which parser"));
        assert!(wide[0].contains("answer"), "keys beside: {}", wide[0]);
        let narrow = row(&app, 100, true);
        assert_eq!(narrow[0].trim_end(), "▸! #1   Paging", "{narrow:?}");
        assert!(
            narrow[1].starts_with("   Which parser should the tests use?"),
            "under the card, from the fourth column: {narrow:?}"
        );
        assert!(!narrow.iter().any(|l| l.contains("answer")), "{narrow:?}");
        // `lines` stacks under eighty columns, not at eighty.
        let at = |width| lines(&app, width).iter().position(|l| l.contains("Paging"));
        let (stacked, beside) = (lines(&app, 79), lines(&app, 80));
        assert!(!stacked[at(79).unwrap()].contains("Which"), "{stacked:?}");
        assert!(beside[at(80).unwrap()].contains("Which"), "{beside:?}");
        // A title too long for the row is cut to it.
        app.world.cards[0].title = "A title long enough to be cut at forty columns".into();
        let narrow = row(&app, 40, true);
        assert_eq!(narrow[0].chars().count(), 39, "{narrow:?}");
        assert!(narrow[0].ends_with('…'), "{narrow:?}");
    }

    #[test]
    fn a_narrow_row_that_says_what_it_does_keeps_one_line_while_it_fits() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.at_home.scope = Some(app.world.agents[a].project);
        app.world.agents[a].card = Some(0);
        app.world.cards[0].title = "Paging".into();
        let row = |width: usize| -> Vec<String> {
            row_lines(&app, HomeRow::Agent(a), width, true, false)
                .iter()
                .map(text)
                .collect()
        };
        let one = row(79);
        assert_eq!(one.len(), 1, "{one:?}");
        assert!(one[0].contains("Paging  idle"), "{one:?}");
        // Exactly as wide as the screen is too wide: the edge cell stays
        // free.
        let whole = one[0].chars().count();
        assert_eq!(row(whole + 1).len(), 1, "{:?}", row(whole + 1));
        let two = row(whole);
        assert_eq!(two.len(), 2, "{two:?}");
        assert_eq!(two[1].trim(), "idle", "{two:?}");
    }

    #[test]
    fn a_narrow_row_s_part_wider_than_a_line_is_cut_on_the_line_under_the_title() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        app.at_home.scope = Some(app.world.agents[a].project);
        app.world.agents[a].card = None;
        let long = "dotnet test --filter Category=Integration --no-build --verbosity quiet";
        app.world.changes.push(change(a, vec![ran(long, 0)]));
        app.world.inbox.push(item(InboxKind::Change, a, ""));
        let got: Vec<String> = row_lines(&app, HomeRow::Item(0), 40, true, false)
            .iter()
            .map(text)
            .collect();
        assert!(got[0].contains("Paging in product search"), "{got:?}");
        assert!(
            got[1].starts_with("   ✓ dotnet test"),
            "no blank line: {got:?}"
        );
        assert!(got[1].chars().count() <= 40, "{got:?}");
        assert!(got.iter().all(|l| l.chars().count() <= 40), "{got:?}");
    }

    /// The first worker of `home`, blocked: its leader, and the block's
    /// message as its leader read it.
    fn with_block(app: &mut App) -> (AgentId, AgentId) {
        let a = worker(app);
        let lead = app.world.projects[app.world.agents[a].project].leader;
        let said = "The retry test needs a clock.\nI own only model.rs.";
        app.world.blocked(a, lead, 4, said);
        app.world.block_read(a, 4);
        (a, lead)
    }

    #[test]
    fn a_blocked_worker_stays_at_work_and_its_row_says_who_has_its_block() {
        let mut app = home(Lang::En);
        let (a, lead) = with_block(&mut app);
        app.unseen.insert(a);
        assert!(
            app.home_rows_in(None)
                .contains(&(HomeGroup::Work, HomeRow::Agent(a)))
        );
        assert_eq!(
            app.rungs()[a],
            Rung::Idle,
            "not done, not the person's turn"
        );
        let leader = clean(&app.world.agents[lead].name);
        let row = |app: &App| {
            let got = lines(app, 140);
            got.into_iter().find(|l| l.contains("clock")).unwrap()
        };
        let at_work = row(&app);
        assert!(at_work.contains("■"), "{at_work}");
        let said = format!("blocked \"The retry test needs a clock.\" · {leader} is on it");
        assert!(at_work.contains(&said), "{at_work}");
        // The preview reads it whole, then what the leader did.
        app.world.agents[lead].trace(0, "tool_use Grep RETRY_TICKS".to_string());
        app.at_home.at = Some(HomeRow::Agent(a));
        let s = render(&app, 150, 40);
        let order = |needle: &str| s.find(needle).unwrap_or_else(|| panic!("{needle}: {s}"));
        assert!(order(&format!("told {leader} at ")) < order("I own only model.rs."));
        assert!(order("I own only model.rs.") < order(&format!("{leader}, so far:")));
        assert!(order(&format!("{leader}, so far:")) < order("RETRY_TICKS"));
        // The leader asked the person: the row says so while they have
        // not answered.
        app.world.asked_you(lead, "Let it in?");
        app.world.block_asked(lead, Some(4), "", None);
        let asked = row(&app);
        assert!(asked.contains(&format!("· {leader} asked you")), "{asked}");
        app.world.close_questions(lead);
        assert!(row(&app).contains(&format!("· {leader} is on it")));
        // Turkish says the same.
        app.lang = Lang::Tr;
        let said = format!("takıldı \"The retry test needs a clock.\" · {leader} ilgileniyor");
        assert!(row(&app).contains(&said), "{}", row(&app));
    }

    #[test]
    fn a_leader_s_question_about_a_block_is_the_blocked_work_s_row_and_reads_in_parts() {
        let mut app = home(Lang::En);
        let (a, lead) = with_block(&mut app);
        let card = app.world.agents[a].card;
        let (worker, leader) = (
            clean(&app.world.agents[a].name),
            clean(&app.world.agents[lead].name),
        );
        app.world.note(Event::ToYou {
            from: lead,
            kind: MsgKind::Ask,
            text: "Let it into live/mod.rs?".into(),
            options: vec!["let it in".into(), "wait".into()],
        });
        app.world.asked_you(lead, "Let it into live/mod.rs?");
        let tried = "Looked for a test clock.\nThere is none.";
        app.world
            .block_asked(lead, Some(4), tried, Some("it widens the card"));
        let i = app.world.open_inbox()[0];
        assert_eq!(app.world.inbox[i].card, card, "the blocked work's card");
        let got = lines(&app, 140);
        let said = format!("{worker} is blocked · {leader} asks: Let it into live/mod.rs?");
        let row = got.iter().position(|l| l.contains(&said));
        let row = row.unwrap_or_else(|| panic!("{got:?}"));
        assert_eq!(got[row + 1].trim(), "1  let it in");
        let s = render(&app, 150, 40);
        let order = |needle: &str| s.find(needle).unwrap_or_else(|| panic!("{needle}: {s}"));
        let head = format!("{leader} asks you · risky: it widens the card");
        assert!(order(&head) < order("│blocked"));
        assert!(order("│blocked") < order("│  I own only model.rs."));
        assert!(order("│  I own only model.rs.") < order(&format!("│{leader} tried")));
        assert!(order(&format!("│{leader} tried")) < order("│  There is none."));
        assert!(order("│  There is none.") < order("│asks"));
        assert!(order("│asks") < order("│ 1  let it in"));
        assert!(order("│ 1  let it in") < order(&format!("{leader}, so far:")));
        // The row is called by the blocked work, not by who asks.
        let row = got.iter().find(|l| l.contains(&said)).unwrap();
        let before = &row[..row.find(&said).unwrap()];
        assert!(
            before.contains(&worker) && !before.contains(&leader),
            "{row}"
        );
        // No risk named: the leader could not solve it; nothing tried,
        // no such part.
        app.world.block_asked(lead, Some(4), " ", None);
        let s = render(&app, 150, 40);
        assert!(
            s.contains(&format!("{leader} asks you · could not solve it")),
            "{s}"
        );
        assert!(!s.contains(&format!("{leader} tried")), "{s}");
        // A question of its that is about no block reads as any other.
        app.world.agents[a].block = None;
        let got = lines(&app, 140);
        assert!(got.iter().all(|l| !l.contains("is blocked")), "{got:?}");
        assert!(got.iter().any(|l| l.contains("Let it into live/mod.rs?")));
    }

    #[test]
    fn a_block_the_leader_let_lie_is_a_row_to_decide_with_its_keys_and_its_words() {
        let mut app = home(Lang::En);
        let (a, lead) = with_block(&mut app);
        let leader = clean(&app.world.agents[lead].name);
        app.world.blocks_unsolved(lead, false);
        let i = app.world.open_inbox()[0];
        assert_eq!(
            app.home_rows_in(None)[0],
            (HomeGroup::Question, HomeRow::Item(i))
        );
        assert!(
            !app.home_rows_in(None)
                .iter()
                .any(|r| r.1 == HomeRow::Agent(a)),
            "the item is its row"
        );
        assert_eq!(app.rungs()[a], Rung::Waiting);
        let got = lines(&app, 170);
        let said = format!("blocked: The retry test needs a clock. · {leader} did not solve it");
        let row = got.iter().find(|l| l.contains(&said));
        let row = row.unwrap_or_else(|| panic!("{got:?}"));
        assert!(
            row.contains(" a  answer") && row.contains(" enter  open"),
            "{row}"
        );
        let s = render(&app, 150, 40);
        let name = clean(&app.world.agents[a].name);
        assert!(
            s.contains(&format!(
                "{name} is blocked and its leader did not solve it"
            )),
            "{s}"
        );
        assert!(s.contains("I own only model.rs."), "{s}");
        assert!(s.contains(&format!("{leader}, so far:")), "{s}");
        // Read back from the store the block is gone: the item's own
        // words are what is left.
        app.world.agents[a].block = None;
        let got = lines(&app, 170);
        assert!(
            got.iter()
                .any(|l| l.contains("blocked: The retry test needs a clock.  ")),
            "{got:?}"
        );
        assert!(
            got.iter().all(|l| !l.contains("did not solve it")),
            "{got:?}"
        );
    }
}
