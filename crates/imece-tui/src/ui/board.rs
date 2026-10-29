//! The board: cards in four columns, each with the agent that holds it and,
//! when several projects run, the project it belongs to.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::*;
use crate::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let w = &app.world;
    let t = app.lang.text();
    let many = w.projects.len() > 1;
    let areas: [Rect; 4] = Layout::horizontal([Constraint::Ratio(1, 4); 4]).areas(area);
    let columns = [
        Column::Waiting,
        Column::Working,
        Column::Verifying,
        Column::Done,
    ];
    for ((column, title), rect) in columns.into_iter().zip(t.columns).zip(areas) {
        let mut cards: Vec<&Card> = w.cards.iter().filter(|c| c.column == column).collect();
        if column == Column::Done {
            cards.sort_by_key(|c| std::cmp::Reverse(c.finished));
        }
        let block = Block::bordered()
            .title(format!(" {title} ({}) ", cards.len()))
            .border_style(border());
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let width = usize::from(inner.width);
        let mut lines = Vec::new();
        for c in cards {
            lines.push(Line::from(vec![
                Span::styled(format!("#{:<3}", c.id + 1), dim()),
                Span::raw(fit(&c.title, width.saturating_sub(4))),
            ]));
            let mut spans = vec![Span::raw("    ")];
            if column != Column::Waiting
                && let Some(owner) = c.owner
            {
                let a = &w.agents[owner];
                let (g, color) = glyph(a);
                spans.push(Span::styled(format!("{g} "), fg(color)));
                spans.push(Span::styled(clean(&a.name), fg(vendor_color(a.vendor))));
                if column == Column::Working {
                    spans.push(Span::styled(format!(" {}", bar(a.progress)), fg(color)));
                }
            }
            if many {
                spans.push(Span::styled(
                    format!("  {}", w.projects[c.project].name),
                    dim(),
                ));
            }
            if spans.len() > 1 {
                lines.push(Line::from(spans));
            }
        }
        f.render_widget(Paragraph::new(lines), inner);
    }
}
