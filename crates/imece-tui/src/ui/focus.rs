//! One agent up close: its card's story (intent, decisions, assumptions,
//! conversations, evidence) and, one level deeper, its raw stream.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::*;
use crate::app::App;

fn story_icon(k: StoryKind) -> (&'static str, usize, Color) {
    match k {
        StoryKind::Intent => ("◎", 0, Color::Cyan),
        StoryKind::Assumption => ("⚑", 1, Color::Yellow),
        StoryKind::Decision => ("⚖", 2, Color::Magenta),
        StoryKind::Talk => ("⇄", 3, Color::Cyan),
        StoryKind::Evidence => ("◌", 4, Color::Green),
        StoryKind::Change => ("Δ", 5, Color::White),
        StoryKind::Person => ("›", 6, Color::White),
    }
}

pub fn draw(f: &mut Frame, area: Rect, app: &App, id: AgentId) {
    let w = &app.world;
    let t = app.lang.text();
    let a = &w.agents[id];
    let block = Block::bordered()
        .title(fill(t.focus_title, &[&a.name]))
        .border_style(fg(vendor_color(a.vendor)));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut head = Vec::new();
    let card = a.card.map(|c| &w.cards[c]);
    let mut title = vec![Span::styled(
        format!("{} · ", w.projects[a.project].name),
        dim(),
    )];
    if let Some(c) = card {
        let minutes = w.tick.saturating_sub(c.started) * SECONDS_PER_TICK / 60;
        title.push(Span::styled(
            format!("#{} {}", c.id + 1, clean(&c.title)),
            bold(Color::White),
        ));
        title.push(Span::styled(
            fill(t.minutes, &[&minutes.to_string()]),
            dim(),
        ));
    } else if a.role == Role::Researcher {
        title.push(Span::styled(
            fill(t.research, &[&a.topic]),
            bold(Color::White),
        ));
    } else {
        title.push(Span::styled(t.no_card, dim()));
    }
    title.push(Span::styled(
        format!(" · {} ({})", clean(&a.name), a.vendor.label()),
        fg(vendor_color(a.vendor)),
    ));
    title.push(Span::styled(format!(" · {}", dollars(a.cost)), dim()));
    head.push(Line::from(title));

    let mut status = vec![Span::styled(t.status, dim())];
    status.extend(status_spans(app.lang, w, a));
    head.push(Line::from(status));
    let taint = match a.taint {
        Taint::Clean => Span::styled(t.clean, fg(Color::Green)),
        Taint::Soft => Span::styled(t.soft, fg(Color::Yellow)),
        Taint::Full => Span::styled(t.full, fg(Color::Red)),
    };
    head.push(Line::from(vec![Span::styled(t.origin, dim()), taint]));
    if let Some(c) = card {
        let leases = c
            .symbols
            .iter()
            .map(|s| format!("{}#{}", s.file, s.name))
            .collect::<Vec<_>>()
            .join(", ");
        head.push(Line::from(vec![
            Span::styled(t.lease, dim()),
            Span::raw(leases),
        ]));
    }

    let [top, body] = Layout::vertical([
        Constraint::Length(head.len() as u16 + 1),
        Constraint::Min(0),
    ])
    .areas(inner);
    f.render_widget(Paragraph::new(head), top);

    let story: Vec<Line> = match card {
        Some(c) => c
            .story
            .iter()
            .map(|s| {
                let (icon, label, color) = story_icon(s.event.kind());
                Line::from(vec![
                    Span::styled(format!("{} ", clock(s.tick)), dim()),
                    Span::styled(
                        format!("{icon} {} ", fit(t.story_labels[label], 10)),
                        fg(color),
                    ),
                    Span::raw(words::story(app.lang, w, c, &s.event)),
                ])
            })
            .collect(),
        None => vec![Line::from(Span::styled(t.no_story, dim()))],
    };
    tail(f, body, story);
}

pub fn trace(f: &mut Frame, area: Rect, app: &App, id: AgentId) {
    let a = &app.world.agents[id];
    let block = Block::bordered()
        .title(fill(app.lang.text().trace_title, &[&a.name]))
        .border_style(dim());
    let inner = block.inner(area);
    f.render_widget(block, area);
    let lines: Vec<Line> = a
        .trace
        .iter()
        .map(|l| Line::from(Span::raw(clean(l))))
        .collect();
    tail(f, inner, lines);
}
