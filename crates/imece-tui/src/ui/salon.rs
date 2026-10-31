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
use crate::app::{App, Pane, Sel};

/// Drawn before an agent's name in its speaker colour.
pub(crate) const SPEAKER_MARK: &str = "▎";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// The salon's rows for `capacity` lines. A folded project is one line; a
/// worker in `folded_workers` shows no helpers. In an open project, workers
/// that need the person come first; the rest keep their order so the screen
/// does not jump. When the rows do not fit, calm workers (working or idle,
/// not talking, nothing new) fold into one line per project.
pub fn rows(
    w: &World,
    collapsed: &[bool],
    folded_workers: &BTreeSet<AgentId>,
    selected: Sel,
    capacity: usize,
) -> Vec<Row> {
    let t = w.tick;
    let researcher_of = |a: &Agent| {
        w.agents.iter().find(|r| {
            r.role == Role::Researcher
                && !r.gone
                && matches!(r.status, Status::Researching { for_agent, .. } if for_agent == a.id)
        })
    };
    let helpers_of = |a: &Agent| -> Vec<AgentId> {
        if folded_workers.contains(&a.id) {
            return Vec::new();
        }
        w.helpers_of(a.id).map(|h| h.id).collect()
    };
    let lines = |a: &Agent| {
        1 + usize::from(a.talk.is_some())
            + usize::from(researcher_of(a).is_some())
            + helpers_of(a).len()
    };
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
            if p.closed || collapsed[p.id] {
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
            if p.closed {
                return 0;
            }
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
    for p in w.open_projects() {
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
            for h in helpers_of(a) {
                out.push(Row::Agent { id: h, depth: 3 });
            }
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
    let block = pane_block(app.lang.text().salon.into(), app.pane == Pane::Salon);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [list, queue] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);

    let rows = rows(
        w,
        &app.collapsed,
        &app.folded_workers,
        app.selected,
        usize::from(list.height),
    );
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
                agent_line(app, &w.agents[id], prefix)
            }
            Row::Talk(id) => {
                let a = &w.agents[id];
                let prefix = match (a.role, under_last) {
                    (Role::Leader, _) => "    ",
                    (_, true) => "      ",
                    (_, false) => "  │   ",
                };
                let (with, color) = match a.talk.as_ref().map(|k| &k.with) {
                    Some(Party::Agent(o)) => (clean(&w.agents[*o].name), speaker_color(*o)),
                    _ => (app.lang.text().you.to_string(), pal().strong),
                };
                let text = a.talk.as_ref().map(|k| k.text.as_str()).unwrap_or_default();
                let talk = Style::new().fg(pal().info).add_modifier(Modifier::ITALIC);
                Line::from(vec![
                    Span::styled(prefix, dim()),
                    Span::styled("⇄ ", talk),
                    Span::styled(with, talk.fg(color)),
                    Span::styled(format!(" \"{}\"", clean(text)), talk),
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
        .filter(|a| matches!(a.role, Role::Worker | Role::Helper) && a.status != Status::Idle)
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
            bold(pal().strong),
        ),
        Span::styled(fit(&project.name, 12), bold(pal().strong)),
    ];
    if let Some(trunk) = project.trunk {
        spans.push(Span::styled(format!(" trunk #{trunk} "), dim()));
        spans.push(Span::styled("✓", fg(pal().ok)));
    }
    spans.push(Span::styled(
        format!("  {}", fill(t.group_working, &[&working.to_string()])),
        dim(),
    ));
    spans.push(Span::styled(format!("  {}", dollars(cost)), dim()));
    if blocked > 0 {
        spans.push(Span::styled(format!("  ✕{blocked}"), fg(pal().bad)));
    }
    if waiting > 0 {
        // A folded project is quiet, not deaf.
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            fill(t.group_waiting, &[&waiting.to_string()]),
            attention(),
        ));
    }
    let line = Line::from(spans);
    if app.pane == Pane::Salon && app.selected == Sel::Project(p) {
        line.style(Style::new().add_modifier(Modifier::REVERSED))
    } else {
        line
    }
}

pub(super) fn agent_line(app: &App, a: &Agent, prefix: &'static str) -> Line<'static> {
    let w = &app.world;
    let (g, color) = glyph(a);
    let mut spans = vec![
        Span::styled(prefix, dim()),
        Span::styled(format!("{g} "), fg(color)),
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
    spans.extend(status_spans(app.lang, w, a));
    if a.cost > 0 {
        spans.push(Span::styled(format!("  {}", dollars(a.cost)), dim()));
    }
    let helpers = w.helpers_of(a.id).count();
    if helpers > 0 {
        // Its helpers are under it, or folded away: the mark says which.
        let mark = if app.folded_workers.contains(&a.id) {
            "▸"
        } else {
            "▾"
        };
        spans.push(Span::styled(
            format!(
                "  {mark} {}",
                fill(app.lang.text().helpers, &[&helpers.to_string()])
            ),
            dim(),
        ));
    }

    let line = Line::from(spans);
    let recent = w.tick > 4 && w.tick.saturating_sub(a.changed_at) <= 4;
    if app.pane == Pane::Salon && app.selected == Sel::Agent(a.id) {
        line.style(Style::new().add_modifier(Modifier::REVERSED))
    } else if recent {
        // A change is a signal: it stands out for a moment, then calms down.
        line.style(changed())
    } else {
        line
    }
}

fn queue_line(app: &App) -> Line<'static> {
    let w = &app.world;
    let t = app.lang.text();
    let mut spans = vec![Span::styled(t.queue, bold(pal().strong))];
    let verifying: Vec<&Card> = w
        .cards
        .iter()
        .filter(|c| c.column == Column::Verifying)
        .collect();
    if verifying.is_empty() {
        spans.push(Span::styled(t.queue_empty, dim()));
    }
    for c in verifying {
        spans.push(Span::styled(format!(" #{} ◌", c.id + 1), fg(pal().info)));
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
            fill(t.inbox_title, &[&open.len().to_string()]),
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
        lines.push(Line::from(Span::styled(t.nothing_needed, dim())));
    }
    for (n, &i) in open.iter().enumerate() {
        let item = &w.inbox[i];
        let line = inbox_line(app, i);
        if focused && n == app.inbox_sel {
            lines.push(line.style(Style::new().add_modifier(Modifier::REVERSED)));
            lines.extend(whole.iter().cloned());
            let keys = match item.kind {
                InboxKind::Reply => t.reply_keys,
                InboxKind::Change => t.change_keys,
                InboxKind::Approval => t.approval_keys,
                _ => t.item_keys,
            };
            lines.push(Line::from(Span::styled(keys, dim())));
        } else {
            lines.push(line);
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
        let mut line = entry_line(app, &w.journal[i]);
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
/// assignments are visible on the board and in the leader's trace.
pub(crate) fn journal_rows(w: &World) -> Vec<(usize, usize)> {
    let shown = w
        .journal
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, e)| e.event.kind() != EntryKind::Assign);
    fold_threads(shown).collect()
}

/// Card `c`'s journal as its screen shows it, newest first, folded like
/// [`journal_rows`]: the lines that name the card (posted, claimed,
/// assigned, merged) and what was said with its number (`#323`) in it.
pub(crate) fn card_rows(w: &World, c: CardId) -> Vec<(usize, usize)> {
    let shown = w.journal.iter().enumerate().rev().filter(|(_, e)| {
        e.event.card() == Some(c)
            || words::said(&e.event).is_some_and(|s| !mentions(s.text, c).is_empty())
    });
    fold_threads(shown).collect()
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

/// Each entry with how many older lines of the same conversation directly
/// follow it and are folded under it. Display only: the agents' stories
/// keep every line.
fn fold_threads<'a>(
    entries: impl Iterator<Item = (usize, &'a Entry)>,
) -> impl Iterator<Item = (usize, usize)> {
    let mut entries = entries.peekable();
    std::iter::from_fn(move || {
        let (i, e) = entries.next()?;
        let mut more = 0;
        if let Some(pair) = words::said(&e.event).map(|s| s.pair()) {
            while entries
                .next_if(|(_, n)| words::said(&n.event).is_some_and(|s| s.pair() == pair))
                .is_some()
            {
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
            },
        }
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
        let folded: Vec<usize> = fold_threads(journal.iter().enumerate())
            .map(|(_, n)| n)
            .collect();
        // Both directions are one conversation; another pair or any other
        // event ends the fold.
        assert_eq!(folded, [2, 0, 0, 1]);
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
