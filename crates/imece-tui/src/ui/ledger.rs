//! The cost ledger's window (`:ledger`, record 133): per open project a
//! line of what it spent, kept apart from wasted, at work and the
//! leader's, then its attempts newest first, one row each; the card's
//! screen lists its own the same way.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::*;
use crate::app::App;
use crate::i18n::Text;
use crate::ledger::{Attempt, Outcome};
use crate::model::{ProjectId, SECONDS_PER_TICK, World, date, day};
use std::collections::BTreeMap;

/// The rows the window lists, by project in the salon's order: each open
/// project with its rows, newest first; a project nothing was spent in
/// still says so.
pub fn listed(w: &World) -> Vec<(ProjectId, Vec<usize>)> {
    (w.salon_order().into_iter())
        .map(|p| (p, w.ledger_of(p)))
        .collect()
}

/// The ledger rows the window can be at, in its order.
pub fn rows(w: &World) -> Vec<usize> {
    listed(w).into_iter().flat_map(|(_, rows)| rows).collect()
}

/// How an attempt ended, in words.
pub fn outcome(t: &Text, o: Outcome) -> &'static str {
    let i = match o {
        Outcome::Open => 0,
        Outcome::Applied => 1,
        Outcome::Done => 2,
        Outcome::Failed => 3,
        Outcome::Withdrawn => 4,
        Outcome::HandedOn => 5,
        Outcome::Late => 6,
    };
    t.ledger_outcomes[i]
}

/// Project `p`'s figures in a line.
pub fn totals(lang: Lang, w: &World, p: ProjectId) -> String {
    let t = lang.text();
    let s = w.totals(p);
    let parts = [s.kept, s.wasted, s.open, s.lead];
    let words: Vec<String> = (0..4).map(|i| w.money(parts[i], s.unpriced[i])).collect();
    let refs: Vec<&str> = words.iter().map(String::as_str).collect();
    fill(t.ledger_totals, &refs)
}

/// One attempt's row: the card and try (or no card), the worker, the
/// model, the changes and those back, what it cost, how long it took, how
/// it ended, the day and Claude Code's version.
pub fn row_spans(lang: Lang, w: &World, r: &Attempt) -> Vec<Span<'static>> {
    let t = lang.text();
    let which = match r.card {
        Some(c) => fill(
            t.ledger_try,
            &[&(c + 1).to_string(), &r.attempt.to_string()],
        ),
        None => t.ledger_no_card.to_string(),
    };
    let mut spans = vec![Span::styled(format!("{which}  "), dim())];
    spans.push(Span::styled(
        w.agents[r.agent].name.clone(),
        fg(pal().agent),
    ));
    if !r.model.is_empty() {
        spans.push(Span::styled(format!("  {}", clean(&r.model)), dim()));
    }
    let mut rounds = fill(t.ledger_rounds, &[&r.rounds.to_string()]);
    if r.sent_back > 0 {
        rounds.push_str(&fill(t.ledger_back, &[&r.sent_back.to_string()]));
    }
    spans.push(Span::styled(format!("  {rounds}"), dim()));
    spans.push(Span::raw(format!("  {}", w.money(r.cost, r.unpriced))));
    spans.push(Span::styled(
        format!("  {}", lasted(lang, r.ticks(w.tick))),
        dim(),
    ));
    let style = match r.outcome {
        Outcome::Open => fg(pal().info),
        o if o.kept() => fg(pal().ok),
        _ => fg(pal().warn),
    };
    spans.push(Span::styled(format!("  {}", outcome(t, r.outcome)), style));
    let mut when = date(day(r.ended));
    if let Some(v) = r
        .version
        .as_deref()
        .and_then(|v| v.split_whitespace().next())
    {
        when.push_str(&format!(" · {}", clean(v)));
    }
    spans.push(Span::styled(format!("  {when}"), dim()));
    spans
}

/// What `models` spent, model by model (record 134), most first, and
/// `unsplit`, what no model's figure said, when a cent or more: `by
/// model: claude-opus-5-5 $0.70 · claude-haiku-5-5 $0.12 · not split
/// $0.02`; none when no model's figure was said.
pub fn by_model(
    lang: Lang,
    w: &World,
    models: &BTreeMap<String, u64>,
    unsplit: u64,
) -> Option<String> {
    if models.is_empty() {
        return None;
    }
    let t = lang.text();
    let mut by: Vec<(&String, &u64)> = models.iter().collect();
    by.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    let mut parts: Vec<String> = (by.into_iter())
        .map(|(model, &cost)| format!("{} {}", clean(model), w.money(cost, false)))
        .collect();
    if unsplit >= 100 {
        parts.push(fill(t.ledger_unsplit, &[&w.money(unsplit, false)]));
    }
    Some(fill(t.ledger_by_model, &[&parts.join(" · ")]))
}

/// How long something took, `ticks` long: whole seconds under a minute,
/// else as [`words::ago`] says it.
pub fn lasted(lang: Lang, ticks: u64) -> String {
    let seconds = ticks * SECONDS_PER_TICK;
    match seconds < 60 {
        true => fill(lang.text().took_s, &[&seconds.to_string()]),
        false => words::ago(lang, seconds),
    }
}

/// The window, `at` the row at hand among [`rows`], over the shaded
/// screen: its title counts the attempts and says when its figures are
/// API-equivalent; the row at hand stays in view; its keys at the foot.
pub fn window(f: &mut Frame, app: &App, at: usize) -> Rect {
    let full = f.area();
    let area = overlay::centered(
        full,
        full.width.saturating_sub(8),
        full.height.saturating_sub(4),
    );
    shade(f, area);
    clear(f, area);
    let t = app.lang.text();
    let w = &app.world;
    let listed = listed(w);
    let count: usize = listed.iter().map(|(_, r)| r.len()).sum();
    let mut title = fill(t.ledger_title, &[&count.to_string()]);
    if w.estimated {
        title.push_str(t.ledger_estimated);
    }
    let block = Block::bordered()
        .title(title)
        .border_style(fg(pal().accent));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let mut lines = Vec::new();
    let mut n = 0;
    let mut at_line = 0;
    for (p, rows) in &listed {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        lines.push(Line::from(vec![
            Span::styled(clean(&w.projects[*p].name), bold(pal().fg)),
            Span::styled(format!("  {}", totals(app.lang, w, *p)), dim()),
        ]));
        let (models, unsplit) = w.models_of(*p);
        if let Some(said) = by_model(app.lang, w, &models, unsplit) {
            lines.push(Line::from(Span::styled(format!("  {said}"), dim())));
        }
        if rows.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("  {}", t.ledger_none),
                dim(),
            )));
        }
        for &i in rows {
            let here = n == at;
            if here {
                at_line = lines.len();
            }
            let mut spans = vec![Span::raw(if here { "▶ " } else { "  " })];
            spans.extend(row_spans(app.lang, w, &w.ledger[i]));
            let line = Line::from(spans);
            lines.push(match here {
                true => line.style(bold(pal().fg)),
                false => line,
            });
            // The row at hand says what each model spent of it.
            let r = &w.ledger[i];
            if here && let Some(said) = by_model(app.lang, w, &r.models, r.unsplit()) {
                at_line = lines.len();
                lines.push(Line::from(Span::styled(format!("    {said}"), dim())));
            }
            n += 1;
        }
    }
    let [body, keys] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
    // The row at hand stays on the window: the rows above it scroll away.
    let room = usize::from(body.height).max(1);
    let skip = (at_line + 1).saturating_sub(room);
    f.render_widget(Paragraph::new(lines).scroll((skip as u16, 0)), body);
    f.render_widget(
        Paragraph::new(Line::from(hint_spans(&t.ledger_hints))),
        keys,
    );
    area
}
