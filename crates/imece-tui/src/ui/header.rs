//! The header: the projects, what waits, the pot and the time, most wanted
//! first, fitted to the width.

use super::*;

/// The projects open, the one Home is narrowed to lit: one of the
/// header's four things (record 125).
pub(crate) fn projects_spans(app: &App) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for p in app.world.open_projects() {
        if !spans.is_empty() {
            spans.push(Span::styled(" · ", dim()));
        }
        let lit = app.at_home.scope == Some(p.id);
        let style = if lit {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
        };
        spans.push(Span::styled(clean(&p.name), style));
        // Every card it handed out is through and nothing waits (record
        // 145).
        if app.world.finished(p.id).is_some_and(|f| f.all()) {
            spans.push(Span::styled(" ✓", fg(pal().ok)));
        }
        // Its solo writes straight in the folder: no Δ (record 73).
        if p.no_git {
            let t = app.lang.text();
            spans.push(Span::styled(format!(" ⌀ {}", t.no_git), fg(pal().warn)));
        }
    }
    spans
}

/// Percent of a usage window used from which the header says
/// it the way it says what waits for the person.
pub(super) const LIMIT_LOUD: u8 = 90;

/// A Claude Code's tightest usage window as the header shows it.
pub(super) fn limit_span(lang: Lang, l: &Limit, now: u64) -> Span<'static> {
    let said = words::limit(lang, l, now);
    match l.used >= LIMIT_LOUD {
        true => Span::styled(said, attention()),
        false => Span::raw(said),
    }
}

/// A part of the header: what it says, whether it follows the one before
/// without a `│` (the pot's rate, the clock), and how long it stays when
/// the line is wider than the screen: the lowest `keep` goes first, the
/// last of equals first; `KEEP` parts never go.
pub(crate) struct HeadPart {
    pub spans: Vec<Span<'static>>,
    pub glued: bool,
    pub keep: u8,
}

/// A header part that never goes: what waits for the person, the pause, a
/// pot or a window running out.
pub(crate) const KEEP: u8 = u8::MAX;

pub(super) fn header(f: &mut Frame, area: Rect, app: &App) {
    let line = fit_header(header_parts(app), usize::from(area.width));
    f.render_widget(Paragraph::new(line), area);
}

/// The header's parts, most wanted first (record 125): what waits for
/// the person, the fleet, the pot, a usage window nearly full, the clock
/// with the pause or the demo's speed glued to it; on a narrow screen the
/// clock goes, then the fleet, never the rest. The pot's rate, how
/// long it lasts while that is not pressing, and a window not yet loud
/// are the `:budget` note's (`App::command_note`).
pub(crate) fn header_parts(app: &App) -> Vec<HeadPart> {
    let w = &app.world;
    let t = app.lang.text();
    let part = |spans: Vec<Span<'static>>, keep: u8| HeadPart {
        spans,
        glued: false,
        keep,
    };
    let glued = |spans: Vec<Span<'static>>, keep: u8| HeadPart {
        spans,
        glued: true,
        keep,
    };
    let mut parts = Vec::new();
    let mut project_spans = Vec::new();
    if let Some(spin) = app.head_spin() {
        project_spans.push(Span::styled(format!("{spin} "), fg(pal().ok)));
    }
    project_spans.extend(projects_spans(app));
    if !project_spans.is_empty() {
        parts.push(part(project_spans, 8));
    }
    // A reply asks for nothing: counted apart, never as a decision.
    let open = w.open_decisions();
    if open > 0 {
        parts.push(part(
            vec![Span::styled(
                fill(t.waiting_for_you, &[&open.to_string()]),
                attention(),
            )],
            KEEP,
        ));
    }
    let replies = w.open_inbox().len() - open;
    if replies > 0 {
        parts.push(part(
            vec![Span::styled(
                fill(t.replies_waiting, &[&replies.to_string()]),
                dim(),
            )],
            7,
        ));
    }
    let spent = fill(t.budget, &[&w.spent_words(), &dollars(w.budget)]);
    match w.pot_lasts().filter(|_| w.pot_low()) {
        Some(minutes) => parts.push(part(
            vec![Span::styled(
                format!(
                    "{spent}, {}",
                    fill(t.pot_lasts, &[&words::ago(app.lang, minutes * 60)])
                ),
                attention(),
            )],
            KEEP,
        )),
        // Spent: said loud, as a low pot is.
        None if w.spent() >= w.budget => parts.push(part(
            vec![Span::styled(fill(t.pot_spent, &[&spent]), attention())],
            KEEP,
        )),
        // One of the header's four things: kept, as what waits is.
        None => parts.push(part(vec![Span::raw(spent)], KEEP)),
    }
    // A window only when little is left: then it stops the team before
    // the pot does.
    if let Some(l) = w.tightest().filter(|l| l.used >= LIMIT_LOUD) {
        parts.push(part(vec![limit_span(app.lang, &l, w.now())], KEEP));
    }
    parts.push(glued(
        vec![Span::styled(format!("  {}", clock(w.tick)), dim())],
        3,
    ));
    if app.paused {
        parts.push(glued(
            vec![Span::styled(
                t.paused,
                Style::new().add_modifier(Modifier::REVERSED),
            )],
            KEEP,
        ));
    } else if app.speed > 1 {
        parts.push(glued(
            vec![Span::styled(format!(" ×{}", app.speed), dim())],
            3,
        ));
    }
    parts
}

/// The header in `width` columns: ` imece `, then the parts, a `│`
/// before each that is not glued to the one before; while the line is
/// wider, the part with the lowest `keep` goes, the last of equals first,
/// so what matters most is never what the screen's edge cuts off; when
/// what is left is still wider, the brand goes too (record 131: a phone).
pub(crate) fn fit_header(mut parts: Vec<HeadPart>, width: usize) -> Line<'static> {
    const BRAND: usize = 8;
    const SEP: usize = 3;
    let columns = |parts: &[HeadPart]| -> usize {
        let text: usize = parts.iter().flat_map(|p| &p.spans).map(Span::width).sum();
        let seps = parts.iter().skip(1).filter(|p| !p.glued).count();
        text + SEP * seps
    };
    while BRAND + columns(&parts) > width {
        let Some(i) = (0..parts.len())
            .filter(|&i| parts[i].keep < KEEP)
            .min_by_key(|&i| (parts[i].keep, std::cmp::Reverse(i)))
        else {
            break;
        };
        parts.remove(i);
    }
    let mut spans = if BRAND + columns(&parts) > width {
        vec![Span::raw(" ")]
    } else {
        vec![Span::styled(" imece ", brand()), Span::raw(" ")]
    };
    for (i, p) in parts.into_iter().enumerate() {
        if i > 0 && !p.glued {
            spans.push(Span::styled(" │ ", dim()));
        }
        spans.extend(p.spans);
    }
    Line::from(spans)
}
