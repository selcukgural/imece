//! A finished round as Home says it (record 145): one line above
//! everything else, so that all the work handed out is through is read
//! at once, not worked out row by row.

use super::*;
use crate::round::Finished;

/// The projects Home shows: the one it is narrowed to, else the open
/// ones.
fn in_view(app: &App) -> Vec<ProjectId> {
    match app.at_home.scope {
        Some(p) => vec![p],
        None => app.world.open_projects().map(|q| q.id).collect(),
    }
}

/// What is finished in Home's view; none while a team there has a card
/// open or somebody in a turn.
pub(crate) fn in_home(app: &App) -> Option<Finished> {
    app.world.finished_of(in_view(app).into_iter())
}

/// `n` of something: `one` for one, else `many` with the count.
fn counted_of(n: usize, one: &str, many: &str) -> String {
    match n {
        1 => one.to_string(),
        _ => fill(many, &[&n.to_string()]),
    }
}

/// Home's first lines for what is finished in its view, cut to `width`:
/// `✓ ALL DONE  shop-api · 5 cards · 4 landed · ✕ 1 failed · $4.12 ·
/// 38 min`, under it what is next; `◐ TEAM FINISHED …  2 wait for you`
/// while something of it waits for the person; a blank line after
/// either. Nothing when nothing is finished.
pub(crate) fn banner(app: &App, width: usize) -> Vec<Line<'static>> {
    let Some(f) = in_home(app) else {
        return Vec::new();
    };
    let w = &app.world;
    let t = app.lang.text();
    let r = f.round();
    let (mark, word, colour) = match f {
        Finished::All(_) => ("✓", t.round_all, pal().ok),
        Finished::Team(_) => ("◐", t.round_team, pal().warn),
    };
    let told: Vec<ProjectId> = (in_view(app).into_iter())
        .filter(|&p| w.finished(p).is_some())
        .collect();
    let whose = match told[..] {
        [p] => clean(&w.projects[p].name),
        _ => fill(t.round_projects, &[&told.len().to_string()]),
    };
    let head = vec![
        Span::raw(" "),
        Span::styled(format!("{mark} "), fg(colour)),
        Span::styled(word, bold(if f.all() { colour } else { pal().strong })),
        Span::raw("  "),
        Span::raw(whose),
    ];
    let n = |many: &str, n: usize| fill(many, &[&n.to_string()]);
    // Each figure with its style; the ones at nought are not said.
    let mut parts = vec![(
        counted_of(r.cards, t.round_card, t.round_cards),
        Style::new(),
    )];
    let counts = [
        (r.landed, n(t.round_landed, r.landed), fg(pal().ok)),
        (r.failed, n(t.round_failed, r.failed), fg(pal().bad)),
        (r.withdrawn, n(t.round_withdrawn, r.withdrawn), dim()),
        (
            r.waits,
            counted_of(r.waits, t.round_wait, t.round_waits),
            fg(pal().warn),
        ),
    ];
    parts.extend((counts.into_iter()).filter(|c| c.0 > 0).map(|c| (c.1, c.2)));
    if r.cost > 0 || r.unpriced {
        parts.push((w.money(r.cost, r.unpriced), dim()));
    }
    if r.ticks > 0 {
        parts.push((ledger::lasted(app.lang, r.ticks), dim()));
    }
    // Whole: a figure that does not fit goes on the next line, under the
    // word, never cut (what waits for the person is among the last).
    let room = width.saturating_sub(1);
    let mut lines = Vec::new();
    let mut row = head;
    for (said, style) in parts {
        if chars(&row) + 3 + said.chars().count() > room && chars(&row) > 3 {
            lines.push(Line::from(std::mem::replace(
                &mut row,
                vec![Span::raw("   ")],
            )));
        } else {
            row.push(Span::styled(" · ", dim()));
        }
        row.push(Span::styled(said, style));
    }
    lines.push(overlay::cut(Line::from(row), room).0);
    if f.all() {
        for row in overlay::wrap_words(t.round_next, width.saturating_sub(1).max(1)) {
            lines.push(Line::from(Span::styled(format!(" {row}"), dim())));
        }
    }
    lines.push(Line::default());
    lines
}
