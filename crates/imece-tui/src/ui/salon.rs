//! The default view: every project as a group that folds (its leader,
//! workers and their helpers and researchers; a worker's helpers fold
//! too), who talks to whom right now, what waits for the person, and the
//! journal of meaningful events.

use std::collections::BTreeSet;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::*;
use crate::app::{App, Need, Pane, Rung, Sel, Target};
use crate::motion::Motion;

/// Drawn before an agent's name in its speaker colour.
pub(crate) const SPEAKER_MARK: &str = "▎";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Project(ProjectId),
    Agent {
        id: AgentId,
        depth: u8,
    },
    /// Calm workers of a project folded into one line: how many, and the
    /// loudest of them (the highest rung, the first among equals).
    Folded {
        n: usize,
        loud: AgentId,
    },
    /// The header over an open project's workers of one need, when some
    /// of its workers need the person and it shows more than one: how many
    /// it heads, folded ones too, and how many of those are idle.
    Group {
        need: Need,
        n: usize,
        idle: usize,
    },
}

impl Row {
    /// What selecting this row selects; a folded line is not selectable.
    pub fn sel(&self) -> Option<Sel> {
        match *self {
            Row::Project(p) => Some(Sel::Project(p)),
            Row::Agent { id, .. } => Some(Sel::Agent(id)),
            _ => None,
        }
    }
}

/// The salon's rows for `capacity` lines. A folded project is one line; a
/// worker in `folded_workers` shows no helpers. In an open project, the
/// workers that need the person more come first (waiting for them, stuck,
/// done and not seen); the rest keep their order so the screen does not
/// jump. When the rows do not fit, calm workers (working or idle, not
/// talking, nothing new) fold into one line per project, from the bottom
/// up: those that need the person more fold last.
/// When some workers of an open project need the person and it shows
/// more than one, each need's workers sit under a header (`Row::Group`):
/// the stuck, those waiting for the person, those done and not seen, then
/// the rest, so the eye finds the edge between what needs the person and
/// what does not.
/// With `needs_only` (`f`), only agents that need the person are shown,
/// under their project's header, a worker also for a helper or
/// researcher of its that does; a project with none is left out.
/// Settled agents (`s`) and their rows and settled projects are left out
/// whatever the rest says.
pub fn rows(app: &App, capacity: usize) -> Vec<Row> {
    let w = &app.world;
    let t = w.tick;
    let need = app.needs();
    let kept =
        |a: &Agent| !app.settled.contains(&a.id) && (!app.needs_only || need[a.id] > Need::Nothing);
    let researcher_of = |a: &Agent| {
        w.agents.iter().find(|r| {
            r.role == Role::Researcher
                && !r.gone
                && matches!(r.status, Status::Researching { for_agent, .. } if for_agent == a.id)
                && kept(r)
        })
    };
    let helpers_of = |a: &Agent| -> Vec<AgentId> {
        if app.folded_workers.contains(&a.id) {
            return Vec::new();
        }
        w.helpers_of(a.id)
            .filter(|h| kept(h))
            .map(|h| h.id)
            .collect()
    };
    // One line per agent, whatever it says: rows never change height.
    let lines = |a: &Agent| 1 + usize::from(researcher_of(a).is_some()) + helpers_of(a).len();
    let calm = |a: &Agent| {
        matches!(a.status, Status::Working | Status::Idle)
            && a.talk.as_ref().is_none_or(|k| k.until <= t)
            && t.saturating_sub(a.changed_at) > 6
            && app.selected != Sel::Agent(a.id)
    };
    let shown: Vec<bool> = w
        .projects
        .iter()
        .map(|p| {
            !p.closed
                && !app.settled_projects.contains(&p.id)
                && (!app.needs_only || w.agents.iter().any(|a| a.project == p.id && kept(a)))
        })
        .collect();

    // Workers of each open project, in display order.
    let order: Vec<Vec<&Agent>> = w
        .projects
        .iter()
        .map(|p| {
            if !shown[p.id] || app.collapsed[p.id] {
                return Vec::new();
            }
            let mut o: Vec<&Agent> = w
                .agents
                .iter()
                .filter(|a| a.project == p.id && a.role == Role::Worker)
                .filter(|a| !app.settled.contains(&a.id))
                .filter(|a| kept(a) || !helpers_of(a).is_empty() || researcher_of(a).is_some())
                .collect();
            // Stable: equals keep their order.
            o.sort_by_key(|a| std::cmp::Reverse(need[a.id]));
            o
        })
        .collect();

    // Which projects head their workers by need, and with how many
    // headers (the workers are sorted by need, so each need is one run).
    let headers: Vec<usize> = order
        .iter()
        .map(|o| {
            if o.len() > 1 && o.iter().any(|a| need[a.id] > Need::Nothing) {
                o.chunk_by(|a, b| need[a.id] == need[b.id]).count()
            } else {
                0
            }
        })
        .collect();

    let leads = |p: &Project| kept(&w.agents[p.leader]);
    let mut total: usize = w
        .projects
        .iter()
        .map(|p| {
            if !shown[p.id] {
                return 0;
            }
            if app.collapsed[p.id] {
                return 1;
            }
            let leader = &w.agents[p.leader];
            // A solo's helpers hang under it as a worker's do.
            let lead = if leads(p) {
                1 + helpers_of(leader).len()
            } else {
                0
            };
            1 + lead + order[p.id].iter().map(|a| lines(a)).sum::<usize>()
        })
        .sum();
    // The headers give way first: shown only when every row fits with
    // them, so a crowded salon folds as it did without them.
    let grouped: Vec<bool> = if total + headers.iter().sum::<usize>() <= capacity {
        headers.iter().map(|&n| n > 0).collect()
    } else {
        vec![false; w.projects.len()]
    };
    let mut folded = vec![false; w.agents.len()];
    let mut nfolded = vec![0usize; w.projects.len()];
    // What needs the person is never folded away; a worker shown for its
    // helper would fold the helper with it. Rows that fit fold nothing:
    // the loop stops at once.
    if !app.needs_only {
        // From the bottom of the salon up: a pinned project folds last.
        'fold: for p in w.salon_order().into_iter().rev() {
            for a in order[p].iter().rev() {
                if total <= capacity {
                    break 'fold;
                }
                if calm(a) {
                    folded[a.id] = true;
                    total -= lines(a);
                    if nfolded[p] == 0 {
                        total += 1; // the project's folded line
                    }
                    nfolded[p] += 1;
                }
            }
        }
    }

    let mut out = Vec::new();
    let salon = w.salon_order();
    for p in (salon.iter().map(|&p| &w.projects[p])).filter(|p| shown[p.id]) {
        out.push(Row::Project(p.id));
        if app.collapsed[p.id] {
            continue;
        }
        if leads(p) {
            out.push(Row::Agent {
                id: p.leader,
                depth: 1,
            });
            for h in helpers_of(&w.agents[p.leader]) {
                out.push(Row::Agent { id: h, depth: 2 });
            }
        }
        let header = |g: Need| {
            let of = order[p.id].iter().filter(|a| need[a.id] == g);
            Row::Group {
                need: g,
                n: of.clone().count(),
                idle: of
                    .filter(|a| matches!(a.status, Status::Idle | Status::Asleep))
                    .count(),
            }
        };
        let mut headed = None;
        for a in order[p.id].iter().filter(|a| !folded[a.id]) {
            let g = need[a.id];
            if grouped[p.id] && headed != Some(g) {
                out.push(header(g));
                headed = Some(g);
            }
            out.push(Row::Agent { id: a.id, depth: 2 });
            for h in helpers_of(a) {
                out.push(Row::Agent { id: h, depth: 3 });
            }
            if let Some(r) = researcher_of(a) {
                out.push(Row::Agent { id: r.id, depth: 3 });
            }
        }
        if nfolded[p.id] > 0 {
            let loud = app
                .loudest(order[p.id].iter().filter(|a| folded[a.id]).map(|a| a.id))
                .expect("a folded line folds somebody");
            out.push(Row::Folded {
                n: nfolded[p.id],
                loud,
            });
        }
    }
    out
}

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let (left, right) = if area.width >= 100 {
        let [l, r] = Layout::horizontal([
            Constraint::Percentage(100 - app.side),
            Constraint::Percentage(app.side),
        ])
        .areas(area);
        (l, r)
    } else {
        let [l, r] = Layout::vertical([
            Constraint::Percentage(100 - app.side),
            Constraint::Percentage(app.side),
        ])
        .areas(area);
        (l, r)
    };
    match app.card {
        Some(c) => super::card::draw(f, left, app, c),
        None => salon(f, left, app),
    }
    side(f, right, app);
}

pub(super) fn pane_block(title: String, focused: bool) -> Block<'static> {
    let border = if focused { fg(pal().accent) } else { border() };
    Block::bordered().title(title).border_style(border)
}

fn salon(f: &mut Frame, area: Rect, app: &App) {
    let w = &app.world;
    let t = app.lang.text();
    let mut title = if app.needs_only {
        t.salon_needs
    } else {
        t.salon
    }
    .to_string();
    let settled = app.settled.len() + app.settled_projects.len();
    if settled > 0 {
        title.push_str(&fill(t.settled_count, &[&settled.to_string()]));
    }
    let block = pane_block(title, app.pane == Pane::Salon);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [list, queue] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);

    let rows = rows(app, usize::from(list.height));
    *app.shown.borrow_mut() = rows.iter().filter_map(Row::sel).collect();
    let rungs = app.rungs();

    // Whether a worker (or folded) line is the last branch of its project.
    let mut last = vec![false; rows.len()];
    let mut seen = false;
    for (i, row) in rows.iter().enumerate().rev() {
        match row {
            Row::Project(_) | Row::Group { .. } => seen = false,
            Row::Agent { depth: 2, .. } | Row::Folded { .. } if !seen => {
                last[i] = true;
                seen = true;
            }
            _ => {}
        }
    }

    let mut under_last = false;
    let mut lines = Vec::with_capacity(rows.len());
    // Rows whose loopback URL becomes a link: row, column, width, URL, agent.
    let mut links = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        if let Some(sel) = row.sel() {
            hintable(app, Target::Row(sel), list.x, list.y + i as u16, list);
        }
        let line = match *row {
            Row::Project(p) => project_line(app, p, &rungs),
            Row::Agent { id, depth } => {
                let prefix = match depth {
                    1 => "  ",
                    2 => {
                        under_last = last[i];
                        if last[i] { "  └ " } else { "  ├ " }
                    }
                    _ => {
                        // The last branch under its worker closes it.
                        let last3 = rows
                            .get(i + 1)
                            .is_none_or(|r| !matches!(r, Row::Agent { depth: 3, .. }));
                        match (under_last, last3) {
                            (true, true) => "    └ ",
                            (true, false) => "    ├ ",
                            (false, true) => "  │ └ ",
                            (false, false) => "  │ ├ ",
                        }
                    }
                };
                let line = agent_line(app, &w.agents[id], prefix, &rungs, usize::from(list.width));
                if let Some(url) = linked(&w.agents[id]) {
                    let width = line.spans.last().map_or(0, Span::width);
                    links.push((i, line.width() - width, width, url, id));
                }
                line
            }
            Row::Group { need, n, idle } => group_line(app, need, n, idle),
            Row::Folded { n, loud } => {
                let mut spans = vec![
                    Span::styled("  └ ", dim()),
                    Span::styled(fill(app.lang.text().folded, &[&n.to_string()]), dim()),
                ];
                spans.extend(member(app, loud, &rungs));
                Line::from(spans)
            }
        };
        // Where its glyph breathes, if it does.
        let glyph = match *row {
            Row::Project(p) => project_member(app, p).map(|m| (m, line.spans.len() - 2)),
            Row::Agent { id, .. } => Some((id, 1)),
            Row::Folded { loud, .. } => Some((loud, 2)),
            Row::Group { .. } => None,
        };
        if let Some((id, at)) = glyph
            && super::breathes(&w.agents[id], rungs[id])
        {
            note_glyph(app, &line, at, list, i);
        }
        lines.push(line);
    }
    if lines.is_empty() && app.needs_only {
        lines.push(Line::from(Span::styled(t.salon_none, dim())));
    }
    f.render_widget(Paragraph::new(lines), list);
    for (i, x, width, url, id) in links {
        if let Some(cells) = link_cells(list, i, x, width) {
            hyperlink(f.buffer_mut(), cells, url, id);
        }
    }
    f.render_widget(Paragraph::new(queue_line(app)), queue);
}

/// The header over an open project's workers of one need: `✕ stuck · 1`,
/// `! waits for you · 2`, `✓ done, not seen · 1` in their rung's colour,
/// the rest dim and counted as working and idle (`● 8 working  ○ 1 idle`,
/// a count of none left out).
fn group_line(app: &App, need: Need, n: usize, idle: usize) -> Line<'static> {
    let t = app.lang.text();
    let count = n.to_string();
    let (words, style) = match need {
        Need::Stuck => (fill(t.need_stuck, &[&count]), bold(pal().bad)),
        Need::Waiting => (fill(t.need_waiting, &[&count]), attention()),
        Need::Done => (fill(t.need_done, &[&count]), fg(pal().ok)),
        Need::Nothing => {
            let parts = [(t.need_working, n - idle), (t.need_idle, idle)];
            let words: Vec<String> = parts
                .iter()
                .filter(|(_, k)| *k > 0)
                .map(|(words, k)| fill(words, &[&k.to_string()]))
                .collect();
            (words.join("  "), dim())
        }
    };
    Line::from(vec![Span::raw("  "), Span::styled(words, style)])
}

/// `  ◌ claude-2`: the loudest member of a folded group, its rung's glyph
/// (its own colour) and its name.
fn member(app: &App, id: AgentId, rungs: &[Rung]) -> Vec<Span<'static>> {
    let a = &app.world.agents[id];
    let (g, color) = rung_glyph(a, rungs[id]);
    vec![
        Span::styled(format!("  {g} "), glyph_style(app, a, rungs[id], color)),
        Span::styled(clean(&a.name), fg(vendor_color(a.vendor))),
    ]
}

/// A project's header: folded or open, its trunk, how many work, what it
/// costs, how many are blocked (`✕`) or stopped (`■`), and (even when
/// folded) how much waits for the person. A folded project ends with its
/// loudest member.
pub(super) fn project_line(app: &App, p: ProjectId, rungs: &[Rung]) -> Line<'static> {
    let w = &app.world;
    let t = app.lang.text();
    let project = &w.projects[p];
    let open = !app.collapsed[p];
    let in_project = |a: &&Agent| a.project == p && !a.gone;
    let working = w
        .agents
        .iter()
        .filter(in_project)
        .filter(|a| {
            matches!(a.role, Role::Worker | Role::Helper | Role::Solo) && a.status != Status::Idle
        })
        .count();
    let cost: u64 = w.agents.iter().filter(in_project).map(|a| a.cost).sum();
    let waiting = w
        .open_inbox()
        .iter()
        .filter(|&&i| w.agents[w.inbox[i].agent].project == p)
        .count();
    let blocked = w
        .agents
        .iter()
        .filter(in_project)
        .filter(|a| a.status == Status::Blocked)
        .count();
    let stopped = w
        .agents
        .iter()
        .filter(in_project)
        .filter(|a| a.status == Status::Stopped)
        .count();

    let mut spans = vec![Span::styled(
        format!("{} ", if open { "▾" } else { "▸" }),
        bold(pal().strong),
    )];
    // A pinned project's name gives two of its columns to the mark.
    if project.pinned {
        spans.push(Span::styled("▴ ", bold(pal().accent)));
        spans.push(Span::styled(fit(&project.name, 10), bold(pal().strong)));
    } else {
        spans.push(Span::styled(fit(&project.name, 12), bold(pal().strong)));
    }
    if let Some(trunk) = project.trunk {
        spans.push(Span::styled(format!(" trunk #{trunk} "), dim()));
        spans.push(Span::styled("✓", fg(pal().ok)));
    }
    // Its solo writes straight in the folder: nothing comes as a Δ.
    if project.no_git {
        spans.push(Span::styled(format!("  ⌀ {}", t.no_git), fg(pal().warn)));
    }
    spans.push(Span::styled(
        format!("  {}", fill(t.group_working, &[&working.to_string()])),
        dim(),
    ));
    spans.push(Span::styled(format!("  {}", dollars(cost)), dim()));
    if blocked > 0 {
        spans.push(Span::styled(format!("  ✕{blocked}"), fg(pal().bad)));
    }
    if stopped > 0 {
        spans.push(Span::styled(format!("  ■{stopped}"), fg(pal().stopped)));
    }
    if waiting > 0 {
        // A folded project is quiet, not deaf.
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            fill(t.group_waiting, &[&waiting.to_string()]),
            attention(),
        ));
    }
    if let Some(loud) = project_member(app, p) {
        spans.extend(member(app, loud, rungs));
    }
    let line = Line::from(spans);
    if app.pane == Pane::Salon && app.selected == Sel::Project(p) {
        line.style(Style::new().add_modifier(Modifier::REVERSED))
    } else {
        line
    }
}

/// The member a folded project's header ends with: its loudest; none
/// while it is open.
fn project_member(app: &App, p: ProjectId) -> Option<AgentId> {
    let w = &app.world;
    let in_project = |a: &&Agent| a.project == p && !a.gone;
    (app.collapsed[p])
        .then(|| app.loudest(w.agents.iter().filter(in_project).map(|a| a.id)))
        .flatten()
}

/// An agent's row, `width` columns wide: its status, its newest exchange,
/// how long it has been in that state and how far its to-do list is, in
/// the room the row's fixed parts (cost, stand, helpers, link) leave, then
/// those parts.
pub(super) fn agent_line(
    app: &App,
    a: &Agent,
    prefix: &'static str,
    rungs: &[Rung],
    width: usize,
) -> Line<'static> {
    let w = &app.world;
    let done = app.done_unseen(a.id);
    let (g, color) = rung_glyph(a, rungs[a.id]);
    let mut spans = vec![
        Span::styled(prefix, dim()),
        Span::styled(format!("{g} "), glyph_style(app, a, rungs[a.id], color)),
        // The speaker's colour, so this row and its conversations are seen
        // as one agent; the name keeps its vendor's colour.
        Span::styled(SPEAKER_MARK, fg(speaker_color(a.id))),
        Span::styled(fit(&a.name, 16), bold(vendor_color(a.vendor))),
        taint_span(a),
    ];
    match a.role {
        Role::Leader => spans.push(Span::styled(
            fit(&format!(" {}", a.vendor.label()), 27),
            dim(),
        )),
        // What the person chose to run: the vendor, its model and effort.
        Role::Solo => {
            let runs = match app.model_of(a.id) {
                Some(m) => format!(" {} {m}", a.vendor.label()),
                None => format!(" {}", a.vendor.label()),
            };
            spans.push(Span::styled(fit(&runs, 27), dim()));
        }
        Role::Researcher => {}
        Role::Helper => spans.push(Span::raw(" ".repeat(27))),
        Role::Worker => {
            let card = match a.card {
                Some(c) => format!(" #{:<3} {}", c + 1, fit(&w.cards[c].title, 21)),
                None => " ".repeat(27),
            };
            spans.push(Span::raw(card));
        }
    }
    spans.push(Span::raw(" "));
    let since = since_spans(app, a);

    let mut tail = Vec::new();
    if a.cost > 0 {
        tail.push(Span::styled(format!("  {}", dollars(a.cost)), dim()));
    }
    if matches!(a.role, Role::Worker | Role::Solo) {
        tail.extend(stand_spans(&a.stand, a.start.as_deref()));
    }
    let helpers = w.helpers_of(a.id).count();
    if helpers > 0 {
        // Its helpers are under it, or folded away: the mark says which,
        // and a folded group shows its loudest member.
        let folded = app.folded_workers.contains(&a.id);
        let mark = if folded { "▸" } else { "▾" };
        tail.push(Span::styled(
            format!(
                "  {mark} {}",
                fill(app.lang.text().helpers, &[&helpers.to_string()])
            ),
            dim(),
        ));
        if folded && let Some(loud) = app.loudest(w.helpers_of(a.id).map(|h| h.id)) {
            tail.extend(member(app, loud, rungs));
        }
    }
    // Last, so the screen knows where the text it links starts.
    tail.extend(local_spans(a));

    let status = if done {
        vec![Span::styled(app.lang.text().done_unseen, fg(pal().ok))]
    } else if let Some(step) = on_step(w, a) {
        // The step takes what the row's fixed parts leave it, at most
        // `STEP_WIDTH`, so it never pushes them off the row.
        let free = width.saturating_sub(columns(&spans) + columns(&tail) + 2);
        vec![step_span(step, free.clamp(STEP_MIN, STEP_WIDTH))]
    } else {
        dotted(app, status_spans(app.lang, w, a))
    };
    let used = chars(&status);
    spans.extend(status);

    // What is new on the row gives way to what was there: the context
    // warning, the to-do mark, the time, then the quote, then the
    // activity bar, take only what the row has left once the rest is on
    // it.
    let mut room = width.saturating_sub(columns(&spans) + columns(&tail));
    let context = fits(&mut room, context_spans(a));
    let plan = fits(&mut room, plan_spans(a));
    let since = fits(&mut room, since);
    // A row that says its step says what the agent does for it in the
    // quote's place.
    let quote = if on_step(w, a).is_some() {
        doing_spans(app.lang, a, used, room)
    } else {
        quote_spans(app, a, used, room)
    };
    room = room.saturating_sub(columns(&quote));
    let stirs = fits(&mut room, stir_spans(app, a));
    spans.extend(quote);
    spans.extend(stirs);
    spans.extend(since);
    spans.extend(plan);
    spans.extend(context);
    spans.extend(tail);

    let line = Line::from(spans);
    let recent = w.tick > 4 && w.tick.saturating_sub(a.changed_at) <= 4;
    if app.pane == Pane::Salon && app.selected == Sel::Agent(a.id) {
        line.style(Style::new().add_modifier(Modifier::REVERSED))
    } else if let Some(lit) = app.lit(GlowOn::Agent(a.id)) {
        line.style(lit_style(lit))
    } else if recent {
        // A change is a signal: it stands out for a moment, then calms down.
        line.style(changed())
    } else {
        line
    }
}

/// `part` when it fits in `room` columns, which it then takes; else
/// nothing.
fn fits(room: &mut usize, part: Vec<Span<'static>>) -> Vec<Span<'static>> {
    if columns(&part) <= *room {
        *room -= columns(&part);
        part
    } else {
        Vec::new()
    }
}

/// A status that says `starting…`, its dots counting while motion moves.
fn dotted(app: &App, mut status: Vec<Span<'static>>) -> Vec<Span<'static>> {
    let starting = app.lang.text().starting;
    if let Some(dots) = app.dots() {
        for span in status.iter_mut().filter(|s| s.content == starting) {
            span.content = format!("{}{dots}", starting.trim_end_matches('…')).into();
        }
    }
    status
}

/// The agent's activity bar, `▁▃▅▂▁`: its trace lines in each of the last
/// [`STIR_TICKS`] ticks, the oldest first, while any of them has one and
/// motion is not off. It changes with what the agent does and as the
/// ticks pass, never on a timer of its own.
pub(super) fn stir_spans(app: &App, a: &Agent) -> Vec<Span<'static>> {
    if app.motion == Motion::Off {
        return Vec::new();
    }
    let counts = a.stirs.at(app.world.tick);
    if counts.iter().all(|&n| n == 0) {
        return Vec::new();
    }
    let bar: String = counts.iter().map(|&n| stir_glyph(n)).collect();
    vec![Span::styled(format!("  {bar}"), dim())]
}

/// A column of the activity bar for `n` lines in a tick: the same scale on
/// every row, so rows compare; `▁` is a tick with none.
pub(super) fn stir_glyph(n: u16) -> char {
    match n {
        0 => '▁',
        1 => '▂',
        2 => '▃',
        3..=4 => '▄',
        5..=6 => '▅',
        7..=9 => '▆',
        10..=14 => '▇',
        _ => '█',
    }
}

/// How full the agent's context window is, `◔ 72%`, from `CONTEXT_SHOWN`
/// on, in the attention colour from `CONTEXT_LOUD`; the focus has the
/// figure whatever it is.
pub(super) fn context_spans(a: &Agent) -> Vec<Span<'static>> {
    let Some(p) = a.context.and_then(|f| f.percent()) else {
        return Vec::new();
    };
    if p < CONTEXT_SHOWN {
        return Vec::new();
    }
    let style = if p >= CONTEXT_LOUD {
        fg(pal().warn)
    } else {
        dim()
    };
    vec![Span::styled(format!("  ◔ {p}%"), style)]
}

/// How far the agent's own to-do list is, `≡ 3/7`, when it has one; the
/// focus lists the steps.
pub(super) fn plan_spans(a: &Agent) -> Vec<Span<'static>> {
    let (done, all) = a.plan_count();
    if all == 0 {
        return Vec::new();
    }
    vec![Span::styled(format!("  ≡ {done}/{all}"), dim())]
}

/// Whether the status words already carry a text of their own (what it
/// does, waits for, asked, or why it stopped), so the row quotes nothing
/// more after them.
fn status_says(w: &World, a: &Agent) -> bool {
    match a.status {
        Status::Idle => {
            w.open_item(a.id, InboxKind::Reply).is_some()
                || w.open_item(a.id, InboxKind::Question).is_some()
        }
        Status::Working => {
            w.open_item(a.id, InboxKind::Approval).is_some()
                || a.activity.as_deref().is_some_and(|d| !d.is_empty())
        }
        Status::Blocked | Status::Researching { .. } => true,
        Status::Stopped => !a.topic.is_empty(),
        Status::Asking { .. } | Status::WaitingHuman | Status::Asleep => false,
    }
}

/// Columns `spans` take on the screen.
fn columns(spans: &[Span<'_>]) -> usize {
    spans.iter().map(Span::width).sum()
}

/// After a row's status, which took `used` characters: the agent's newest
/// exchange when the status says no text of its own, the arrow saying who
/// said it (`→ claude-2 "which parser?"`, `← you "fix the parser"`; the
/// question to the teammate it waits for is quoted alone, the status
/// names them). It takes what `STATUS_WIDTH` leaves after the status, and
/// never more than `room`, the row's free columns; too little room and
/// nothing is quoted.
pub(super) fn quote_spans(app: &App, a: &Agent, used: usize, room: usize) -> Vec<Span<'static>> {
    let w = &app.world;
    if status_says(w, a) {
        return Vec::new();
    }
    let Some(k) = &a.talk else {
        return Vec::new();
    };
    let (name, color) = match k.with {
        Party::Agent(o) => (clean(&w.agents[o].name), speaker_color(o)),
        Party::Person => (app.lang.text().you.to_string(), pal().strong),
    };
    let asked =
        k.mine && matches!(a.status, Status::Asking { to, .. } if k.with == Party::Agent(to));
    let arrow = if k.mine { "→ " } else { "← " };
    // The arrow, the name and the space before the quote.
    let head = if asked {
        0
    } else {
        2 + name.chars().count() + 1
    };
    let room = STATUS_WIDTH
        .saturating_sub(used + 1)
        .min(room.saturating_sub(1))
        .saturating_sub(head);
    let Some(q) = quoted(&k.text, room) else {
        return Vec::new();
    };
    let mut spans = vec![Span::raw(" ")];
    if !asked {
        spans.push(Span::styled(arrow, dim()));
        spans.push(Span::styled(name, fg(color)));
        spans.push(Span::raw(" "));
    }
    spans.push(Span::styled(
        q,
        Style::new().fg(pal().info).add_modifier(Modifier::ITALIC),
    ));
    spans
}

/// How long the agent has been in its state, from a minute on: ` · 3 min`
/// after its status; nothing under a minute, so rows do not tick.
pub(super) fn since_spans(app: &App, a: &Agent) -> Vec<Span<'static>> {
    let seconds = app.world.tick.saturating_sub(app.since(a.id)) * SECONDS_PER_TICK;
    if seconds < 60 {
        return Vec::new();
    }
    vec![Span::styled(
        format!(" · {}", words::ago(app.lang, seconds)),
        dim(),
    )]
}

/// Where a worker's worktree stands, `  ⎇ fix-parser ↑2 ↓1 +12 −3`: the
/// branch an agent put it on (else the `start` point it was hired from,
/// `⎇ #41`), its own commits the checkout lacks, the
/// checkout's it lacks, and the lines a done would hand over now. Each
/// part only when there is something to say; nothing for a worktree at
/// the checkout's HEAD with nothing written.
pub(super) fn stand_spans(s: &Stand, start: Option<&str>) -> Vec<Span<'static>> {
    let mut parts = Vec::new();
    if let Some(b) = s.branch.as_deref().or(start) {
        // The agent named it: cleaned, and cut where it would push the
        // rest off the row.
        parts.push(Span::styled(format!("⎇ {}", fit(b, 16).trim_end()), dim()));
    }
    if s.ahead > 0 {
        parts.push(Span::styled(format!("↑{}", s.ahead), fg(pal().info)));
    }
    if s.behind > 0 {
        parts.push(Span::styled(format!("↓{}", s.behind), fg(pal().warn)));
    }
    if s.added > 0 {
        parts.push(Span::styled(format!("+{}", s.added), fg(pal().ok)));
    }
    if s.removed > 0 {
        parts.push(Span::styled(format!("−{}", s.removed), fg(pal().bad)));
    }
    let mut spans = Vec::with_capacity(parts.len() * 2);
    for (i, p) in parts.into_iter().enumerate() {
        spans.push(Span::raw(if i == 0 { "  " } else { " " }));
        spans.push(p);
    }
    spans
}

/// The cells of `list` that row `i`'s link covers, `width` of them from
/// column `x`: only a URL the row shows whole, so what is clicked is what
/// is read; none for a row below the list.
pub(super) fn link_cells(list: Rect, i: usize, x: usize, width: usize) -> Option<Rect> {
    let y = u16::try_from(i).ok()?;
    let x = u16::try_from(x).ok()?;
    let width = u16::try_from(width).ok()?;
    (y < list.height && u32::from(x) + u32::from(width) <= u32::from(list.width))
        .then(|| Rect::new(list.x + x, list.y + y, width, 1))
}

/// The loopback URL a worker's or helper's row links to, when it has one
/// the row can show as it is: one cleaning would change (a path that
/// looks like a secret) is not shown, so the text is the link.
pub(super) fn linked(a: &Agent) -> Option<&str> {
    if !matches!(a.role, Role::Worker | Role::Helper | Role::Solo) {
        return None;
    }
    a.local.as_deref().filter(|url| clean(url) == *url)
}

/// A worker's or helper's loopback URL, `  ↗ localhost:5173/`: the
/// arrow, then the text the terminal links, the last spans of its row.
pub(super) fn local_spans(a: &Agent) -> Vec<Span<'static>> {
    let Some(url) = linked(a) else {
        return Vec::new();
    };
    let shown = url.strip_prefix("http://").unwrap_or(url).to_string();
    vec![
        Span::styled("  ↗ ", dim()),
        Span::styled(shown, fg(pal().info).add_modifier(Modifier::UNDERLINED)),
    ]
}

fn queue_line(app: &App) -> Line<'static> {
    let w = &app.world;
    let t = app.lang.text();
    let mut spans = vec![Span::styled(t.queue, bold(pal().strong))];
    let verifying: Vec<&Card> = w
        .shown_cards()
        .filter(|c| c.column == Column::Verifying)
        .collect();
    if verifying.is_empty() {
        spans.push(Span::styled(t.queue_empty, dim()));
    }
    for c in verifying {
        spans.push(Span::styled(format!(" #{} ◌", c.id + 1), fg(pal().info)));
    }
    let mut done: Vec<&Card> = w
        .shown_cards()
        .filter(|c| c.column == Column::Done)
        .collect();
    done.sort_by_key(|c| std::cmp::Reverse(c.finished));
    if !done.is_empty() {
        spans.push(Span::styled(t.latest, dim()));
        for c in done.into_iter().take(4) {
            spans.push(Span::styled(format!(" #{} ✓", c.id + 1), fg(pal().ok)));
        }
    }
    Line::from(spans)
}

/// What waits for the person and the journal: the whole guild's, or on a
/// card's screen only that card's, its number marked where it is named.
fn side(f: &mut Frame, area: Rect, app: &App) {
    let w = &app.world;
    let t = app.lang.text();
    let open = app.inbox_rows();
    let rows = app.journal_rows();
    let (inbox_title, journal_title) = match app.card {
        Some(c) => {
            let tag = format!("#{}", c + 1);
            (
                fill(t.card_inbox, &[&tag, &open.len().to_string()]),
                fill(t.card_journal, &[&tag, &rows.len().to_string()]),
            )
        }
        None => (
            match w.hidden_inbox() {
                0 => fill(t.inbox_title, &[&open.len().to_string()]),
                n => fill(t.inbox_hidden, &[&open.len().to_string(), &n.to_string()]),
            },
            t.journal.to_string(),
        ),
    };
    // The selected ask is shown whole under its row: the person allows
    // what they read, not a row cut at the pane's edge.
    let whole = match open.get(app.inbox_sel) {
        Some(&i) if app.pane == Pane::Inbox && w.inbox[i].kind == InboxKind::Approval => wrap(
            Line::from(vec![
                Span::raw("  "),
                Span::raw(clean(&words::one_line(&w.inbox[i].detail))),
            ]),
            area.width.saturating_sub(2) as usize,
        ),
        _ => Vec::new(),
    };
    // One line per item, one more for the selected item's keys, and the
    // selected ask whole.
    let want = (open.len() as u16 + 3 + whole.len() as u16).max(3);
    let inbox_h = want.min(area.height / 2).max(3.min(area.height));
    let [top, bottom] =
        Layout::vertical([Constraint::Length(inbox_h), Constraint::Min(0)]).areas(area);

    let focused = app.pane == Pane::Inbox;
    let mut block = pane_block(inbox_title, focused);
    if !open.is_empty() && !focused {
        block = block.border_style(fg(pal().warn));
    }
    let inner = block.inner(top);
    f.render_widget(block, top);
    let mut lines = Vec::new();
    if open.is_empty() {
        let hidden = match app.card {
            Some(_) => 0,
            None => w.hidden_inbox(),
        };
        let said = match hidden {
            0 => t.nothing_needed.to_string(),
            n => fill(t.inbox_all_hidden, &[&n.to_string()]),
        };
        lines.push(Line::from(Span::styled(said, dim())));
    }
    for (n, &i) in open.iter().enumerate() {
        let item = &w.inbox[i];
        let line = inbox_line(app, i);
        hintable(
            app,
            Target::Waiting(i),
            inner.x,
            inner.y + lines.len() as u16,
            inner,
        );
        if focused && n == app.inbox_sel {
            lines.push(line.style(Style::new().add_modifier(Modifier::REVERSED)));
            lines.extend(whole.iter().cloned());
            let keys = match item.kind {
                InboxKind::Reply => t.reply_keys,
                InboxKind::Change => t.change_keys,
                InboxKind::Approval => t.approval_keys,
                InboxKind::Script => t.script_keys,
                _ => t.item_keys,
            };
            lines.push(Line::from(Span::styled(keys, dim())));
        } else {
            lines.push(lit_line(app, GlowOn::Item(i), line));
        }
    }
    f.render_widget(Paragraph::new(lines), inner);

    let focused = app.pane == Pane::Journal;
    let block = pane_block(journal_title, focused);
    let inner = block.inner(bottom);
    f.render_widget(block, bottom);
    let sel = app
        .journal_sel
        .filter(|_| focused)
        .and_then(|i| rows.iter().position(|&(r, _)| r == i));
    let height = usize::from(inner.height);
    let today = day(w.tick);
    let skip = first_shown(w, &rows, sel, height, today);
    // Which day the line above the first shown is of: today when there
    // is none, so a journal whose newest line is older says so first.
    let mut above = match skip {
        0 => today,
        n => day(w.journal[rows[n - 1].0].tick),
    };
    let mut lines = Vec::new();
    for (n, &(i, more)) in rows.iter().enumerate().skip(skip) {
        if lines.len() >= height {
            break;
        }
        let of = day(w.journal[i].tick);
        if of != above {
            lines.push(date_row(of));
            above = of;
        }
        hintable(
            app,
            Target::Line(i),
            inner.x,
            inner.y + lines.len() as u16,
            inner,
        );
        let mut line = entry_line(app, &w.journal[i]);
        if w.marks.contains(&i) {
            line = bookmarked(line);
        }
        if let Some(c) = app.card {
            line = marked(line, c);
        }
        if more > 0 {
            line.spans.push(Span::styled(format!("  +{more}"), dim()));
        }
        if sel == Some(n) {
            lines.push(line.style(Style::new().add_modifier(Modifier::REVERSED)));
            lines.push(Line::from(Span::styled(t.journal_keys, dim())));
        } else {
            lines.push(line);
        }
    }
    f.render_widget(Paragraph::new(lines), inner);
}

/// The first of `rows` the journal pane draws, so that the selected row
/// (`sel`, an index into `rows`) and its key hints stay in view in `height`
/// rows: as far down as the lines above the selection, the day rows among
/// them included, make necessary, and no further.
fn first_shown(
    w: &World,
    rows: &[(usize, usize)],
    sel: Option<usize>,
    height: usize,
    today: u64,
) -> usize {
    let Some(p) = sel else {
        return 0;
    };
    let skip = (p + 2).saturating_sub(height);
    // Day rows between the first shown and the selection take rows too.
    // Skipping further can only drop day rows from the window, never add
    // one, so one count is enough.
    let mut above = match skip {
        0 => today,
        n => day(w.journal[rows[n - 1].0].tick),
    };
    let mut days = 0;
    for &(i, _) in &rows[skip..=p] {
        let of = day(w.journal[i].tick);
        if of != above {
            days += 1;
            above = of;
        }
    }
    (p + 2 + days).saturating_sub(height)
}

/// The journal as the screen shows it, newest first: (journal index, how
/// many older lines of the same conversation fold under it). The journal
/// keeps every event; the screen shows the meaningful ones. Routine
/// assignments are visible on the board and in the leader's trace; an
/// archived project's lines wait in its archive; the lines `:clear` took
/// off are not listed, but for the marked ones.
pub(crate) fn journal_rows(w: &World) -> Vec<(usize, usize)> {
    let shown = w
        .journal
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, e)| e.event.kind() != EntryKind::Assign)
        .filter(|(_, e)| !w.in_archive(e.event.agent()))
        .filter(|(i, _)| *i >= w.cleared.journal || w.marks.contains(i));
    fold_threads(shown, &w.marks).collect()
}

/// Card `c`'s journal as its screen shows it, newest first, folded like
/// [`journal_rows`]: the lines that name the card (posted, claimed,
/// assigned, merged) and what was said with its number (`#323`) in it.
pub(crate) fn card_rows(w: &World, c: CardId) -> Vec<(usize, usize)> {
    let shown = (w.journal.iter().enumerate().rev()).filter(|(_, e)| on_card(c, e));
    fold_threads(shown, &w.marks).collect()
}

/// Card `c`'s journal lines, oldest first and none folded (for `:export`).
pub(crate) fn card_lines(w: &World, c: CardId) -> Vec<usize> {
    (w.journal.iter().enumerate())
        .filter(|(_, e)| on_card(c, e))
        .map(|(i, _)| i)
        .collect()
}

/// Whether journal line `e` is one of card `c`'s: it names the card, or
/// what was said in it has its number.
fn on_card(c: CardId, e: &Entry) -> bool {
    e.event.card() == Some(c)
        || words::said(&e.event).is_some_and(|s| !mentions(s.text, c).is_empty())
}

/// `line` with card `c`'s number marked wherever it is named.
pub(crate) fn marked(line: Line<'static>, c: CardId) -> Line<'static> {
    let mut spans = Vec::with_capacity(line.spans.len());
    for span in line.spans {
        let text = span.content.as_ref();
        let mut at = 0;
        for r in mentions(text, c) {
            if r.start > at {
                spans.push(Span::styled(text[at..r.start].to_string(), span.style));
            }
            spans.push(Span::styled(
                text[r.clone()].to_string(),
                bold(pal().accent),
            ));
            at = r.end;
        }
        if at == 0 {
            spans.push(span);
        } else if at < text.len() {
            spans.push(Span::styled(text[at..].to_string(), span.style));
        }
    }
    Line::from(spans).style(line.style)
}

/// The whole conversation journal line `i` belongs to, oldest first: every
/// line between the same two parties. Any other event stands alone.
pub(crate) fn thread_of(w: &World, i: usize) -> Vec<usize> {
    let Some(pair) = w
        .journal
        .get(i)
        .and_then(|e| words::said(&e.event))
        .map(|s| s.pair())
    else {
        return vec![i];
    };
    w.journal
        .iter()
        .enumerate()
        .filter(|(_, e)| words::said(&e.event).is_some_and(|s| s.pair() == pair))
        .map(|(n, _)| n)
        .collect()
}

/// Where each turn of conversation `entries` starts, as positions in it:
/// a turn is the lines one party says before the other answers, so the
/// first line starts one and so does every line whose speaker is not the
/// one before. Lines that are no conversation have no speaker.
pub(crate) fn turn_starts(w: &World, entries: &[usize]) -> Vec<usize> {
    let speaker = |&i: &usize| {
        w.journal
            .get(i)
            .and_then(|e| words::said(&e.event))
            .map(|s| s.from)
    };
    let speakers: Vec<_> = entries.iter().map(speaker).collect();
    (0..speakers.len())
        .filter(|&n| n == 0 || speakers[n] != speakers[n - 1])
        .collect()
}

/// Where `%` goes from position `sel` of conversation `entries`: from a
/// reply to the message it answers, else from a message to its first
/// reply; none when the conversation has neither.
pub(crate) fn reply_jump(w: &World, entries: &[usize], sel: usize) -> Option<usize> {
    let said = |n: usize| {
        w.journal
            .get(*entries.get(n)?)
            .and_then(|e| words::said(&e.event))
    };
    let me = said(sel)?;
    let find = |hit: &dyn Fn(&words::Said) -> bool| {
        (0..entries.len()).find(|&n| said(n).is_some_and(|o| hit(&o)))
    };
    me.re
        .and_then(|re| find(&|o| o.id == re))
        .or_else(|| find(&|o| o.re == Some(me.id)))
}

/// Each entry with how many older lines of the same conversation directly
/// follow it and are folded under it. A line the person marked is never
/// folded: it keeps a row of its own, for `'` to come back to. Display
/// only: the agents' stories keep every line.
fn fold_threads<'a>(
    entries: impl Iterator<Item = (usize, &'a Entry)> + 'a,
    marks: &'a BTreeSet<usize>,
) -> impl Iterator<Item = (usize, usize)> + 'a {
    let mut entries = entries.peekable();
    std::iter::from_fn(move || {
        let (i, e) = entries.next()?;
        let mut more = 0;
        if let Some(pair) = words::said(&e.event).map(|s| s.pair()) {
            let same = |(n, l): &(usize, &Entry)| {
                !marks.contains(n) && words::said(&l.event).is_some_and(|s| s.pair() == pair)
            };
            while entries.next_if(same).is_some() {
                more += 1;
            }
        }
        Some((i, more))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sent(from: AgentId, to: AgentId) -> Entry {
        Entry {
            tick: 0,
            event: Event::Sent {
                from,
                to,
                kind: MsgKind::Ask,
                text: String::new(),
                id: 0,
                re: None,
            },
        }
    }

    #[test]
    fn a_glyph_is_noted_where_it_is_drawn_or_spoils_the_patch() {
        let app = App::new(1, 1, 0, 1, crate::i18n::Lang::En);
        let glyph = |x, y| super::super::Glyph { x, y, width: 2 };
        let line = Line::from(vec![Span::raw("  ├ "), Span::raw("● "), Span::raw("name")]);
        let list = Rect::new(3, 2, 10, 4);
        *app.breathing.borrow_mut() = Some(Vec::new());
        note_glyph(&app, &line, 1, list, 1);
        assert_eq!(*app.breathing.borrow(), Some(vec![glyph(7, 3)]));
        // Below the list's foot: not on the screen.
        note_glyph(&app, &line, 1, list, 4);
        assert_eq!(*app.breathing.borrow(), Some(vec![glyph(7, 3)]));
        // Up to the right edge: noted; a column past it: cut.
        let edge = Line::from(vec![Span::raw("12345678"), Span::raw("● ")]);
        note_glyph(&app, &edge, 1, list, 3);
        assert_eq!(
            *app.breathing.borrow(),
            Some(vec![glyph(7, 3), glyph(11, 5)])
        );
        let past = Line::from(vec![Span::raw("123456789"), Span::raw("● ")]);
        note_glyph(&app, &past, 1, list, 0);
        assert_eq!(*app.breathing.borrow(), None, "cut");
        note_glyph(&app, &line, 1, list, 1);
        assert_eq!(*app.breathing.borrow(), None, "spoiled for the drawing");
    }

    #[test]
    fn a_folded_projects_member_is_its_own_loudest_not_anothers() {
        let mut app = App::new(2, 1, 0, 1, crate::i18n::Lang::En);
        for a in &mut app.world.agents {
            a.status = Status::Idle;
            a.starting = false;
        }
        // The loudest of all is in project 1.
        let other = app.world.projects[1].leader;
        app.world.agents[other].status = Status::Blocked;
        assert_eq!(project_member(&app, 0), None, "open: none");
        app.collapsed[0] = true;
        let loud = project_member(&app, 0).expect("a member");
        assert_eq!(app.world.agents[loud].project, 0);
        app.collapsed[1] = true;
        assert_eq!(project_member(&app, 1), Some(other));
    }

    #[test]
    fn the_first_shown_row_leaves_room_for_the_day_rows_above_the_selection() {
        let (mut w, _) = crate::sim::world(1, 2, 0, 1, crate::i18n::Lang::En);
        w.journal.clear();
        // Six events that stand alone, three a day: rows newest first are
        // entries 5, 4, 3 (the second day) and 2, 1, 0 (the first).
        for i in 0..6u64 {
            w.journal.push(Entry {
                tick: if i < 3 { 0 } else { 7200 },
                event: Event::Failed { agent: 0 },
            });
        }
        let rows = journal_rows(&w);
        assert_eq!(
            rows.iter().map(|r| r.0).collect::<Vec<_>>(),
            [5, 4, 3, 2, 1, 0]
        );
        let today = day(7300);
        assert_eq!(
            first_shown(&w, &rows, None, 4, today),
            0,
            "nothing selected"
        );
        assert_eq!(first_shown(&w, &rows, Some(1), 4, today), 0, "fits");
        // Row 4 selected in 4 rows: rows 2, 3, 4 and the keys would fit,
        // but the day row above row 3 takes one more.
        assert_eq!(first_shown(&w, &rows, Some(4), 4, today), 3);
        assert_eq!(
            first_shown(&w, &rows, Some(2), 4, today),
            0,
            "no day row before it"
        );
        // Row 3 is the first of the earlier day: rows 2, its day row, 3 and
        // the keys.
        assert_eq!(first_shown(&w, &rows, Some(3), 4, today), 2);
        // Tall enough for everything but the day row.
        assert_eq!(first_shown(&w, &rows, Some(5), 7, today), 1);
        assert_eq!(first_shown(&w, &rows, Some(5), 8, today), 0);
        // Four rows for the last one: rows 3, 4, 5 and the keys would be
        // it, but the day row above row 3 (row 2 is of the other day)
        // takes one more. The day above the first shown is row 2's, not
        // row 3's own.
        assert_eq!(first_shown(&w, &rows, Some(5), 4, today), 4);
        // The newest line is not of today: a day row stands above it too.
        let today = day(7300) + 1;
        assert_eq!(first_shown(&w, &rows, Some(5), 8, today), 1);
        assert_eq!(first_shown(&w, &rows, Some(5), 9, today), 0);
    }

    #[test]
    fn only_neighbouring_lines_of_one_conversation_fold() {
        let journal = [
            sent(0, 1),
            sent(1, 0),
            sent(0, 1),
            sent(0, 2),
            Entry {
                tick: 0,
                event: Event::Failed { agent: 0 },
            },
            sent(2, 0),
            sent(0, 2),
        ];
        let none = BTreeSet::new();
        let folded: Vec<usize> = fold_threads(journal.iter().enumerate(), &none)
            .map(|(_, n)| n)
            .collect();
        // Both directions are one conversation; another pair or any other
        // event ends the fold.
        assert_eq!(folded, [2, 0, 0, 1]);
        // A marked line keeps a row of its own, and may fold the ones
        // after it.
        let marks = BTreeSet::from([1]);
        let rows: Vec<(usize, usize)> = fold_threads(journal.iter().enumerate(), &marks).collect();
        assert_eq!(rows, [(0, 0), (1, 1), (3, 0), (4, 0), (5, 1)]);
        let first = BTreeSet::from([0]);
        let rows: Vec<(usize, usize)> = fold_threads(journal.iter().enumerate(), &first).collect();
        assert_eq!(rows[0], (0, 2), "a marked line folds what follows it");
    }

    #[test]
    fn a_card_s_journal_is_what_names_it_and_what_was_said_with_its_number() {
        let (mut w, _) = crate::sim::world(1, 2, 0, 1, crate::i18n::Lang::En);
        w.journal.clear();
        let said = |from, to, text: &str| Event::Sent {
            from,
            to,
            kind: MsgKind::Ask,
            text: text.into(),
            id: 0,
            re: None,
        };
        // Card 2 is #3.
        for event in [
            Event::Posted { agent: 0, card: 2 },
            Event::Posted { agent: 0, card: 3 },
            Event::Assign { agent: 1, card: 2 },
            said(1, 2, "#30 is another"),
            said(1, 2, "about #3"),
            said(2, 1, "#3, yes"),
            Event::Failed { agent: 1 },
            Event::Claimed { agent: 1, card: 2 },
            Event::Merged { agent: 1, card: 2 },
            Event::Merged { agent: 1, card: 1 },
        ] {
            w.journal.push(Entry { tick: 0, event });
        }
        // Newest first; the conversation's two lines fold into one.
        assert_eq!(card_rows(&w, 2), [(8, 0), (7, 0), (5, 1), (2, 0), (0, 0)]);
        assert_eq!(card_rows(&w, 3), [(1, 0)]);
        assert!(card_rows(&w, 4).is_empty());
    }

    #[test]
    fn a_card_s_number_is_marked_where_it_is_named() {
        let base = Style::new().fg(Color::Indexed(3));
        let line = Line::from(vec![
            Span::styled("#3 and #30, #3", base),
            Span::raw("none"),
            Span::raw("#3"),
        ])
        .style(Style::new().add_modifier(Modifier::REVERSED));
        let out = marked(line, 2);
        let mark = bold(pal().accent);
        let spans: Vec<(&str, Style)> = out
            .spans
            .iter()
            .map(|s| (s.content.as_ref(), s.style))
            .collect();
        assert_eq!(
            spans,
            [
                ("#3", mark),
                (" and #30, ", base),
                ("#3", mark),
                ("none", Style::new()),
                ("#3", mark),
            ]
        );
        assert_eq!(out.style, Style::new().add_modifier(Modifier::REVERSED));
        let tail = marked(Line::from(Span::styled("x #3 y", base)), 2);
        let texts: Vec<&str> = tail.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(texts, ["x ", "#3", " y"]);
        assert_eq!(tail.spans[2].style, base);
    }
}
