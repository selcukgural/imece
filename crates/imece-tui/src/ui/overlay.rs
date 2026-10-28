//! Windows over the screen: help, what happened while the person was away,
//! and a conversation read whole.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use super::*;
use crate::app::{App, Digest, Thread};

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
    f.render_widget(Clear, area);
    let block = Block::bordered()
        .title(title)
        .border_style(fg(Color::Indexed(173)));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

pub fn help(f: &mut Frame, lang: Lang) {
    let t = lang.text();
    let mut lines: Vec<Line> = t
        .help_keys
        .iter()
        .map(|(k, what)| {
            Line::from(vec![
                Span::styled(format!(" {}", fit(k, 12)), bold(Color::White)),
                Span::raw(*what),
            ])
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
    window(f, t.help_title.into(), lines, 74);
}

pub fn digest(f: &mut Frame, app: &App, d: &Digest) {
    let t = app.lang.text();
    let counts = [
        ("✓", Color::Green, d.merged),
        ("!", Color::Yellow, d.waiting),
        ("✕", Color::Red, d.blocked),
        ("⚑", Color::Yellow, d.assumptions),
        ("⇄", Color::Cyan, d.talks),
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

/// A conversation, every line whole and wrapped, oldest first; the selected
/// line is marked and kept in view.
pub fn thread(f: &mut Frame, app: &App, th: &Thread) {
    let w = &app.world;
    let t = app.lang.text();
    let screen = f.area();
    let area = centered(
        screen,
        screen.width.saturating_sub(4).min(120),
        screen.height.saturating_sub(2),
    );
    f.render_widget(Clear, area);
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
                &[&name(a), &name(b), &th.entries.len().to_string()],
            )
        }
        None => t.event_title.to_string(),
    };
    let block = Block::bordered()
        .title(title)
        .border_style(fg(Color::Indexed(173)));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [body, keys] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
    f.render_widget(Paragraph::new(Span::styled(t.thread_keys, dim())), keys);

    // Each line: a head (when, who to whom) and its text, then a gap.
    let mut lines: Vec<Line> = Vec::new();
    let mut starts = Vec::new();
    for (n, &i) in th.entries.iter().enumerate() {
        let e = &w.journal[i];
        let selected = n == th.sel;
        let mark = if selected {
            Span::styled("▌", fg(Color::Indexed(173)))
        } else {
            Span::raw(" ")
        };
        let mut part = Vec::new();
        let mut head = vec![
            mark.clone(),
            Span::styled(format!("{} ", clock(e.tick)), dim()),
        ];
        match words::said(&e.event) {
            Some(s) => {
                let mut said = said_spans(app.lang, w, &s);
                said.pop(); // the one-line text; it follows whole below
                head.extend(said);
                part.push(Line::from(head));
                for mut l in markdown(s.text, "  ") {
                    l.spans.insert(0, mark.clone());
                    part.push(l);
                }
            }
            None => {
                head.push(Span::raw(words::event(app.lang, w, &e.event)));
                part.push(Line::from(head));
            }
        }
        part.push(Line::raw(""));
        starts.push(lines.len());
        lines.extend(part);
    }
    // Rows each line takes once wrapped, to scroll to the selected one.
    let rows_before = |upto: usize| {
        Paragraph::new(lines[..upto].to_vec())
            .wrap(Wrap { trim: false })
            .line_count(body.width)
    };
    let p = Paragraph::new(lines.clone()).wrap(Wrap { trim: false });
    let total = p.line_count(body.width);
    let max = total.saturating_sub(usize::from(body.height));
    let max = u16::try_from(max).unwrap_or(u16::MAX);
    if th.follow.take()
        && let Some(&start) = starts.get(th.sel)
    {
        th.scroll
            .set(u16::try_from(rows_before(start)).unwrap_or(u16::MAX));
    }
    th.scroll.set(th.scroll.get().min(max));
    th.page.set(body.height.saturating_sub(1).max(1));
    f.render_widget(p.scroll((th.scroll.get(), 0)), body);
}
