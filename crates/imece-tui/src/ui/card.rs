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
    // The detail's head names the card: the block has no title of its own.
    let block = salon::pane_block(String::new(), app.pane == Pane::Salon);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut head = vec![Line::from(vec![
        Span::styled(format!("{} · ", w.projects[card.project].name), dim()),
        Span::styled(state(t, w, card), bold(pal().info)),
        Span::styled(tries(t, card), dim()),
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
    if !card.paths.is_empty() {
        head.push(Line::from(vec![
            Span::styled(t.card_owns, dim()),
            Span::raw(clean(&crate::paths::listed(&card.paths))),
        ]));
    }
    if !card.checks.is_empty() {
        head.push(Line::from(vec![
            Span::styled(t.card_checks, dim()),
            Span::raw(clean(&crate::checks::listed(&card.checks))),
        ]));
    }
    if !card.after.is_empty() {
        head.push(Line::from(after_spans(t, w, card)));
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
        Some(me) if !card.story.is_empty() => {
            focus::story_lines(app, card, me, usize::from(body.width))
        }
        _ => vec![Line::from(Span::styled(t.card_no_story, dim()))],
    };
    tail(f, body, story);
}

/// What a card waits on (record 126, F10), on its screen: each card it
/// waits on, `✓` done, `✕` failed, `⧗` not yet.
pub(crate) fn after_spans(t: &crate::i18n::Text, w: &World, card: &Card) -> Vec<Span<'static>> {
    let mut spans = vec![Span::styled(t.card_after, dim())];
    for (i, &d) in card.after.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(", ", dim()));
        }
        let (mark, style) = match w.cards[d].column {
            Column::Done => ("✓", fg(pal().ok)),
            Column::Failed | Column::Withdrawn => ("✕", fg(pal().bad)),
            _ => ("⧗", dim()),
        };
        spans.push(Span::raw(format!("#{} ", d + 1)));
        spans.push(Span::styled(mark, style));
    }
    spans
}

/// The board's column of a finished card whose change waits for the
/// person: its agent is through, the work is not in yet.
pub(crate) const TO_REVIEW: usize = 2;

/// Where `column` stands in the board's column titles.
pub(crate) fn column_index(column: Column) -> usize {
    match column {
        Column::Waiting => 0,
        Column::Working => 1,
        Column::Review => TO_REVIEW,
        Column::Verifying => 3,
        // A failed or withdrawn card is through: it stands with the done
        // ones, marked.
        Column::Done | Column::Failed | Column::Withdrawn => 4,
    }
}

/// Which of the board's columns `card` stands in. A card saved done
/// with its change still waiting (before record 126 a card was done at
/// its owner's turn end) stands to review.
pub(crate) fn board_column(w: &World, card: &Card) -> usize {
    let waits = |i: &InboxItem| i.kind == InboxKind::Change && i.card == Some(card.id);
    if card.column == Column::Done && w.inbox.iter().any(|i| i.open && waits(i)) {
        TO_REVIEW
    } else {
        column_index(card.column)
    }
}

/// The word for where `card` stands: its board column's, or that it
/// failed (record 126).
pub(crate) fn state(t: &crate::i18n::Text, w: &World, card: &Card) -> &'static str {
    match card.column {
        Column::Failed => t.card_failed,
        Column::Withdrawn => t.card_withdrawn,
        _ => t.columns[board_column(w, card)],
    }
}

/// Which try `card` is on, from the second; nothing before.
pub(crate) fn tries(t: &crate::i18n::Text, card: &Card) -> String {
    match card.attempt {
        0 | 1 => String::new(),
        n => fill(t.card_try, &[&n.to_string()]),
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
    let end = if card.over() { card.finished } else { w.tick };
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
        assert_eq!(columns.map(column_index), [0, 1, 3, 4]);
        let parts = [Part::Posted, Part::Owner, Part::Helper, Part::Talked];
        assert_eq!(parts.map(part_index), [0, 1, 2, 3]);
    }

    #[test]
    fn a_finished_card_is_to_review_while_its_change_waits() {
        let (mut w, _) = crate::sim::world(1, 1, 0, 1, crate::i18n::Lang::En);
        let mut card = w.cards[0].clone();
        card.column = Column::Done;
        let done = column_index(Column::Done);
        assert_eq!(board_column(&w, &card), done);
        let change = |kind, card_id, open| InboxItem {
            kind,
            agent: 0,
            detail: String::new(),
            researcher: None,
            open,
            tick: 0,
            card: card_id,
        };
        // Another kind, another card, a closed item: nothing to review.
        w.inbox = vec![
            change(InboxKind::Question, Some(card.id), true),
            change(InboxKind::Change, Some(card.id + 1), true),
            change(InboxKind::Change, Some(card.id), false),
            change(InboxKind::Change, None, true),
        ];
        assert_eq!(board_column(&w, &card), done);
        w.inbox.push(change(InboxKind::Change, Some(card.id), true));
        assert_eq!(board_column(&w, &card), TO_REVIEW);
        // Not finished: where it is, whatever waits.
        card.column = Column::Working;
        assert_eq!(board_column(&w, &card), column_index(Column::Working));
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
            paths: Vec::new(),
            checks: Vec::new(),
            started: 12,
            finished: 24,
            attempt: 1,
            breaker: Default::default(),
            after: Vec::new(),
        };
        // 24 ticks of 5 s.
        assert_eq!(minutes(&w, &card), 2);
        card.column = Column::Done;
        assert_eq!(minutes(&w, &card), 1);
        card.started = 30;
        assert_eq!(minutes(&w, &card), 0, "never below nothing");
    }

    #[test]
    fn a_failed_card_says_so_and_a_card_says_its_try_from_the_second() {
        let mut w = crate::live::testing::team(1);
        let c = crate::live::testing::with_card(&mut w, 1);
        let t = crate::i18n::Lang::En.text();
        let mut card = w.cards[c].clone();
        assert_eq!(state(t, &w, &card), "IN PROGRESS");
        card.column = Column::Failed;
        assert_eq!(state(t, &w, &card), "FAILED");
        assert_eq!(state(crate::i18n::Lang::Tr.text(), &w, &card), "BAŞARISIZ");
        card.column = Column::Review;
        assert_eq!(state(t, &w, &card), "TO REVIEW");
        assert_eq!(column_index(Column::Review), TO_REVIEW);
        assert_eq!(column_index(Column::Failed), column_index(Column::Done));
        for (attempt, said) in [(0, ""), (1, ""), (2, " · try 2"), (11, " · try 11")] {
            card.attempt = attempt;
            assert_eq!(tries(t, &card), said, "{attempt}");
        }
        card.attempt = 2;
        assert_eq!(tries(crate::i18n::Lang::Tr.text(), &card), " · 2. deneme");
        // A failed card's time ends where it failed.
        card.column = Column::Failed;
        (card.started, card.finished) = (0, 24);
        w.tick = 240;
        assert_eq!(minutes(&w, &card), 2);
    }
}
