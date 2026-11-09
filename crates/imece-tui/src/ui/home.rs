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
    let [store, body] =
        Layout::vertical([Constraint::Length(tall), Constraint::Min(0)]).areas(area);
    if let Some(line) = lonca {
        f.render_widget(Paragraph::new(line), store);
    }
    let at = app.home_at();
    if body.width < WIDE {
        let drawn = list_drawn(app, usize::from(body.width), body.width < NARROW, at);
        let scroll = drawn.scroll.min(drawn.lines.len().saturating_sub(1) as u16);
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
    let scroll = drawn.scroll.min(drawn.lines.len().saturating_sub(1) as u16);
    let used = drawn.lines.len() as u16;
    note_breath(app, &drawn.lines, &drawn.spots, scroll, list);
    f.render_widget(Paragraph::new(drawn.lines).scroll((scroll, 0)), list);
    mark_under(f, list, app, used.saturating_sub(scroll));
    let rule = vec![Line::from(Span::styled("│", dim())); usize::from(gap.height)];
    f.render_widget(Paragraph::new(rule), gap);
    let room = usize::from(preview.width).saturating_sub(1);
    let text = at.map_or_else(
        || {
            vec![Line::from(Span::styled(
                app.lang.text().home_nothing,
                dim(),
            ))]
        },
        |row| preview_lines(app, row, room, usize::from(preview.height)),
    );
    f.render_widget(Paragraph::new(text), preview);
}

/// While no project has a worker, imece's mark in the rows the list
/// leaves free under it (record 101).
fn mark_under(f: &mut Frame, area: Rect, app: &App, used: u16) {
    let working = (app.home_rows().iter()).any(|r| matches!(r.1, HomeRow::Agent(_)));
    if working {
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

/// `TO DECIDE  2 permissions · 1 question · 3 changes`: the heading and
/// what is under it, each kind counted, a count of none left out.
fn decide_heading(app: &App, listed: &Listed) -> Line<'static> {
    let t = app.lang.text();
    let w = &app.world;
    let mut spans = vec![Span::styled(format!(" {} ", t.home_turn), bold(pal().warn))];
    let counts: Vec<String> = [
        HomeGroup::Permission,
        HomeGroup::Question,
        HomeGroup::Change,
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
    let mut words = counts;
    // What `:clear` took off the list, still open (record 90).
    let hidden = w.hidden_inbox();
    if hidden > 0 {
        words.push(fill(t.home_hidden, &[&hidden.to_string()]));
    }
    spans.push(Span::styled(words.join(" · "), dim()));
    Line::from(spans)
}

/// `AT WORK  12 · stuck 2 · idle 1 · 2 hidden (:unsettle)`: who works
/// (the projects' headings not counted), how many of them are stuck or
/// idle, and how many agents and projects `s` hid, when any are.
fn work_heading(app: &App, listed: &Listed) -> Line<'static> {
    let t = app.lang.text();
    let mut spans = vec![Span::styled(format!(" {} ", t.home_work), bold(pal().ok))];
    let agents = (listed.rows.iter())
        .filter(|r| r.0.heading() == Heading::Work && r.0 != HomeGroup::Team)
        .count();
    let mut words = vec![agents.to_string()];
    for (group, word) in [
        (HomeGroup::Stuck, t.home_stuck_count),
        (HomeGroup::Quiet, t.home_idle_count),
    ] {
        let n = listed.many(group);
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
        HomeGroup::Permission => t.home_groups[0],
        HomeGroup::Question => t.home_groups[1],
        HomeGroup::Change => t.home_groups[2],
        _ => t.home_groups[3],
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
                    HomeRow::Agent(_) | HomeRow::Project(_) => false,
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

/// Home's list as drawn: its lines, how far they scroll for the row at
/// hand to show, and where the agents' glyphs stand (the line, the span,
/// the agent) for the breath's patch.
struct Drawn {
    lines: Vec<Line<'static>>,
    scroll: u16,
    spots: Vec<(usize, usize, AgentId)>,
}

/// The list in rows `width` wide, on a screen as wide, and how far to
/// scroll it for the row at hand to show.
#[cfg(test)]
fn list_lines(app: &App, width: usize, at: Option<HomeRow>) -> (Vec<Line<'static>>, u16) {
    let drawn = list_drawn(app, width, width < usize::from(NARROW), at);
    (drawn.lines, drawn.scroll)
}

/// The list drawn: see [`Drawn`]. `stacked` on a narrow screen (record
/// 131), not where the list is narrow beside the preview.
fn list_drawn(app: &App, width: usize, stacked: bool, at: Option<HomeRow>) -> Drawn {
    let t = app.lang.text();
    let listed = Listed {
        rows: app.home_rows(),
    };
    let mut lines = Vec::new();
    let mut scroll = 0;
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
        if here {
            scroll = lines.len().saturating_sub(4) as u16;
        }
        spots.extend(glyph_spots(app, lines.len(), row));
        lines.extend(row_lines(app, row, width, stacked, here));
    }
    // Nothing to decide but what `:clear` hid: the heading says so.
    if !listed.rows.iter().any(|r| r.0.heading() == Heading::Decide) && app.world.hidden_inbox() > 0
    {
        lines.insert(0, decide_heading(app, &listed));
        for spot in &mut spots {
            spot.0 += 1;
        }
        if scroll > 0 {
            scroll += 1;
        }
    }
    if listed.rows.is_empty() {
        lines.push(Line::from(Span::styled(t.home_nothing, dim())));
    }
    Drawn {
        lines,
        scroll,
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
        HomeRow::Project(_) => None,
    };
    id.and_then(|c| w.cards.get(c))
}

/// What a row is called: its change's title, else its card's, else the
/// agent's name; a project's heading its name.
fn title_of(w: &World, row: HomeRow) -> String {
    if let HomeRow::Project(p) = row {
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
    clean(&w.agents[agent_of(w, row)].name)
}

/// The agent a row belongs to; a project's heading its leader.
fn agent_of(w: &World, row: HomeRow) -> AgentId {
    match row {
        HomeRow::Item(i) => w.inbox[i].agent,
        HomeRow::Agent(a) => a,
        HomeRow::Project(p) => w.projects[p].leader,
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
            return Tail::Asked(plain(&item.detail), options.unwrap_or_default());
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
                &[&cards.to_string(), &dollars(cost), &item.detail],
            )
        }
        InboxKind::Tries | InboxKind::Rounds => {
            let words = match item.kind {
                InboxKind::Tries => t.row_tries,
                _ => t.row_rounds,
            };
            fill(words, &[&words::cap_count(w, item).to_string()])
        }
        InboxKind::Sensitive => fill(t.row_sensitive, &[&item.detail]),
        InboxKind::Change => {
            return match (w.change_of(item.agent), w.verdict(item.agent)) {
                (Some(c), Some(v)) => Tail::Spans(change_spans(app.lang, c, &v)),
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
    if since >= 60 {
        text.push_str(&format!(" · {}", words::ago(lang, since)));
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
            InboxKind::Question | InboxKind::Fault => {
                vec![("a", t.home_answer), ("enter", t.home_open)]
            }
            // A reply asks for nothing: `enter` reads it, `r` puts it away.
            InboxKind::Reply => vec![("enter", t.home_open), ("r", t.home_dismiss)],
            InboxKind::Tries | InboxKind::Rounds => {
                vec![("a", t.home_once_more), ("r", t.home_fail)]
            }
            _ => vec![("a", t.home_yes), ("r", t.home_no)],
        },
        Some(HomeRow::Agent(_)) => vec![("enter", t.home_open)],
        // A heading decides nothing: the status line has its keys.
        Some(HomeRow::Project(_)) | None => Vec::new(),
    };
    let mut spans = hint_spans(&keys);
    for s in &mut spans {
        if s.style != key_style() {
            s.style = dim();
        }
    }
    spans
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
    let agent = &w.agents[agent_of(w, row)];
    let (glyph, colour) = match row {
        HomeRow::Agent(_) => glyph(agent),
        HomeRow::Item(_) | HomeRow::Project(_) => ("!", pal().warn),
    };
    let tail = match row {
        HomeRow::Item(i) => item_tail(app, i),
        HomeRow::Agent(id) => agent_tail(app, id, glyph),
        HomeRow::Project(_) => unreachable!("a heading is its own row"),
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
        HomeRow::Item(_) | HomeRow::Project(_) => fg(colour),
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
        head.push(Span::styled(
            format!("{:<title_width$}  ", fit(&title_of(w, row), title_width)),
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

/// `text` in rows of `room`, under `label`.
fn labelled_text(lines: &mut Vec<Line<'static>>, label: &str, text: &str, room: usize) {
    let wrapped = overlay::wrap_words(&clean(text), room.saturating_sub(9));
    for (n, row) in wrapped.into_iter().enumerate() {
        let label = if n == 0 { label } else { "" };
        lines.push(labelled(label, vec![Span::raw(row)]));
    }
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
    labelled_text(lines, why, w.done_why(c.agent).unwrap_or(&c.title), room);
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
    lines.push(Line::default());
    lines.extend(focus::plan_lines(t, &a.plan));
    if !a.plan.is_empty() {
        lines.push(Line::default());
    }
    let left = rows.saturating_sub(lines.len());
    let colour = speaker_color(id);
    let newest = a.trace.len().saturating_sub(left);
    for raw in a.trace.iter().skip(newest) {
        lines.extend(focus::trace_lines(raw, colour, room));
    }
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
    if !q.options.is_empty() {
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
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(t.home_so_far.to_string(), dim())));
}

/// What a row holds, read without a key: a heading, then what it is.
fn preview_lines(app: &App, row: HomeRow, room: usize, rows: usize) -> Vec<Line<'static>> {
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
            } else {
                let said = words::inbox(app.lang, w, item);
                for row in overlay::wrap_words(&said, room) {
                    lines.push(Line::from(Span::raw(row)));
                }
                if let Some(q) = w.question_of(item.agent) {
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
        list_lines(app, width, app.home_at())
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
        assert_eq!(got[0], " TO DECIDE 1 permission · 2 questions · 1 change");
        assert!(got[1].starts_with(" permissions"), "{}", got[1]);
        assert!(got[1].ends_with("one key each"), "{}", got[1]);
        assert_eq!(
            got[1].chars().count(),
            99,
            "the note a cell short of the edge"
        );
        assert_eq!(got[3], " questions");
        assert!(got[6].starts_with(" changes"), "{}", got[6]);
        assert!(got[6].ends_with("1 ready to approve"), "{}", got[6]);
        // A change that was not proved clean is not ready: no note.
        app.world.changes[0].runs.clear();
        let got = lines(&app, 100);
        assert_eq!(got[6], " changes");
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
        assert_eq!(got[0], " TO DECIDE 2 questions · 1 change · 1 put off");
        assert_eq!(got[1], " questions");
        assert_eq!(got[6], " later");
        assert!(got[7].contains("run cargo test?"), "{}", got[7]);
        app.deferred.clear();
        // One of a kind, many of another, in Turkish.
        app.lang = Lang::Tr;
        let got = lines(&app, 100);
        assert_eq!(got[0], " KARAR BEKLEYEN 1 izin · 2 soru · 1 değişiklik");
        assert!(got[1].ends_with("her biri tek tuş"), "{}", got[1]);
        app.deferred.push(0);
        let got = lines(&app, 100);
        assert!(got[0].ends_with("· 1 ertelenen"), "{}", got[0]);
        assert_eq!(got[6], " sonraya");
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
        assert!(got.contains("TO DECIDE 4 questions · 1 change"), "{got}");
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
        let line = &list_lines(&app, 100, app.home_at()).0[2];
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
    fn the_list_scrolls_to_keep_the_row_at_hand_four_lines_from_the_top() {
        let mut app = home(Lang::En);
        let a = worker(&app);
        for n in 0..12 {
            app.world
                .inbox
                .push(item(InboxKind::Question, a, &format!("q{n}")));
        }
        let scroll = |app: &App| list_lines(app, 100, app.home_at()).1;
        app.home_go(0);
        assert_eq!(scroll(&app), 0);
        app.home_go(3);
        assert_eq!(
            scroll(&app),
            1,
            "row 3 is line 5: TO DECIDE, questions, then rows"
        );
        app.home_go(11);
        assert_eq!(scroll(&app), 9);
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
        assert!(row.contains("  idle · 1 min"), "{row}");
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
}
