//! The default view: every project as a group that folds (its leader,
//! workers and their researchers), who talks to whom right now, what waits
//! for the person, and the journal of meaningful events.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::*;
use crate::app::{App, Pane, Sel};

pub enum Row {
    Project(ProjectId),
    Agent {
        id: AgentId,
        depth: u8,
    },
    Talk(AgentId),
    /// Calm workers of a project folded into one line.
    Folded(usize),
}

/// The salon's rows for `capacity` lines. A folded project is one line. In
/// an open project, workers that need the person come first; the rest keep
/// their order so the screen does not jump. When the rows do not fit, calm
/// workers (working or idle, not talking, nothing new) fold into one line per
/// project.
pub fn rows(w: &World, collapsed: &[bool], selected: Sel, capacity: usize) -> Vec<Row> {
    let t = w.tick;
    let researcher_of = |a: &Agent| {
        w.agents.iter().find(|r| {
            r.role == Role::Researcher
                && !r.gone
                && matches!(r.status, Status::Researching { for_agent, .. } if for_agent == a.id)
        })
    };
    let lines =
        |a: &Agent| 1 + usize::from(a.talk.is_some()) + usize::from(researcher_of(a).is_some());
    let calm = |a: &Agent| {
        matches!(a.status, Status::Working | Status::Idle)
            && a.talk.is_none()
            && t.saturating_sub(a.changed_at) > 6
            && selected != Sel::Agent(a.id)
    };

    // Workers of each open project, in display order.
    let order: Vec<Vec<&Agent>> = w
        .projects
        .iter()
        .map(|p| {
            if collapsed[p.id] {
                return Vec::new();
            }
            let workers: Vec<&Agent> = w
                .agents
                .iter()
                .filter(|a| a.project == p.id && a.role == Role::Worker)
                .collect();
            let mut o: Vec<&Agent> = workers
                .iter()
                .copied()
                .filter(|a| a.status.needs_attention())
                .collect();
            o.extend(
                workers
                    .iter()
                    .copied()
                    .filter(|a| !a.status.needs_attention()),
            );
            o
        })
        .collect();

    let mut total: usize = w
        .projects
        .iter()
        .map(|p| {
            if collapsed[p.id] {
                return 1;
            }
            let leader = &w.agents[p.leader];
            2 + usize::from(leader.talk.is_some())
                + order[p.id].iter().map(|a| lines(a)).sum::<usize>()
        })
        .sum();
    let mut folded = vec![false; w.agents.len()];
    let mut nfolded = vec![0usize; w.projects.len()];
    if total > capacity {
        'fold: for p in (0..w.projects.len()).rev() {
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
    for p in &w.projects {
        out.push(Row::Project(p.id));
        if collapsed[p.id] {
            continue;
        }
        out.push(Row::Agent {
            id: p.leader,
            depth: 1,
        });
        if w.agents[p.leader].talk.is_some() {
            out.push(Row::Talk(p.leader));
        }
        for a in order[p.id].iter().filter(|a| !folded[a.id]) {
            out.push(Row::Agent { id: a.id, depth: 2 });
            if let Some(r) = researcher_of(a) {
                out.push(Row::Agent { id: r.id, depth: 3 });
            }
            if a.talk.is_some() {
                out.push(Row::Talk(a.id));
            }
        }
        if nfolded[p.id] > 0 {
            out.push(Row::Folded(nfolded[p.id]));
        }
    }
    out
}

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let (left, right) = if area.width >= 100 {
        let [l, r] = Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)])
            .areas(area);
        (l, r)
    } else {
        let [l, r] = Layout::vertical([Constraint::Min(6), Constraint::Length(12)]).areas(area);
        (l, r)
    };
    salon(f, left, app);
    side(f, right, app);
}

fn pane_block(title: String, focused: bool) -> Block<'static> {
    let border = if focused {
        Style::new().fg(Color::Indexed(173))
    } else {
        dim()
    };
    Block::bordered().title(title).border_style(border)
}

fn salon(f: &mut Frame, area: Rect, app: &App) {
    let w = &app.world;
    let block = pane_block(app.lang.text().salon.into(), app.pane == Pane::Salon);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [list, queue] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);

    let rows = rows(w, &app.collapsed, app.selected, usize::from(list.height));
    *app.shown.borrow_mut() = rows
        .iter()
        .filter_map(|r| match r {
            Row::Project(p) => Some(Sel::Project(*p)),
            Row::Agent { id, .. } => Some(Sel::Agent(*id)),
            _ => None,
        })
        .collect();

    // Whether a worker (or folded) line is the last branch of its project.
    let mut last = vec![false; rows.len()];
    let mut seen = false;
    for (i, row) in rows.iter().enumerate().rev() {
        match row {
            Row::Project(_) => seen = false,
            Row::Agent { depth: 2, .. } | Row::Folded(_) if !seen => {
                last[i] = true;
                seen = true;
            }
            _ => {}
        }
    }

    let mut under_last = false;
    let mut lines = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let line = match *row {
            Row::Project(p) => project_line(app, p),
            Row::Agent { id, depth } => {
                let prefix = match depth {
                    1 => "  ",
                    2 => {
                        under_last = last[i];
                        if last[i] { "  └ " } else { "  ├ " }
                    }
                    _ => {
                        if under_last {
                            "    └ "
                        } else {
                            "  │ └ "
                        }
                    }
                };
                agent_line(app, &w.agents[id], prefix)
            }
            Row::Talk(id) => {
                let a = &w.agents[id];
                let prefix = match (a.role, under_last) {
                    (Role::Leader, _) => "    ",
                    (_, true) => "      ",
                    (_, false) => "  │   ",
                };
                let with = match a.talk.as_ref().map(|k| &k.with) {
                    Some(Party::Agent(o)) => w.agents[*o].name.clone(),
                    _ => app.lang.text().you.to_string(),
                };
                let text = a.talk.as_ref().map(|k| k.text.as_str()).unwrap_or_default();
                Line::from(vec![
                    Span::styled(prefix, dim()),
                    Span::styled(
                        fill("⇄ {} \"{}\"", &[&with, text]),
                        Style::new().fg(Color::Cyan).add_modifier(Modifier::ITALIC),
                    ),
                ])
            }
            Row::Folded(n) => Line::from(vec![
                Span::styled("  └ ", dim()),
                Span::styled(fill(app.lang.text().folded, &[&n.to_string()]), dim()),
            ]),
        };
        lines.push(line);
    }
    f.render_widget(Paragraph::new(lines), list);
    f.render_widget(Paragraph::new(queue_line(app)), queue);
}

/// A project's header: folded or open, its trunk, how many work, what it
/// costs, and (even when folded) how much waits for the person.
fn project_line(app: &App, p: ProjectId) -> Line<'static> {
    let w = &app.world;
    let t = app.lang.text();
    let project = &w.projects[p];
    let open = !app.collapsed[p];
    let in_project = |a: &&Agent| a.project == p && !a.gone;
    let working = w
        .agents
        .iter()
        .filter(in_project)
        .filter(|a| a.role == Role::Worker && a.status != Status::Idle)
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

    let mut spans = vec![
        Span::styled(
            format!("{} ", if open { "▾" } else { "▸" }),
            bold(Color::White),
        ),
        Span::styled(fit(&project.name, 12), bold(Color::White)),
        Span::styled(format!(" trunk #{} ", project.trunk), dim()),
        Span::styled("✓", fg(Color::Green)),
        Span::styled(
            format!("  {}", fill(t.group_working, &[&working.to_string()])),
            dim(),
        ),
        Span::styled(format!("  {}", dollars(cost)), dim()),
    ];
    if blocked > 0 {
        spans.push(Span::styled(format!("  ✕{blocked}"), fg(Color::Red)));
    }
    if waiting > 0 {
        // A folded project is quiet, not deaf.
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            fill(t.group_waiting, &[&waiting.to_string()]),
            Style::new()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }
    let line = Line::from(spans);
    if app.pane == Pane::Salon && app.selected == Sel::Project(p) {
        line.style(Style::new().add_modifier(Modifier::REVERSED))
    } else {
        line
    }
}

fn agent_line(app: &App, a: &Agent, prefix: &'static str) -> Line<'static> {
    let w = &app.world;
    let (g, color) = glyph(a);
    let mut spans = vec![
        Span::styled(prefix, dim()),
        Span::styled(format!("{g} "), fg(color)),
        Span::styled(fit(&a.name, 16), bold(vendor_color(a.vendor))),
        taint_span(a),
    ];
    match a.role {
        Role::Leader => spans.push(Span::styled(
            fit(&format!(" {}", a.vendor.label()), 27),
            dim(),
        )),
        Role::Researcher => {}
        Role::Worker => {
            let card = match a.card {
                Some(c) => format!(" #{:<3} {}", c + 1, fit(&w.cards[c].title, 21)),
                None => " ".repeat(27),
            };
            spans.push(Span::raw(card));
        }
    }
    spans.push(Span::raw(" "));
    spans.extend(status_spans(app.lang, w, a));
    if a.cost > 0 {
        spans.push(Span::styled(format!("  {}", dollars(a.cost)), dim()));
    }

    let line = Line::from(spans);
    let recent = w.tick > 4 && w.tick.saturating_sub(a.changed_at) <= 4;
    if app.pane == Pane::Salon && app.selected == Sel::Agent(a.id) {
        line.style(Style::new().add_modifier(Modifier::REVERSED))
    } else if recent {
        // A change is a signal: it stands out for a moment, then calms down.
        line.style(Style::new().bg(Color::Indexed(236)))
    } else {
        line
    }
}

fn queue_line(app: &App) -> Line<'static> {
    let w = &app.world;
    let t = app.lang.text();
    let mut spans = vec![Span::styled(t.queue, bold(Color::White))];
    let verifying: Vec<&Card> = w
        .cards
        .iter()
        .filter(|c| c.column == Column::Verifying)
        .collect();
    if verifying.is_empty() {
        spans.push(Span::styled(t.queue_empty, dim()));
    }
    for c in verifying {
        spans.push(Span::styled(format!(" #{} ◌", c.id + 1), fg(Color::Cyan)));
    }
    let mut done: Vec<&Card> = w
        .cards
        .iter()
        .filter(|c| c.column == Column::Done)
        .collect();
    done.sort_by_key(|c| std::cmp::Reverse(c.finished));
    if !done.is_empty() {
        spans.push(Span::styled(t.latest, dim()));
        for c in done.into_iter().take(4) {
            spans.push(Span::styled(format!(" #{} ✓", c.id + 1), fg(Color::Green)));
        }
    }
    Line::from(spans)
}

fn side(f: &mut Frame, area: Rect, app: &App) {
    let w = &app.world;
    let t = app.lang.text();
    let many = w.projects.len() > 1;
    let open = w.open_inbox();
    // One line per item, one more for the selected item's keys.
    let want = (open.len() as u16 + 3).max(3);
    let inbox_h = want.min(area.height / 2).max(3.min(area.height));
    let [top, bottom] =
        Layout::vertical([Constraint::Length(inbox_h), Constraint::Min(0)]).areas(area);

    let focused = app.pane == Pane::Inbox;
    let mut block = pane_block(fill(t.inbox_title, &[&open.len().to_string()]), focused);
    if !open.is_empty() && !focused {
        block = block.border_style(fg(Color::Yellow));
    }
    let inner = block.inner(top);
    f.render_widget(block, top);
    let mut lines = Vec::new();
    if open.is_empty() {
        lines.push(Line::from(Span::styled(t.nothing_needed, dim())));
    }
    for (n, &i) in open.iter().enumerate() {
        let item = &w.inbox[i];
        let (icon, style) = match item.kind {
            InboxKind::Sensitive => ("!", bold(Color::Yellow)),
            InboxKind::Research => ("?", bold(Color::Blue)),
        };
        let mut spans = vec![
            Span::styled(format!("{icon} "), style),
            Span::styled(format!("{} ", clock(item.tick)), dim()),
        ];
        if many {
            let p = w.agents[item.agent].project;
            spans.push(Span::styled(format!("{} ", w.projects[p].name), dim()));
        }
        spans.push(Span::raw(words::inbox(app.lang, w, item)));
        let line = Line::from(spans);
        if focused && n == app.inbox_sel {
            lines.push(line.style(Style::new().add_modifier(Modifier::REVERSED)));
            lines.push(Line::from(Span::styled(t.item_keys, dim())));
        } else {
            lines.push(line);
        }
    }
    f.render_widget(Paragraph::new(lines), inner);

    let block = pane_block(t.journal.into(), false);
    let inner = block.inner(bottom);
    f.render_widget(block, bottom);
    // The journal keeps every event; the screen shows the meaningful ones.
    // Routine assignments are visible on the board and in the leader's trace.
    let lines: Vec<Line> = w
        .journal
        .iter()
        .rev()
        .filter(|e| e.event.kind() != EntryKind::Assign)
        .take(usize::from(inner.height))
        .map(|e| entry_line(app, e))
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}
