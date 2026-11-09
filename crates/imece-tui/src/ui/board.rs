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
        // The ready filter leaves out what waits on other cards (record
        // 126, F10).
        let mut cards: Vec<&Card> = w
            .shown_cards()
            .filter(|c| board_column(w, c) == index)
            .filter(|c| !app.board_ready || !waits(w, c))
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
            let mark = match c.column {
                Column::Failed => "✕ ",
                Column::Withdrawn => "– ",
                _ => "",
            };
            let tries = super::card::tries(t, c);
            let room = width.saturating_sub(4 + mark.chars().count() + tries.chars().count());
            lines.push(Line::from(vec![
                Span::styled(format!("#{:<3}", c.id + 1), dim()),
                Span::styled(
                    mark,
                    match c.column {
                        Column::Withdrawn => dim(),
                        _ => fg(pal().bad),
                    },
                ),
                Span::raw(fit(&c.title, room)),
                Span::styled(tries, dim()),
            ]));
            let mut spans = vec![Span::raw("    ")];
            // A card waiting says on what, and a held one for whom.
            if index == waiting {
                spans.extend(waiting_spans(t, w, c));
            }
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
                // The demo's made-up progress; a live agent has none.
                if c.column == Column::Working && w.demo {
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

/// Whether card `c` waits for other cards on the board (record 126, F10).
fn waits(w: &World, c: &Card) -> bool {
    c.column == Column::Waiting && !w.ready(c.id)
}

/// The cards the ready filter leaves out: the shown ones waiting on others.
pub fn hidden(w: &World) -> usize {
    w.shown_cards().filter(|c| waits(w, c)).count()
}

/// What a waiting card's second line says (record 126, F10): the card it
/// waits on that failed, else those not done, then whom an assign holds it
/// for; nothing for a card any worker may take.
pub(crate) fn waiting_spans(t: &crate::i18n::Text, w: &World, c: &Card) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    if let Some(f) = w.blocked_on(c.id) {
        let text = match w.cards[f].column {
            Column::Withdrawn => t.board_withdrawn,
            _ => t.board_blocked,
        };
        spans.push(Span::styled(
            fill(text, &[&format!("#{}", f + 1)]),
            fg(pal().bad),
        ));
    } else if !w.ready(c.id) {
        spans.push(Span::styled(
            fill(t.board_after, &[&words::card_refs(&w.waits_on(c.id))]),
            dim(),
        ));
    }
    if let Some(o) = c.owner {
        let gap = if spans.is_empty() { "" } else { " " };
        spans.push(Span::styled(
            format!("{gap}→ {}", clean(&w.agents[o].name)),
            fg(pal().agent),
        ));
    }
    spans
}
