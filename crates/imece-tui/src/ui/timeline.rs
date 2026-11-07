//! The timeline (record 100), view `4`: one lane per agent of the open
//! projects, time going right to now, each column what the agent did then
//! (writing, reading or running, thinking, asking a teammate, waiting for
//! the person, stuck, a turn's end, idle), and under each project how many
//! messages its agents sent. What works side by side, who sat idle and
//! where something got stuck, and since when, read at a glance.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::*;
use crate::app::App;

/// Ticks a column stands for, at each scale `[` and `]` step through:
/// 5 s, 30 s, a minute.
pub(crate) const SCALES: [usize; 3] = [1, 6, 12];

/// Columns of a row's name part: its glyph, its indent and its name.
const NAME: usize = 20;

/// Columns of a row's status, after its lane.
const STATUS: usize = 28;

/// A lane is never narrower than this; past it the status goes.
const LANE_MIN: usize = 10;

/// The agents the timeline shows, in the salon's order of projects: each
/// open project's leader, its workers and each worker's helpers, as
/// (agent, depth); none gone, settled or of a project put away.
pub(crate) fn agents(app: &App) -> Vec<(AgentId, u8)> {
    let w = &app.world;
    let here = |a: &Agent| !a.gone && !app.settled.contains(&a.id);
    let mut out = Vec::new();
    for p in w.salon_order() {
        let project = &w.projects[p];
        if project.closed || app.settled_projects.contains(&p) {
            continue;
        }
        let leader = &w.agents[project.leader];
        if here(leader) {
            out.push((leader.id, 0));
        }
        let of = |parent: AgentId| {
            (w.agents.iter()).filter(move |a| a.parent == Some(parent) && a.role == Role::Helper)
        };
        for h in of(leader.id).filter(|h| here(h)) {
            out.push((h.id, 1));
        }
        for a in (w.agents.iter()).filter(|a| a.project == p && a.role == Role::Worker && here(a)) {
            out.push((a.id, 0));
            for h in of(a.id).filter(|h| here(h)) {
                out.push((h.id, 1));
            }
        }
    }
    out
}

/// The glyph and look of a column that saw `cell`.
pub(crate) fn cell_glyph(cell: LaneCell) -> (char, Style) {
    match cell {
        LaneCell::Gap => (' ', Style::new()),
        LaneCell::Idle => ('·', dim()),
        LaneCell::Starting => ('●', fg(pal().ok)),
        LaneCell::Thinking => ('░', fg(pal().ok)),
        LaneCell::Reading => ('▒', fg(pal().ok)),
        LaneCell::Writing => ('█', fg(pal().ok)),
        LaneCell::Asking => ('?', fg(pal().info)),
        LaneCell::Done => ('✓', bold(pal().ok)),
        LaneCell::Waiting => ('⚑', fg(pal().warn)),
        LaneCell::Stuck => ('■', fg(pal().bad)),
    }
}

/// `n` columns of `per` ticks each up to `tick`, the oldest first: each
/// the most the lane saw in its ticks.
pub(crate) fn columns(lane: &Lane, tick: u64, n: usize, per: usize) -> Vec<LaneCell> {
    let cells = lane.at(tick, n * per);
    cells
        .chunks(per)
        .map(|c| c.iter().copied().max().unwrap_or_default())
        .collect()
}

/// How many messages project `p`'s agents sent in each of `n` columns of
/// `per` ticks up to `tick`, the oldest first. The journal is read back
/// from its newest line to the first column's start only.
pub(crate) fn messages(w: &World, p: ProjectId, tick: u64, n: usize, per: usize) -> Vec<u16> {
    let mut counts = vec![0u16; n];
    let span = (n * per) as u64;
    for e in w.journal.iter().rev() {
        let back = tick.saturating_sub(e.tick);
        if back >= span {
            break;
        }
        if let Event::Sent { from, .. } = e.event
            && w.agents[from].project == p
        {
            let column = n - 1 - (back as usize) / per;
            counts[column] = counts[column].saturating_add(1);
        }
    }
    counts
}

/// The lane's part of a row in `width` columns, from the lane's columns.
pub(crate) fn lane_spans(cells: &[LaneCell]) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run = String::new();
    let mut look = None;
    for &c in cells {
        let (g, style) = cell_glyph(c);
        if look != Some(style) && !run.is_empty() {
            spans.push(Span::styled(
                std::mem::take(&mut run),
                look.unwrap_or_default(),
            ));
        }
        look = Some(style);
        run.push(g);
    }
    if !run.is_empty() {
        spans.push(Span::styled(run, look.unwrap_or_default()));
    }
    spans
}

/// The time under a project's name: the first column's clock at the left,
/// the middle one's, and `now` at the right, in `width` columns.
pub(crate) fn axis(lang: Lang, tick: u64, width: usize, per: usize) -> String {
    let mut row = vec![' '; width];
    let at = |column: usize| tick.saturating_sub(((width - 1 - column) * per) as u64);
    let now = lang.text().timeline_now;
    let mut put = |column: usize, words: &str| {
        for (k, ch) in words.chars().enumerate() {
            if let Some(slot) = row.get_mut(column + k) {
                *slot = ch;
            }
        }
    };
    if width >= 24 {
        put(0, &clock(at(0)));
        put(width / 2 - 2, &clock(at(width / 2 - 2)));
    }
    put(width.saturating_sub(now.chars().count()), now);
    row.into_iter().collect()
}

/// How wide a lane is in a list `width` columns wide, and whether the
/// status fits after it: the name, the lane and the status with a space
/// between each; on a narrower list the status goes and the lane takes the
/// rest.
pub(crate) fn lane_width(width: usize) -> (usize, bool) {
    if width >= NAME + LANE_MIN + STATUS + 2 {
        (width - NAME - STATUS - 2, true)
    } else {
        (width.saturating_sub(NAME + 1).max(1), false)
    }
}

pub(crate) fn draw(f: &mut Frame, area: Rect, app: &App) {
    let w = &app.world;
    let t = app.lang.text();
    let per = SCALES[app.timeline_scale.min(SCALES.len() - 1)];
    let title = fill(
        t.timeline_title,
        &[t.timeline_scales[app.timeline_scale.min(SCALES.len() - 1)]],
    );
    let block = Block::bordered()
        .title(title)
        .border_style(fg(pal().accent));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [list, legend] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
    let (lane, with_status) = lane_width(usize::from(list.width));
    let rungs = app.rungs();
    let shown = agents(app);
    let sel = app.timeline_sel.min(shown.len().saturating_sub(1));

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut at_sel = None;
    let mut last_project = None;
    for (n, &(id, depth)) in shown.iter().enumerate() {
        let a = &w.agents[id];
        if last_project != Some(a.project) {
            if let Some(p) = last_project {
                lines.push(message_line(app, p, lane, per));
            }
            let name = fit(&w.projects[a.project].name, NAME - 1);
            lines.push(Line::from(vec![
                Span::styled(format!("{name} "), bold(pal().strong)),
                Span::styled(axis(app.lang, w.tick, lane, per), dim()),
            ]));
            last_project = Some(a.project);
        }
        let (g, color) = rung_glyph(a, rungs[id]);
        let indent = if depth > 0 { "  " } else { "" };
        let mut spans = vec![
            Span::styled(format!("{g} "), fg(color)),
            Span::styled(
                fit(&format!("{indent}{}", clean(&a.name)), NAME - 3),
                fg(pal().agent),
            ),
            Span::raw(" "),
        ];
        spans.extend(lane_spans(&columns(&a.lane, w.tick, lane, per)));
        if with_status {
            spans.push(Span::raw("  "));
            spans.extend(status_spans(app.lang, w, a));
        }
        let line = Line::from(spans);
        if n == sel {
            at_sel = Some(lines.len());
            lines.push(line.style(Style::new().add_modifier(Modifier::REVERSED)));
        } else {
            lines.push(line);
        }
    }
    if let Some(p) = last_project {
        lines.push(message_line(app, p, lane, per));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(t.timeline_none, dim())));
    }
    let height = usize::from(list.height);
    let skip = at_sel.map_or(0, |n| (n + 1).saturating_sub(height));
    let lines: Vec<Line<'static>> = lines.into_iter().skip(skip).collect();
    f.render_widget(Paragraph::new(lines), list);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(t.timeline_legend, dim()))),
        legend,
    );
}

/// Under a project's lanes: how many messages its agents sent, a column
/// each, on the activity bar's scale.
fn message_line(app: &App, p: ProjectId, lane: usize, per: usize) -> Line<'static> {
    let w = &app.world;
    let counts = messages(w, p, w.tick, lane, per);
    let bar: String = counts
        .iter()
        .map(|&n| super::salon::stir_glyph(n))
        .collect();
    Line::from(vec![
        Span::styled(fit(app.lang.text().timeline_messages, NAME), dim()),
        Span::styled(bar, fg(pal().info)),
    ])
}
