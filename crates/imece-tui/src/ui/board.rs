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
    let rungs = app.rungs();
    // An archived project's cards wait in its archive.
    let many = w.projects.iter().filter(|p| p.archived.is_none()).count() > 1;
    let areas: [Rect; 4] = Layout::horizontal([Constraint::Ratio(1, 4); 4]).areas(area);
    let columns = [
        Column::Waiting,
        Column::Working,
        Column::Verifying,
        Column::Done,
    ];
    for ((column, title), rect) in columns.into_iter().zip(t.columns).zip(areas) {
        let mut cards: Vec<&Card> = w.shown_cards().filter(|c| c.column == column).collect();
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
                // A working owner's glyph breathes as on the salon.
                spans.push(Span::styled(
                    format!("{g} "),
                    glyph_style(app, a, rungs[owner], color),
                ));
                spans.push(Span::styled(clean(&a.name), fg(pal().agent)));
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
                let line = Line::from(spans);
                if let Some(owner) = c.owner
                    && column != Column::Waiting
                    && breathes(&w.agents[owner], rungs[owner])
                {
                    note_glyph(app, &line, 1, inner, lines.len());
                }
                lines.push(line);
            }
        }
        f.render_widget(Paragraph::new(lines), inner);
    }
}
