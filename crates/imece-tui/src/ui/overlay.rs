//! Windows over the screen: help, what happened while the person was away,
//! and a conversation read whole.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::*;
use crate::app::{App, Digest, LeftOff, Thread};

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

fn window(f: &mut Frame, title: String, lines: Vec<Line<'static>>, width: u16) {
    let area = centered(f.area(), width, lines.len() as u16 + 2);
    shade(f, area);
    clear(f, area);
    let block = Block::bordered()
        .title(title)
        .border_style(fg(pal().accent));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

pub fn help(f: &mut Frame, lang: Lang) {
    let t = lang.text();
    let key = |(k, what): &(&str, &str)| {
        vec![
            Span::styled(format!(" {}", fit(k, 13)), bold(pal().strong)),
            Span::raw(fit(what, 44)),
        ]
    };
    // Two columns where the screen is wide enough, so the help fits a
    // short screen too.
    let wide = f.area().width >= 120;
    let keys = &t.help_keys;
    let rows = if wide {
        keys.len().div_ceil(2)
    } else {
        keys.len()
    };
    let mut lines: Vec<Line> = (0..rows)
        .map(|r| {
            let mut spans = key(&keys[r]);
            if wide && let Some(k) = keys.get(r + rows) {
                spans.extend(key(k));
            }
            Line::from(spans)
        })
        .collect();
    lines.push(Line::raw(""));
    lines.extend(
        t.help_legend
            .iter()
            .map(|l| Line::from(Span::styled(*l, dim()))),
    );
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(t.close, dim())));
    window(f, t.help_title.into(), lines, if wide { 118 } else { 60 });
}

pub fn digest(f: &mut Frame, app: &App, d: &Digest) {
    let t = app.lang.text();
    let counts = [
        ("✓", pal().ok, d.merged),
        ("!", pal().warn, d.waiting),
        ("✕", pal().bad, d.blocked),
        ("⚑", pal().warn, d.assumptions),
        ("⇄", pal().info, d.talks),
    ];
    let mut lines: Vec<Line> = counts
        .iter()
        .zip(t.away_counts)
        .map(|(&(icon, color, n), what)| {
            Line::from(vec![
                Span::styled(format!(" {icon} "), fg(color)),
                Span::raw(format!("{n} {what}")),
            ])
        })
        .collect();
    lines.push(Line::raw(""));
    for e in &d.highlights {
        let mut line = entry_line(app, e);
        line.spans.insert(0, Span::raw(" "));
        lines.push(line);
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(t.close, dim())));
    window(
        f,
        fill(t.away_title, &[&d.minutes.max(1).to_string()]),
        lines,
        84,
    );
}

/// Where the person left off, at the start of a run that remembers an
/// earlier one: the pot, each project in a line, what waits (the oldest
/// items as the inbox lists them), turns the earlier run cut short, and
/// its last journal lines, with a day row where they are not of today.
pub fn left_off(f: &mut Frame, app: &App, l: &LeftOff) {
    let t = app.lang.text();
    let w = &app.world;
    let mut lines: Vec<Line> = vec![Line::from(Span::styled(
        format!(
            " {}",
            fill(t.left_spent, &[&dollars(l.spent), &dollars(l.budget)])
        ),
        dim(),
    ))];
    for (name, agents, cards) in &l.projects {
        lines.push(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                fill(
                    t.left_project,
                    &[name, &agents.to_string(), &cards.to_string()],
                ),
                fg(pal().strong),
            ),
        ]));
    }
    lines.push(Line::raw(""));
    if l.waiting.is_empty() {
        lines.push(Line::from(Span::styled(
            format!(" {}", t.nothing_needed),
            dim(),
        )));
    } else {
        lines.push(Line::from(vec![
            Span::styled(" ! ", bold(pal().warn)),
            Span::styled(
                fill(t.left_waiting, &[&l.waiting_total.to_string()]),
                bold(pal().warn),
            ),
        ]));
        for &i in &l.waiting {
            let mut line = inbox_line(app, i);
            line.spans.insert(0, Span::raw("   "));
            lines.push(line);
        }
        let more = l.waiting_total.saturating_sub(l.waiting.len());
        if more > 0 {
            lines.push(Line::from(Span::styled(
                format!("   {}", fill(t.left_more, &[&more.to_string()])),
                dim(),
            )));
        }
    }
    if !l.interrupted.is_empty() {
        let (icon, style) = entry_icon(EntryKind::Failed);
        let names: Vec<&str> = l
            .interrupted
            .iter()
            .map(|&id| w.agents[id].name.as_str())
            .collect();
        lines.push(Line::from(vec![
            Span::styled(format!(" {icon} "), style),
            Span::raw(fill(t.left_interrupted, &[&names.join(", ")])),
        ]));
    }
    if !l.last.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(format!(" {}", t.left_last), dim())));
        let mut above = day(w.tick);
        for e in &l.last {
            let of = day(e.tick);
            if of != above {
                let mut row = date_row(of);
                row.spans.insert(0, Span::raw("   "));
                lines.push(row);
                above = of;
            }
            let mut line = entry_line(app, e);
            line.spans.insert(0, Span::raw("   "));
            lines.push(line);
        }
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(t.close, dim())));
    let title = match l.ago {
        Some(ago) => fill(t.left_title_ago, &[&words::ago(app.lang, ago)]),
        None => t.left_title.to_string(),
    };
    window(f, title, lines, 84);
}

/// Where `scroll` must be for rows `start..end` to show in `height` rows,
/// moving as little as it can; a part longer than the screen shows from
/// its start.
fn in_view(start: usize, end: usize, scroll: usize, height: usize) -> usize {
    // No further down than `start`, no further up than what shows `end`.
    scroll.clamp(end.saturating_sub(height).min(start), start)
}

/// The rows of a conversation `width` cells wide, oldest first: each
/// message is a head (when, who to whom, the kind) and its text as
/// Markdown, wrapped, then a blank row; a day row stands before the first
/// message of each day that is not the day of the message above it (today,
/// for the first). The selected message carries a mark in the gutter on
/// every row and the selection background. Returns the rows and each
/// message's row range (the day row and the blank row not included).
pub(crate) fn thread_rows(
    lang: Lang,
    w: &World,
    th: &Thread,
    width: u16,
) -> (Vec<Line<'static>>, Vec<(usize, usize)>) {
    const GUTTER: usize = 2;
    let text_width = usize::from(width).saturating_sub(GUTTER);
    let mut rows: Vec<Line> = Vec::new();
    let mut ranges = Vec::new();
    let mut above = day(w.tick);
    for (n, &i) in th.entries.iter().enumerate() {
        let e = &w.journal[i];
        let of = day(e.tick);
        if of != above {
            let mut row = date_row(of);
            row.spans.insert(0, Span::raw("  "));
            rows.push(row);
            above = of;
        }
        let selected = n == th.sel;
        let mut part = vec![Line::from(vec![Span::styled(
            format!("{} ", clock(e.tick)),
            dim(),
        )])];
        match words::said(&e.event) {
            Some(s) => {
                let mut said = said_spans(lang, w, &s);
                said.pop(); // the one-line text; it follows whole below
                part[0].spans.extend(said);
                part.extend(markdown(s.text, "  "));
            }
            None => part[0]
                .spans
                .push(Span::raw(words::event(lang, w, &e.event))),
        }
        let start = rows.len();
        for line in part {
            for mut row in wrap(line, text_width) {
                if selected {
                    row.spans.insert(0, Span::styled("▌ ", fg(pal().accent)));
                    row = row.style(changed());
                } else {
                    row.spans.insert(0, Span::raw("  "));
                }
                rows.push(row);
            }
        }
        ranges.push((start, rows.len()));
        rows.push(Line::raw(""));
    }
    (rows, ranges)
}

/// A conversation read whole, oldest first; the selected message is
/// marked on every row and kept in view.
pub fn thread(f: &mut Frame, app: &App, th: &Thread) {
    let w = &app.world;
    let t = app.lang.text();
    let screen = f.area();
    let area = centered(
        screen,
        screen.width.saturating_sub(4).min(120),
        screen.height.saturating_sub(2),
    );
    shade(f, area);
    clear(f, area);
    let first = th.entries.first().map(|&i| &w.journal[i].event);
    let title = match first.and_then(words::said) {
        Some(s) => {
            let name = |p: Party| match p {
                Party::Agent(id) => w.agents[id].name.clone(),
                Party::Person => t.you.to_string(),
            };
            let (a, b) = s.pair();
            fill(
                t.thread_title,
                &[
                    &name(a),
                    &name(b),
                    &(th.sel + 1).to_string(),
                    &th.entries.len().to_string(),
                ],
            )
        }
        None => t.event_title.to_string(),
    };
    let block = Block::bordered()
        .title(title)
        .border_style(fg(pal().accent));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [body, keys] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
    f.render_widget(
        Paragraph::new(Line::from(hint_spans(&t.thread_hints))),
        keys,
    );

    let (rows, ranges) = thread_rows(app.lang, w, th, body.width);
    let height = usize::from(body.height);
    let max = rows.len().saturating_sub(height);
    if th.follow.take()
        && let Some(&(start, end)) = ranges.get(th.sel)
    {
        let at = in_view(start, end, usize::from(th.scroll.get()), height);
        th.scroll.set(u16::try_from(at).unwrap_or(u16::MAX));
    }
    th.scroll
        .set(th.scroll.get().min(u16::try_from(max).unwrap_or(u16::MAX)));
    th.page.set(body.height.saturating_sub(1).max(1));
    let skip = usize::from(th.scroll.get());
    for (row, at) in rows.iter().skip(skip).zip(body.rows()) {
        f.render_widget(row, at);
    }
}

#[cfg(test)]
mod tests {
    use super::in_view;

    #[test]
    fn in_view_moves_the_scroll_as_little_as_it_can() {
        // Already shown: nothing moves.
        assert_eq!(in_view(5, 8, 3, 10), 3);
        assert_eq!(in_view(3, 8, 3, 10), 3, "starting on the first row counts");
        assert_eq!(in_view(5, 13, 3, 10), 3, "ending on the last row too");
        // Above: up to its start.
        assert_eq!(in_view(2, 4, 3, 10), 2);
        // Below: just far enough for its end.
        assert_eq!(in_view(5, 14, 3, 10), 4);
        // Longer than the screen: its start.
        assert_eq!(in_view(5, 16, 3, 10), 5);
        assert_eq!(in_view(5, 15, 3, 10), 5, "exactly the screen fits");
        assert_eq!(in_view(0, 0, 0, 0), 0);
    }
}
