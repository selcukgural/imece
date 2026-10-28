//! Windows over the screen: help, and what happened while the person was
//! away.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use super::*;
use crate::app::{App, Digest};

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
