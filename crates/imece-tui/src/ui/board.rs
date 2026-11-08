//! The board: cards in five columns, each with the agent that holds it and,
//! when several projects run, the project it belongs to.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::card::{TO_REVIEW, board_column, column_index};
use super::*;
use crate::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let w = &app.world;
    let t = app.lang.text();
    let rungs = app.rungs();
    // An archived project's cards wait in its archive.
    let many = w.projects.iter().filter(|p| p.archived.is_none()).count() > 1;
    let areas: [Rect; 5] = Layout::horizontal([Constraint::Ratio(1, 5); 5]).areas(area);
    let waiting = column_index(Column::Waiting);
    let done = column_index(Column::Done);
    for (index, (title, rect)) in t.columns.into_iter().zip(areas).enumerate() {
        let mut cards: Vec<&Card> = w
            .shown_cards()
            .filter(|c| board_column(w, c) == index)
            .collect();
        if index == done || index == TO_REVIEW {
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
            // A failed card stands with the done ones, marked; a card on
            // its second try or later says which (record 126).
            let mark = if c.column == Column::Failed {
                "✕ "
            } else {
                ""
            };
            let tries = super::card::tries(t, c);
            let room = width.saturating_sub(4 + mark.chars().count() + tries.chars().count());
            lines.push(Line::from(vec![
                Span::styled(format!("#{:<3}", c.id + 1), dim()),
                Span::styled(mark, fg(pal().bad)),
                Span::raw(fit(&c.title, room)),
                Span::styled(tries, dim()),
            ]));
            let mut spans = vec![Span::raw("    ")];
            // The owner's state says what the card's agent does now, which
            // is the card's only while it is at work on it.
            let live = c.column == Column::Working || c.column == Column::Verifying;
            if index != waiting
                && let Some(owner) = c.owner
            {
                let a = &w.agents[owner];
                let (g, color) = glyph(a);
                if live {
                    // A working owner's glyph breathes as on the salon.
                    spans.push(Span::styled(
                        format!("{g} "),
                        glyph_style(app, a, rungs[owner], color),
                    ));
                }
                spans.push(Span::styled(clean(&a.name), fg(pal().agent)));
                if c.column == Column::Working {
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
                    && live
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
