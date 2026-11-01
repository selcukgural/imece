//! One card up close, in the salon's place: where it stands, who had a
//! part in it (as the salon draws them, so their state is live) and its
//! story. What waits for the person on it and its journal lines take the
//! side panel.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::*;
use crate::app::{App, Pane, Sel};

pub fn draw(f: &mut Frame, area: Rect, app: &App, c: CardId) {
    let w = &app.world;
    let t = app.lang.text();
    let card = &w.cards[c];
    let block = salon::pane_block(
        format!(" #{} {} ", c + 1, clean(&card.title)),
        app.pane == Pane::Salon,
    );
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut head = vec![Line::from(vec![
        Span::styled(format!("{} · ", w.projects[card.project].name), dim()),
        Span::styled(t.columns[column_index(card.column)], bold(pal().info)),
        Span::styled(fill(t.minutes, &[&minutes(w, card).to_string()]), dim()),
    ])];
    if !card.symbols.is_empty() {
        let leases = card
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
    head.push(Line::raw(""));
    head.push(Line::from(Span::styled(t.card_who, bold(pal().strong))));
    let people = w.card_people(c);
    *app.shown.borrow_mut() = people.iter().map(|&(id, _)| Sel::Agent(id)).collect();
    for &(id, part) in &people {
        // After the part's ten columns.
        let width = usize::from(inner.width).saturating_sub(10);
        let line = salon::agent_line(app, &w.agents[id], "", &app.rungs(), width);
        let mut spans = vec![Span::styled(fit(t.card_parts[part_index(part)], 10), dim())];
        spans.extend(line.spans);
        head.push(Line::from(spans).style(line.style));
    }
    head.push(Line::raw(""));
    head.push(Line::from(Span::styled(t.card_story, bold(pal().strong))));

    let [top, body] =
        Layout::vertical([Constraint::Length(head.len() as u16), Constraint::Min(0)]).areas(inner);
    f.render_widget(Paragraph::new(head), top);
    // Conversation lines are told as the owner had them; a card nobody
    // took yet has only who posted it.
    let me = card.owner.or(people.first().map(|&(id, _)| id));
    let story = match me {
        Some(me) if !card.story.is_empty() => focus::story_lines(app, card, me),
        _ => vec![Line::from(Span::styled(t.card_no_story, dim()))],
    };
    tail(f, body, story);
}

/// Where `column` stands in the board's column titles.
pub(crate) fn column_index(column: Column) -> usize {
    match column {
        Column::Waiting => 0,
        Column::Working => 1,
        Column::Verifying => 2,
        Column::Done => 3,
    }
}

/// Where `part` stands in the words for the parts.
pub(crate) fn part_index(part: Part) -> usize {
    match part {
        Part::Posted => 0,
        Part::Owner => 1,
        Part::Helper => 2,
        Part::Talked => 3,
    }
}

/// How long the card has run: from its start to its end, or to now while
/// it is not done.
pub(crate) fn minutes(w: &World, card: &Card) -> u64 {
    let end = if card.column == Column::Done {
        card.finished
    } else {
        w.tick
    };
    end.saturating_sub(card.started) * SECONDS_PER_TICK / 60
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_and_parts_have_their_place_in_the_words() {
        let columns = [
            Column::Waiting,
            Column::Working,
            Column::Verifying,
            Column::Done,
        ];
        assert_eq!(columns.map(column_index), [0, 1, 2, 3]);
        let parts = [Part::Posted, Part::Owner, Part::Helper, Part::Talked];
        assert_eq!(parts.map(part_index), [0, 1, 2, 3]);
    }

    #[test]
    fn a_card_runs_until_now_or_until_it_was_done() {
        let (mut w, _) = crate::sim::world(1, 1, 0, 1, crate::i18n::Lang::En);
        w.tick = 36;
        let mut card = Card {
            id: 0,
            project: 0,
            title: String::new(),
            owner: None,
            column: Column::Working,
            story: Vec::new(),
            symbols: Vec::new(),
            started: 12,
            finished: 24,
        };
        // 24 ticks of 5 s.
        assert_eq!(minutes(&w, &card), 2);
        card.column = Column::Done;
        assert_eq!(minutes(&w, &card), 1);
        card.started = 30;
        assert_eq!(minutes(&w, &card), 0, "never below nothing");
    }
}
