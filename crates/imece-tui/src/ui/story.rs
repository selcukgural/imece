//! The story beside the salon (record 99): what happened, told by card,
//! newest first, in the journal pane's place; the raw journal is `J`
//! away. Each card the journal lately spoke of is a block: its header
//! (number, title, owner, how far it is), its newest lines (an ask that
//! got no answer says so) and what its owner does now. Lines of no card
//! (the leader's planning, a worktree's setup) are folded by kind under
//! `elsewhere`, each kind once with how many times it happened. Above
//! all, whom the person holds up: every agent that waits for them, and
//! every agent that waits for one of those.

use std::collections::BTreeSet;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::*;
use crate::app::App;

/// The newest lines a card's block keeps.
pub(crate) const STORY_LINES: usize = 3;

/// How far back the story reads the journal in one frame, at most: a
/// long journal costs no more than this.
pub(crate) const STORY_BACK: usize = 2_000;

/// Kinds of line of no card the story keeps.
pub(crate) const STORY_OTHER: usize = 4;

/// What a row of the story stands for, when it can be selected: a card's
/// header (`enter` opens the card's screen) or a line of no card (`enter`
/// shows it in the journal).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Item {
    Card(CardId),
    Line(usize),
}

/// A row of the story as drawn, and what it stands for.
pub(crate) struct Row {
    pub line: Line<'static>,
    pub item: Option<Item>,
}

/// A card's block as read from the journal: its lines, newest first.
struct Block {
    card: CardId,
    lines: Vec<usize>,
}

/// A kind of line of no card: its newest line and how many there were.
struct Other {
    newest: usize,
    count: usize,
}

/// Each agent's cards as (when it finished it, or never, the card), in
/// that order: the card an agent was on at a tick is the first it
/// finished then or after, else the one still open (a worker takes one
/// card after the other).
pub(crate) struct Held(Vec<Vec<(u64, CardId)>>);

impl Held {
    pub(crate) fn of(w: &World) -> Held {
        let mut by = vec![Vec::new(); w.agents.len()];
        for c in w.shown_cards().filter(|c| c.column != Column::Waiting) {
            if let Some(cards) = c.owner.and_then(|o| by.get_mut(o)) {
                let done = if c.column == Column::Done {
                    c.finished
                } else {
                    u64::MAX
                };
                cards.push((done, c.id));
            }
        }
        for cards in &mut by {
            cards.sort_unstable();
        }
        Held(by)
    }

    /// The card agent `a` (a helper: its worker) was on at `tick`.
    pub(crate) fn at(&self, w: &World, a: AgentId, tick: u64) -> Option<CardId> {
        let a = w.agents[a].parent.unwrap_or(a);
        (self.0.get(a)?.iter())
            .find(|&&(done, _)| done >= tick)
            .map(|&(_, c)| c)
    }
}

/// The card journal line `e` belongs to in the story: the card it names;
/// for something said, the card of whoever asked (the sender of a new
/// line, the one answered for a reply) when the line was said, else of
/// the other party; for an agent's own news (a change, a stop, a failure,
/// a block, an apply) the card it was on. A line about setting things up
/// belongs to no card.
pub(crate) fn card_for(w: &World, held: &Held, e: &Entry) -> Option<CardId> {
    if let Some(c) = e.event.card() {
        return Some(c);
    }
    let at = |a: AgentId| held.at(w, a, e.tick);
    if let Some(s) = words::said(&e.event) {
        let of = |p: Party| match p {
            Party::Agent(a) => at(a),
            Party::Person => None,
        };
        let (first, second) = if s.re.is_some() {
            (s.to, s.from)
        } else {
            (s.from, s.to)
        };
        return of(first).or_else(|| of(second));
    }
    match e.event.kind() {
        EntryKind::Blocked | EntryKind::Merged | EntryKind::Sensitive => at(e.event.agent()),
        EntryKind::Failed
            if matches!(
                e.event,
                Event::Failed { .. }
                    | Event::Stopped { .. }
                    | Event::Interrupted { .. }
                    | Event::ApplyFailed { .. }
            ) =>
        {
            at(e.event.agent())
        }
        _ => None,
    }
}

/// The agents the person holds up, in order: those something of which
/// waits for the person (an open item, or waiting for them), then, each
/// with the one it waits for, those asking a teammate already held up.
pub(crate) fn held(w: &World) -> Vec<(AgentId, Option<AgentId>)> {
    let mut out: Vec<(AgentId, Option<AgentId>)> = Vec::new();
    let mut seen = BTreeSet::new();
    let here = |a: &Agent| !a.gone && !w.put_away(a.id) && !w.in_archive(a.id);
    for i in w.open_inbox() {
        let a = w.inbox[i].agent;
        if here(&w.agents[a]) && seen.insert(a) {
            out.push((a, None));
        }
    }
    for a in w.agents.iter().filter(|a| here(a)) {
        if a.status == Status::WaitingHuman && seen.insert(a.id) {
            out.push((a.id, None));
        }
    }
    // Each round adds those waiting on one added before; as many rounds
    // as there are agents reach every chain, and none runs forever.
    for _ in 0..w.agents.len() {
        let before = out.len();
        for a in w.agents.iter().filter(|a| here(a)) {
            if let Status::Asking { to, .. } = a.status
                && seen.contains(&to)
                && seen.insert(a.id)
            {
                out.push((a.id, Some(to)));
            }
        }
        if out.len() == before {
            break;
        }
    }
    out
}

/// The story for a pane `height` rows high.
pub(crate) fn rows(app: &App, height: usize) -> Vec<Row> {
    let w = &app.world;
    let t = app.lang.text();
    let mut out = Vec::new();

    let held = held(w);
    if !held.is_empty() {
        let names: Vec<String> = held
            .iter()
            .map(|&(a, via)| match via {
                Some(v) => fill(
                    t.story_via,
                    &[&clean(&w.agents[a].name), &clean(&w.agents[v].name)],
                ),
                None => clean(&w.agents[a].name),
            })
            .collect();
        out.push(Row {
            line: Line::from(Span::styled(
                fill(t.story_holds, &[&held.len().to_string(), &names.join(", ")]),
                attention(),
            )),
            item: None,
        });
    }

    // One pass back through the journal, `STORY_BACK` lines at most: the
    // cards in the order their newest line came (as many as fill the
    // pane), their newest lines, the kinds of the rest.
    let mut blocks: Vec<Block> = Vec::new();
    let mut others: Vec<Other> = Vec::new();
    let mut kinds: Vec<std::mem::Discriminant<Event>> = Vec::new();
    let mut answered: BTreeSet<MsgId> = BTreeSet::new();
    let mut unanswered: BTreeSet<usize> = BTreeSet::new();
    let enough = height / 2 + 1;
    let cards = Held::of(w);
    for (i, e) in w.journal.iter().enumerate().rev().take(STORY_BACK) {
        if w.in_archive(e.event.agent())
            || w.put_away(e.event.agent())
            || (i < w.cleared.journal && !w.marks.contains(&i))
        {
            continue;
        }
        // Lines come newest first: a reply is seen before its ask.
        if let Some(s) = words::said(&e.event) {
            if let Some(re) = s.re {
                answered.insert(re);
            }
            if s.kind == Some(MsgKind::Ask) && s.id > 0 && !answered.contains(&s.id) {
                unanswered.insert(i);
            }
        }
        match card_for(w, &cards, e) {
            Some(c) => match blocks.iter().position(|b| b.card == c) {
                Some(at) if blocks[at].lines.len() < STORY_LINES => blocks[at].lines.push(i),
                Some(_) => {}
                None if blocks.len() < enough => blocks.push(Block {
                    card: c,
                    lines: vec![i],
                }),
                None => {}
            },
            None if e.event.kind() == EntryKind::Assign => {}
            None => {
                let d = std::mem::discriminant(&e.event);
                match kinds.iter().position(|k| *k == d) {
                    Some(at) => others[at].count += 1,
                    None if others.len() < STORY_OTHER => {
                        kinds.push(d);
                        others.push(Other {
                            newest: i,
                            count: 1,
                        });
                    }
                    None => {}
                }
            }
        }
    }

    for b in &blocks {
        out.push(Row {
            line: card_head(app, b.card),
            item: Some(Item::Card(b.card)),
        });
        for &i in b.lines.iter().rev() {
            let mut line = story_line(app, &w.journal[i]);
            line.spans.insert(0, Span::raw("  "));
            if unanswered.contains(&i) {
                let seconds = w.tick.saturating_sub(w.journal[i].tick) * SECONDS_PER_TICK;
                let wait = match seconds {
                    0..60 => t.story_unanswered_new.to_string(),
                    _ => fill(t.story_unanswered, &[&words::ago(app.lang, seconds)]),
                };
                line.spans
                    .push(Span::styled(format!("  {wait}"), fg(pal().warn)));
            }
            out.push(Row { line, item: None });
        }
        if let Some(now) = now_line(app, b.card) {
            out.push(Row {
                line: now,
                item: None,
            });
        }
    }
    if !others.is_empty() {
        out.push(Row {
            line: Line::from(Span::styled(t.story_elsewhere, dim())),
            item: None,
        });
        for o in &others {
            let mut line = story_line(app, &w.journal[o.newest]);
            line.spans.insert(0, Span::raw("  "));
            if o.count > 1 {
                line.spans
                    .push(Span::styled(format!("  ×{}", o.count), dim()));
            }
            out.push(Row {
                line,
                item: Some(Item::Line(o.newest)),
            });
        }
    }
    if out.is_empty() {
        out.push(Row {
            line: Line::from(Span::styled(t.story_none, dim())),
            item: None,
        });
    }
    out
}

/// A card's header in the story: `#3 Parse the manifest · claude-1`, then
/// how far it is: done, a change waiting for the person, else its owner's
/// to-do list.
fn card_head(app: &App, c: CardId) -> Line<'static> {
    let w = &app.world;
    let t = app.lang.text();
    let card = &w.cards[c];
    let mut spans = vec![Span::styled(
        format!("#{} {}", c + 1, clean(&card.title)),
        bold(pal().strong),
    )];
    if let Some(o) = card.owner {
        let a = &w.agents[o];
        spans.push(Span::styled(" · ", dim()));
        spans.push(Span::styled(clean(&a.name), fg(vendor_color(a.vendor))));
    }
    let change = (w.open_inbox().into_iter())
        .any(|i| w.inbox[i].kind == InboxKind::Change && w.inbox[i].card == Some(c));
    let done = card.column == Column::Done;
    if done {
        spans.push(Span::styled(format!("  {}", t.story_done), fg(pal().ok)));
    }
    if change {
        let gap = if done { " · " } else { "  " };
        spans.push(Span::styled(gap, dim()));
        spans.push(Span::styled(t.story_change, attention()));
    }
    if !done
        && !change
        && let Some(o) = card.owner
    {
        spans.extend(super::salon::plan_spans(&w.agents[o]));
    }
    Line::from(spans)
}

/// What a card's owner does now, `  now ▸ Run the tests · 2 min`, while the
/// card is not done and its owner is there.
fn now_line(app: &App, c: CardId) -> Option<Line<'static>> {
    let w = &app.world;
    let card = &w.cards[c];
    let o = card.owner.filter(|_| card.column != Column::Done)?;
    let a = &w.agents[o];
    if a.gone || a.card != Some(c) {
        return None;
    }
    let mut spans = vec![Span::styled(
        format!("  {} ", app.lang.text().story_now),
        dim(),
    )];
    spans.extend(status_spans(app.lang, w, a));
    spans.extend(super::salon::since_spans(app, a));
    Some(Line::from(spans))
}

/// A journal line as the story tells it: its time, its glyph and its
/// words, without the project (a block is one card's).
fn story_line(app: &App, e: &Entry) -> Line<'static> {
    let w = &app.world;
    let (icon, style) = entry_icon(e.event.kind());
    let mut spans = vec![
        Span::styled(format!("{} ", clock(e.tick)), dim()),
        Span::styled(format!("{icon} "), style),
    ];
    match words::said(&e.event) {
        Some(said) => spans.extend(said_spans(app.lang, w, &said)),
        None => spans.push(Span::raw(words::event(app.lang, w, &e.event))),
    }
    Line::from(spans)
}

/// `line` as the selected row: reversed.
pub(crate) fn selected(line: Line<'static>) -> Line<'static> {
    line.style(Style::new().add_modifier(Modifier::REVERSED))
}

/// The story in the journal pane's place: its rows, the selected one
/// reversed and kept in view while the pane is focused. What can be
/// selected is left in `App::story_items` for the keys.
pub(crate) fn draw(f: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    let focused = app.pane == crate::app::Pane::Journal;
    let block = super::salon::pane_block(app.lang.text().story.to_string(), focused);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let height = usize::from(inner.height);
    let rows = rows(app, height);
    let items: Vec<Item> = rows.iter().filter_map(|r| r.item).collect();
    let sel = app.story_sel.min(items.len().saturating_sub(1));
    // The row of the selected item, while the pane is focused.
    let at = focused
        .then(|| {
            (rows.iter().enumerate())
                .filter(|(_, r)| r.item.is_some())
                .nth(sel)
                .map(|(n, _)| n)
        })
        .flatten();
    let skip = at.map_or(0, |n| (n + 1).saturating_sub(height));
    let lines: Vec<Line<'static>> = (rows.into_iter().enumerate())
        .skip(skip)
        .take(height)
        .map(|(n, r)| match at == Some(n) {
            true => selected(r.line),
            false => r.line,
        })
        .collect();
    *app.story_items.borrow_mut() = items;
    f.render_widget(ratatui::widgets::Paragraph::new(lines), inner);
}
